//! What depends on the operating system: the Chromium-based browser Flutter drives, free ports, the number of
//! each running device (device 1, 2, 3...) and whether a process is alive.
use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// A Chromium-based browser: `CHROME_EXECUTABLE` (Flutter's own variable) or the first one installed.
pub fn browser() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("CHROME_EXECUTABLE").map(PathBuf::from).filter(|p| p.exists()) {
        return Some(p);
    }
    let candidates: Vec<PathBuf> = if cfg!(target_os = "macos") {
        ["Google Chrome", "Chromium", "Microsoft Edge", "Brave Browser", "Vivaldi"]
            .iter()
            .map(|app| PathBuf::from(format!("/Applications/{app}.app/Contents/MacOS/{app}")))
            .collect()
    } else if cfg!(windows) {
        let roots = ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"]
            .iter()
            .filter_map(|v| std::env::var_os(v).map(PathBuf::from))
            .collect::<Vec<_>>();
        let apps = [
            "Google/Chrome/Application/chrome.exe",
            "Chromium/Application/chrome.exe",
            "Microsoft/Edge/Application/msedge.exe",
            "BraveSoftware/Brave-Browser/Application/brave.exe",
            "Vivaldi/Application/vivaldi.exe",
        ];
        roots.iter().flat_map(|r| apps.iter().map(move |a| r.join(a))).collect()
    } else {
        let names = [
            "google-chrome-stable",
            "google-chrome",
            "chromium",
            "chromium-browser",
            "microsoft-edge-stable",
            "microsoft-edge",
            "brave",
            "brave-browser",
            "vivaldi-stable",
            "vivaldi",
        ];
        let path = std::env::var_os("PATH").unwrap_or_default();
        let dirs: Vec<PathBuf> = std::env::split_paths(&path).collect();
        let mut found: Vec<PathBuf> = names.iter().flat_map(|n| dirs.iter().map(move |d| d.join(n))).collect();
        found.push("/var/lib/flatpak/exports/bin/com.google.Chrome".into());
        found.push("/var/lib/flatpak/exports/bin/org.chromium.Chromium".into());
        found
    };
    candidates.into_iter().find(|p| p.is_file())
}

pub const NO_BROWSER: &str = "\
no Chromium-based browser found (Chrome, Chromium, Edge, Brave or Vivaldi).
A Dragonfly device runs the app in one, like `flutter run -d chrome`. Install one, or point to it:
  export CHROME_EXECUTABLE=/path/to/chrome";

/// A free TCP port on 127.0.0.1.
pub fn free_port() -> Result<u16, String> {
    let l = TcpListener::bind(("127.0.0.1", 0)).map_err(|e| format!("no free port: {e}"))?;
    Ok(l.local_addr().map_err(|e| e.to_string())?.port())
}

pub fn alive(pid: u32) -> bool {
    if cfg!(target_os = "linux") {
        return Path::new(&format!("/proc/{pid}")).exists();
    }
    if cfg!(windows) {
        let out = Command::new("tasklist").args(["/FI", &format!("PID eq {pid}"), "/NH"]).output();
        return out.is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()));
    }
    Command::new("kill").args(["-0", &pid.to_string()]).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
}

/// Ends a process and its children (the browser Flutter started, if it is still there).
pub fn kill_tree(pid: u32) {
    if cfg!(windows) {
        let _ = Command::new("taskkill").args(["/T", "/F", "/PID", &pid.to_string()]).output();
        return;
    }
    let _ = Command::new("pkill").args(["-TERM", "-P", &pid.to_string()]).status();
    let _ = Command::new("kill").args(["-TERM", &pid.to_string()]).stderr(Stdio::null()).status();
}

/// Where Flutter keeps the temporary files of device `number` (~500 MB: the compiled app and the browser's
/// profile), on Linux only: there /tmp is often in RAM (tmpfs), and a run that ends abruptly leaves them behind.
/// A short folder on disk (`~/.cache/dfly/<n>`), emptied on every start and on close. The browser puts a socket
/// in its profile and a socket path has at most 108 bytes: a longer home keeps the system's temporary folder.
pub fn device_tmp(number: u32) -> Option<PathBuf> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    let dir = cache.join("dfly").join(number.to_string());
    // flutter_tools.XXXXXX/flutter_tools_chrome_device.XXXXXX/SingletonSocket
    if dir.as_os_str().len() + 72 > 107 {
        return None;
    }
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

fn slots_dir() -> PathBuf {
    std::env::temp_dir().join("dragonfly-devices")
}

/// A running device: its number, what it shows and the process that owns it.
pub struct Slot {
    pub number: u32,
    path: PathBuf,
}

impl Slot {
    /// The lowest free number (device 1, 2, 3...); the number of a device whose process died is free again.
    pub fn take(info: &str) -> Result<Slot, String> {
        let dir = slots_dir();
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        for number in 1..100 {
            let path = dir.join(number.to_string());
            if let Some(pid) = fs::read_to_string(&path).ok().and_then(|t| t.lines().next()?.parse::<u32>().ok())
                && alive(pid)
            {
                continue;
            }
            let _ = fs::remove_file(&path);
            let created = fs::OpenOptions::new().write(true).create_new(true).open(&path);
            if let Ok(mut f) = created {
                use std::io::Write;
                writeln!(f, "{}\n{info}", std::process::id()).map_err(|e| e.to_string())?;
                return Ok(Slot { number, path });
            }
        }
        Err("too many Dragonfly devices are running".into())
    }

    /// The running devices: `(number, info)`.
    pub fn running() -> Vec<(u32, String)> {
        let mut list: Vec<(u32, String)> = fs::read_dir(slots_dir())
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let number = e.file_name().to_str()?.parse().ok()?;
                let text = fs::read_to_string(e.path()).ok()?;
                let (pid, info) = text.split_once('\n')?;
                alive(pid.parse().ok()?).then(|| (number, info.trim().to_string()))
            })
            .collect();
        list.sort();
        list
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_take_the_lowest_free_number() {
        let a = Slot::take("test a").unwrap();
        let b = Slot::take("test b").unwrap();
        assert_ne!(a.number, b.number);
        let freed = a.number;
        drop(a);
        let c = Slot::take("test c").unwrap();
        assert_eq!(c.number, freed);
        assert!(Slot::running().iter().any(|(n, info)| *n == b.number && info == "test b"));
    }

    #[test]
    fn this_process_is_alive() {
        assert!(alive(std::process::id()));
    }
}
