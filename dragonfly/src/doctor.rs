//! `dragonfly doctor`: checks what NuclearLang and Dragonfly need to work, like `flutter doctor` but only for
//! their own resources: the CLIs, the flutter dialect and its type definitions, the dragonfly Dart package,
//! the Flutter SDK, VS Code with its extensions, and the project of the current folder.

use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(PartialEq)]
enum Status {
    Ok,
    Warn,
    Fail,
}

struct Check {
    status: Status,
    title: String,
    details: Vec<String>,
}

fn check(status: Status, title: impl Into<String>, details: Vec<String>) -> Check {
    Check { status, title: title.into(), details }
}

pub fn doctor(project: Option<&Path>) {
    use crate::landing::{bad, cmd, dim, name, ok, warn};
    println!(
        "{} {}\n",
        name("Doctor summary for NuclearLang and Dragonfly"),
        dim(&format!("(Flutter's own checks: {})", cmd("dragonfly flutter doctor")))
    );
    let mut checks = vec![dragonfly(), nuclear(), dialect(), package(), flutter(), vscode()];
    checks.extend(project.map(project_check));
    for c in &checks {
        let mark = match c.status {
            Status::Ok => ok("[✓]"),
            Status::Warn => warn("[!]"),
            Status::Fail => bad("[✗]"),
        };
        println!("{mark} {}", c.title);
        for d in &c.details {
            println!("    {} {d}", dim("•"));
        }
    }
    let issues = checks.iter().filter(|c| c.status != Status::Ok).count();
    match issues {
        0 => println!("\n{}", ok("• No issues found!")),
        1 => println!("\n{}", warn("! Doctor found issues in 1 category.")),
        n => println!("\n{}", warn(&format!("! Doctor found issues in {n} categories."))),
    }
}

/// A program in the PATH.
fn in_path(name: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?).map(|d| d.join(name)).find(|p| p.is_file())
}

fn home_relative(p: &Path) -> String {
    match std::env::var_os("HOME").map(PathBuf::from) {
        Some(home) if p.starts_with(&home) => format!("~/{}", p.strip_prefix(&home).unwrap_or(p).display()),
        _ => p.display().to_string(),
    }
}

fn dragonfly() -> Check {
    let exe = std::env::current_exe().map(|p| home_relative(&p)).unwrap_or_default();
    let files = match nuclear_dialect::install_dir() {
        Some(dir) => format!("files: {}", home_relative(&dir)),
        None => "files: the repository it was built from (not installed: ./install.sh in the repository)".into(),
    };
    check(Status::Ok, concat!("dragonfly ", env!("CARGO_PKG_VERSION")), vec![format!("binary: {exe}"), files])
}

fn nuclear() -> Check {
    match in_path("nuclear") {
        Some(p) => check(Status::Ok, "nuclear CLI", vec![home_relative(&p)]),
        None => check(
            Status::Warn,
            "nuclear CLI not in the PATH",
            vec![
                "agents use it: nuclear check, nuclear spec, nuclear defs".into(),
                "install it with ./install.sh in the NuclearLang repository".into(),
            ],
        ),
    }
}

fn dialect() -> Check {
    let (d, report) = crate::loader().load("flutter");
    match d {
        Some(d) => {
            let folder =
                d.files.first().and_then(|f| Path::new(&f.name).parent().map(home_relative)).unwrap_or_default();
            let ids: Vec<String> = d.typedefs.packages.iter().map(|p| p.id()).collect();
            let defs = format!("type definitions: {} packages ({})", ids.len(), ids.join(", "));
            check(Status::Ok, format!("Dialect {} ({})", d.name, d.chain.join(" → ")), vec![folder, defs])
        }
        None => {
            let mut details: Vec<String> = report.diags.iter().take(3).map(|(_, d)| d.message.clone()).collect();
            details.push("reinstall with ./install.sh, or point NUCLEAR_DIALECT_PATH to a dialects folder".into());
            check(Status::Fail, "Dialect flutter does not load", details)
        }
    }
}

fn package() -> Check {
    let p = crate::dragonfly_package();
    let pubspec = std::fs::read_to_string(p.join("pubspec.yaml"));
    match pubspec {
        Ok(text) => {
            let version = text.lines().find_map(|l| l.strip_prefix("version:")).unwrap_or("").trim().to_string();
            check(Status::Ok, format!("Dart package dragonfly {version}"), vec![home_relative(&p)])
        }
        Err(_) => check(
            Status::Fail,
            "Dart package dragonfly not found",
            vec![
                format!("looked in {}", home_relative(&p)),
                "reinstall with ./install.sh, or point DRAGONFLY_PACKAGE to dragonfly-mobile/packages/dragonfly".into(),
            ],
        ),
    }
}

fn flutter() -> Check {
    let Some(bin) = crate::find_flutter() else {
        return check(
            Status::Fail,
            "Flutter SDK not found",
            vec![
                "dragonfly needs it to create, build and run apps (dragonfly check works without it)".into(),
                "install it step by step: dragonfly setup".into(),
                "already installed somewhere else? export FLUTTER_ROOT=/path/to/flutter".into(),
            ],
        );
    };
    let source = match () {
        _ if std::env::var_os("FLUTTER").is_some() => "from $FLUTTER",
        _ if std::env::var_os("FLUTTER_ROOT").is_some() => "from $FLUTTER_ROOT",
        _ if bin.to_string_lossy().contains(".fvm/flutter_sdk") => "the project's fvm version",
        _ if bin.to_string_lossy().contains("fvm/default") => "fvm's global version",
        _ if in_path("flutter").is_some_and(|p| p == bin) => "from the PATH",
        _ => "a usual install folder",
    };
    let out = Command::new(&bin).args(["--version", "--machine"]).output();
    let json = out.ok().and_then(|o| {
        let text = String::from_utf8_lossy(&o.stdout).to_string();
        serde_json::from_str::<serde_json::Value>(&text[text.find('{')?..]).ok()
    });
    let Some(v) = json else {
        return check(
            Status::Fail,
            "Flutter SDK found but it does not run",
            vec![home_relative(&bin), "try it by hand: flutter --version".into()],
        );
    };
    let version = v["frameworkVersion"].as_str().unwrap_or("?").to_string();
    let mut details = vec![format!("{} ({source})", home_relative(&bin))];
    let mut status = Status::Ok;
    // the dialect's type definitions name the Flutter version they were generated from
    let (d, _) = crate::loader().load("flutter");
    let defs = d.and_then(|d| d.typedefs.packages.iter().find(|p| p.name == "flutter").map(|p| p.id()));
    if let Some(defs) = defs.and_then(|id| id.split_once('@').map(|(_, v)| v.to_string())) {
        let minor = |v: &str| v.split('.').take(2).collect::<Vec<_>>().join(".");
        if minor(&defs) != minor(&version) {
            status = Status::Warn;
            details.push(format!(
                "the type definitions are for Flutter {defs}: widgets added or changed since then may lack auto-imports"
            ));
        }
    }
    if in_path("flutter").is_none() {
        status = Status::Warn;
        // $HOME, not ~: the shell does not expand ~ inside quotes
        let dir = bin.parent().map(home_relative).unwrap_or_default().replacen("~/", "$HOME/", 1);
        details.push(format!(
            "`flutter` is not in your PATH (dragonfly finds it anyway): add export PATH=\"{dir}:$PATH\" to ~/.zshenv or ~/.bashrc"
        ));
    }
    let dart = v["dartSdkVersion"].as_str().unwrap_or("?");
    check(status, format!("Flutter {version} (Dart {dart})"), details)
}

fn vscode() -> Check {
    let Some(code) = in_path("code") else {
        return check(
            Status::Warn,
            "VS Code: the `code` command is not in the PATH",
            vec![
                "dragonfly code needs it; install VS Code: https://code.visualstudio.com".into(),
                "already installed? in VS Code run \"Shell Command: Install 'code' command in PATH\"".into(),
            ],
        );
    };
    let list = Command::new(&code).arg("--list-extensions").output();
    let list = list.map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase()).unwrap_or_default();
    let wanted = [
        ("nuclearlang.nuclearlang", "NuclearLang (./install.sh in the repository installs it)"),
        ("dart-code.dart-code", "Dart: code --install-extension Dart-Code.dart-code"),
        ("dart-code.flutter", "Flutter (devices, run, hot reload): code --install-extension Dart-Code.flutter"),
    ];
    let missing: Vec<String> = wanted
        .iter()
        .filter(|(id, _)| !list.lines().any(|l| l == *id))
        .map(|(_, how)| format!("missing {how}"))
        .collect();
    match missing.is_empty() {
        true => {
            check(Status::Ok, "VS Code with the NuclearLang, Dart and Flutter extensions", vec![home_relative(&code)])
        }
        false => check(Status::Warn, "VS Code: extensions missing", missing),
    }
}

fn project_check(root: &Path) -> Check {
    let name = root.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let app = root.join("build/app");
    match app.join("pubspec.yaml").is_file() {
        true => check(Status::Ok, format!("Project {name}"), vec![home_relative(root), "app: build/app".into()]),
        false => check(
            Status::Warn,
            format!("Project {name}: the Flutter app is not created yet"),
            vec![home_relative(root), "create it with: dragonfly build (needs Flutter)".into()],
        ),
    }
}
