//! `dragonfly setup`: installs the Flutter SDK step by step, following https://docs.flutter.dev/install/manual:
//! the system packages (with the distribution's own package manager), the latest stable SDK from Flutter's
//! release index (checked against its sha256), the PATH of the user's shell and a final check. Every step
//! says what it does and why, shows the command and asks before running it.

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

const RELEASES: &str = "https://storage.googleapis.com/flutter_infra_release/releases";
const STEPS: usize = 5;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Manager {
    Apt,
    Dnf,
    Pacman,
    Zypper,
}

pub fn setup(mut rest: Vec<String>) -> Result<(), String> {
    let dir = crate::take_option(&mut rest, "--dir").map(PathBuf::from);
    let os = std::env::consts::OS;
    if os != "linux" && os != "macos" {
        return Err("dragonfly setup installs Flutter on Linux and macOS. On Windows follow \
                    https://docs.flutter.dev/install/manual"
            .into());
    }
    if let (Some(bin), None) = (crate::find_flutter(), &dir) {
        println!("Flutter is already installed: {}", bin.display());
        println!("Check everything dragonfly needs with: dragonfly doctor");
        println!("To install another copy anyway: dragonfly setup --dir <folder>");
        return Ok(());
    }
    let home = PathBuf::from(std::env::var_os("HOME").ok_or("HOME is not set")?);
    let dest = dir.unwrap_or_else(|| home.join("develop"));
    let sdk = dest.join("flutter");
    if sdk.exists() {
        return Err(format!("{} already exists: remove it or choose another folder with --dir", sdk.display()));
    }
    println!("This installs the Flutter SDK in {} in {STEPS} steps:", sdk.display());
    println!("  1. the system packages Flutter needs");
    println!("  2. download the latest stable Flutter");
    println!("  3. unpack it in {}", dest.display());
    println!("  4. add it to the PATH of your shell");
    println!("  5. check that it works");
    println!("Each step asks before doing anything.");
    if !ask("Start?") {
        return Ok(());
    }
    packages(os)?;
    let archive = download(os, &dest)?;
    unpack(&archive, &dest)?;
    path(&sdk, &home)?;
    verify(&sdk)
}

/// Step 1: the packages of https://docs.flutter.dev/install/manual (download and unpack) and of
/// https://docs.flutter.dev/platform-integration/linux/setup (run the app on this computer).
fn packages(os: &str) -> Result<(), String> {
    step(1, "System packages");
    if os == "macos" {
        println!("Flutter needs Apple's command line tools (git, clang).");
        if Command::new("xcode-select").arg("-p").output().is_ok_and(|o| o.status.success()) {
            println!("{} already installed", crate::landing::ok("✓"));
        } else if ask("Install them now? (a window from macOS opens)") {
            shell("xcode-select", &["--install"])?;
            pause("When the installation window finishes, press Enter to go on.");
        }
        return Ok(());
    }
    println!("Flutter needs: curl, git, unzip, xz, zip and GLU to download and run its tools; clang, cmake,");
    println!("ninja, pkg-config and GTK 3 to run your app on this computer (Linux desktop: try it without a phone).");
    let Some(manager) = manager() else {
        println!("I don't know the package manager of this distribution. Install those packages with yours,");
        println!("then press Enter.");
        pause("");
        return Ok(());
    };
    let missing: Vec<&str> = names(manager).iter().copied().filter(|p| !installed(manager, p)).collect();
    if missing.is_empty() {
        println!("{} all installed", crate::landing::ok("✓"));
        return Ok(());
    }
    let (program, mut args): (&str, Vec<&str>) = match manager {
        Manager::Apt => ("apt-get", vec!["install", "-y"]),
        Manager::Dnf => ("dnf", vec!["install", "-y"]),
        Manager::Pacman => ("pacman", vec!["-S", "--needed"]),
        Manager::Zypper => ("zypper", vec!["install"]),
    };
    args.extend(&missing);
    println!("Missing: {}", missing.join(" "));
    println!("It needs administrator rights (sudo asks for your password).");
    if !ask("Install them?") {
        println!("{} Flutter may fail until they are installed.", crate::landing::warn("Skipped:"));
        return Ok(());
    }
    if manager == Manager::Apt {
        shell("sudo", &["apt-get", "update", "-y"])?;
    }
    shell("sudo", &[&[program][..], &args].concat())
}

/// The package manager, from /etc/os-release (`ID` and `ID_LIKE`).
fn manager() -> Option<Manager> {
    let text = std::fs::read_to_string("/etc/os-release").ok()?;
    let ids: Vec<String> = text
        .lines()
        .filter_map(|l| l.strip_prefix("ID=").or_else(|| l.strip_prefix("ID_LIKE=")))
        .flat_map(|v| v.trim_matches('"').split_whitespace().map(str::to_string).collect::<Vec<_>>())
        .collect();
    let has = |names: &[&str]| ids.iter().any(|id| names.contains(&id.as_str()));
    match () {
        _ if has(&["debian", "ubuntu"]) => Some(Manager::Apt),
        _ if has(&["fedora", "rhel", "centos"]) => Some(Manager::Dnf),
        _ if has(&["arch"]) => Some(Manager::Pacman),
        _ if has(&["suse", "opensuse"]) => Some(Manager::Zypper),
        _ => None,
    }
}

fn names(m: Manager) -> &'static [&'static str] {
    match m {
        Manager::Apt => &[
            "curl",
            "git",
            "unzip",
            "xz-utils",
            "zip",
            "libglu1-mesa",
            "clang",
            "cmake",
            "ninja-build",
            "pkg-config",
            "libgtk-3-dev",
            "libstdc++-12-dev",
        ],
        Manager::Dnf => &[
            "curl",
            "git",
            "unzip",
            "xz",
            "zip",
            "mesa-libGLU",
            "clang",
            "cmake",
            "ninja-build",
            "pkgconf-pkg-config",
            "gtk3-devel",
            "libstdc++-devel",
        ],
        Manager::Pacman => &["curl", "git", "unzip", "xz", "zip", "glu", "clang", "cmake", "ninja", "pkgconf", "gtk3"],
        Manager::Zypper => &[
            "curl",
            "git",
            "unzip",
            "xz",
            "zip",
            "libGLU1",
            "clang",
            "cmake",
            "ninja",
            "pkg-config",
            "gtk3-devel",
            "libstdc++-devel",
        ],
    }
}

fn installed(m: Manager, package: &str) -> bool {
    let (program, args) = match m {
        Manager::Apt => ("dpkg", ["-s", package]),
        Manager::Dnf | Manager::Zypper => ("rpm", ["-q", package]),
        Manager::Pacman => ("pacman", ["-Q", package]),
    };
    Command::new(program).args(args).output().is_ok_and(|o| o.status.success())
}

/// Step 2: the latest stable release from Flutter's index, checked against its sha256.
fn download(os: &str, dest: &Path) -> Result<PathBuf, String> {
    step(2, "Download Flutter");
    let index = Command::new("curl").args(["-fsSL", &format!("{RELEASES}/releases_{os}.json")]).output();
    let index = index.map_err(|e| format!("cannot run curl ({e}): install curl and run dragonfly setup again"))?;
    let json: serde_json::Value = serde_json::from_slice(&index.stdout)
        .map_err(|_| "cannot read Flutter's release index: check your internet connection and try again")?;
    let arch = if std::env::consts::ARCH == "aarch64" { "arm64" } else { "x64" };
    let hash = json["current_release"]["stable"].as_str().unwrap_or_default();
    let release = json["releases"]
        .as_array()
        .and_then(|rs| rs.iter().find(|r| r["hash"] == hash && r["dart_sdk_arch"].as_str().unwrap_or("x64") == arch))
        .ok_or(format!(
            "there is no stable Flutter download for {os} {arch}: see https://docs.flutter.dev/install/manual"
        ))?;
    let (version, archive) = (release["version"].as_str().unwrap_or("?"), release["archive"].as_str().unwrap_or(""));
    let file = dest.join(archive.rsplit('/').next().unwrap_or(archive));
    println!(
        "Latest stable: Flutter {version} ({})",
        release["release_date"].as_str().unwrap_or("").get(..10).unwrap_or("")
    );
    println!("From {RELEASES}/{archive}");
    println!("To   {} (about 1 GB, it takes a while)", file.display());
    if !ask("Download it?") {
        return Err("stopped: nothing was installed".into());
    }
    std::fs::create_dir_all(dest).map_err(|e| format!("cannot create {}: {e}", dest.display()))?;
    shell("curl", &["-fL", "--progress-bar", "-o", &file.to_string_lossy(), &format!("{RELEASES}/{archive}")])?;
    println!("Checking the download (sha256)...");
    let sum = match os {
        "macos" => Command::new("shasum").args(["-a", "256"]).arg(&file).output(),
        _ => Command::new("sha256sum").arg(&file).output(),
    };
    let sum = sum.map(|o| String::from_utf8_lossy(&o.stdout).split_whitespace().next().unwrap_or("").to_string());
    if sum.as_deref().unwrap_or("") != release["sha256"].as_str().unwrap_or("-") {
        let _ = std::fs::remove_file(&file);
        return Err("the download is damaged (its sha256 does not match): run dragonfly setup again".into());
    }
    println!("{} verified", crate::landing::ok("✓"));
    Ok(file)
}

/// Step 3: unpack the archive next to it and remove it.
fn unpack(archive: &Path, dest: &Path) -> Result<(), String> {
    step(3, "Unpack");
    let (a, d) = (archive.to_string_lossy(), dest.to_string_lossy());
    if a.ends_with(".zip") {
        shell("unzip", &["-q", &a, "-d", &d])?
    } else {
        shell("tar", &["-xf", &a, "-C", &d])?
    }
    let _ = std::fs::remove_file(archive);
    println!("{} Flutter is in {}", crate::landing::ok("✓"), dest.join("flutter").display());
    Ok(())
}

/// Step 4: the SDK's bin folder in the PATH, in the file the docs name for each shell.
fn path(sdk: &Path, home: &Path) -> Result<(), String> {
    step(4, "PATH");
    let bin = sdk.join("bin");
    // $HOME instead of the home folder: the line still works if the user name changes
    let shown = match bin.strip_prefix(home) {
        Ok(rel) => format!("$HOME/{}", rel.display()),
        Err(_) => bin.display().to_string(),
    };
    let shell_name = std::env::var("SHELL").unwrap_or_default();
    let shell_name = shell_name.rsplit('/').next().unwrap_or("");
    println!("So that `flutter` works in every terminal, its folder goes into the PATH.");
    if shell_name == "fish" {
        println!(
            "{} {}",
            crate::landing::dim("$"),
            crate::landing::cmd(&format!("fish_add_path -U -p {}", bin.display()))
        );
        if ask("Add it?") {
            shell("fish", &["-c", &format!("fish_add_path -U -p {}", bin.display())])?;
        }
        return Ok(());
    }
    let file = home.join(match (shell_name, std::env::consts::OS) {
        ("zsh", "macos") => ".zprofile",
        ("zsh", _) => ".zshenv",
        ("bash", _) => ".bashrc",
        _ => ".profile",
    });
    let line = format!("export PATH=\"{shown}:$PATH\"");
    if std::fs::read_to_string(&file).is_ok_and(|t| t.contains(&line)) {
        println!("{} already in {}", crate::landing::ok("✓"), file.display());
        return Ok(());
    }
    println!("This line goes at the end of {}:\n  {line}", file.display());
    if ask("Add it?") {
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file)
            .map_err(|e| format!("cannot write {}: {e}", file.display()))?;
        writeln!(f, "\n# Flutter (added by dragonfly setup)\n{line}").map_err(|e| e.to_string())?;
        println!(
            "{} added. New terminals will have it; in this one run: source {}",
            crate::landing::ok("✓"),
            file.display()
        );
    } else {
        println!(
            "{} add that line yourself, or dragonfly finds Flutter in {} anyway.",
            crate::landing::warn("Skipped:"),
            sdk.display()
        );
    }
    Ok(())
}

/// Step 5: the first run of flutter (it downloads the Dart SDK it carries) and what to do next.
fn verify(sdk: &Path) -> Result<(), String> {
    step(5, "Check");
    println!("The first run of flutter prepares its tools (it takes a minute).");
    shell(&sdk.join("bin/flutter").to_string_lossy(), &["--version"])?;
    println!("\n{} Flutter is installed. Next:", crate::landing::ok("✓"));
    println!(
        "  {}{}",
        crate::landing::cmd("dragonfly doctor"),
        crate::landing::dim("        everything NuclearLang and Dragonfly need")
    );
    println!(
        "  {}{}",
        crate::landing::cmd("flutter doctor"),
        crate::landing::dim("          devices: Android, iOS, web, this computer")
    );
    println!("  {}{}", crate::landing::cmd("dragonfly new <name>"), crate::landing::dim("    your first app"));
    Ok(())
}

fn step(n: usize, title: &str) {
    println!("\n{}", crate::landing::name(&format!("── Step {n} of {STEPS} · {title} ──")));
}

/// Asks a yes/no question; Enter means yes. Without an answer (no terminal) it is no.
fn ask(question: &str) -> bool {
    print!("{} {} ", crate::landing::name(question), crate::landing::dim("[Y/n]"));
    let _ = std::io::stdout().flush();
    let mut answer = String::new();
    match std::io::stdin().lock().read_line(&mut answer) {
        Ok(0) | Err(_) => {
            println!();
            false
        }
        Ok(_) => matches!(answer.trim().to_lowercase().as_str(), "" | "y" | "yes" | "s" | "si" | "sí"),
    }
}

fn pause(message: &str) {
    if !message.is_empty() {
        println!("{message}");
    }
    let _ = std::io::stdin().lock().read_line(&mut String::new());
}

/// Runs a command in front of the user (it may ask for a password), showing it first.
fn shell(program: &str, args: &[&str]) -> Result<(), String> {
    println!("{} {}", crate::landing::dim("$"), crate::landing::cmd(&format!("{program} {}", args.join(" "))));
    let status = Command::new(program).args(args).status().map_err(|e| format!("cannot run {program}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} failed: fix what it says and run dragonfly setup again"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every distribution has the packages of the docs' list (download tools + Linux desktop).
    #[test]
    fn every_manager_names_the_packages() {
        for m in [Manager::Apt, Manager::Dnf, Manager::Pacman, Manager::Zypper] {
            let n = names(m);
            for tool in ["curl", "git", "unzip", "zip", "clang", "cmake"] {
                assert!(n.contains(&tool), "{m:?} lacks {tool}");
            }
        }
    }
}
