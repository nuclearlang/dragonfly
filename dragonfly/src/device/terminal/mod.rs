//! Terminal front end: the Dragonfly Terminal device. The app runs in a browser without window (the same
//! backend as the window device); the terminal shows its screen and sends the mouse and the keys back.
pub mod caps;
pub mod render;

use super::backend::{Device, FlutterEvent, Launch, Phase, TextRun};
use super::presets::{PRESETS, Preset};
use super::system;
use base64::Engine;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};
use crossterm::{cursor, execute, terminal};
use render::{Area, Cell, Colors, Render};
use std::collections::VecDeque;
use std::io::{Stdout, Write, stdout};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub struct Options {
    /// `--render`: forces a render (else the best the terminal supports)
    pub render: Option<Render>,
    /// where `s` saves screenshots
    pub screenshots: PathBuf,
}

/// Restores the terminal however the device ends (also on a panic).
struct Screen;

impl Screen {
    fn enter() -> Result<Screen, String> {
        terminal::enable_raw_mode().map_err(|e| format!("this is not an interactive terminal: {e}"))?;
        let mut out = stdout();
        execute!(out, terminal::EnterAlternateScreen, cursor::Hide, event::EnableMouseCapture)
            .map_err(|e| e.to_string())?;
        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            restore();
            hook(info);
        }));
        Ok(Screen)
    }
}

fn restore() {
    let mut out = stdout();
    let _ = write!(out, "{}\x1b[0m", render::kitty_clear());
    let _ = execute!(out, event::DisableMouseCapture, cursor::Show, terminal::LeaveAlternateScreen);
    let _ = terminal::disable_raw_mode();
}

impl Drop for Screen {
    fn drop(&mut self) {
        restore();
    }
}

#[derive(PartialEq, Clone, Copy)]
enum State {
    Starting,
    Running,
    Reloading,
}

#[derive(PartialEq, Clone, Copy)]
enum Overlay {
    None,
    Help,
    Logs,
}

struct Ui {
    out: Stdout,
    number: u32,
    colors: Colors,
    modes: Vec<Render>,
    mode: usize,
    cell: (f64, f64),
    term: (u16, u16),
    area: Area,
    prev: Option<Vec<Cell>>,
    png: Option<Vec<u8>>,
    texts: Vec<TextRun>,
    typing: bool,
    touching: bool,
    state: State,
    overlay: Overlay,
    logs: VecDeque<String>,
    progress: String,
    frames: VecDeque<Instant>,
    screenshots: PathBuf,
}

const KEYS: &[(&str, &str)] = &[
    ("mouse", "tap, drag (touch on a phone), wheel = scroll, right click = back"),
    ("r / R", "hot reload / hot restart"),
    ("b", "back (the system's back button)"),
    ("o", "rotate (portrait / landscape)"),
    ("d / D", "next / previous device (Pixel, iPhone, iPad...)"),
    ("m", "render: graphics, blocks + text, blocks"),
    ("s", "screenshot (build/screenshots)"),
    ("l", "Flutter's log"),
    ("arrows, Enter, Tab", "keys for the app"),
    ("typing", "when a text field has the focus, keys are text; commands with Alt (Alt+r, Alt+q); Esc leaves it"),
    ("q, Ctrl+C", "quit"),
];

pub fn run(mut launch: Launch, opts: Options) -> Result<(), String> {
    let caps = caps::detect();
    let best = opts.render.unwrap_or(caps.render);
    let mut modes = caps::modes(caps.render);
    if !modes.contains(&best) {
        modes.insert(0, best);
    }
    let _screen = Screen::enter()?;
    let (cols, rows) = terminal::size().unwrap_or((80, 24));
    let mut ui = Ui {
        out: stdout(),
        number: launch.number,
        colors: caps.colors,
        mode: modes.iter().position(|m| *m == best).unwrap_or(0),
        modes,
        cell: caps::cell_size(),
        term: (cols, rows),
        area: Area::default(),
        prev: None,
        png: None,
        texts: vec![],
        typing: false,
        touching: false,
        state: State::Starting,
        overlay: Overlay::None,
        logs: VecDeque::new(),
        progress: "starting Flutter".into(),
        frames: VecDeque::new(),
        screenshots: opts.screenshots,
    };

    // Flutter compiles first (half a minute or more the first time): a splash with its progress meanwhile
    let (ftx, frx) = channel();
    let (ptx, prx) = channel();
    launch.on_spawn = Some(ptx);
    let preset = launch.preset;
    let landscape = launch.landscape;
    let (dtx, drx) = channel();
    thread::spawn(move || {
        let _ = dtx.send(Device::launch(launch, Some(ftx)));
    });
    let mut flutter_pid = None;
    let mut device = loop {
        if let Ok(pid) = prx.try_recv() {
            flutter_pid = Some(pid);
        }
        ui.flutter(&frx);
        ui.splash(preset);
        if quit_requested()? {
            if let Some(pid) = flutter_pid {
                system::kill_tree(pid);
            }
            return Ok(());
        }
        match drx.try_recv() {
            Ok(d) => break d.map_err(|e| ui.failed(e))?,
            Err(_) => continue,
        }
    };
    device.emulate(preset, landscape, 1.0)?;
    device.open_app()?;
    ui.state = State::Running;
    ui.layout(&device);
    let result = ui.main_loop(&mut device, &frx);
    device.close();
    result
}

/// During the splash only `q` and Ctrl+C count.
fn quit_requested() -> Result<bool, String> {
    if event::poll(Duration::from_millis(80)).map_err(|e| e.to_string())?
        && let Event::Key(k) = event::read().map_err(|e| e.to_string())?
    {
        return Ok(k.kind == KeyEventKind::Press
            && (k.code == KeyCode::Char('q')
                || k.code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL)));
    }
    Ok(false)
}

impl Ui {
    fn render(&self) -> Render {
        self.modes[self.mode]
    }

    fn write(&mut self, s: &str) {
        let _ = self.out.write_all(s.as_bytes());
        let _ = self.out.flush();
    }

    fn log(&mut self, line: String) {
        if self.logs.len() > 1000 {
            self.logs.pop_front();
        }
        self.logs.push_back(line);
    }

    /// What Flutter said since the last call.
    fn flutter(&mut self, rx: &Receiver<FlutterEvent>) -> bool {
        let mut stopped = false;
        while let Ok(e) = rx.try_recv() {
            match e {
                FlutterEvent::Log(l) => self.log(l),
                FlutterEvent::Progress(p) => {
                    if p.is_empty() && self.state == State::Reloading {
                        self.state = State::Running;
                    }
                    self.progress = p;
                }
                FlutterEvent::Started => self.state = State::Running,
                FlutterEvent::Stopped => stopped = true,
            }
        }
        stopped
    }

    fn failed(&mut self, e: String) -> String {
        // the log explains why (a compile error...): it stays printed after the terminal is restored
        let tail: Vec<String> = self.logs.iter().rev().take(30).rev().cloned().collect();
        format!("{e}\n{}", tail.join("\n"))
    }

    fn splash(&mut self, preset: Preset) {
        let (cols, rows) = terminal::size().unwrap_or((80, 24));
        let mid = rows / 2;
        let lines = [
            format!(
                "\x1b[1;38;2;224;165;38mDragonfly Terminal device\x1b[0m · device {} · {}",
                self.number, preset.name
            ),
            String::new(),
            format!("◌ {}", if self.progress.is_empty() { "compiling the app" } else { &self.progress }),
            format!("\x1b[2m{}\x1b[0m", self.logs.back().map(String::as_str).unwrap_or("the first build takes longer")),
            String::new(),
            "\x1b[2mq quit\x1b[0m".into(),
        ];
        let mut s = String::from("\x1b[?2026h\x1b[2J");
        for (i, l) in lines.iter().enumerate() {
            let width = visible_len(l).min(cols as usize);
            let col = (cols as usize).saturating_sub(width) / 2;
            s += &format!("\x1b[{};{}H{}", mid as usize + i - 2, col + 1, clip(l, cols as usize));
        }
        s += "\x1b[?2026l";
        self.write(&s);
    }

    /// Places the screen in the terminal (also after a resize, a rotation or another preset) and asks the
    /// browser for frames of that size.
    fn layout(&mut self, device: &Device) {
        self.term = terminal::size().unwrap_or(self.term);
        self.cell = caps::cell_size();
        let (cols, rows) = self.term;
        let avail = Area { col: 1, row: 1, cols: cols.saturating_sub(2).max(4), rows: rows.saturating_sub(4).max(2) };
        self.area = render::fit(device.css_size(), avail, self.cell);
        self.prev = None;
        let (w, h) = render::frame_size(self.render(), self.area, self.cell);
        device.start_frames(w, h);
        let mut s = format!("{}\x1b[0m\x1b[2J", render::kitty_clear());
        s += &self.border(device);
        self.write(&s);
        self.draw_screen(device);
        self.status(device);
    }

    fn border(&self, device: &Device) -> String {
        let a = self.area;
        let (w, h) = device.css_size();
        let dim = "\x1b[38;2;120;130;125m";
        let title = format!(
            " \x1b[1;38;2;224;165;38mDragonfly Terminal device\x1b[0m{dim} · device {} · {} · {w}×{h} ",
            self.number, device.preset.name
        );
        let inner = a.cols as usize;
        let top_fill = inner.saturating_sub(visible_len(&title) + 1);
        let mut s = format!(
            "{dim}\x1b[{};{}H╭─{}{dim}{}╮",
            a.row,
            a.col,
            clip(&title, inner.saturating_sub(1)),
            "─".repeat(top_fill)
        );
        for r in 0..a.rows {
            s += &format!("\x1b[{};{}H│\x1b[{};{}H│", a.row + r + 1, a.col, a.row + r + 1, a.col + a.cols + 1);
        }
        s += &format!("\x1b[{};{}H╰{}╯\x1b[0m", a.row + a.rows + 1, a.col, "─".repeat(inner));
        s
    }

    /// The last frame, in the current render (or the overlay over it).
    fn draw_screen(&mut self, device: &Device) {
        let Some(png) = self.png.clone() else { return };
        if self.overlay != Overlay::None {
            return;
        }
        let css = device.css_size();
        let out = match self.render() {
            Render::Kitty => render::kitty(&png, self.area),
            Render::Sixel => {
                let Some(img) = render::decode_png(&png) else { return };
                let (w, h) = render::frame_size(Render::Sixel, self.area, self.cell);
                let img = if (img.width, img.height) == (w as usize, h as usize) {
                    img
                } else {
                    img.resize(w as usize, h as usize)
                };
                render::sixel(&img, self.area)
            }
            mode => {
                let Some(img) = render::decode_png(&png) else { return };
                let texts = if mode == Render::Text { &self.texts[..] } else { &[] };
                let cells = render::cells(&img, self.area, css, texts);
                let s = render::diff(self.prev.as_deref(), &cells, self.area, self.colors);
                self.prev = Some(cells);
                s
            }
        };
        self.write(&format!("\x1b[?2026h{out}\x1b[?2026l"));
        let now = Instant::now();
        self.frames.push_back(now);
        while self.frames.front().is_some_and(|t| now.duration_since(*t) > Duration::from_secs(1)) {
            self.frames.pop_front();
        }
    }

    /// The two lines under the screen: state, device, render, fps and keys; then Flutter's progress or last log.
    fn status(&mut self, device: &Device) {
        let (cols, rows) = self.term;
        let (dot, state) = match self.state {
            State::Starting => ("\x1b[38;2;255;214;0m◌", "starting"),
            State::Running => ("\x1b[38;2;120;220;90m●", "running"),
            State::Reloading => ("\x1b[38;2;255;214;0m↻", "reloading"),
        };
        let keys = if self.typing {
            "\x1b[38;2;47;224;240m⌨ typing into the app\x1b[0m · Esc leave · Alt+r reload · Alt+q quit".to_string()
        } else {
            hints(&[
                ("r", "reload"),
                ("R", "restart"),
                ("b", "back"),
                ("o", "rotate"),
                ("d", "device"),
                ("m", "mode"),
                ("s", "shot"),
                ("l", "log"),
                ("?", "help"),
                ("q", "quit"),
            ])
        };
        let line1 = format!(
            " {dot} {state}\x1b[0m \x1b[2m· {} · {} · {} fps ·\x1b[0m {keys}",
            device.preset.name,
            self.render().name(),
            self.frames.len()
        );
        let info = if !self.progress.is_empty() {
            format!("◌ {}", self.progress)
        } else {
            self.logs.back().cloned().unwrap_or_default()
        };
        let line2 = format!(" \x1b[2m{}\x1b[0m", plain(&info));
        let s = format!(
            "\x1b[?2026h\x1b[{};1H\x1b[2K{}\x1b[{};1H\x1b[2K{}\x1b[?2026l",
            rows - 1,
            clip(&line1, cols as usize),
            rows,
            clip(&line2, cols as usize)
        );
        self.write(&s);
    }

    fn draw_overlay(&mut self) {
        let a = self.area;
        let lines: Vec<String> = match self.overlay {
            Overlay::None => return,
            Overlay::Help => std::iter::once("\x1b[1mDragonfly Terminal device\x1b[22m — keys".to_string())
                .chain(std::iter::once(String::new()))
                .chain(KEYS.iter().map(|(k, v)| format!("\x1b[38;2;224;165;38m{k:<18}\x1b[39m {v}")))
                .chain(["".into(), "\x1b[2mEsc or ? closes this\x1b[22m".into()])
                .collect(),
            Overlay::Logs => {
                let n = a.rows as usize - 1;
                let mut l: Vec<String> = self.logs.iter().rev().take(n).rev().cloned().collect();
                l.insert(0, "\x1b[1mFlutter\x1b[22m \x1b[2m(l or Esc closes it)\x1b[22m".into());
                l
            }
        };
        let mut s = format!("\x1b[?2026h{}", render::kitty_clear());
        for r in 0..a.rows as usize {
            let text = lines.get(r).map(String::as_str).unwrap_or("");
            let body = clip(&if r == 0 { text.to_string() } else { plain(text) }, a.cols as usize);
            let pad = (a.cols as usize).saturating_sub(visible_len(&body));
            s += &format!(
                "\x1b[{};{}H\x1b[48;2;24;30;27m\x1b[38;2;230;230;230m{body}{}\x1b[0m",
                a.row as usize + r + 1,
                a.col + 1,
                " ".repeat(pad)
            );
        }
        s += "\x1b[?2026l";
        self.prev = None;
        self.write(&s);
    }

    fn main_loop(&mut self, device: &mut Device, frx: &Receiver<FlutterEvent>) -> Result<(), String> {
        let mut last_poll = Instant::now() - Duration::from_secs(1);
        let mut last_status = Instant::now();
        loop {
            // input first: taps feel immediate
            while event::poll(Duration::from_millis(4)).map_err(|e| e.to_string())? {
                match event::read().map_err(|e| e.to_string())? {
                    Event::Key(k) if k.kind != KeyEventKind::Release => {
                        if !self.key(device, k) {
                            return Ok(());
                        }
                    }
                    Event::Mouse(m) => self.mouse(device, m.kind, m.column, m.row),
                    Event::Resize(..) => self.layout(device),
                    Event::Paste(text) if self.typing => device.insert_text(&text),
                    _ => {}
                }
            }
            if self.flutter(frx) {
                return Ok(());
            }
            if device.cdp.is_closed() {
                return Err("the browser closed".into());
            }
            // the newest frame only; every frame is acknowledged so the browser keeps sending them
            let mut newest = None;
            while let Ok(e) = device.events.try_recv() {
                match e.method.as_str() {
                    "Page.screencastFrame" => {
                        device
                            .cdp
                            .fire("Page.screencastFrameAck", serde_json::json!({ "sessionId": e.params["sessionId"] }));
                        newest = e.params["data"].as_str().map(String::from);
                    }
                    // a navigation (hot restart) stops the frames
                    "Page.loadEventFired" => {
                        let (w, h) = render::frame_size(self.render(), self.area, self.cell);
                        device.start_frames(w, h);
                    }
                    _ => {}
                }
            }
            if last_poll.elapsed() > Duration::from_millis(250) {
                last_poll = Instant::now();
                let typing = device.typing();
                if typing != self.typing {
                    self.typing = typing;
                    self.status(device);
                }
                if self.render() == Render::Text {
                    // a still screen sends no frames: new texts redraw the last one
                    let texts = device.text_layer();
                    if texts != self.texts {
                        self.texts = texts;
                        if newest.is_none() {
                            self.draw_screen(device);
                        }
                    }
                }
            }
            if let Some(data) = newest
                && let Ok(png) = base64::engine::general_purpose::STANDARD.decode(data)
            {
                self.png = Some(png);
                self.draw_screen(device);
            }
            if last_status.elapsed() > Duration::from_millis(500) {
                last_status = Instant::now();
                self.status(device);
            }
        }
    }

    /// A key: a command, or a key for the app. `false` = quit.
    fn key(&mut self, device: &mut Device, k: KeyEvent) -> bool {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let alt = k.modifiers.contains(KeyModifiers::ALT);
        if ctrl && k.code == KeyCode::Char('c') {
            return false;
        }
        if self.overlay != Overlay::None && matches!(k.code, KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('l')) {
            self.overlay = Overlay::None;
            self.layout(device);
            return true;
        }
        // a text field has the focus: keys are text, commands need Alt
        if self.typing && !alt {
            match k.code {
                KeyCode::Char(c) => device.insert_text(&c.to_string()),
                KeyCode::Esc => {
                    let _ = device.cdp.eval("document.activeElement && document.activeElement.blur()");
                }
                code => app_key(device, code, k.modifiers),
            }
            return true;
        }
        let KeyCode::Char(c) = k.code else {
            app_key(device, k.code, k.modifiers);
            return true;
        };
        match c {
            'q' => return false,
            'r' | 'R' => {
                self.state = State::Reloading;
                device.reload(c == 'R');
                self.status(device);
            }
            'b' => device.back(),
            'o' => {
                let (p, l) = (device.preset, !device.landscape);
                let _ = device.emulate(p, l, 1.0);
                self.layout(device);
            }
            'd' | 'D' => {
                let i = PRESETS.iter().position(|p| p.name == device.preset.name).unwrap_or(0);
                let n = PRESETS.len();
                let next = PRESETS[if c == 'd' { (i + 1) % n } else { (i + n - 1) % n }];
                let _ = device.emulate(next, device.landscape, 1.0);
                // the user agent changed: the app reads it when it starts
                let _ = device.cdp.call("Page.reload", serde_json::json!({}));
                self.layout(device);
            }
            'm' => {
                self.mode = (self.mode + 1) % self.modes.len();
                self.layout(device);
            }
            's' => match device.screenshot() {
                Ok(png) => {
                    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                    let path = self.screenshots.join(format!("device-{}-{secs}.png", self.number));
                    let saved = std::fs::create_dir_all(&self.screenshots).and_then(|_| std::fs::write(&path, png));
                    let msg = match saved {
                        Ok(()) => format!("screenshot: {}", path.display()),
                        Err(e) => format!("screenshot not saved: {e}"),
                    };
                    self.log(msg);
                    self.status(device);
                }
                Err(e) => self.log(format!("screenshot failed: {e}")),
            },
            'l' => {
                self.overlay = Overlay::Logs;
                self.draw_overlay();
            }
            '?' | 'h' => {
                self.overlay = Overlay::Help;
                self.draw_overlay();
            }
            _ => {}
        }
        true
    }

    fn mouse(&mut self, device: &Device, kind: MouseEventKind, col: u16, row: u16) {
        if self.overlay != Overlay::None {
            return;
        }
        let a = self.area;
        let inside = col > a.col && col <= a.col + a.cols && row > a.row && row <= a.row + a.rows;
        let (w, h) = device.css_size();
        let x = (col as f64 - a.col as f64 - 0.5) / a.cols as f64 * w as f64;
        let y = (row as f64 - a.row as f64 - 0.5) / a.rows as f64 * h as f64;
        match kind {
            MouseEventKind::Down(MouseButton::Left) if inside => {
                self.touching = true;
                device.touch(Phase::Down, x, y);
            }
            MouseEventKind::Drag(MouseButton::Left) if self.touching => device.touch(Phase::Move, x, y),
            MouseEventKind::Up(MouseButton::Left) if self.touching => {
                self.touching = false;
                device.touch(Phase::Up, x, y);
            }
            MouseEventKind::Down(MouseButton::Right) if inside => device.back(),
            MouseEventKind::ScrollDown if inside => device.wheel(x, y, 120.0),
            MouseEventKind::ScrollUp if inside => device.wheel(x, y, -120.0),
            _ => {}
        }
    }
}

/// Keys that are not text, for the app (DOM key name + Windows key code, what CDP wants).
fn app_key(device: &Device, code: KeyCode, mods: KeyModifiers) {
    let (key, vk) = match code {
        KeyCode::Enter => ("Enter", 13),
        KeyCode::Backspace => ("Backspace", 8),
        KeyCode::Tab => ("Tab", 9),
        KeyCode::BackTab => ("Tab", 9),
        KeyCode::Delete => ("Delete", 46),
        KeyCode::Left => ("ArrowLeft", 37),
        KeyCode::Up => ("ArrowUp", 38),
        KeyCode::Right => ("ArrowRight", 39),
        KeyCode::Down => ("ArrowDown", 40),
        KeyCode::Home => ("Home", 36),
        KeyCode::End => ("End", 35),
        KeyCode::PageUp => ("PageUp", 33),
        KeyCode::PageDown => ("PageDown", 34),
        KeyCode::Esc => ("Escape", 27),
        _ => return,
    };
    let shift = mods.contains(KeyModifiers::SHIFT) || code == KeyCode::BackTab;
    let modifiers = u32::from(mods.contains(KeyModifiers::ALT))
        | u32::from(mods.contains(KeyModifiers::CONTROL)) << 1
        | u32::from(shift) << 3;
    device.key(key, vk, modifiers);
}

fn hints(keys: &[(&str, &str)]) -> String {
    keys.iter()
        .map(|(k, v)| format!("\x1b[38;2;224;165;38m{k}\x1b[0m \x1b[2m{v}\x1b[0m"))
        .collect::<Vec<_>>()
        .join("  ")
}

/// A log line without its own colors and control characters (Dragonfly's logs come colored).
fn plain(s: &str) -> String {
    let mut out = String::new();
    let mut esc = false;
    for c in s.chars() {
        match (esc, c) {
            (false, '\x1b') => esc = true,
            (true, c) if c.is_ascii_alphabetic() => esc = false,
            (true, _) => {}
            (false, c) if c.is_control() => out.push(' '),
            (false, c) => out.push(c),
        }
    }
    out
}

/// Columns a string takes (escape codes take none).
fn visible_len(s: &str) -> usize {
    let mut n = 0;
    let mut esc = false;
    for c in s.chars() {
        match (esc, c) {
            (false, '\x1b') => esc = true,
            (true, c) if c.is_ascii_alphabetic() => esc = false,
            (true, _) => {}
            _ => n += 1,
        }
    }
    n
}

/// Cuts a string to `width` visible columns, keeping its escape codes.
fn clip(s: &str, width: usize) -> String {
    let mut out = String::new();
    let mut n = 0;
    let mut esc = false;
    for c in s.chars() {
        if esc {
            out.push(c);
            esc = !c.is_ascii_alphabetic();
        } else if c == '\x1b' {
            out.push(c);
            esc = true;
        } else if n < width {
            out.push(c);
            n += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_width_ignores_escape_codes() {
        let s = "\x1b[1;38;2;1;2;3mab\x1b[0mc";
        assert_eq!(visible_len(s), 3);
        assert_eq!(visible_len(&clip(s, 2)), 2);
        assert!(clip(s, 2).ends_with("\x1b[0m"));
    }
}
