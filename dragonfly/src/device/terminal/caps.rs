//! What the terminal can draw, from its environment variables (no queries: they would mix with the input).
//!
//! | Terminal | Graphics | Colors |
//! |---|---|---|
//! | kitty, Ghostty, WezTerm | kitty | truecolor |
//! | foot, Konsole, Windows Terminal, iTerm2, mlterm, contour | sixel | truecolor |
//! | GNOME Terminal, Tilix, XFCE Terminal, LXQt QTerminal, Terminator, Alacritty, Zed, VS Code | — (text) | truecolor |
//! | macOS Terminal.app | — (text) | 256 |
//! | Linux console (`TERM=linux`) | — (text) | 16 |
use super::render::{Colors, Render};

pub struct Caps {
    pub render: Render,
    pub colors: Colors,
}

fn var(k: &str) -> String {
    std::env::var(k).unwrap_or_default()
}

pub fn detect() -> Caps {
    let (term, program) = (var("TERM"), var("TERM_PROGRAM"));
    let has = |k: &str| std::env::var_os(k).is_some();
    let kitty = has("KITTY_WINDOW_ID")
        || term == "xterm-kitty"
        || term == "xterm-ghostty"
        || program == "ghostty"
        || program == "WezTerm"
        || has("WEZTERM_PANE");
    let sixel = term.starts_with("foot")
        || term.starts_with("mlterm")
        || term.starts_with("contour")
        || has("KONSOLE_VERSION")
        || has("WT_SESSION")
        || program == "iTerm.app";
    let render = if kitty {
        Render::Kitty
    } else if sixel {
        Render::Sixel
    } else {
        Render::Text
    };
    let truecolor = matches!(var("COLORTERM").as_str(), "truecolor" | "24bit")
        || kitty
        || sixel
        || has("VTE_VERSION")
        || matches!(program.as_str(), "vscode" | "zed" | "Hyper" | "tmux")
        || term.contains("direct")
        || term == "alacritty";
    let colors = if truecolor {
        Colors::True
    } else if term == "linux" || term == "vt100" || term == "xterm" && cfg!(not(windows)) && !has("DISPLAY") {
        Colors::Ansi16
    } else {
        // Terminal.app, xterm, the old Windows console: 256 colors at least
        Colors::Ansi256
    };
    Caps { render, colors }
}

/// The renders a terminal can switch between with `m` (graphics only where `detect` found them).
pub fn modes(best: Render) -> Vec<Render> {
    let mut m = vec![];
    if best.graphics() {
        m.push(best);
    }
    m.extend([Render::Text, Render::Blocks]);
    m
}

/// Pixel size of a cell: from the terminal when it says it (most Unix terminals), else the usual 8x16.
pub fn cell_size() -> (f64, f64) {
    match (crossterm::terminal::window_size(), crossterm::terminal::size()) {
        (Ok(w), Ok((cols, rows))) if w.width > 0 && w.height > 0 && cols > 0 && rows > 0 => {
            (w.width as f64 / cols as f64, w.height as f64 / rows as f64)
        }
        _ => (8.0, 16.0),
    }
}
