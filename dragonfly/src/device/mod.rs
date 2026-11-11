//! Dragonfly devices: test devices that run the app with the size, touch and identity of a phone or a tablet.
//! - `dragonfly device`: its own window, like an emulator (`window.rs`).
//! - `dragonfly terminal-device`: the Dragonfly Terminal device, inside the terminal (`terminal/`).
//!
//! Both share the backend (`backend.rs`: Flutter, the browser, CDP, emulation, input). Several can run at the
//! same time: device 1, 2, 3...
mod backend;
mod cdp;
pub mod presets;
mod system;
mod terminal;
mod window;

use backend::{Device, Launch};
use presets::{PRESETS, Preset};
use std::path::Path;
use system::Slot;

#[derive(Debug)]
pub struct Args {
    pub preset: Preset,
    pub landscape: bool,
    pub render: Option<terminal::render::Render>,
    pub flutter_args: Vec<String>,
}

/// `[<device>] [--landscape] [--render kitty|sixel|text|blocks] [-- <flutter run args>]`
pub fn parse(args: &[String]) -> Result<Args, String> {
    let (own, flutter) = match args.iter().position(|a| a == "--") {
        Some(i) => (&args[..i], args[i + 1..].to_vec()),
        None => (args, vec![]),
    };
    let mut out = Args { preset: PRESETS[0], landscape: false, render: None, flutter_args: flutter };
    let mut it = own.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--landscape" => out.landscape = true,
            "--render" => {
                let r = it.next().ok_or("--render needs kitty, sixel, text or blocks")?;
                out.render = Some(terminal::render::Render::parse(r).ok_or(format!("unknown render '{r}'"))?);
            }
            flag if flag.starts_with('-') => return Err(format!("unknown option '{flag}'")),
            name => {
                out.preset =
                    Preset::find(name).ok_or_else(|| format!("unknown device '{name}'\n\n{}", presets_text()))?
            }
        }
    }
    Ok(out)
}

pub fn presets_text() -> String {
    let mut s = String::from("devices:\n");
    for p in PRESETS {
        s += &format!("  {:<16} {} ({}×{})\n", p.id, p.name, p.width, p.height);
    }
    s
}

/// `dragonfly device list`: the running devices and the presets.
pub fn list() -> String {
    let running = Slot::running();
    let mut s = if running.is_empty() {
        "no Dragonfly device is running\n\n".to_string()
    } else {
        running.iter().map(|(n, info)| format!("device {n}: {info}\n")).collect::<String>() + "\n"
    };
    s += &presets_text();
    s
}

/// Runs a device for the app in `app` (already built); `root` is the project (screenshots go to its build/).
pub fn run(root: &Path, app: &Path, flutter: &str, args: Args, in_terminal: bool) -> Result<(), String> {
    let kind = if in_terminal { "terminal" } else { "window" };
    let project = root.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let slot = Slot::take(&format!("{} · {kind} · {project}", args.preset.name))?;
    let css = args.preset.size(args.landscape);
    let launch = Launch {
        app: app.to_path_buf(),
        flutter: flutter.to_string(),
        number: slot.number,
        preset: args.preset,
        landscape: args.landscape,
        headless: in_terminal,
        window: window::first_size(css),
        flutter_args: args.flutter_args,
        on_spawn: None,
    };
    if in_terminal {
        let opts = terminal::Options { render: args.render, screenshots: root.join("build").join("screenshots") };
        return terminal::run(launch, opts);
    }
    eprintln!(
        "Dragonfly device {} · {} — r reload, R restart, q quit (in this terminal)",
        slot.number, args.preset.name
    );
    let mut device = Device::launch(launch, None)?;
    let title = format!("Dragonfly device {} · {}", slot.number, args.preset.name);
    let result = window::run(&mut device, &title, slot.number);
    device.close();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments() {
        let a = parse(&["iphonese".into(), "--landscape".into(), "--".into(), "--release".into()]).unwrap();
        assert_eq!(a.preset.name, "iPhone SE");
        assert!(a.landscape);
        assert_eq!(a.flutter_args, vec!["--release".to_string()]);
        let b = parse(&["--render".into(), "blocks".into()]).unwrap();
        assert_eq!(b.preset.name, "Pixel 7");
        assert_eq!(b.render, Some(terminal::render::Render::Blocks));
        assert!(parse(&["nokia3310".into()]).unwrap_err().contains("pixel7"));
        assert!(parse(&["--render".into(), "ascii".into()]).is_err());
    }
}
