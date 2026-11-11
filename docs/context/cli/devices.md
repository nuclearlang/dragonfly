---
layer: L3
component: cli
read-when: you change dragonfly device or dragonfly terminal-device (dragonfly/dragonfly/src/device/)
---

# Dragonfly devices (ADR-0081, `spec/flutter.md` §11)

`dragonfly device` (dv, `window.rs`) and `terminal-device` (td, `terminal/`) on the same backend. Dependencies:
crossterm, tungstenite, png, base64 (binary 4.1 → 5.3 MB).

- `backend.rs`: `flutter run -d chrome` with `--web-launch-url` to an empty 404 page marked
  `__dragonfly_device_<n>_<pid>__` (the window is named so on Wayland and the mark names the processes); CDP of
  that page; `emulate` (size, density, touch, orientation, user agent of `presets.rs`, the same as VS Code);
  `open_app`; input (`touch`/`wheel`/`insert_text`/`key`/`back`); `text_layer` (`__dragonflyTextLayer` of the
  package); `typing`, `screenshot`, `start_frames` (PNG screencast), `reload` (terminal: `flutter run
  --machine`, `app.restart`), `close` (`Browser.close` + `kill_tree`).
- `cdp.rs`: minimal client (tungstenite in a thread; `http_get` with Content-Length: Chrome keeps the connection).
- `system.rs`: `device_tmp` (Linux: Flutter's `TMPDIR` = `~/.cache/dfly/<n>`, ~500 MB off the RAM /tmp; short
  path because the browser profile socket allows ≤ 108 bytes), browser per OS (`CHROME_EXECUTABLE`, PATH,
  flatpak, /Applications, Program Files), ports, `Slot` (numbers in `temp/dragonfly-devices/<n>` with the pid;
  a dead one is reused), `alive`, `kill_tree`.
- `window.rs`: waits 1.2 s for the browser bars (`--enable-automation`), measures page and window WITHOUT
  emulation, `fit_scale` (85 % of the height), places it (CDP; Hyprland 0.55+ `hl.dsp.window.*` by class with
  the mark, else classic dispatchers; Sway `[app_id=...]`; i3 `--class`), corrects once, emulates and opens the
  app; screen size from Hyprland/Sway (`wm_screen`: Chrome does not know it on Wayland); `offset`: device 2 to
  the right, 3 to the left.
- `terminal/`: `caps.rs` (render and colors from env vars, cell size), `render.rs` (pure, tested: `fit`, blocks
  with text, palettes, cell `diff`, kitty, sixel), `mod.rs` (startup with progress, loop, status bar, `?` help,
  `l` log, keys and mouse, raw mode restored also on panic).

Tested on Linux/Hyprland: two windows at once, real kitty, blocks+text in a pty (pyte), touch → detail → back
→ rotate; closing the window, `q` or SIGHUP leaves no process. Windows and macOS: only `cargo check --target`.
Untested: GNOME/KDE (floating window through CDP), real sixel (foot, Konsole, Windows Terminal ≥ 1.22; older
Windows Terminal: `--render text`). An own engine without a browser (flutter-pi as a guide) is not started.
User feedback (2026-10-28): a run called `flutter pub get` three times, closed with an exception and used
1.5–2 GB.
