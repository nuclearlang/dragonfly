//! The device itself, shared by both front ends (window and terminal): `flutter run -d chrome` with the browser
//! the device needs, the CDP connection to the app's page, the emulation of the preset (size, touch, user agent),
//! input, frames, the text layer of the app and hot reload.
use super::cdp::{self, Cdp, Event};
use super::presets::Preset;
use super::system;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const HOST: &str = "127.0.0.1";

pub struct Launch {
    pub app: PathBuf,
    pub flutter: String,
    pub number: u32,
    pub preset: Preset,
    pub landscape: bool,
    /// terminal front end: a browser without window and `flutter run --machine` driven by the device
    pub headless: bool,
    /// window front end: the first size of the window (the device fits it to the screen afterwards)
    pub window: (u32, u32),
    pub flutter_args: Vec<String>,
    /// gets the process id of `flutter run` as soon as it starts (to stop it while it compiles)
    pub on_spawn: Option<Sender<u32>>,
}

/// What `flutter run --machine` says (terminal front end).
pub enum FlutterEvent {
    Log(String),
    Progress(String),
    Started,
    Stopped,
}

pub struct Device {
    pub cdp: Cdp,
    pub events: Receiver<Event>,
    pub preset: Preset,
    pub landscape: bool,
    /// the device's window is `scale` times the CSS size (window front end; 1 in the terminal)
    pub scale: f64,
    /// in the start URL and so in the browser's command line and window class: names this device's processes
    pub marker: String,
    app_url: String,
    tmp: Option<PathBuf>,
    child: Child,
    machine: Option<Machine>,
}

struct Machine {
    stdin: ChildStdin,
    app_id: Arc<Mutex<Option<String>>>,
    next: u64,
}

impl Device {
    /// Starts Flutter and waits until the browser shows the device's start page (the app is compiled by then).
    /// `flutter_events` gets what Flutter says when it runs with `--machine` (headless).
    pub fn launch(l: Launch, flutter_events: Option<Sender<FlutterEvent>>) -> Result<Device, String> {
        let browser = system::browser().ok_or(system::NO_BROWSER)?;
        let app_port = system::free_port()?;
        let cdp_port = system::free_port()?;
        // the browser opens a missing asset of Flutter's server (404, no app; Flutter wants an http URL): the
        // device emulates the preset first and then opens the app, so it loads once and already emulated. The
        // marker in the URL also names the processes of this device.
        let marker = format!("__dragonfly_device_{}_{}__", l.number, std::process::id());
        let launch_url = format!("http://{HOST}:{app_port}/assets/{marker}");
        // `dragonfly build` already resolved the packages: --no-pub
        let mut args: Vec<String> =
            ["run", "-d", "chrome", "--no-pub", "--web-hostname", HOST].iter().map(|s| s.to_string()).collect();
        args.extend([
            "--web-port".into(),
            app_port.to_string(),
            "--web-browser-debug-port".into(),
            cdp_port.to_string(),
        ]);
        args.extend(["--web-launch-url".into(), launch_url.clone()]);
        let flags: Vec<String> = if l.headless {
            // CanvasKit needs WebGL: without a GPU the browser draws it in software
            vec!["--headless=new".into(), "--enable-unsafe-swiftshader".into(), "--hide-scrollbars".into()]
        } else {
            vec![
                // a window without tabs nor address bar, like an emulator
                format!("--app={launch_url}"),
                format!("--window-size={},{}", l.window.0, l.window.1),
                // X11 window class (tiling window managers match it); Wayland ignores it
                format!("--class=dragonfly-device-{}", l.number),
                "--hide-scrollbars".into(),
            ]
        };
        args.extend(flags.iter().map(|f| format!("--web-browser-flag={f}")));
        if l.headless {
            args.push("--machine".into());
        }
        args.extend(l.flutter_args.iter().cloned());

        let tmp = system::device_tmp(l.number);
        let mut cmd = Command::new(&l.flutter);
        cmd.args(&args).current_dir(&l.app).env("CHROME_EXECUTABLE", &browser);
        if let Some(tmp) = &tmp {
            cmd.env("TMPDIR", tmp);
        }
        if l.headless {
            cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        }
        let mut child = cmd.spawn().map_err(|e| format!("cannot run {}: {e}", l.flutter))?;
        if let Some(tx) = &l.on_spawn {
            let _ = tx.send(child.id());
        }
        let machine = match (l.headless, flutter_events) {
            (true, Some(tx)) => Some(read_machine(&mut child, tx)),
            _ => None,
        };

        // the start page appears once Flutter compiled the app and opened the browser
        let started = Instant::now();
        let ws = loop {
            if let Some((_, ws)) = cdp::pages(cdp_port).into_iter().find(|(url, _)| url.contains(&marker)) {
                break ws;
            }
            if let Ok(Some(status)) = child.try_wait() {
                return Err(format!("flutter run ended before the device started ({status})"));
            }
            if started.elapsed() > Duration::from_secs(900) {
                system::kill_tree(child.id());
                return Err("the browser did not open in 15 minutes".into());
            }
            thread::sleep(Duration::from_millis(300));
        };
        let (cdp, events) = Cdp::connect(&ws)?;
        cdp.call("Page.enable", json!({}))?;
        let app_url = format!("http://{HOST}:{app_port}/");
        let mut device = Device {
            cdp,
            events,
            preset: l.preset,
            landscape: l.landscape,
            scale: 1.0,
            marker,
            app_url,
            tmp,
            child,
            machine,
        };
        if let Some(m) = device.machine.as_mut() {
            m.next = 1;
        }
        Ok(device)
    }

    /// Opens the app (once, after the first `emulate`).
    pub fn open_app(&self) -> Result<(), String> {
        self.cdp.call("Page.navigate", json!({ "url": self.app_url })).map(|_| ())
    }

    /// Emulates the preset: CSS size, pixel ratio, touch, orientation and user agent. `scale` shrinks the page to
    /// fit a window smaller than the device (window front end).
    pub fn emulate(&mut self, preset: Preset, landscape: bool, scale: f64) -> Result<(), String> {
        self.preset = preset;
        self.landscape = landscape;
        self.scale = scale;
        let (width, height) = preset.size(landscape);
        let mobile = preset.mobile();
        let mut metrics = json!({
            "width": width, "height": height, "deviceScaleFactor": preset.dpr, "mobile": mobile, "scale": scale,
        });
        if mobile {
            metrics["screenOrientation"] = if landscape {
                json!({ "type": "landscapePrimary", "angle": 90 })
            } else {
                json!({ "type": "portraitPrimary", "angle": 0 })
            };
        }
        self.cdp.call("Emulation.setDeviceMetricsOverride", metrics)?;
        self.cdp.call("Emulation.setTouchEmulationEnabled", json!({ "enabled": mobile, "maxTouchPoints": 5 }))?;
        let own = self.cdp.call("Browser.getVersion", json!({}))?["userAgent"].as_str().unwrap_or_default().to_string();
        let (ua, platform) = preset.identity().unwrap_or((&own, ""));
        self.cdp.call("Emulation.setUserAgentOverride", json!({ "userAgent": ua, "platform": platform }))?;
        Ok(())
    }

    /// The CSS size of the screen being emulated.
    pub fn css_size(&self) -> (u32, u32) {
        self.preset.size(self.landscape)
    }

    // ------------------------------------------------------------------ input (CSS pixels of the device)

    pub fn touch(&self, phase: Phase, x: f64, y: f64) {
        if self.preset.mobile() {
            let kind = match phase {
                Phase::Down => "touchStart",
                Phase::Move => "touchMove",
                Phase::Up => "touchEnd",
            };
            let points = if phase == Phase::Up {
                json!([])
            } else {
                json!([{ "x": x, "y": y, "id": 1, "radiusX": 1, "radiusY": 1, "force": 1 }])
            };
            self.cdp.fire("Input.dispatchTouchEvent", json!({ "type": kind, "touchPoints": points }));
        } else {
            let (kind, buttons) = match phase {
                Phase::Down => ("mousePressed", 1),
                Phase::Move => ("mouseMoved", 1),
                Phase::Up => ("mouseReleased", 0),
            };
            let button = if phase == Phase::Move { "none" } else { "left" };
            self.cdp.fire(
                "Input.dispatchMouseEvent",
                json!({ "type": kind, "x": x, "y": y, "button": button, "buttons": buttons, "clickCount": 1 }),
            );
        }
    }

    pub fn wheel(&self, x: f64, y: f64, dy: f64) {
        self.cdp.fire(
            "Input.dispatchMouseEvent",
            json!({ "type": "mouseWheel", "x": x, "y": y, "deltaX": 0, "deltaY": dy }),
        );
    }

    /// Text typed into the focused text field.
    pub fn insert_text(&self, text: &str) {
        self.cdp.fire("Input.insertText", json!({ "text": text }));
    }

    /// A key that is not text: Enter, Backspace, arrows... (`key` as in the DOM: "Enter", "ArrowLeft").
    pub fn key(&self, key: &str, code: u32, modifiers: u32) {
        for kind in ["rawKeyDown", "keyUp"] {
            let mut e = json!({
                "type": kind, "key": key, "code": key, "windowsVirtualKeyCode": code,
                "nativeVirtualKeyCode": code, "modifiers": modifiers,
            });
            if kind == "rawKeyDown" && key == "Enter" {
                e["type"] = json!("keyDown");
                e["text"] = json!("\r");
            }
            self.cdp.fire("Input.dispatchKeyEvent", e);
        }
    }

    /// The system's back (Android): the browser's history, which Flutter's router follows.
    pub fn back(&self) {
        let _ = self.cdp.eval("history.back()");
    }

    /// The app's text fields have the focus: keys are text for the app.
    pub fn typing(&self) -> bool {
        self.cdp
            .eval("(() => { const a = document.activeElement; return !!a && (a.tagName === 'INPUT' || a.tagName === 'TEXTAREA'); })()")
            .is_ok_and(|v| v == Value::Bool(true))
    }

    /// The texts the app shows, with their exact place (debug builds of apps with the dragonfly package; empty
    /// otherwise): see `packages/dragonfly/lib/src/device/probe_web.dart`.
    pub fn text_layer(&self) -> Vec<TextRun> {
        let Ok(Value::String(json)) = self.cdp.eval("globalThis.__dragonflyTextLayer ? __dragonflyTextLayer() : ''")
        else {
            return vec![];
        };
        let runs: Value = serde_json::from_str(&json).unwrap_or_default();
        runs.as_array()
            .into_iter()
            .flatten()
            .filter_map(|r| {
                Some(TextRun {
                    text: r["t"].as_str()?.to_string(),
                    x: r["x"].as_f64()?,
                    y: r["y"].as_f64()?,
                    w: r["w"].as_f64()?,
                    h: r["h"].as_f64()?,
                    color: r["c"].as_u64().map(|c| [(c >> 16) as u8, (c >> 8) as u8, c as u8]),
                    bold: r["b"].as_bool().unwrap_or(false),
                })
            })
            .collect()
    }

    /// A PNG of the screen at the device's pixel ratio.
    pub fn screenshot(&self) -> Result<Vec<u8>, String> {
        use base64::Engine;
        let r = self.cdp.call("Page.captureScreenshot", json!({ "format": "png" }))?;
        base64::engine::general_purpose::STANDARD
            .decode(r["data"].as_str().unwrap_or_default())
            .map_err(|e| e.to_string())
    }

    /// Frames of the screen as `Page.screencastFrame` events, at most `width` x `height` pixels.
    pub fn start_frames(&self, width: u32, height: u32) {
        let _ = self.cdp.call("Page.stopScreencast", json!({}));
        let _ = self.cdp.call(
            "Page.startScreencast",
            json!({ "format": "png", "maxWidth": width.max(16), "maxHeight": height.max(16), "everyNthFrame": 1 }),
        );
    }

    // ------------------------------------------------------------------ Flutter

    /// Hot reload (`full` = hot restart). Only for the terminal front end: in the window one, Flutter reads the
    /// keys of its own terminal.
    pub fn reload(&mut self, full: bool) {
        let Some(m) = self.machine.as_mut() else { return };
        let Some(app_id) = m.app_id.lock().unwrap().clone() else { return };
        m.next += 1;
        let call = json!([{ "id": m.next, "method": "app.restart",
            "params": { "appId": app_id, "fullRestart": full, "pause": false, "reason": "manual" } }]);
        let _ = writeln!(m.stdin, "{call}");
    }

    /// Waits until Flutter ends (window front end: the user quits with `q` in its terminal or closes the window).
    pub fn wait(&mut self) {
        let _ = self.child.wait();
    }

    /// Ends everything: the browser (by CDP, so it never stays alive), the app and `flutter run`.
    pub fn close(&mut self) {
        if let Some(m) = self.machine.as_mut()
            && let Some(app_id) = m.app_id.lock().unwrap().clone()
        {
            let _ = writeln!(m.stdin, "{}", json!([{ "id": 0, "method": "app.stop", "params": { "appId": app_id } }]));
        }
        self.cdp.fire("Browser.close", json!({}));
        let deadline = Instant::now() + Duration::from_secs(4);
        while Instant::now() < deadline {
            if let Ok(Some(_)) = self.child.try_wait() {
                if let Some(tmp) = &self.tmp {
                    let _ = std::fs::remove_dir_all(tmp);
                }
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
        system::kill_tree(self.child.id());
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(tmp) = &self.tmp {
            let _ = std::fs::remove_dir_all(tmp);
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Phase {
    Down,
    Move,
    Up,
}

/// A line of text the app draws: CSS pixels of the device, color as RGB.
#[derive(Clone, Debug, PartialEq)]
pub struct TextRun {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub color: Option<[u8; 3]>,
    pub bold: bool,
}

/// Reads `flutter run --machine`: one JSON array per line (`[{"event": "app.started", ...}]`); other lines are
/// logs.
fn read_machine(child: &mut Child, tx: Sender<FlutterEvent>) -> Machine {
    let app_id = Arc::new(Mutex::new(None));
    if let Some(out) = child.stdout.take() {
        let (tx, app_id) = (tx.clone(), app_id.clone());
        thread::spawn(move || {
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                for event in machine_events(&line, &app_id) {
                    if tx.send(event).is_err() {
                        return;
                    }
                }
            }
            let _ = tx.send(FlutterEvent::Stopped);
        });
    }
    if let Some(err) = child.stderr.take() {
        thread::spawn(move || {
            for line in BufReader::new(err).lines().map_while(Result::ok) {
                if tx.send(FlutterEvent::Log(line)).is_err() {
                    return;
                }
            }
        });
    }
    Machine { stdin: child.stdin.take().expect("piped stdin"), app_id, next: 0 }
}

fn machine_events(line: &str, app_id: &Mutex<Option<String>>) -> Vec<FlutterEvent> {
    let Ok(Value::Array(items)) = serde_json::from_str::<Value>(line) else {
        return if line.trim().is_empty() { vec![] } else { vec![FlutterEvent::Log(line.to_string())] };
    };
    let mut out = vec![];
    for item in items {
        let p = &item["params"];
        match item["event"].as_str() {
            Some("app.start") => *app_id.lock().unwrap() = p["appId"].as_str().map(String::from),
            Some("app.started") => out.push(FlutterEvent::Started),
            Some("app.stop") => out.push(FlutterEvent::Stopped),
            Some("app.log") => out.push(FlutterEvent::Log(p["log"].as_str().unwrap_or_default().to_string())),
            Some("daemon.logMessage") => {
                out.push(FlutterEvent::Log(p["message"].as_str().unwrap_or_default().to_string()))
            }
            Some("app.progress") if p["finished"] != true => {
                out.push(FlutterEvent::Progress(p["message"].as_str().unwrap_or_default().to_string()))
            }
            Some("app.progress") => out.push(FlutterEvent::Progress(String::new())),
            _ => {}
        }
        // the answer of app.restart: an error is worth a log line
        if let Some(e) = item.get("error") {
            out.push(FlutterEvent::Log(format!("flutter: {e}")));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machine_lines_become_events() {
        let id = Mutex::new(None);
        assert!(machine_events(r#"[{"event":"app.start","params":{"appId":"a1"}}]"#, &id).is_empty());
        assert_eq!(id.lock().unwrap().as_deref(), Some("a1"));
        let logs = machine_events(r#"[{"event":"app.log","params":{"log":"hello"}}]"#, &id);
        assert!(matches!(&logs[..], [FlutterEvent::Log(l)] if l == "hello"));
        assert!(matches!(&machine_events("Launching lib/main.dart", &id)[..], [FlutterEvent::Log(_)]));
        let p = machine_events(r#"[{"event":"app.progress","params":{"message":"Compiling","finished":false}}]"#, &id);
        assert!(matches!(&p[..], [FlutterEvent::Progress(m)] if m == "Compiling"));
    }
}
