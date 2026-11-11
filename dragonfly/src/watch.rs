//! `dragonfly watch`: transpiles the project every time a `.nuc` changes.
//!
//! VS Code starts it when the workspace opens (task `dragonfly: watch`). The Dart extension reloads the
//! running app when the generated `.dart` files change (`dart.previewHotReloadOnSaveWatcher`), so saving
//! a `.nuc` is enough to see the change. The lines `dragonfly: transpiling` / `dragonfly: ready` /
//! `dragonfly: errors` mark each round for the VS Code problem matcher.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// How often the `.nuc` files are checked: a check reads only their dates.
const PERIOD: Duration = Duration::from_millis(250);

/// Every `.nuc` of the project and its date.
fn snapshot(root: &Path) -> BTreeMap<PathBuf, SystemTime> {
    nuclear_transpile::project::sources(root, &root.join("build"))
        .into_iter()
        .map(|rel| {
            let date = std::fs::metadata(root.join(&rel)).and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
            (rel, date)
        })
        .collect()
}

fn round(root: &Path, build: &dyn Fn(&Path) -> Result<PathBuf, String>) {
    // the VS Code task matches the plain `dragonfly: transpiling|ready|errors`: colors only after it
    eprintln!("dragonfly: transpiling {}", crate::landing::dim("…"));
    match build(root) {
        Ok(_) => eprintln!(
            "dragonfly: ready {} {}",
            crate::landing::ok("✓"),
            crate::landing::dim("(watching the .nuc files)")
        ),
        Err(e) => {
            if !e.is_empty() {
                eprintln!("{e}");
            }
            eprintln!(
                "dragonfly: errors {} {}",
                crate::landing::bad("✗"),
                crate::landing::dim("(watching the .nuc files)")
            );
        }
    }
}

/// Transpiles now and after every change, until the process is stopped.
pub fn watch(root: &Path, build: fn(&Path) -> Result<PathBuf, String>) -> Result<(), String> {
    let mut last = snapshot(root);
    round(root, &build);
    let started_by = parent();
    loop {
        std::thread::sleep(PERIOD);
        // the editor that started it is gone (closed or killed): stop instead of staying orphaned
        if parent() != started_by {
            return Ok(());
        }
        let now = snapshot(root);
        if now != last {
            last = now;
            round(root, &build);
        }
    }
}

#[cfg(unix)]
fn parent() -> u32 {
    std::os::unix::process::parent_id()
}

// shortcut: Windows does not reparent orphans, so there is nothing to compare; it relies on the terminal closing
#[cfg(not(unix))]
fn parent() -> u32 {
    0
}
