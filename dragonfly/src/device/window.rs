//! Window front end: the device in its own window, like an emulator. The browser runs in app mode (no tabs, no
//! address bar); the window gets the device's aspect and at most 85% of the screen's height. Tiling window
//! managers (Hyprland, Sway, i3) would give it a whole tile: there it floats, centered, with that size.
use super::backend::Device;
use serde_json::json;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

/// The first size of the window (before the device knows the screen): the device at a usual laptop height.
pub fn first_size(css: (u32, u32)) -> (u32, u32) {
    let scale = fit_scale(css, (1440, 860), (0, 0));
    ((css.0 as f64 * scale) as u32, (css.1 as f64 * scale) as u32)
}

/// How much the device shrinks to fit the screen: at most 85% of its height and 90% of its width, never bigger
/// than its real CSS size. `frame` = what the window adds around the page (title bar, borders).
pub fn fit_scale(css: (u32, u32), screen: (u32, u32), frame: (u32, u32)) -> f64 {
    let h = (screen.1 as f64 * 0.85 - frame.1 as f64) / css.1 as f64;
    let w = (screen.0 as f64 * 0.9 - frame.0 as f64) / css.0 as f64;
    h.min(w).clamp(0.2, 1.0)
}

/// Fits the window to the screen, emulates the preset in it and opens the app; then waits for Flutter, which
/// reads its keys (r, R, q...) in this terminal. `number` spreads several devices side by side.
pub fn run(device: &mut Device, title: &str, number: u32) -> Result<(), String> {
    let css = device.css_size();
    // the browser's own bars appear with the window (Flutter starts it with --enable-automation: "controlled by
    // automated test software"); they take height from the page, so the page is measured once they are there
    thread::sleep(Duration::from_millis(1200));
    let m = measure(device)?;
    let frame = (m.outer.0.saturating_sub(m.inner.0), m.outer.1.saturating_sub(m.inner.1));
    let scale = fit_scale(css, m.screen, frame);
    let page = ((css.0 as f64 * scale).round() as u32, (css.1 as f64 * scale).round() as u32);
    let mut outer = (page.0 + frame.0, page.1 + frame.1);
    place(device, outer, m.screen, number, true);
    // the window manager may round the size, and a late bar changes the frame: correct once
    thread::sleep(Duration::from_millis(600));
    if let Ok(now) = measure(device)
        && (now.inner.0.abs_diff(page.0) > 2 || now.inner.1.abs_diff(page.1) > 2)
    {
        outer = (
            (outer.0 as i64 + page.0 as i64 - now.inner.0 as i64).max(100) as u32,
            (outer.1 as i64 + page.1 as i64 - now.inner.1 as i64).max(100) as u32,
        );
        place(device, outer, m.screen, number, false);
    }
    device.emulate(device.preset, device.landscape, scale)?;
    // a mouse in the window is a finger on a phone
    device.cdp.call(
        "Emulation.setEmitTouchEventsForMouse",
        json!({ "enabled": device.preset.mobile(), "configuration": "mobile" }),
    )?;
    // the title the window manager shows: "Dragonfly device 1 · Pixel 7 · <app title>"
    let script = format!(
        "(() => {{ const p = {title:?} + ' · '; const d = Object.getOwnPropertyDescriptor(Document.prototype, 'title');
           Object.defineProperty(document, 'title', {{ get() {{ return d.get.call(this); }},
             set(v) {{ d.set.call(this, v.startsWith(p) ? v : p + v); }} }}); document.title = ''; }})()"
    );
    device.cdp.call("Page.addScriptToEvaluateOnNewDocument", json!({ "source": script }))?;
    device.open_app()?;
    device.wait();
    Ok(())
}

struct Measure {
    screen: (u32, u32),
    /// the page area (before the emulation)
    inner: (u32, u32),
    /// the whole window, as the browser knows it
    outer: (u32, u32),
}

fn measure(device: &Device) -> Result<Measure, String> {
    let v =
        device.cdp.eval("[screen.availWidth, screen.availHeight, innerWidth, innerHeight, outerWidth, outerHeight]")?;
    let n = |i: usize| v[i].as_f64().unwrap_or(0.0).max(0.0) as u32;
    let mut outer = (n(4), n(5));
    if let Ok(w) = device.cdp.call("Browser.getWindowForTarget", json!({})) {
        let b = &w["bounds"];
        if let (Some(bw), Some(bh)) = (b["width"].as_u64(), b["height"].as_u64()) {
            outer = (bw as u32, bh as u32);
        }
    }
    let inner = (n(2), n(3));
    // on Wayland the browser does not know the screen: the window manager does
    let screen = wm_screen().unwrap_or((n(0), n(1)));
    if std::env::var_os("DRAGONFLY_DEVICE_DEBUG").is_some() {
        eprintln!("device window: page {inner:?}, window {outer:?}, screen {screen:?}");
    }
    Ok(Measure {
        screen: (screen.0.max(640), screen.1.max(480)),
        inner,
        outer: (outer.0.max(inner.0), outer.1.max(inner.1)),
    })
}

/// The usable size of the focused monitor in logical pixels, from Hyprland or Sway (bars excluded).
fn wm_screen() -> Option<(u32, u32)> {
    let json = |program: &str, args: &[&str]| -> Option<serde_json::Value> {
        let out = Command::new(program).args(args).stderr(Stdio::null()).output().ok()?;
        serde_json::from_slice(&out.stdout).ok()
    };
    if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
        let monitors = json("hyprctl", &["monitors", "-j"])?;
        let m = monitors.as_array()?.iter().find(|m| m["focused"] == true)?;
        let scale = m["scale"].as_f64().filter(|s| *s > 0.0)?;
        let rotated = m["transform"].as_u64().is_some_and(|t| t % 2 == 1);
        let (w, h) = (m["width"].as_f64()? / scale, m["height"].as_f64()? / scale);
        let (w, h) = if rotated { (h, w) } else { (w, h) };
        // reserved: [left, top, right, bottom] taken by bars
        let r: Vec<f64> = m["reserved"].as_array()?.iter().filter_map(|v| v.as_f64()).collect();
        let (rw, rh) = if r.len() == 4 { (r[0] + r[2], r[1] + r[3]) } else { (0.0, 0.0) };
        return Some(((w - rw) as u32, (h - rh) as u32));
    }
    if std::env::var_os("SWAYSOCK").is_some() {
        let outputs = json("swaymsg", &["-t", "get_outputs", "-r"])?;
        let o = outputs.as_array()?.iter().find(|o| o["focused"] == true)?;
        return Some((o["rect"]["width"].as_u64()? as u32, o["rect"]["height"].as_u64()? as u32));
    }
    None
}

/// Sway and i3 move by a positive amount to a side.
fn sway_move(dx: i64) -> String {
    if dx < 0 { format!("move left {} px", -dx) } else { format!("move right {dx} px") }
}

/// Device 1 in the middle, 2 to its right, 3 to its left, 4 further right...
fn offset(number: u32, width: u32) -> i64 {
    let k = number.saturating_sub(1) as i64;
    // other devices may be wider: a phone's usual width at least, plus a gap
    let step = (width as i64).max(430) + 24;
    if k % 2 == 1 { (k + 1) / 2 * step } else { -(k / 2) * step }
}

/// Size and place of the window: by CDP where the system lets a program move its windows (X11, macOS,
/// Windows; on Wayland only the size), and with the tiling window manager's own command where there is one.
fn place(device: &Device, outer: (u32, u32), screen: (u32, u32), number: u32, first: bool) {
    let (w, h) = outer;
    let dx = offset(number, w);
    if let Ok(win) = device.cdp.call("Browser.getWindowForTarget", json!({})) {
        let left = (screen.0 as i64 - w as i64) / 2 + dx;
        let mut bounds = json!({ "width": w, "height": h, "windowState": "normal" });
        if first {
            bounds["left"] = json!(left.clamp(0, screen.0.saturating_sub(w) as i64));
            bounds["top"] = json!(screen.1.saturating_sub(h) / 2);
        }
        let _ = device.cdp.call("Browser.setWindowBounds", json!({ "windowId": win["windowId"], "bounds": bounds }));
    }
    // Chrome names its app window after the URL: on Wayland the app id carries the device's marker
    let marker = &device.marker;
    let env = |k: &str| std::env::var_os(k).is_some();
    let run = |program: &str, args: &[&str]| {
        let _ = Command::new(program).args(args).stdout(Stdio::null()).stderr(Stdio::null()).status();
    };
    if env("HYPRLAND_INSTANCE_SIGNATURE") {
        // Hyprland 0.55+ takes Lua dispatchers; older ones the classic commands
        let hyprctl = |cmd: &str| {
            Command::new("hyprctl")
                .args(["dispatch", cmd])
                .output()
                .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).trim() == "ok")
        };
        let win = format!("window = \"class:.*{marker}.*\"");
        let lua = [
            format!("hl.dsp.window.float({{ action = \"enable\", {win} }})"),
            format!("hl.dsp.window.resize({{ x = {w}, y = {h}, relative = false, {win} }})"),
        ];
        let place = [
            format!("hl.dsp.window.center({{ {win} }})"),
            format!("hl.dsp.window.move({{ x = {dx}, y = 0, relative = true, {win} }})"),
        ];
        let lua: Vec<String> = if first { lua.into_iter().chain(place).collect() } else { lua.into() };
        if !lua.iter().all(|c| hyprctl(c)) {
            let p = format!("class:.*{marker}.*");
            run(
                "hyprctl",
                &[
                    "--batch",
                    &format!(
                        "dispatch setfloating {p}; dispatch resizewindowpixel exact {w} {h},{p}{}",
                        if first {
                            format!("; dispatch centerwindow; dispatch movewindowpixel {dx} 0,{p}")
                        } else {
                            String::new()
                        }
                    ),
                ],
            );
        }
    } else if env("SWAYSOCK") {
        let criteria = format!("[app_id=\".*{marker}.*\"]");
        let mv = if first { format!(", move position center, {}", sway_move(dx)) } else { String::new() };
        run("swaymsg", &[&format!("{criteria} floating enable, resize set {w} px {h} px{mv}")]);
    } else if env("I3SOCK") || std::env::var("XDG_CURRENT_DESKTOP").is_ok_and(|d| d.eq_ignore_ascii_case("i3")) {
        // i3 has no pid criteria: the X11 class the browser got (--class)
        let mv = if first { format!(", move position center, {}", sway_move(dx)) } else { String::new() };
        let class = format!("[class=\"^dragonfly-device-{number}$\"] floating enable, resize set {w} px {h} px{mv}");
        run("i3-msg", &[&class]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_keeps_the_aspect_and_fits_the_screen() {
        // a phone on a 1080p screen: 85% of the height, never bigger than the device
        let s = fit_scale((412, 915), (1920, 1080), (0, 30));
        assert!((s - (1080.0 * 0.85 - 30.0) / 915.0).abs() < 1e-9);
        assert_eq!(fit_scale((412, 915), (3840, 2160), (0, 0)), 1.0);
        // a wide desktop preset is limited by the width
        assert!(fit_scale((1920, 1080), (1366, 768), (0, 0)) < 0.65);
        let (w, h) = first_size((393, 852));
        assert_eq!((offset(1, 400), offset(2, 400), offset(3, 400), offset(4, 400)), (0, 454, -454, 908));
        assert_eq!(sway_move(-454), "move left 454 px");
        assert!((w as f64 / h as f64 - 393.0 / 852.0).abs() < 0.01);
    }
}
