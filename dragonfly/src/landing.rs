//! `dragonfly` without arguments: a landing page. On a terminal it opens with an animation, drawn as a
//! concept and not as a picture: a radioactive mushroom cloud rises, its shock wave rolls up to the camera
//! and fills the screen; it dissolves, life grows back and a dragonfly lands on a flower. It ends on the
//! NuclearLang and Dragonfly logos. Only characters on the terminal's own background, in its own text
//! color (normal or faint) with a few accents, so it fits any theme. Piped, too small or with `NO_COLOR`,
//! only the page is printed. Every other command starts with the header (`header`).

use std::f32::consts::PI;
use std::io::{IsTerminal, Write};
use std::time::{Duration, Instant};

type Rgb = (u8, u8, u8);

/// Width and height of the landing page in cells; the animation uses the same height.
const PAGE_W: usize = 80;
const H: usize = 22;
/// The horizon row.
const GROUND: i32 = 16;
/// Scene boundaries in seconds: the cloud rises, the wave, the screen fills and dissolves, life grows
/// back, the dragonfly lands and rests, the scene dissolves, then the page.
const RISE: f32 = 2.6;
const WAVE: f32 = 4.0;
const FLASH: f32 = 6.1;
const GROW: f32 = 7.6;
const FLY_START: f32 = 7.1;
const LAND: f32 = 8.8;
const REST: f32 = 9.8;
const FADE: f32 = 10.2;
const END: f32 = 11.2;
/// On the live page, the dragonfly shines every this many seconds.
const SHINE_EVERY: f32 = 5.0;
/// "WELCOME" in 5×5 / 4×5 letters (`#` = a pixel, two cells wide), shown when the blast ends.
const WELCOME: [&str; 5] = [
    "#...#.####.#.....###..##..#...#.####",
    "#...#.#....#....#....#..#.##.##.#...",
    "#.#.#.###..#....#....#..#.#.#.#.###.",
    "##.##.#....#....#....#..#.#...#.#...",
    "#...#.####.####..###..##..#...#.####",
];

const LOGO: [&str; 17] = [
    "                               ",
    "             .=#=.             ",
    "            (@@#@@)            ",
    "   .:=+**+=-. '#' .-=+**+=:.   ",
    " .=*#########*=#=*#########*=. ",
    "=*############*#*############*=",
    "'=*##########*=#=*##########*='",
    "  ':=+****+=-:.#.:-=+****+=:'  ",
    "   .-=+****+=::#::=+****+=-.   ",
    "  =*#########*=#=*#########*=  ",
    "  '=*#######*=-#-=*#######*='  ",
    "    ':-==-:'  '#'  ':-==-:'    ",
    "              :#:              ",
    "              :#:              ",
    "              :#:              ",
    "              :+:              ",
    "               '               ",
];
/// The dragonfly logo, small (the header of `dragonfly help`).
const MINI: [&str; 6] = [
    "       .=#=.       ",
    "  .=*##(@#@)##*=.  ",
    " '=*#*=-'#'-=*#*=' ",
    "  .=*#*=:#:=*#*=.  ",
    "        :#:        ",
    "         '         ",
];
/// The dragonfly seen from the side (head to the right): wings up, level, down.
const SPRITE: [[&str; 3]; 3] = [
    [r"     \\ \\ ", r"~~~~==##@>", r"          "],
    [r"     __ __ ", r"~~~~==##@>", r"          "],
    [r"          ", r"~~~~==##@>", r"     // // "],
];
/// Cloud characters from thin to dense.
const RAMP: [char; 9] = ['.', ':', '-', '=', '+', '*', '#', '%', '@'];
const YELLOW: Rgb = (253, 216, 63);
const SLATE: Rgb = (55, 71, 79);
/// Dragonfly: an intense, almost radioactive, metallic purple.
const PURPLE: Rgb = (176, 84, 255);
/// How much wider than tall a half-block pixel must be drawn to look square: terminal cells are usually
/// more than twice as tall as wide (line spacing).
const ASPECT: f32 = 1.35;
const GREEN: Rgb = (120, 230, 150);
const BLUE: Rgb = (90, 180, 255);
const TOXIC: Rgb = (140, 255, 90);
const GRASS: Rgb = (100, 200, 100);
const PINK: Rgb = (255, 110, 180);
const RED: Rgb = (255, 95, 95);
/// Text attributes: normal, bold, faint (SGR 0, 1, 2).
const NORMAL: u8 = 0;
const BOLD: u8 = 1;
const FAINT: u8 = 2;

/// `dragonfly` without arguments: the animation the first time (then the page alone); `credits`: always the
/// animation, ending on the author and the repository.
pub fn show(credits: bool) {
    let color = std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none();
    let seen = seen_marker();
    let first = !seen.as_ref().is_some_and(|p| p.exists());
    match term_size().filter(|&(cols, rows)| color && cols >= PAGE_W && rows > H && (credits || first)) {
        Some((cols, _)) => {
            animate(cols.min(100), credits);
            if let Some(p) = seen {
                let _ = p.parent().map(std::fs::create_dir_all);
                let _ = std::fs::write(p, "");
            }
        }
        None => print!("{}", frame(END, PAGE_W, credits).render(color)),
    }
}

/// Marks that the intro was shown: `$XDG_STATE_HOME/dragonfly/intro_seen` (`~/.local/state/...`).
fn seen_marker() -> Option<std::path::PathBuf> {
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| Some(std::path::PathBuf::from(std::env::var_os("HOME")?).join(".local/state")))?;
    Some(state.join("dragonfly/intro_seen"))
}

/// The header every command (and `help`) starts with: both logos, small, with their names. On stderr and only
/// on a terminal, so piped output (`check --json`) stays clean.
pub fn header() {
    if !std::io::stderr().is_terminal() {
        return;
    }
    eprintln!();
    let mut c = Canvas::new(PAGE_W, 6);
    sign(&mut c, 1, 0, 6.0);
    c.ink_text(19, 1, "NuclearLang", Some(YELLOW), BOLD);
    c.ink_text(19, 2, "Write only the logic;", None, FAINT);
    c.ink_text(19, 3, "transpilers do the rest.", None, FAINT);
    for (y, row) in MINI.iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            let color = if ch == '@' { TOXIC } else { metal(x as i32, y as i32) };
            if ch != ' ' {
                c.ink(44 + x as i32, y as i32, ch, Some(color), NORMAL);
            }
        }
    }
    c.ink_text(64, 1, "Dragonfly", Some(PURPLE), BOLD);
    c.ink_text(64, 2, "Flutter apps in", None, NORMAL);
    c.ink_text(64, 3, "NuclearLang", None, NORMAL);
    c.ink_text(64, 4, concat!("v", env!("CARGO_PKG_VERSION")), None, FAINT);
    eprintln!("{}", c.render(std::env::var_os("NO_COLOR").is_none()));
}

/// Command output in the page's colors. Only when stdout and stderr are terminals and without `NO_COLOR`, so
/// piped or redirected output stays plain.
fn paint(fg: Option<Rgb>, sgr: u8, s: &str) -> String {
    let on =
        std::io::stdout().is_terminal() && std::io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none();
    match (on, fg) {
        (false, _) => s.to_string(),
        (true, Some((r, g, b))) => format!("\x1b[{sgr};38;2;{r};{g};{b}m{s}\x1b[0m"),
        (true, None) => format!("\x1b[{sgr}m{s}\x1b[0m"),
    }
}

/// Success.
pub fn ok(s: &str) -> String {
    paint(Some(GREEN), BOLD, s)
}

/// Errors.
pub fn bad(s: &str) -> String {
    paint(Some(RED), BOLD, s)
}

/// Warnings and notes.
pub fn warn(s: &str) -> String {
    paint(Some(YELLOW), NORMAL, s)
}

/// Commands the user can type.
pub fn cmd(s: &str) -> String {
    paint(Some(BLUE), NORMAL, s)
}

/// Project, step and section names.
pub fn name(s: &str) -> String {
    paint(Some(PURPLE), BOLD, s)
}

/// Paths and secondary details.
pub fn dim(s: &str) -> String {
    paint(None, FAINT, s)
}

/// Compiler diagnostics (`error[N0106]: ...`, `--> file:line:col`, `= fix: ...`) with those colors.
pub fn diagnostics(text: &str) -> String {
    text.lines()
        .map(|l| match l.split_once(": ") {
            Some((k, rest)) if k.starts_with("error[") => format!("{}: {rest}", bad(k)),
            Some((k, rest)) if k.starts_with("warning[") => format!("{}: {rest}", warn(k)),
            _ if l.trim_start().starts_with('|') && l.contains('^') => bad(l),
            _ if l.trim_start().starts_with("-->") || l.trim_start().starts_with('|') => dim(l),
            _ if l.trim_start().starts_with("= fix:") => ok(l),
            _ => l.to_string(),
        })
        .map(|l| l + "\n")
        .collect()
}

/// `dragonfly help` on a terminal: every section and command with its symbol (one cell wide, so the columns
/// stay aligned) and its section's color. Piped or with `NO_COLOR`, the plain text.
pub fn usage(text: &str) -> String {
    if !std::io::stdout().is_terminal() || std::env::var_os("NO_COLOR").is_some() {
        return text.to_string();
    }
    let paint = |c: Option<Rgb>, sgr: u8, s: &str| {
        let fg = c.map_or(String::new(), |(r, g, b)| format!("\x1b[38;2;{r};{g};{b}m"));
        format!("\x1b[{sgr}m{fg}{s}\x1b[0m")
    };
    let mut section: Option<Rgb> = None;
    let mut out = String::new();
    for line in text.lines() {
        let word = line.trim_start().split([' ', ':']).next().unwrap_or("");
        let styled = match () {
            _ if line.is_empty() => String::new(),
            _ if line.starts_with("usage:") => paint(None, BOLD, line),
            _ if !line.starts_with(' ') => {
                let (icon, color) = match word {
                    "project" => ("✦", Some(YELLOW)),
                    "code" => ("✎", Some(PURPLE)),
                    "packages" => ("◈", Some(BLUE)),
                    "setup" => ("✚", Some(GREEN)),
                    "options" => ("⚑", None),
                    _ => ("❯", None),
                };
                section = color;
                format!("{} {}", paint(color, BOLD, icon), paint(color, BOLD, line))
            }
            _ if line.starts_with("  ") && !line.starts_with("   ") => {
                let icon = match word {
                    "new" => "✿",
                    "code" => "⌨",
                    "watch" => "◉",
                    "build" => "⚙",
                    "run" => "▶",
                    "device" => "▯",
                    "terminal-device" => "▤",
                    "test" => "⚗",
                    "analyze" => "⌕",
                    "check" => "✔",
                    "clean" => "♻",
                    "component" => "❖",
                    "screen" => "▣",
                    "usecase" => "➜",
                    "repository" => "⇅",
                    "model" => "◇",
                    "service" => "✱",
                    "aggregate" => "⊕",
                    "config" => "☰",
                    "event" => "✉",
                    "widget" => "◧",
                    "guard" => "⊘",
                    "global" => "◎",
                    "page" => "▤",
                    "integration" => "⇄",
                    "pub" => "◈",
                    "devices" => "▭",
                    "flutter" | "FLUTTER" => "◆",
                    "doctor" => "✚",
                    "setup" => "⇩",
                    "credits" => "★",
                    "--project" => "⌂",
                    _ => " ",
                };
                let rest = &line[2..];
                let name_end = rest.find(' ').unwrap_or(rest.len());
                format!(
                    "{} {}{}",
                    paint(section, NORMAL, icon),
                    paint(section, BOLD, &rest[..name_end]),
                    &rest[name_end..]
                )
            }
            _ => paint(None, FAINT, line),
        };
        out.push_str(&styled);
        out.push('\n');
    }
    out
}

/// Plays the animation, then keeps the page alive (the dragonfly shines every `SHINE_EVERY` seconds) until
/// Enter. Enter during the animation skips to the page, except in `credits`.
fn animate(w: usize, credits: bool) {
    let mut out = std::io::stdout().lock();
    let _ = write!(out, "{}", "\n".repeat(H));
    let (tx, enter) = std::sync::mpsc::channel();
    let live = std::io::stdin().is_terminal();
    if live {
        std::thread::spawn(move || {
            // true = a real Enter; EOF or an error (false) only ends the page, never the animation
            let _ = tx.send(std::io::stdin().read_line(&mut String::new()).is_ok_and(|n| n > 0));
        });
    }
    let start = Instant::now();
    let mut pressed = None;
    loop {
        let t = start.elapsed().as_secs_f32();
        // `credits` always plays the whole animation; an early Enter ends it once the page is complete
        pressed = pressed.or(enter.try_recv().ok());
        if pressed.is_some_and(|real| t >= END || (real && !credits)) {
            // the echoed Enter moved the cursor one line further down
            let _ = write!(out, "\x1b[?2026h\x1b[{}F{}\x1b[?2026l", H + 1, frame(END, w, credits).render(true));
            break;
        }
        let mut c = frame(if live { t } else { t.min(END) }, w, credits);
        if live && t >= END {
            c.ink_text((w - PAGE_W) as i32 / 2 + 35, H as i32 - 1, "↵ Enter to continue", None, FAINT);
        }
        // synchronized output (?2026): the terminal shows each frame whole
        let _ = write!(out, "\x1b[?2026h\x1b[{H}F{}\x1b[?2026l", c.render(true));
        let _ = out.flush();
        if !live && t >= END {
            break;
        }
        std::thread::sleep(Duration::from_millis(33));
    }
    let _ = out.flush();
}

/// Rows and columns of the terminal (`stty size`, no dependencies).
fn term_size() -> Option<(usize, usize)> {
    let tty = std::fs::File::open("/dev/tty").ok()?;
    let out = std::process::Command::new("stty").arg("size").stdin(tty).output().ok()?;
    let text = String::from_utf8(out.stdout).ok()?;
    let mut nums = text.split_whitespace().map(|n| n.parse::<usize>().ok());
    let (rows, cols) = (nums.next()??, nums.next()??);
    Some((cols, rows))
}

/// The picture at `t` seconds, `w` cells wide.
fn frame(t: f32, w: usize, credits: bool) -> Canvas {
    let mut c = Canvas::new(w, H);
    let off = (w - PAGE_W) as i32 / 2;
    if t < RISE {
        let p = ease(t / RISE);
        c.cells(|x, y| blast(x, y, w, p, t));
    } else if t < WAVE {
        let q = (t - RISE) / (WAVE - RISE);
        c.cells(|x, y| wave(x, y, w, q, t));
    } else if t < FLASH {
        welcome(&mut c, w, (t - WAVE) / (FLASH - WAVE));
    } else if t < FADE {
        meadow(&mut c, t, w, off + 18);
        motto(&mut c, t, w);
        c.dissolve(1.0 - ((t - REST) / (FADE - REST)).clamp(0.0, 1.0));
    } else {
        let h = ((t - FADE) / (END - FADE)).min(1.0);
        // a bright band crosses the dragonfly every SHINE_EVERY seconds once the page is complete
        let s = t - END - 1.0;
        let shine = (s >= 0.0 && s % SHINE_EVERY < 0.9).then(|| lerp(-6.0, 50.0, (s % SHINE_EVERY) / 0.9));
        logo(&mut c, off + 2, 2, shine);
        page(&mut c, off + 35, h, credits);
        c.dissolve(h * 2.0);
    }
    c
}

/// Center of a cell in square pixels (a cell is two pixels tall) and the ground line.
fn px(x: i32, y: i32) -> (f32, f32, f32) {
    (x as f32 + 0.5, y as f32 * 2.0 + 1.0, GROUND as f32 * 2.0 + 1.0)
}

/// Scene 1: the mushroom cloud rising (`p` 0 → 1): dense characters glow green, the thin ones are faint.
fn blast(x: i32, y: i32, w: usize, p: f32, t: f32) -> Cell {
    let (fx, fy, _) = px(x, y);
    match heat(fx, fy, w as f32, p, t) {
        Some(h) => {
            // brighter and denser than the raw heat: in characters a thin cloud reads as empty
            let h = (h * 1.5 + 0.12).min(1.0);
            let ch = RAMP[(h * (RAMP.len() - 1) as f32).round() as usize];
            match h {
                _ if h > 0.85 => Cell::new(ch, None, BOLD),
                _ if h > 0.5 => Cell::new(ch, Some(TOXIC), NORMAL),
                _ => Cell::new(ch, None, FAINT),
            }
        }
        None => scenery(x, y),
    }
}

/// How hot the cloud is at a pixel (`None` outside it): cap, stem and the dust skirt at its base.
fn heat(x: f32, y: f32, w: f32, p: f32, t: f32) -> Option<f32> {
    let (cx, gy, n) = (w / 2.0, GROUND as f32 * 2.0 + 1.0, noise(x, y, t));
    let top = gy - H as f32 * 2.0 * 0.58 * p;
    let (rx, ry) = (2.0 + w * 0.16 * p, (2.0 + w * 0.16 * p) * 0.55);
    let mut heat: Option<f32> = None;
    let mut add = |h: f32| heat = Some(heat.map_or(h, |o: f32| o.max(h)));
    let cap = ((x - cx) / rx).powi(2) + ((y - top) / ry).powi(2);
    if cap < 1.0 + 0.25 * n {
        let shade = if y > top { 0.8 } else { 1.0 };
        add(((1.0 - cap).max(0.0) * (1.15 - p * 0.7) + 0.15 * n) * shade);
    }
    let sw = 1.5 + 2.5 * p + (y - top) / (gy - top).max(1.0) * 2.0 + (y * 0.8 + t * 6.0).sin() * 0.5;
    if y >= top && y <= gy && (x - cx).abs() < sw {
        add((0.55 - 0.3 * p) * (1.0 - (x - cx).abs() / sw) + 0.1 * n);
    }
    let skirt = ((x - cx) / (3.0 + w * 0.2 * p)).powi(2) + ((y - gy) / (1.5 + 3.0 * p)).powi(2);
    if y <= gy && skirt < 1.0 + 0.2 * n {
        add(0.3 * (1.0 - skirt).max(0.0) + 0.1 * n);
    }
    heat
}

/// Scene 2: the shock wave spreads over the ground (`q` 0 → 0.55), then rolls up to the camera.
fn wave(x: i32, y: i32, w: usize, q: f32, t: f32) -> Cell {
    let (fx, fy, gy) = px(x, y);
    let n = noise(fx, fy, t);
    let rise = ease(((q - 0.55) / 0.45).clamp(0.0, 1.0));
    let wall_top = H as f32 * 2.0 + 2.0 - rise * (H as f32 * 2.0 + 8.0);
    if fy > wall_top + 3.0 {
        return dust(x, y, n, t, rise);
    } else if fy > wall_top {
        return crest(x, t);
    }
    let r = ease((q / 0.6).min(1.0));
    let (rx, ry) = (0.5 + r * w as f32 * 0.9, (0.5 + r * w as f32 * 0.9) * 0.35);
    let dx = (fx - w as f32 / 2.0) / rx;
    if fy > gy {
        let e = (dx * dx + ((fy - gy) / ry).powi(2)).sqrt();
        if e > 0.84 && e < 1.0 {
            return crest(x, t);
        } else if e < 1.0 {
            return dust(x, y, n, t, 0.0);
        }
    } else if dx.abs() < 1.0 && gy - fy < (2.0 + 6.0 * r) * (1.0 - dx * dx).sqrt() {
        return dust(x, y, n, t, 0.0);
    }
    blast(x, y, w, 1.0, t)
}

/// Dust of the wave; `dense` 0 → 1 packs it toward a full screen.
fn dust(x: i32, y: i32, n: f32, t: f32, dense: f32) -> Cell {
    const DUST: [char; 6] = ['.', ':', ';', '%', '#', '&'];
    let k = (n * 0.5 + 0.5) * 4.0 + hash(x, y + (t * 6.0) as i32) * 2.0 + dense * 3.0;
    Cell::new(DUST[(k as usize).min(5)], None, if dense > 0.6 { NORMAL } else { FAINT })
}

fn crest(x: i32, t: f32) -> Cell {
    Cell::new(if (x + (t * 14.0) as i32) % 3 == 0 { '^' } else { '~' }, None, BOLD)
}

/// A faint horizon and a few faint stars.
fn scenery(x: i32, y: i32) -> Cell {
    match () {
        _ if y == GROUND => Cell::new('_', None, FAINT),
        _ if y < GROUND / 2 && hash(x, y) < 0.02 => Cell::new('.', None, FAINT),
        _ => Cell::new(' ', None, NORMAL),
    }
}

/// Scene 3 (`f` 0 → 1): the screen is full; "WELCOME" shows through it, the rest dissolves (█ ▓ ▒ ░) and
/// the word stays in radioactive green with "to NuclearLang · Dragonfly" under it, then it dissolves too.
fn welcome(c: &mut Canvas, w: usize, f: f32) {
    let rest = ((f - 0.12) / 0.4).clamp(0.0, 1.0);
    c.cells(|x, y| {
        let level = ((1.0 - rest) * 4.0 + hash(x, y) - 0.5).clamp(0.0, 4.0) as usize;
        Cell::new([' ', '░', '▒', '▓', '█'][level], None, NORMAL)
    });
    let (x0, y0) = ((w as i32 - WELCOME[0].len() as i32 * 2) / 2, H as i32 / 2 - 4);
    let keep = 1.0 - ((f - 0.75) / 0.25).clamp(0.0, 1.0);
    for (y, row) in WELCOME.iter().enumerate() {
        for (x, px) in row.chars().enumerate() {
            for dx in 0..2 {
                let (cx, cy) = (x0 + x as i32 * 2 + dx, y0 + y as i32);
                match () {
                    _ if px != '#' => {}
                    // first the word is a hole in the full screen
                    _ if f < 0.12 => c.ink(cx, cy, ' ', None, NORMAL),
                    _ if hash(cx, cy) < keep => c.ink(cx, cy, '█', Some(TOXIC), NORMAL),
                    _ => c.ink(cx, cy, ' ', None, NORMAL),
                }
            }
        }
    }
    let typed = ((f - 0.3) / 0.25).clamp(0.0, 1.0);
    let line = [
        ("to ", None, FAINT),
        ("NuclearLang", Some(YELLOW), BOLD),
        (" · ", None, FAINT),
        ("Dragonfly", Some(PURPLE), BOLD),
    ];
    let total: usize = line.iter().map(|p| p.0.chars().count()).sum();
    let (mut x, mut left) = ((w as i32 - total as i32) / 2, (typed * total as f32) as usize);
    for (text, color, sgr) in line {
        for ch in text.chars() {
            let shown = left > 0 && hash(x, y0 + 7) < keep;
            c.ink(x, y0 + 7, if shown { ch } else { ' ' }, color, sgr);
            left = left.saturating_sub(1);
            x += 1;
        }
    }
}

/// Scenes 4 and 5: life grows back (`g` 0 → 1) and the dragonfly lands on the flower at column `fx`.
fn meadow(c: &mut Canvas, t: f32, w: usize, fx: i32) {
    let g = ((t - FLASH) / (GROW - FLASH)).clamp(0.0, 1.0);
    c.cells(|x, y| match () {
        _ if y == GROUND => Cell::new('_', None, FAINT),
        _ if y > GROUND && hash(x, y) < g * 0.25 => {
            Cell::new(if hash(x, y) < 0.1 { ',' } else { '.' }, Some(GRASS), FAINT)
        }
        _ => Cell::new(' ', None, NORMAL),
    });
    // a new day: a faint sun coming up
    let sun_y = GROUND - 1 - (4.0 * g) as i32;
    let sun_x = (w as f32 * 0.72) as i32;
    c.ink_text(sun_x - 3, sun_y - 1, r" \  |  /", None, FAINT);
    c.ink_text(sun_x - 3, sun_y, "-- ( ) --", None, FAINT);
    // grass, and a few small flowers on top of it
    for x in 0..w as i32 {
        let r = hash(x, 7);
        let tall = ((1.0 + r * 2.5) * ease(g * 1.6 - r * 0.6)).round() as i32;
        for k in 1..=tall {
            let ch = [',', ';', '\'', '|', '/', '\\', '"'][(hash(x, k) * 7.0) as usize];
            c.ink(x, GROUND - k + 1, ch, Some(GRASS), if hash(x, k + 3) < 0.5 { FAINT } else { NORMAL });
        }
        if r > 0.88 && tall > 0 && g > 0.5 + (r - 0.88) * 2.0 {
            let color = if r > 0.91 { YELLOW } else { PINK };
            c.ink(x, GROUND - tall, if r > 0.94 { '*' } else { 'o' }, Some(color), NORMAL);
        }
    }
    // the flower: stem, leaves, then bud → half open → open
    let stem = (6.0 * ease(g * 1.2)).round() as i32;
    for k in 1..=stem {
        c.ink(fx, GROUND - k, '|', Some(GRASS), NORMAL);
    }
    if stem >= 2 {
        c.ink(fx - 1, GROUND - 2, '\\', Some(GRASS), NORMAL);
    }
    if stem >= 4 {
        c.ink(fx + 1, GROUND - 4, '/', Some(GRASS), NORMAL);
    }
    let head = GROUND - stem - 1;
    if g >= 0.95 {
        c.ink_text(fx - 2, head - 2, ".-.-.", Some(PINK), NORMAL);
        c.ink_text(fx - 3, head - 1, "( (o) )", Some(PINK), NORMAL);
        c.ink_text(fx - 2, head, "'-.-'", Some(PINK), NORMAL);
        c.ink(fx, head - 1, 'o', Some(YELLOW), BOLD);
    } else if g >= 0.8 {
        c.ink_text(fx - 1, head, "(o)", Some(PINK), NORMAL);
        c.ink(fx, head, 'o', Some(YELLOW), BOLD);
    } else if stem >= 3 {
        c.ink(fx, head, 'o', Some(GRASS), NORMAL);
    }
    // the dragonfly: flies in on a wavy path, lands on the flower, its wings slow down and rest
    if t >= FLY_START {
        let f = ((t - FLY_START) / (LAND - FLY_START)).clamp(0.0, 1.0);
        let body_y = GROUND - 6 - 4;
        let x = lerp(-12.0, (fx - 6) as f32, ease(f));
        let y = body_y as f32 + (f * PI * 3.0).sin() * 3.0 * (1.0 - f);
        let beat = if t < LAND { 16.0 } else { 5.0 };
        let wings = match () {
            _ if t > LAND + 0.6 => 0,
            // close to the flower the wings stay up, off the petals
            _ if t > LAND - 0.3 => [0, 1][(t * beat) as usize % 2],
            _ => [0, 1, 2, 1][(t * beat) as usize % 4],
        };
        for (i, row) in SPRITE[wings].iter().enumerate() {
            for (j, ch) in row.chars().enumerate() {
                let (color, sgr) = match ch {
                    ' ' => continue,
                    '@' => (Some(TOXIC), BOLD),
                    '\\' | '/' | '_' => (None, FAINT),
                    _ => (Some(metal(j as i32, i as i32)), NORMAL),
                };
                c.ink(x as i32 + j as i32, y.round() as i32 - 1 + i as i32, ch, color, sgr);
            }
        }
    }
}

/// After WELCOME, on the right of the meadow: the motto, typed in, in Spanish when the locale is Spanish.
fn motto(c: &mut Canvas, t: f32, w: usize) {
    let spanish = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
        .is_some_and(|v| v.starts_with("es"));
    let lines = if spanish {
        ["Antes todo era caos,", "ahora todo es más simple", "con "]
    } else {
        ["Before, everything was chaos;", "now everything is simpler", "with "]
    };
    let total = lines.iter().map(|l| l.chars().count()).sum::<usize>() + "Dragonfly".len();
    let mut left = (((t - FLASH - 0.4) / 1.6).clamp(0.0, 1.0) * total as f32) as usize;
    let x0 = w as i32 / 2 + 4;
    for (y, line) in lines.iter().enumerate() {
        let tail = if y == 2 { "Dragonfly" } else { "" };
        for (x, ch) in line.chars().chain(tail.chars()).enumerate() {
            if left == 0 {
                return;
            }
            let (color, sgr) = if x >= line.chars().count() { (Some(PURPLE), BOLD) } else { (None, NORMAL) };
            c.ink(x0 + x as i32, 3 + y as i32, ch, color, sgr);
            left -= 1;
        }
    }
}

/// The dragonfly logo, with the shine band at diagonal position `shine`.
fn logo(c: &mut Canvas, x0: i32, y0: i32, shine: Option<f32>) {
    for (y, row) in LOGO.iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            let color = match ch {
                '@' => TOXIC,
                ' ' => continue,
                _ => metal(x as i32, y as i32),
            };
            // the shine: a diagonal band, brightest in its middle
            let glow = shine.map_or(0.0, |pos| (1.0 - (x as f32 + y as f32 * 1.6 - pos).abs() / 3.5).max(0.0));
            let sgr = if glow > 0.5 { BOLD } else { NORMAL };
            c.ink(x0 + x as i32, y0 + y as i32, ch, Some(mix(color, (255, 245, 255), glow)), sgr);
        }
    }
}

/// The right column: the NuclearLang sign and both names, then how to start (or the credits); `h` 0 → 1
/// types it in.
fn page(c: &mut Canvas, x0: i32, h: f32, credits: bool) {
    sign(c, x0, 1, 7.0);
    let mut lines: Vec<(i32, i32, &str, Option<Rgb>, u8)> = vec![
        (21, 3, "NuclearLang", Some(YELLOW), BOLD),
        (21, 4, "Write only the logic;", None, FAINT),
        (21, 5, "transpilers do the rest.", None, FAINT),
        (0, 10, "Dragonfly", Some(PURPLE), BOLD),
        (0, 11, "Flutter apps written in NuclearLang", None, NORMAL),
        (0, 12, concat!("v", env!("CARGO_PKG_VERSION")), None, FAINT),
        (0, 13, "────────────────────────────────────────", None, FAINT),
    ];
    lines.extend(match credits {
        true => [
            (0, 15, "Credits", None, BOLD),
            (0, 16, "★ Julian Molina", Some(PURPLE), BOLD),
            (0, 17, "  github.com/nodelorien", None, NORMAL),
            (0, 18, "◆ github.com/nuclearlang", Some(BLUE), NORMAL),
            (0, 20, "  thanks for trying NuclearLang", None, FAINT),
        ]
        .to_vec(),
        false => [
            (0, 15, "Get started", None, BOLD),
            (0, 16, "> dragonfly new <name>     a new app", Some(GREEN), NORMAL),
            (0, 17, "  dragonfly code           open in VS Code", None, NORMAL),
            (0, 18, "  dragonfly run            run on a device", None, NORMAL),
            (0, 19, "  dragonfly g s <c> <s>    add a screen", None, NORMAL),
            (0, 20, "  dragonfly help           all commands", None, FAINT),
        ]
        .to_vec(),
    });
    for (i, (dx, y, text, color, sgr)) in lines.into_iter().enumerate() {
        let shown = ((h * 1.6 - i as f32 * 0.05) * text.chars().count() as f32).max(0.0) as usize;
        c.ink_text(x0 + dx, y, &text.chars().take(shown).collect::<String>(), color, sgr);
    }
}

/// The NuclearLang sign (the radiation symbol) in half-block pixels, `r` cells tall and `2r × ASPECT` wide.
fn sign(c: &mut Canvas, x0: i32, y0: i32, r: f32) {
    let (cx, cy) = (x0 as f32 + r * ASPECT, y0 as f32 * 2.0 + r);
    let pixel = |px: f32, py: f32| {
        let (dx, dy) = ((px + 0.5 - cx) / (r * ASPECT), (py + 0.5 - cy) / r);
        let d = (dx * dx + dy * dy).sqrt();
        let angle = dy.atan2(dx).to_degrees() + 90.0; // 0 = up
        let blade = [0.0f32, 120.0, 240.0].iter().any(|b| ((angle - b).rem_euclid(360.0) + 30.0) % 360.0 < 60.0);
        (d <= 1.0).then_some(if d < 0.25 || (blade && d > 0.3 && d < 0.78) { SLATE } else { YELLOW })
    };
    c.pixels_in(x0, y0, (2.0 * r * ASPECT).round() as usize, r as usize, pixel);
}

/// Metallic purple at a cell of a dragonfly drawing: dark and vivid bands with a bright sheen across.
fn metal(x: i32, y: i32) -> Rgb {
    let (x, y) = (x as f32, y as f32);
    let band = (x * 0.45 + y * 1.1).sin() * 0.5 + 0.5;
    let sheen = (1.0 - (x * 0.3 - y * 0.6 - 2.0).sin().abs()).powi(6);
    mix(mix((105, 35, 200), (200, 115, 255), band), (245, 220, 255), sheen)
}

fn noise(x: f32, y: f32, t: f32) -> f32 {
    ((x * 0.7 + t * 2.0).sin() * (y * 0.9 - t * 1.5).cos() + (x * 0.31 - y * 0.47 + t * 3.1).sin()) * 0.5
}

fn hash(x: i32, y: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(374_761_393) ^ (y as u32).wrapping_mul(668_265_263);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    (h ^ (h >> 16)) as f32 / u32::MAX as f32
}

fn ease(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn lerp(a: f32, b: f32, k: f32) -> f32 {
    a + (b - a) * k
}

fn mix(a: Rgb, b: Rgb, k: f32) -> Rgb {
    let k = k.clamp(0.0, 1.0);
    let ch = |a: u8, b: u8| lerp(a as f32, b as f32, k) as u8;
    (ch(a.0, b.0), ch(a.1, b.1), ch(a.2, b.2))
}

/// One character cell. `fg`/`bg` `None` = the terminal's own colors; `sgr` = NORMAL, BOLD or FAINT.
#[derive(Clone, Copy, PartialEq)]
struct Cell {
    ch: char,
    fg: Option<Rgb>,
    bg: Option<Rgb>,
    sgr: u8,
}

impl Cell {
    fn new(ch: char, fg: Option<Rgb>, sgr: u8) -> Self {
        Cell { ch, fg, bg: None, sgr }
    }
}

struct Canvas {
    w: usize,
    cells: Vec<Cell>,
}

impl Canvas {
    fn new(w: usize, h: usize) -> Self {
        Canvas { w, cells: vec![Cell::new(' ', None, NORMAL); w * h] }
    }

    fn put(&mut self, x: i32, y: i32, cell: Cell) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) * self.w < self.cells.len() {
            self.cells[y as usize * self.w + x as usize] = cell;
        }
    }

    /// Fills every cell with `f(x, y)`.
    fn cells(&mut self, f: impl Fn(i32, i32) -> Cell) {
        for y in 0..(self.cells.len() / self.w) as i32 {
            for x in 0..self.w as i32 {
                self.put(x, y, f(x, y));
            }
        }
    }

    fn ink(&mut self, x: i32, y: i32, ch: char, fg: Option<Rgb>, sgr: u8) {
        self.put(x, y, Cell::new(ch, fg, sgr));
    }

    fn ink_text(&mut self, x: i32, y: i32, s: &str, fg: Option<Rgb>, sgr: u8) {
        for (i, ch) in s.chars().enumerate() {
            self.ink(x + i as i32, y, ch, fg, sgr);
        }
    }

    /// Keeps a share `k` (0 → 1) of the cells, always the same ones: a fade that works on any background.
    fn dissolve(&mut self, k: f32) {
        for (i, cell) in self.cells.iter_mut().enumerate() {
            if hash(i as i32, 99) >= k {
                *cell = Cell::new(' ', None, NORMAL);
            }
        }
    }

    /// Pixels of a `w`×`h` cell area at (`x0`, `y0`), two per cell (`▀`: top = fg, bottom = bg); `f` gets
    /// canvas pixel coordinates (y doubled).
    fn pixels_in(&mut self, x0: i32, y0: i32, w: usize, h: usize, f: impl Fn(f32, f32) -> Option<Rgb>) {
        for cy in 0..h as i32 {
            for cx in 0..w as i32 {
                let (x, y) = (x0 + cx, y0 + cy);
                let (top, bottom) = (f(x as f32, y as f32 * 2.0), f(x as f32, y as f32 * 2.0 + 1.0));
                match (top, bottom) {
                    (None, None) => {}
                    (Some(t), b) => self.put(x, y, Cell { ch: '▀', fg: Some(t), bg: b, sgr: NORMAL }),
                    (None, b) => self.put(x, y, Cell::new('▄', b, NORMAL)),
                }
            }
        }
    }

    /// ANSI text (escapes only where the style changes); without `color`, plain text.
    fn render(&self, color: bool) -> String {
        let mut out = String::new();
        for row in self.cells.chunks(self.w) {
            let mut last: Option<Cell> = None;
            let mut line = String::new();
            for cell in row {
                if color && last.is_none_or(|l| (l.fg, l.bg, l.sgr) != (cell.fg, cell.bg, cell.sgr)) {
                    line.push_str("\x1b[0m");
                    if cell.sgr != NORMAL {
                        line.push_str(&format!("\x1b[{}m", cell.sgr));
                    }
                    if let Some((r, g, b)) = cell.fg {
                        line.push_str(&format!("\x1b[38;2;{r};{g};{b}m"));
                    }
                    if let Some((r, g, b)) = cell.bg {
                        line.push_str(&format!("\x1b[48;2;{r};{g};{b}m"));
                    }
                }
                line.push(cell.ch);
                last = Some(*cell);
            }
            if color {
                line.push_str("\x1b[0m\x1b[K");
            } else {
                line.truncate(line.trim_end().len());
            }
            out.push_str(&line);
            out.push('\n');
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every scene draws without panicking at the smallest and largest width, the animation never paints a
    /// background (only the NuclearLang sign uses one), and the last frame is the page.
    #[test]
    fn every_frame_draws_and_ends_on_the_page() {
        for w in [PAGE_W, 100] {
            let mut t = 0.0;
            while t < FADE {
                assert!(frame(t, w, false).cells.iter().all(|c| c.bg.is_none()), "background at {t}s");
                t += 0.05;
            }
        }
        let page = frame(END, PAGE_W, false).render(false);
        assert!(page.contains("NuclearLang") && page.contains("Dragonfly") && page.contains("dragonfly help"));
        let credits = frame(END, PAGE_W, true).render(false);
        assert!(credits.contains("Julian Molina") && credits.contains("github.com/nodelorien"));
    }
}
