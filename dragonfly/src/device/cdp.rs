//! A minimal Chrome DevTools Protocol client: `call` waits for the answer; events go to a channel.
//! One thread owns the WebSocket (tungstenite is not split): it writes the queued calls and reads with a short
//! timeout.
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{ErrorKind, Read, Write};
use std::net::TcpStream;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tungstenite::{Message, WebSocket};

type Pending = Arc<Mutex<HashMap<u64, Sender<Result<Value, String>>>>>;

pub struct Cdp {
    out: Sender<String>,
    pending: Pending,
    next: Mutex<u64>,
    closed: Arc<Mutex<bool>>,
}

/// An event of the browser: `Page.screencastFrame`, `Page.loadEventFired`...
pub struct Event {
    pub method: String,
    pub params: Value,
}

/// `GET http://127.0.0.1:<port><path>` (the browser's `/json` endpoints) without an HTTP library.
pub fn http_get(port: u16, path: &str) -> Result<String, String> {
    let mut s = TcpStream::connect(("127.0.0.1", port)).map_err(|e| e.to_string())?;
    s.set_read_timeout(Some(Duration::from_secs(3))).ok();
    write!(s, "GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n")
        .map_err(|e| e.to_string())?;
    // the browser may keep the connection open: read the headers, then Content-Length bytes
    let mut data = Vec::new();
    let mut buf = [0u8; 8192];
    let body_start = loop {
        let n = s.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("bad HTTP answer".into());
        }
        data.extend_from_slice(&buf[..n]);
        if let Some(i) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
    };
    let head = String::from_utf8_lossy(&data[..body_start]).to_lowercase();
    let len = head.lines().find_map(|l| l.strip_prefix("content-length:").and_then(|v| v.trim().parse::<usize>().ok()));
    while len.is_some_and(|len| data.len() - body_start < len) {
        let n = s.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        data.extend_from_slice(&buf[..n]);
    }
    Ok(String::from_utf8_lossy(&data[body_start..]).to_string())
}

/// The pages of the browser: `(url, webSocketDebuggerUrl)`.
pub fn pages(port: u16) -> Vec<(String, String)> {
    let Ok(body) = http_get(port, "/json") else { return vec![] };
    let list: Value = serde_json::from_str(&body).unwrap_or_default();
    list.as_array()
        .into_iter()
        .flatten()
        .filter(|t| t["type"] == "page")
        .filter_map(|t| Some((t["url"].as_str()?.to_string(), t["webSocketDebuggerUrl"].as_str()?.to_string())))
        .collect()
}

impl Cdp {
    pub fn connect(ws_url: &str) -> Result<(Cdp, Receiver<Event>), String> {
        let rest = ws_url.strip_prefix("ws://").ok_or("not a ws:// URL")?;
        let host = rest.split('/').next().unwrap_or_default();
        let stream = TcpStream::connect(host).map_err(|e| format!("cannot connect to the browser: {e}"))?;
        let (mut ws, _) = tungstenite::client(ws_url, stream).map_err(|e| format!("CDP handshake: {e}"))?;
        ws.get_mut().set_read_timeout(Some(Duration::from_millis(5))).ok();
        let (out, queue) = channel::<String>();
        let (events, rx) = channel();
        let pending: Pending = Arc::default();
        let closed = Arc::new(Mutex::new(false));
        let (p, c) = (pending.clone(), closed.clone());
        thread::spawn(move || io_loop(ws, queue, events, p, c));
        Ok((Cdp { out, pending, next: Mutex::new(0), closed }, rx))
    }

    pub fn is_closed(&self) -> bool {
        *self.closed.lock().unwrap()
    }

    /// Sends a call and waits for its answer (10 s at most).
    pub fn call(&self, method: &str, params: Value) -> Result<Value, String> {
        let rx = self.send(method, params)?;
        rx.recv_timeout(Duration::from_secs(10)).map_err(|_| format!("{method}: no answer"))?
    }

    /// Sends a call without waiting (input events, frame acks).
    pub fn fire(&self, method: &str, params: Value) {
        let _ = self.send(method, params);
    }

    fn send(&self, method: &str, params: Value) -> Result<Receiver<Result<Value, String>>, String> {
        if self.is_closed() {
            return Err("the browser closed".into());
        }
        let id = {
            let mut n = self.next.lock().unwrap();
            *n += 1;
            *n
        };
        let (tx, rx) = channel();
        self.pending.lock().unwrap().insert(id, tx);
        self.out
            .send(json!({ "id": id, "method": method, "params": params }).to_string())
            .map_err(|_| "the browser closed".to_string())?;
        Ok(rx)
    }

    /// `Runtime.evaluate` of an expression whose value is JSON-serializable.
    pub fn eval(&self, expression: &str) -> Result<Value, String> {
        let r = self.call("Runtime.evaluate", json!({ "expression": expression, "returnByValue": true }))?;
        Ok(r["result"]["value"].clone())
    }
}

fn io_loop(
    mut ws: WebSocket<TcpStream>,
    queue: Receiver<String>,
    events: Sender<Event>,
    pending: Pending,
    closed: Arc<Mutex<bool>>,
) {
    'run: loop {
        while let Ok(text) = queue.try_recv() {
            if ws.send(Message::text(text)).is_err() {
                break 'run;
            }
        }
        match ws.read() {
            Ok(Message::Text(text)) => {
                let Ok(msg) = serde_json::from_str::<Value>(&text) else { continue };
                if let Some(id) = msg["id"].as_u64() {
                    if let Some(tx) = pending.lock().unwrap().remove(&id) {
                        let r = match msg.get("error") {
                            Some(e) => Err(e["message"].as_str().unwrap_or("CDP error").to_string()),
                            None => Ok(msg["result"].clone()),
                        };
                        let _ = tx.send(r);
                    }
                } else if let Some(method) = msg["method"].as_str() {
                    let _ = events.send(Event { method: method.to_string(), params: msg["params"].clone() });
                }
            }
            Ok(Message::Close(_)) => break,
            Ok(_) => {}
            Err(tungstenite::Error::Io(e)) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(_) => break,
        }
    }
    *closed.lock().unwrap() = true;
    // calls still waiting: the browser is gone
    for (_, tx) in pending.lock().unwrap().drain() {
        let _ = tx.send(Err("the browser closed".into()));
    }
}
