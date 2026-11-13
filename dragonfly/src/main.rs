//! `dragonfly`: mobile apps, web apps and backends written in NuclearLang.
//!
//! A project has only `.nuc` files (main.nuc, config/, shared/, components/, test/, integration_test/);
//! everything generated goes to `build/`: the Flutter app (`build/app`) and the VS Code workspace
//! (`build/<name>.code-workspace`). Running, building, packages and devices are the Flutter CLI's job:
//! dragonfly transpiles first and then calls `flutter` inside the app. Code generation is dragonfly's own
//! (`dragonfly make`, alias `g`).

mod assets;
mod device;
mod doctor;
mod explain;
mod generate;
mod landing;
mod packages;
mod scaffold;
mod setup;
mod vscode;
mod watch;

use nuclear_diag::{SourceFile, json_items, render_text};
use nuclear_dialect::{Dialect, Loader};
use nuclear_transpile::project::{self, Manifest};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const USAGE: &str = "usage: dragonfly <command> [arguments]          (names in any case: user_profile, UserProfile)

project
  new <name> [-p mobile] [-d flutter]   (n)  new project of a platform (mobile: flutter; backend: fastapi, nestjs; web: react):
      [--org com.example]                    only .nuc files (+ the Flutter app in build/app for mobile)
  code                                       open the project in VS Code (Flutter run, debug, devices, hot reload)
  watch                                      transpile every time a .nuc changes (VS Code starts it by itself)
  build [apk|appbundle|ios|ipa|web|macos|linux|windows] [-- <flutter build args>]
                                             transpile, pub get and build_runner (and flutter build <target>)
  run [-- <flutter run args>]                build and run on a device (flutter run, -d <device>)
  device [<device>] [--landscape] [-- <args>] (dv) a test device in its own window, like an emulator (pixel7,
                                             iphone14pro, ipadair...; several at once: device 1, 2, 3)
  device list                                the running devices and the devices it can be
  terminal-device [<device>] [--render kitty|sixel|text|blocks]
                                        (td) the Dragonfly Terminal device: the app inside this terminal
  test [--integration] [-- <args>]           build and run test/, or integration_test/ on a device
  analyze                                    build and run flutter analyze on the generated app
  check [--json]                             errors and architecture rules of every .nuc (no Flutter needed)

backend (fastapi, nestjs; the dialect's [toolchain] runs in build/)
  build                                      transpile (nuclear build .)
  install                                    the runtime and the app's packages (a venv, npm install)
  run [-- <args>]                            the server (uvicorn, npm run dev)
  test [-- <args>]                           the tests written in Nuclear (pytest, node:test)
  migrate make <message>                     one migration of the database: the entities minus the last one
  migrate [up] | migrate down                applies the pending migrations, or reverts the last one
  add <folder>                               install a kind package (kinds/<name>/, ADR-0095) for every project
  agent init                                 AGENTS.md, CLAUDE.md, a Cursor rule and a check hook for AI agents
  clean                                      flutter clean in the app
  repair                                     when the build or the app fails: build, then pub get + build_runner
                                             from scratch, then flutter clean + everything again, until
                                             flutter analyze finds no errors (exit 2: the .nuc have errors)
  explain [--json]                           a run-time error or a test's output (stdin) at its .nuc lines, explained

code generation: dragonfly make <what> ...   (alias: dragonfly g <alias> ...)
  component <component>                 (c)  facade + an example use case
  screen <component> <screen>           (s)  screen + state manager + state model
      [--manager|-m Name] [--state|-st Name] [--route /path]
  statemanager <component> <screen> <name> (sm) a state manager (Cubit) + its state model, in a screen
  statemodel <component> <screen> <name> (smo) a state model, in a screen
  stateful <name>                       (sf) a stateful widget in shared/widgets
  usecase <component> <name>            (uc) a use case
  repository <component> <name>         (r)  a repository and its model
  model <component> <name>              (mo) a model
  service <component> <name>            (sv) a service
  aggregate <component> <name>          (ag) an aggregate
  config <component>                    (cf) the config of a component
  test <component>                      (t)  test/<component>_test.nuc
  event <name>                          (e)  a global event in shared/events
  widget <name>                         (w)  a shared widget in shared/widgets
  guard <name>                          (gd) an access check (canActivate) in shared/guards
  global <name>                         (gl) a global component (@on an event) + its event
  page <name> [--route /x|--not-found]  (pg) a screen shared by the app in shared/screens
  integration <name>                    (it) an integration test in integration_test/
  backend and web projects: <kind> <component> <Name> [--screen <screen>]   any kind of the platform, at its
      place (`nuclear help layout --platform <p>`), from the platform's skeleton (e.g. `make entity books Book`)

finished code from a resource's data (every platform; ADR-0103): dragonfly gen <what> ...
  inspect                               what the project has (components, declarations by kind), as JSON
  component <component>                 a component and its facade
  resource <component> <Name>           models, repository, a use case per action, the facade's aliases, the
                                        controller (backend) or the screen (mobile, web), the tests
      --from-json <file|url> | --from-spec <spec.md> | --fields \"id:int name:string origin:Origin?\"
      [--source rest[:<base url>]|db] [--path p] [--actions list,get,search,create,update,delete]
      [--screen <screen>] [--plural Names] [--dry-run]       (dragonfly gen alone prints every option)
  usecase <component> <Name>            a use case wired into its component: its alias in the facade and, with
                                        --route, its route in the controller (backend)
      [--calls <Repository>.<method>] [--route \"POST orders/{id}/cancel\"] [--dry-run]

packages (config.nuc: the project's pubspec, written in Nuclear)
  flutter add <package>[:version]... [--dev]  add a package to config.nuc and the app, and generate its
                                             type definitions (cached in ~/.cache/nuclearlang/defs)
  flutter remove <package>...                remove it from config.nuc and the app (its definitions stay cached)

packages and devices (flutter, inside the app)
  pub get|upgrade|outdated                   (pub add|remove = flutter add|remove)
  devices | emulators | logs | install | screenshot | attach | gen-l10n
  flutter <any flutter command>              run any flutter command inside the app

setup
  doctor                                     what NuclearLang and Dragonfly need: CLIs, dialect, package, Flutter, VS Code
  setup [--dir ~/develop]                    install Flutter step by step (system packages, SDK, PATH)
  credits | author                           the intro animation, the author and the repository

options
  --project <dir>   project folder (default: the folder with main.nuc, here or above)
  FLUTTER           the flutter binary (default: found by itself: FLUTTER_ROOT, PATH, fvm, ~/development/flutter...)

docs: nuclear help flutter";

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let project_dir = take_option(&mut args, "--project");
    let Some(cmd) = args.first().cloned() else {
        landing::show(false);
        return ExitCode::SUCCESS;
    };
    if matches!(cmd.as_str(), "credits" | "author") {
        landing::show(true);
        return ExitCode::SUCCESS;
    }
    landing::header();
    let rest = args[1..].to_vec();
    let result = match cmd.as_str() {
        "-h" | "--help" | "help" => {
            print!("{}", landing::usage(USAGE));
            Ok(())
        }
        "new" | "n" => new_project(rest),
        "make" | "g" => with_root(project_dir, |root| make(root, rest)),
        "gen" => with_root(project_dir, |root| gen_command(root, rest)),
        "check" => with_root(project_dir, |root| {
            // backend and web projects are checked by `nuclear` (it loads every dialect); the Flutter check below
            // resolves pubspec packages and fonts, which they do not have
            if non_mobile_manifest(root)?.is_some() {
                let mut a = vec!["check", "."];
                if rest.iter().any(|x| x == "--json") {
                    a.push("--json");
                }
                return run("nuclear", root, &a);
            }
            check(root, rest.iter().any(|a| a == "--json"))
        }),
        "agent" if rest.first().is_some_and(|a| a == "init") => {
            with_root(project_dir, |root| agent_init(root, &any_manifest(root).map(|m| m.dialect)?))
        }
        "watch" => with_root(project_dir, |root| watch::watch(root, build)),
        "code" => with_root(project_dir, |root| {
            build(root)?;
            let ws = vscode::workspace_path(root, &manifest(root)?);
            Command::new("code")
                .arg(&ws)
                .spawn()
                .map(|_| ())
                .map_err(|e| {
                    format!(
                        "cannot run `code` ({e}).\n\nVS Code's `code` command is not in the PATH. Install VS Code \
                         (https://code.visualstudio.com) or, in VS Code, run \"Shell Command: Install 'code' command in \
                         PATH\". Meanwhile open it by hand: File → Open Workspace from File → {}",
                        ws.display()
                    )
                })
        }),
        "build" => with_root(project_dir, |root| {
            if let Some(m) = non_mobile_manifest(root)? {
                return backend_command(root, "build", &m, &rest, &[]);
            }
            let app = build(root)?;
            match rest.first().filter(|t| !t.starts_with('-')) {
                Some(target) => flutter(&app, &[&["build", target.as_str()][..], &passthrough(&rest)].concat()),
                None => Ok(()),
            }
        }),
        "test" => with_root(project_dir, |root| {
            if let Some(m) = non_mobile_manifest(root)? {
                return backend_command(root, "test", &m, &rest, &[]);
            }
            let app = build(root)?;
            let target = if rest.iter().any(|a| a == "--integration") { "integration_test" } else { "test" };
            flutter(&app, &[&["test", target, "--no-pub"][..], &passthrough(&rest)].concat())
        }),
        "run" | "analyze" => with_root(project_dir, |root| {
            if let Some(m) = non_mobile_manifest(root)? {
                return backend_command(root, &cmd, &m, &rest, &[]);
            }
            let app = build(root)?;
            // build already resolved the packages: flutter run would do it again
            let no_pub: &[&str] = if cmd == "run" { &["--no-pub"] } else { &[] };
            flutter(&app, &[&[cmd.as_str()][..], no_pub, &passthrough(&rest)].concat())
        }),
        "device" | "dv" if rest.first().is_some_and(|a| a == "list") => {
            print!("{}", device::list());
            Ok(())
        }
        "device" | "dv" | "terminal-device" | "td" => with_root(project_dir, |root| {
            let args = device::parse(&rest)?;
            let app = build(root)?;
            device::run(root, &app, &flutter_bin()?, args, matches!(cmd.as_str(), "terminal-device" | "td"))
        }),
        "migrate" => with_root(project_dir, |root| migrate_command(root, &rest)),
        "pub" => with_root(project_dir, |root| pub_command(root, &rest)),
        "repair" => with_root(project_dir, repair),
        "explain" => with_root(project_dir, |root| explain_command(root, rest.iter().any(|a| a == "--json"))),
        "add" => match rest.first() {
            Some(source) => add_package(source),
            None => Err("usage: dragonfly add <folder of a kind package>".into()),
        },
        "doctor" => {
            let start = project_dir.map(PathBuf::from).unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
            doctor::doctor(start.ancestors().find(|d| d.join("main.nuc").is_file()));
            Ok(())
        }
        "setup" => setup::setup(rest),
        "clean" | "devices" | "emulators" | "logs" | "install" | "screenshot" | "attach" | "gen-l10n" => {
            with_root(project_dir, |root| {
                // a backend project: install is the dialect's toolchain (a venv, npm install)
                if let Some(m) = non_mobile_manifest(root)? {
                    return backend_command(root, &cmd, &m, &rest, &[]);
                }
                let app = app_dir(root)?;
                let mut a = vec![cmd.as_str()];
                a.extend(rest.iter().map(String::as_str));
                flutter(&app, &a)
            })
        }
        "flutter" => with_root(project_dir, |root| flutter_command(root, &rest)),
        other => Err(format!("unknown command '{other}'\n\n{}", landing::usage(USAGE))),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            if !e.is_empty() {
                eprintln!("{} {e}", landing::bad("✗"));
            }
            // 2 = the .nuc have errors: the editors do not try to repair the app then
            if e == CODE_ERRORS { ExitCode::from(2) } else { ExitCode::FAILURE }
        }
    }
}

/// Removes `--name value` from `args` and returns the value.
fn take_option(args: &mut Vec<String>, name: &str) -> Option<String> {
    let i = args.iter().position(|a| a == name)?;
    args.remove(i);
    (i < args.len()).then(|| args.remove(i))
}

/// The arguments after `--`, passed to flutter as they are.
fn passthrough(rest: &[String]) -> Vec<&str> {
    match rest.iter().position(|a| a == "--") {
        Some(i) => rest[i + 1..].iter().map(String::as_str).collect(),
        None => Vec::new(),
    }
}

fn with_root(dir: Option<String>, f: impl FnOnce(&Path) -> Result<(), String>) -> Result<(), String> {
    let start = dir.map(PathBuf::from).unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    // a mobile app has main.nuc; a backend project only config.nuc (ADR-0093)
    let root = start
        .ancestors()
        .find(|d| d.join("main.nuc").is_file() || d.join(packages::CONFIG).is_file())
        .ok_or_else(|| format!("no main.nuc or config.nuc in {} or above (create a project: dragonfly new <name>)", start.display()))?;
    f(root)
}

/// A Dragonfly project has only .nuc files: its name is its folder's (or `app(name)` of config.nuc), its
/// dialect flutter and the app goes to build/app. A nuclear.toml, if there is one, says otherwise.
/// The project's manifest: `config.nuc` with `dragonfly(platform = "mobile", dialect = "flutter")` (ADR-0093).
/// An app written before that line is still a flutter app (`dragonfly check` asks for the line).
fn manifest(root: &Path) -> Result<Manifest, String> {
    let mut m = any_manifest(root)?;
    if m.platform.as_deref().is_some_and(|p| p != MOBILE) {
        return Err(format!(
            "this project is written for the {} platform with the {} dialect: the dragonfly CLI builds its apps through the dialect's toolchain (dragonfly build, run, test)",
            m.platform.as_deref().unwrap_or("?"),
            m.dialect
        ));
    }
    if m.out.is_empty() {
        let (d, _) = loader().load(&m.dialect);
        m.out = d.map(|d| d.project.output.clone()).filter(|o| !o.is_empty()).unwrap_or_else(|| "build".into());
    }
    Ok(m)
}

/// The mobile platform: its apps are built here with its dialect's SDK (create, packages, code generation).
const MOBILE: &str = "mobile";

/// The manifest of any project; an app written before `dragonfly(platform, dialect)` is an app of the mobile
/// platform's default dialect (its `dragonfly check` asks for the line).
fn any_manifest(root: &Path) -> Result<Manifest, String> {
    match Manifest::read(root) {
        Ok(m) => Ok(m),
        Err(_) if root.join(packages::CONFIG).is_file() || root.join("main.nuc").is_file() => {
            let l = loader();
            let dialect = l.default_dialect(MOBILE).ok_or("no default dialect for the mobile platform (platform.toml)")?;
            let out = l.load(&dialect).0.map(|d| d.project.output.clone()).unwrap_or_default();
            let mut m = Manifest::infer(root, &dialect, &out);
            m.platform = Some(MOBILE.into());
            if let Ok(text) = std::fs::read_to_string(root.join(packages::CONFIG)) {
                let (c, _) = project::read_config(&text, &["app".to_string()]);
                if let Some(name) = packages::get(&c, "app", "name") {
                    m.name = name.to_string();
                }
            }
            Ok(m)
        }
        Err(e) => Err(e),
    }
}

/// The packages the generated app needs (`dev:` for tests), added when the app's pubspec lacks them.
const PACKAGES: &[&str] = &[
    "flutter_bloc",
    "fpdart",
    "go_router",
    "json_annotation",
    "dev:build_runner",
    "dev:json_serializable",
    "dev:mocktail",
    "dev:bloc_test",
];

/// The packages of the plugins `config.nuc` enables (`plugins(genkit = true)`), as `flutter pub add` takes them
/// (`dev:` for dev packages; a `local` one is a package of NuclearLang's `packages/` folder, by path).
fn plugin_packages(root: &Path, d: &Dialect) -> Vec<(String, String)> {
    let enabled = project::enabled_plugins(root, d);
    let mut out: Vec<(String, String)> = Vec::new();
    for p in d.project.with_requires(&enabled) {
        for (list, prefix) in [(&p.packages, ""), (&p.dev_packages, "dev:")] {
            for (name, version) in list {
                if out.iter().any(|(n, _)| n == name) {
                    continue;
                }
                let spec = if version == "local" {
                    format!("{prefix}{name}:{{\"path\":\"{}\"}}", runtime_package(name).display())
                } else {
                    format!("{prefix}{name}:{version}")
                };
                out.push((name.to_string(), spec));
            }
        }
    }
    out
}

/// The app's packages that came from plugins (`build/app/.dragonfly_plugins`): a plugin turned off removes them.
const PLUGIN_MARK: &str = ".dragonfly_plugins";

/// Adds the packages of the enabled plugins and removes those of the plugins turned off.
fn sync_plugin_packages(app: &Path, wanted: &[(String, String)]) -> Result<(), String> {
    let mark = app.join(PLUGIN_MARK);
    let before: Vec<String> =
        std::fs::read_to_string(&mark).map(|t| t.lines().map(str::to_string).collect()).unwrap_or_default();
    let pubspec = std::fs::read_to_string(app.join("pubspec.yaml")).unwrap_or_default();
    let listed = |name: &str| pubspec.lines().any(|l| l.trim_start().starts_with(&format!("{name}:")));
    let add: Vec<&str> = wanted.iter().filter(|(n, _)| !listed(n)).map(|(_, spec)| spec.as_str()).collect();
    let remove: Vec<&str> =
        before.iter().map(String::as_str).filter(|n| !wanted.iter().any(|(w, _)| w == n) && listed(n)).collect();
    if !add.is_empty() {
        let mut args = vec!["pub", "add"];
        args.extend(add);
        flutter(app, &args)?;
    }
    if !remove.is_empty() {
        let mut args = vec!["pub", "remove"];
        args.extend(remove);
        flutter(app, &args)?;
    }
    let names: Vec<&str> = wanted.iter().map(|(n, _)| n.as_str()).collect();
    std::fs::write(&mark, names.join("\n")).map_err(|e| e.to_string())
}

/// Adds to the app the packages Dragonfly needs that its pubspec.yaml does not list yet.
fn ensure_packages(app: &Path, splash: bool) -> Result<(), String> {
    let pubspec = std::fs::read_to_string(app.join("pubspec.yaml")).unwrap_or_default();
    let listed = |name: &str| pubspec.lines().any(|l| l.trim_start().starts_with(&format!("{name}:")));
    let mut missing: Vec<String> = Vec::new();
    // a `path:` of the dragonfly package that no longer exists (the repository was reorganized, the install moved)
    // points to the current one again
    let stale = pubspec
        .lines()
        .skip_while(|l| l.trim() != "dragonfly:")
        .nth(1)
        .and_then(|l| l.trim().strip_prefix("path:"))
        .is_some_and(|p| !Path::new(p.trim()).is_dir());
    if !listed("dragonfly") || stale {
        missing.push(format!("dragonfly:{{\"path\":\"{}\"}}", dragonfly_package().display()));
    }
    for p in PACKAGES {
        if !listed(p.trim_start_matches("dev:")) {
            missing.push(p.to_string());
        }
    }
    if splash && !listed("flutter_native_splash") {
        missing.push("flutter_native_splash".into());
    }
    if !listed("integration_test") {
        missing.push("dev:integration_test:{\"sdk\":\"flutter\"}".into());
    }
    if missing.is_empty() {
        return Ok(());
    }
    let mut args = vec!["pub", "add"];
    args.extend(missing.iter().map(String::as_str));
    flutter(app, &args)
}

fn loader() -> Loader {
    let mut paths: Vec<PathBuf> = Vec::new();
    if let Ok(env) = std::env::var("NUCLEAR_DIALECT_PATH") {
        paths.extend(env.split(':').filter(|s| !s.is_empty()).map(PathBuf::from));
    }
    paths.push(PathBuf::from("dialects"));
    paths.extend(nuclear_dialect::install_dir().map(|d| d.join("dialects")));
    paths.extend(nuclear_dialect::dialect_dirs(Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))));
    let mut l = Loader::new(paths);
    // kind packages (ADR-0095): the project's and the installed ones
    l.packages.push(PathBuf::from("kinds"));
    l.packages.extend(nuclear_dialect::install_dir().map(|d| d.join("kinds")));
    l
}

/// `dragonfly add <folder>`: installs a kind package (ADR-0095) for every project of this machine, after checking
/// that it loads with the dialects it implements.
fn add_package(source: &str) -> Result<(), String> {
    let src = PathBuf::from(source);
    let pkg = nuclear_dialect::model::kind_package_at(&src)
        .ok_or_else(|| format!("{} is not a kind package: it needs a kind.toml with [package] name, version, platform", src.display()))?;
    if pkg.platform.is_empty() {
        return Err("[package] needs platform (web, mobile, backend)".into());
    }
    if pkg.dialects.is_empty() {
        return Err(format!("the package implements no dialect: add {}/<dialect>/kind.toml", pkg.name));
    }
    // it must load with each dialect it implements (the package's errors are a dialect's errors)
    let mut l = loader();
    l.packages = vec![src.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from("."))];
    for d in &pkg.dialects {
        let (loaded, report) = l.load(d);
        if loaded.is_none() {
            let msgs: Vec<String> = report.diags.iter().map(|(_, x)| x.message.clone()).collect();
            return Err(format!("the package does not load with the {d} dialect: {}", msgs.join("; ")));
        }
    }
    let dest = nuclear_dialect::install_dir().ok_or("no install directory")?.join("kinds").join(&pkg.name);
    if dest.exists() {
        std::fs::remove_dir_all(&dest).map_err(|e| e.to_string())?;
    }
    copy_dir(&src, &dest)?;
    println!("✓ kind package {} ({}: {}) installed in {}", pkg.name, pkg.platform, pkg.dialects.join(", "), dest.display());
    Ok(())
}

fn copy_dir(from: &std::path::Path, to: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(from).map_err(|e| e.to_string())?.flatten() {
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// The dragonfly runtime package: `DRAGONFLY_PACKAGE`, the installed one, or the one of this repository.
fn dragonfly_package() -> PathBuf {
    if let Some(p) = std::env::var_os("DRAGONFLY_PACKAGE") {
        return p.into();
    }
    runtime_package("dragonfly")
}

/// A framework runtime package: the installed one, or the one of a source root of the working tree
/// (`<repo>/dragonfly-*/packages/<name>`, plan 006).
fn runtime_package(name: &str) -> PathBuf {
    let installed = nuclear_dialect::install_dir().map(|d| d.join("packages").join(name)).filter(|p| p.is_dir());
    installed.unwrap_or_else(|| {
        let repo = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
        let p = nuclear_dialect::source_roots(repo)
            .into_iter()
            .map(|r| r.join("packages").join(name))
            .find(|p| p.is_dir())
            .unwrap_or_else(|| repo.join("packages").join(name));
        p.canonicalize().unwrap_or(p)
    })
}

/// The flutter binary: `FLUTTER`, `FLUTTER_ROOT`, a project pinned with fvm (`.fvm/flutter_sdk`), the
/// PATH, then the usual install folders (fvm's default version first).
fn find_flutter() -> Option<PathBuf> {
    if let Some(f) = std::env::var_os("FLUTTER") {
        return Some(f.into());
    }
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let mut dirs: Vec<PathBuf> =
        std::env::var_os("FLUTTER_ROOT").map(|r| PathBuf::from(r).join("bin")).into_iter().collect();
    if let Ok(cwd) = std::env::current_dir() {
        dirs.extend(cwd.ancestors().map(|d| d.join(".fvm/flutter_sdk/bin")));
    }
    dirs.extend(std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect::<Vec<_>>()).unwrap_or_default());
    dirs.extend(
        ["fvm/default/bin", "development/flutter/bin", "flutter/bin", "snap/flutter/common/flutter/bin"]
            .map(|d| home.join(d)),
    );
    dirs.extend(["/opt/flutter/bin", "/usr/lib/flutter/bin", "/usr/local/flutter/bin"].map(PathBuf::from));
    dirs.into_iter().map(|d| d.join("flutter")).find(|f| f.is_file())
}

const NO_FLUTTER: &str = "Flutter was not found.

dragonfly needs the Flutter SDK to create, build and run the app (your .nuc files are fine:
`dragonfly check` works without Flutter).

Where dragonfly looked: $FLUTTER, $FLUTTER_ROOT/bin, .fvm/flutter_sdk (fvm, in the project or above),
the PATH, ~/fvm/default, ~/development/flutter, ~/flutter, ~/snap/flutter, /opt/flutter, /usr/lib/flutter.

Possible causes:
  - Flutter is not installed.
  - It is in another folder: export FLUTTER_ROOT=/path/to/flutter (in ~/.zshrc or ~/.bashrc).
  - You use fvm but chose no version: fvm global stable (or fvm use <version> in the project).
  - Only another shell has it in the PATH: add export PATH=\"$HOME/development/flutter/bin:$PATH\"
    to the config of the shell you use.

Install it: https://docs.flutter.dev/get-started/install
  or with fvm: dart pub global activate fvm && fvm install stable && fvm global stable
Then check it with: flutter doctor";

/// The Flutter SDK folder, when it is known (VS Code gets it as `dart.flutterSdkPath`).
fn flutter_root() -> Option<PathBuf> {
    let bin = find_flutter()?;
    let root = bin.parent()?.parent()?;
    if root.join("packages/flutter").is_dir() {
        // keeps links like fvm's `default`, so VS Code follows `fvm global`
        return Some(root.to_path_buf());
    }
    // a link to the binary (/usr/bin/flutter)
    bin.canonicalize().ok()?.parent()?.parent().map(Path::to_path_buf)
}

fn flutter_bin() -> Result<String, String> {
    find_flutter().map(|f| f.display().to_string()).ok_or_else(|| NO_FLUTTER.to_string())
}

/// The manifest of a backend project (`dragonfly(platform = "backend", ...)`), `None` for a mobile app.
/// A project the CLI does not build with Flutter (backend or web, ADR-0093/0097): `nuclear build .` plus the
/// dialect's `[toolchain]`. `None` for a mobile app (built here) or a folder that is not a project.
fn non_mobile_manifest(root: &Path) -> Result<Option<Manifest>, String> {
    match Manifest::read(root) {
        Ok(m) if matches!(m.platform.as_deref(), Some("backend" | "web")) => Ok(Some(m)),
        Ok(_) => Ok(None),
        Err(_) if root.join(packages::CONFIG).is_file() || root.join("main.nuc").is_file() => Ok(None),
        Err(e) => Err(e),
    }
}

/// A backend or web project: `build` is `nuclear build .`; `install`, `run` and `test` run the dialect's
/// `[toolchain]` command in the generated project (ADR-0072; `{{packages}}` is the install's runtimes folder).
/// A toolchain command of a backend or web project in its generated folder. `vars` are more placeholders (`{{message}}`
/// of `migrate make`), quoted for the system's shell.
fn backend_command(root: &Path, cmd: &str, m: &Manifest, rest: &[String], vars: &[(&str, String)]) -> Result<(), String> {
    run("nuclear", root, &["build", "."])?;
    let (dialect, report) = loader().load(&m.dialect);
    let dialect = dialect.ok_or_else(|| {
        let msgs: Vec<String> = report.diags.iter().map(|(_, x)| x.message.clone()).collect();
        format!("dialect '{}': {}", m.dialect, msgs.join("; "))
    })?;
    let out = root.join(project::output_dir(m, &dialect));
    // the editor's tasks and debuggers of this project, as for a mobile one
    let out_rel = project::output_dir(m, &dialect).replace('\\', "/");
    vscode::write(root, m, &dialect, &out_rel, serde_json::Map::new(), serde_json::Map::new())?;
    let packages = nuclear_dialect::install_dir().ok_or("no install directory: run install.sh")?.join("packages");
    // the placeholders of a toolchain command: the runtimes, the dialect's address and its port
    let fill = |command: &str| {
        let command = command
            .replace("{{packages}}", &packages.display().to_string())
            .replace("{{url}}", dialect.tool("url").unwrap_or_default())
            .replace("{{port}}", &vscode::url_port(&dialect).unwrap_or_default())
            .replace("{{host}}", "localhost");
        vars.iter().fold(command, |c, (k, v)| c.replace(&format!("{{{{{k}}}}}"), &shell_quote(v)))
    };
    if cmd == "build" {
        // `compile` (a TypeScript server): once the app is installed, so the running server restarts on the build
        if let (Some(compile), Some(installed)) = (dialect.tool("compile"), dialect.tool("installed"))
            && out.join(installed).exists()
        {
            return shell(&out, &fill(compile));
        }
        return Ok(());
    }
    let Some(command) = dialect.tool(cmd) else {
        let has: Vec<&str> = dialect.toolchain.keys().map(String::as_str).collect();
        return Err(format!("the {} dialect has no toolchain.{cmd} (it has: {})", m.dialect, has.join(", ")));
    };
    let mut command = fill(command);
    // a dependency that is a folder (`dependencies(lib = "../lib")`) keeps its version while its code changes: the
    // package manager would keep the old copy, so `install_local` installs each one again
    if cmd == "install"
        && let Some(local) = dialect.tool("install_local")
    {
        for version in m.dependencies.values().filter(|v| v.starts_with('.') || v.starts_with('/')) {
            let path = root.join(version);
            let path = path.canonicalize().unwrap_or(path);
            command.push_str(" && ");
            command.push_str(&fill(&local.replace("{{path}}", &shell_quote(&path.display().to_string()))));
        }
    }
    let extra = passthrough(rest);
    if !extra.is_empty() {
        command.push(' ');
        command.push_str(&extra.join(" "));
    }
    shell(&out, &command)
}

/// `value` as one word of the system's shell.
fn shell_quote(value: &str) -> String {
    if cfg!(windows) { format!("\"{}\"", value.replace('"', "\\\"")) } else { format!("'{}'", value.replace('\'', "'\\''")) }
}

/// `dragonfly migrate [make "<message>" | up | down]` (ADR-0100 §5): the dialect's `migrate_make` / `migrate` /
/// `migrate_down` in the generated project, over the project's `migrations/` folder (kept with the sources).
fn migrate_command(root: &Path, rest: &[String]) -> Result<(), String> {
    const USAGE: &str = "usage: dragonfly migrate make \"<what changes>\" | dragonfly migrate [up] | dragonfly migrate down";
    let m = non_mobile_manifest(root)?.ok_or("migrations are of a backend project's database")?;
    let words: Vec<&String> = rest.iter().take_while(|a| *a != "--").collect();
    let (key, message) = match words.first().map(|w| w.as_str()) {
        Some("make") => ("migrate_make", Some(words.get(1).ok_or(USAGE)?.to_string())),
        None | Some("up") => ("migrate", None),
        Some("down") => ("migrate_down", None),
        Some(_) => return Err(USAGE.into()),
    };
    let mut vars = vec![("migrations", root.join("migrations").display().to_string())];
    if let Some(message) = message {
        vars.push(("message", message));
    }
    backend_command(root, key, &m, &[], &vars)
}

/// A toolchain command in the system's shell: `sh -c` (Linux, macOS), `cmd /C` (Windows).
fn shell(dir: &Path, command: &str) -> Result<(), String> {
    if cfg!(windows) { run("cmd", dir, &["/C", command]) } else { run("sh", dir, &["-c", command]) }
}

fn run(program: &str, dir: &Path, args: &[&str]) -> Result<(), String> {
    let shown = format!(
        "{} {}",
        Path::new(program).file_name().map_or(program.into(), |f| f.to_string_lossy()),
        args.join(" ")
    );
    eprintln!("{} {}", landing::dim("$"), landing::cmd(&shown));
    let status = Command::new(program).args(args).current_dir(dir).status().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound if program.ends_with("flutter") => format!("{program}: {e}\n\n{NO_FLUTTER}"),
        _ => format!("cannot run {program}: {e}"),
    })?;
    if status.success() { Ok(()) } else { Err(String::new()) }
}

fn flutter(app: &Path, args: &[&str]) -> Result<(), String> {
    run(&flutter_bin()?, app, args)
}

fn app_dir(root: &Path) -> Result<PathBuf, String> {
    let m = manifest(root)?;
    let app = root.join(&m.out);
    if !app.join("pubspec.yaml").is_file() {
        return Err(format!("the app does not exist yet: run `dragonfly build` first ({})", app.display()));
    }
    Ok(app)
}

// ---------------------------------------------------------------------------------------------------
// create

/// The platform and the dialect of `dragonfly new`: a dialect alone gives its platform; a platform alone gives its
/// default dialect (platform.toml `default_dialect`); neither is mobile with its default.
fn platform_and_dialect(platform: Option<String>, dialect: Option<String>) -> Result<(String, String), String> {
    let l = loader();
    let platform = match (platform, &dialect) {
        (Some(p), _) => p,
        (None, Some(d)) => l.load(d).0.and_then(|d| d.platform).ok_or_else(|| format!("unknown dialect '{d}' (nuclear dialect list)"))?,
        (None, None) => MOBILE.into(),
    };
    let dialect = match dialect {
        Some(d) => d,
        None => l.default_dialect(&platform).ok_or_else(|| format!("unknown platform '{platform}' (mobile, backend, web)"))?,
    };
    Ok((platform, dialect))
}

/// A project name: letters, digits and `_` (written in snake_case); an option glued to it (`shop-p`) is refused.
fn new_name(name: &str) -> Result<String, String> {
    if let Some((_, flag)) = name.rsplit_once('-').filter(|(_, f)| ["p", "d", "-platform", "-dialect", "-org"].contains(f)) {
        return Err(format!("'{name}' looks like a name with the option -{flag} glued to it: leave a space before it"));
    }
    let snake = scaffold::snake(name);
    if snake.is_empty() || !snake.chars().next().is_some_and(|c| c.is_ascii_lowercase()) || !snake.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
        return Err(format!("'{name}' is not a project name: letters, digits and _, starting with a letter"));
    }
    Ok(snake)
}

/// Why a stray word of `dragonfly new` is there: most often a platform or a dialect without its option.
fn hint_for(word: &str) -> String {
    if ["mobile", "backend", "web"].contains(&word) {
        format!("did you mean -p {word}?")
    } else if loader().load(word).0.is_some() {
        format!("did you mean -d {word}?")
    } else {
        "one name only".into()
    }
}

/// `dragonfly new [-p mobile] [-d flutter] <name>`: a project of a platform, built with a dialect (ADR-0093).
/// Only mobile/flutter gets its app created and built here; a backend project gets its `.nuc` skeleton and is
/// built with `nuclear build .`.
fn new_project(rest: Vec<String>) -> Result<(), String> {
    let mut rest = rest;
    let org = take_option(&mut rest, "--org").unwrap_or_else(|| "com.example".into());
    let platform = take_option(&mut rest, "--platform").or_else(|| take_option(&mut rest, "-p"));
    let dialect = take_option(&mut rest, "--dialect").or_else(|| take_option(&mut rest, "-d"));
    let (platform, dialect) = platform_and_dialect(platform, dialect)?;
    let usage = "usage: dragonfly new <name> [-p mobile|backend|web] [-d flutter|fastapi|nestjs|react] [--org com.example]";
    let name = new_name(rest.first().ok_or(usage)?)?;
    if let Some(extra) = rest.get(1) {
        return Err(format!("unexpected argument '{extra}' ({})\n{usage}", hint_for(extra)));
    }
    let root = PathBuf::from(&name);
    if root.join(packages::CONFIG).exists() {
        return Err(format!("{} already has a config.nuc", root.display()));
    }
    let (d, _) = loader().load(&dialect);
    let d = d.ok_or_else(|| format!("unknown dialect '{dialect}' (nuclear dialect list)"))?;
    if d.platform.as_deref() != Some(platform.as_str()) {
        return Err(format!(
            "'{dialect}' is a dialect of the {} platform, not of '{platform}'",
            d.platform.as_deref().unwrap_or("no")
        ));
    }
    if platform == "web" {
        // where a new backend answers: the url of the backend platform's default dialect
        let l = loader();
        let backend = l.default_dialect("backend").and_then(|b| l.load(&b).0).and_then(|b| b.tool("url").map(str::to_string));
        let backend = backend.unwrap_or_else(|| "http://localhost".into());
        let report = scaffold::write(&root, &scaffold::web_project(&name, &dialect, &backend)).map_err(|e| e.to_string())?;
        eprintln!("{}", report.join("\n"));
        if let Err(e) = agent_init(&root, &dialect) {
            eprintln!("{} {e}", landing::dim("agent files not written:"));
        }
        eprintln!("\n{} web project {} created. Next:", landing::ok("✓"), landing::name(&name));
        eprintln!("  {}", landing::cmd(&format!("cd {name} && dragonfly build")));
        eprintln!("  {}   {}", landing::cmd("dragonfly install && dragonfly run"), landing::dim("(npm install, then vite dev)"));
        eprintln!("  {}", landing::dim("add screens with: dragonfly make screen <component> <screen> --route /path"));
        return Ok(());
    }
    if platform != MOBILE {
        let report = scaffold::write(&root, &scaffold::backend_project(&name, &dialect)).map_err(|e| e.to_string())?;
        eprintln!("{}", report.join("\n"));
        if let Err(e) = agent_init(&root, &dialect) {
            eprintln!("{} {e}", landing::dim("agent files not written:"));
        }
        eprintln!("\n{} {platform} project {} created. Next:", landing::ok("✓"), landing::name(&name));
        eprintln!("  {}", landing::cmd(&format!("cd {name} && dragonfly build && dragonfly install")));
        eprintln!("  {}   {}", landing::cmd("dragonfly run"), landing::dim("(dragonfly test runs the tests)"));
        eprintln!("  {}", landing::dim("add pieces with: dragonfly make <kind> <component> <Name>"));
        return Ok(());
    }
    let report = scaffold::write(&root, &scaffold::project(&name)).map_err(|e| e.to_string())?;
    eprintln!("{}", report.join("\n"));
    // AI agents: AGENTS.md, CLAUDE.md, the Cursor rule and the check hook (not fatal: nuclear may be missing)
    if let Err(e) = agent_init(&root, &dialect) {
        eprintln!("{} {e}", landing::dim("agent files not written:"));
    }
    let next = format!("\n\nThe project's .nuc files are in {name}/. Once Flutter works: cd {name} && dragonfly build");
    create_app(&root, &name, &org).map_err(|e| format!("{e}{next}"))?;
    build(&root)?;
    eprintln!("\n{} project {} created. Next:", landing::ok("✓"), landing::name(&name));
    eprintln!("  {}", landing::cmd(&format!("cd {name}")));
    eprintln!(
        "  {}     {}",
        landing::cmd("dragonfly code"),
        landing::dim("(VS Code: run, debug, devices, hot reload)")
    );
    eprintln!("  {}      {}", landing::cmd("dragonfly run"), landing::dim("(from the terminal)"));
    Ok(())
}

/// `dragonfly agent init`: the files AI agents read (AGENTS.md...), written by `nuclear agent init`.
fn agent_init(root: &Path, dialect: &str) -> Result<(), String> {
    let status = Command::new("nuclear")
        .args(["agent", "init", "--dialect", dialect])
        .current_dir(root)
        .status()
        .map_err(|e| format!("cannot run `nuclear` ({e}): run ./install.sh"))?;
    if status.success() { Ok(()) } else { Err("nuclear agent init failed".into()) }
}

/// `flutter create` in the output folder, without the counter example, plus the packages Dragonfly uses.
fn create_app(root: &Path, name: &str, org: &str) -> Result<PathBuf, String> {
    let m = manifest(root)?;
    let app = root.join(&m.out);
    std::fs::create_dir_all(&app).map_err(|e| e.to_string())?;
    flutter(&app, &["create", "--project-name", name, "--org", org, "--empty", "."])?;
    let _ = std::fs::remove_file(app.join("test/widget_test.dart"));
    ensure_packages(&app, false)?;
    Ok(app)
}

// ---------------------------------------------------------------------------------------------------
// build

/// config.nuc → the app's pubspec and the definitions of its packages; then transpiles the project into
/// the app, then `pub get` and `build_runner` when they are needed.
fn build(root: &Path) -> Result<PathBuf, String> {
    let m = manifest(root)?;
    let loader = loader();
    let (d, report) = loader.load(&m.dialect);
    let Some(mut d) = d else {
        let names: Vec<String> = report.diags.iter().map(|(_, x)| x.message.clone()).collect();
        return Err(format!("dialect '{}' has errors: {}", m.dialect, names.join("; ")));
    };
    let app = root.join(&m.out);
    if !app.join("pubspec.yaml").is_file() {
        create_app(root, &m.name, "com.example")?;
    }
    if packages::ensure_config(root, &app, &m.name, &d)? {
        eprintln!("{} {} created: the project had none (from the app's pubspec)", landing::ok("✓"), packages::CONFIG);
    }
    if let Some(c) = packages::read(root, &d) {
        packages::sync(&app, &c, &d)?;
        if needs_pub_get(&app) {
            pub_get(&app)?;
        }
        packages::load_defs(root, &app, &c, &loader, &mut d, true)?;
    }
    let out = project::build(root, &m, &d);
    let _ = project::write_metadata(root, &m, &d);
    print_diagnostics(&out, false);
    if out.has_errors() {
        return Err(CODE_ERRORS.into());
    }
    assets::fonts(&asset_paths(&out))?;
    let splash = out.generated.iter().any(|(p, _)| p == SPLASH_YAML);
    ensure_packages(&app, splash)?;
    sync_plugin_packages(&app, &plugin_packages(root, &d))?;
    let written = write_app(&app, &out)?;
    write_line_map(&app, &out)?;
    assets::write(&app, &written)?;
    // the machine's Flutter SDK: for the tasks (FLUTTER_ROOT) and for the Dart extension
    let (mut env, mut machine) = (serde_json::Map::new(), serde_json::Map::new());
    if let Some(f) = flutter_root() {
        env.insert("FLUTTER_ROOT".into(), serde_json::json!(f.display().to_string()));
        machine.insert("dart.flutterSdkPath".into(), serde_json::json!(f.display().to_string()));
    }
    vscode::write(root, &m, &d, &m.out, env, machine)?;
    eprintln!(
        "{} {}: {} files transpiled → {}",
        landing::ok("✓"),
        landing::name(&m.name),
        out.files.len(),
        landing::dim(&app.display().to_string())
    );
    if needs_pub_get(&app) {
        pub_get(&app)?;
    }
    if needs_build_runner(&app, &written) {
        run(&dart_bin()?, &app, &["run", "build_runner", "build", "--delete-conflicting-outputs"])?;
        std::fs::write(app.join(BUILD_RUNNER_MARK), "").map_err(|e| e.to_string())?;
    }
    if splash {
        native_splash(&app, &out)?;
    }
    Ok(app)
}

const CODE_ERRORS: &str = "the project has errors; nothing was written";

/// No lock, no package config (`flutter clean` deletes `.dart_tool`), or a pubspec changed since the last
/// `pub get`. The lock's date is not enough: `pub get` leaves it untouched when nothing changes, so a pubspec
/// rewritten with the same packages would ask for `pub get` on every build.
fn needs_pub_get(app: &Path) -> bool {
    let lock = app.join("pubspec.lock");
    let config = app.join(".dart_tool/package_config.json");
    let last = modified(&lock).max(modified(&app.join(PUB_GET_MARK)));
    !lock.is_file() || !config.is_file() || modified(&app.join("pubspec.yaml")) > last
}

const PUB_GET_MARK: &str = ".dart_tool/nuclear_pub_get";

fn pub_get(app: &Path) -> Result<(), String> {
    flutter(app, &["pub", "get"])?;
    std::fs::write(app.join(PUB_GET_MARK), "").map_err(|e| e.to_string())
}

/// `dragonfly explain`: reads the error from stdin.
fn explain_command(root: &Path, json: bool) -> Result<(), String> {
    use std::io::Read;
    // backend and web: the output folder and the line map every project build writes; mobile: the Flutter app's
    let (app, map_file) = match non_mobile_manifest(root)? {
        Some(m) => {
            let (dialect, _) = loader().load(&m.dialect);
            let dialect = dialect.ok_or_else(|| format!("dialect '{}' could not be loaded", m.dialect))?;
            let out = root.join(project::output_dir(&m, &dialect));
            (out.clone(), out.join(project::LINE_MAP_FILE))
        }
        None => {
            let app = app_dir(root)?;
            (app.clone(), app.join(LINE_MAP))
        }
    };
    let mut text = String::new();
    std::io::stdin().read_to_string(&mut text).map_err(|e| e.to_string())?;
    let map: serde_json::Value = std::fs::read_to_string(map_file)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .ok_or("no line map yet: run `dragonfly build` first")?;
    let got = explain::explain(root, &app, &map, &text);
    if json {
        println!("{got}");
        return Ok(());
    }
    println!(
        "{} {}
{}",
        landing::bad("✗"),
        got["title"].as_str().unwrap_or_default(),
        got["hint"].as_str().unwrap_or_default()
    );
    for f in got["frames"].as_array().into_iter().flatten() {
        let source = Path::new(f["source"].as_str().unwrap_or_default());
        let shown = source.strip_prefix(root).unwrap_or(source).display();
        println!(
            "  {}:{}  {}",
            landing::cmd(&shown.to_string()),
            f["line"],
            landing::dim(f["name"].as_str().unwrap_or_default())
        );
    }
    Ok(())
}

/// For when the build or the app fails for reasons that are not the `.nuc` code (a broken pub get, stale
/// generated code, a stale Gradle/Xcode build). Each step runs only when the one before did not leave a
/// valid app (`flutter analyze` without errors): build; pub get + build_runner + splash from scratch;
/// flutter clean and everything again. Errors in the `.nuc` stop it at once: no step can fix them.
fn repair(root: &Path) -> Result<(), String> {
    let step = |n: &str, what: &str| eprintln!("\n{} {} {}", landing::warn("▸"), landing::name(n), landing::dim(what));
    let valid = |app: &Path| flutter(app, &["analyze", "--no-pub", "--no-fatal-infos", "--no-fatal-warnings"]).is_ok();
    let attempt = |root: &Path| match build(root) {
        Ok(app) => Ok(valid(&app)),
        Err(e) if e == CODE_ERRORS => Err(e),
        Err(e) => {
            if !e.is_empty() {
                eprintln!("{} {e}", landing::bad("✗"));
            }
            Ok(false)
        }
    };
    step("1/3", "build and analyze");
    if attempt(root)? {
        return Ok(());
    }
    step("2/3", "pub get, build_runner and splash from scratch");
    if let Ok(app) = app_dir(root) {
        for mark in [BUILD_RUNNER_MARK, ".dart_tool/nuclear_splash"] {
            let _ = std::fs::remove_file(app.join(mark));
        }
        let _ = pub_get(&app);
    }
    if attempt(root)? {
        return Ok(());
    }
    step("3/3", "flutter clean and everything again");
    if let Ok(app) = app_dir(root) {
        let _ = flutter(&app, &["clean"]);
    }
    if attempt(root)? {
        return Ok(());
    }
    Err("the app still fails after flutter clean: see the errors above (flutter analyze, pub get or build_runner)"
        .into())
}

/// The config of `@splash` (decision 63), read by `dart run flutter_native_splash:create`.
const SPLASH_YAML: &str = "flutter_native_splash.yaml";

/// Regenerates the native splash (Android, iOS, web resources) when its config or an asset changed since
/// the last time.
// shortcut: removing @splash deletes the yaml but leaves the native splash in place (`:remove` needs the
// yaml); run `dart run flutter_native_splash:remove` by hand before removing it, until it is automated.
fn native_splash(app: &Path, out: &project::ProjectOutput) -> Result<(), String> {
    let mark = app.join(".dart_tool/nuclear_splash");
    let sources = std::iter::once(SPLASH_YAML).chain(out.copies.iter().map(|(rel, _)| rel.as_str()));
    if sources.map(|rel| modified(&app.join(rel))).max() <= Some(modified(&mark)) {
        return Ok(());
    }
    run(&dart_bin()?, app, &["run", "flutter_native_splash:create"])?;
    std::fs::write(&mark, "").map_err(|e| e.to_string())
}

/// The files of the project's `assets/` folders, relative to the app.
fn asset_paths(out: &project::ProjectOutput) -> Vec<String> {
    out.copies.iter().map(|(r, _)| r.clone()).chain(out.generated.iter().map(|(r, _)| r.clone())).collect()
}

fn dart_bin() -> Result<String, String> {
    Ok(flutter_bin()?.strip_suffix("flutter").map_or("dart".into(), |dir| format!("{dir}dart")))
}

fn modified(p: &Path) -> std::time::SystemTime {
    std::fs::metadata(p).and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH)
}

/// Writes the generated files (only the ones whose content changed, so unchanged files keep their date)
/// and removes the files a previous build wrote that no `.nuc` produces any more.
fn write_app(app: &Path, out: &project::ProjectOutput) -> Result<Vec<String>, String> {
    let list_path = app.join(".dart_tool/nuclear_files");
    let before: Vec<String> =
        std::fs::read_to_string(&list_path).map(|t| t.lines().map(str::to_string).collect()).unwrap_or_default();
    let mut now = Vec::new();
    let all = out.files.iter().filter(|f| !f.out_path.is_empty()).map(|f| (f.out_path.as_str(), f.code.as_str()));
    for (rel, code) in all.chain(out.generated.iter().map(|(p, c)| (p.as_str(), c.as_str()))) {
        let target = app.join(rel);
        if std::fs::read_to_string(&target).ok().as_deref() != Some(code) {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::write(&target, code).map_err(|e| format!("cannot write {}: {e}", target.display()))?;
        }
        now.push(rel.to_string());
    }
    for (rel, source) in &out.copies {
        let target = app.join(rel);
        let bytes = std::fs::read(source).map_err(|e| format!("cannot read {}: {e}", source.display()))?;
        if std::fs::read(&target).ok() != Some(bytes) {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::copy(source, &target).map_err(|e| format!("cannot copy to {}: {e}", target.display()))?;
        }
        now.push(rel.clone());
    }
    for old in before.iter().filter(|o| !now.contains(o)) {
        let _ = std::fs::remove_file(app.join(old));
        let _ = std::fs::remove_file(app.join(old.replace(".dart", ".g.dart")));
    }
    std::fs::create_dir_all(app.join(".dart_tool")).map_err(|e| e.to_string())?;
    std::fs::write(&list_path, now.join("\n")).map_err(|e| e.to_string())?;
    Ok(now)
}

/// The line map the editors' debuggers read: for each generated `.dart` (relative to the app), its `.nuc`
/// (relative to the project) and the pairs (dart line, nuc line).
const LINE_MAP: &str = ".dart_tool/nuclear_lines.json";

fn write_line_map(app: &Path, out: &project::ProjectOutput) -> Result<(), String> {
    let files: serde_json::Map<String, serde_json::Value> = out
        .files
        .iter()
        .filter(|f| !f.out_path.is_empty())
        .map(|f| {
            let source = f.source_path.to_string_lossy().replace('\\', "/");
            (f.out_path.clone(), serde_json::json!({ "source": source, "lines": f.lines }))
        })
        .collect();
    let text = serde_json::json!({ "version": 1, "files": files }).to_string();
    let path = app.join(LINE_MAP);
    if std::fs::read_to_string(&path).ok().as_deref() == Some(text.as_str()) {
        return Ok(());
    }
    std::fs::create_dir_all(app.join(".dart_tool")).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// Written when build_runner ends well: it does not rewrite a `.g.dart` that stays the same, so the date of
/// the `.g.dart` cannot tell whether its file changed after the last run.
const BUILD_RUNNER_MARK: &str = ".dart_tool/nuclear_build_runner";

/// A file with a `part 'x.g.dart';` line whose x.g.dart is missing or that changed after the last run.
fn needs_build_runner(app: &Path, written: &[String]) -> bool {
    let last = modified(&app.join(BUILD_RUNNER_MARK));
    written.iter().any(|rel| {
        let p = app.join(rel);
        let has_part = std::fs::read_to_string(&p).is_ok_and(|t| t.lines().any(|l| l.starts_with("part '")));
        has_part && (!p.with_extension("g.dart").is_file() || modified(&p) > last)
    })
}

fn print_diagnostics(out: &project::ProjectOutput, json: bool) {
    let mut items = Vec::new();
    for f in &out.files {
        let file = SourceFile::new(f.source_path.display().to_string(), f.source.clone());
        if json {
            items.extend(json_items(&f.diagnostics, &file));
        } else if !f.diagnostics.is_empty() {
            eprint!("{}", landing::diagnostics(&render_text(&f.diagnostics, &file)));
        }
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&items).unwrap_or_default());
    }
}

fn check(root: &Path, json: bool) -> Result<(), String> {
    let m = manifest(root)?;
    let loader = loader();
    let (d, _) = loader.load(&m.dialect);
    let mut d = d.ok_or(format!("dialect '{}' has errors (nuclear dialect check {})", m.dialect, m.dialect))?;
    // without Flutter: only the definitions already in the cache
    if packages::ensure_config(root, &root.join(&m.out), &m.name, &d)? {
        eprintln!("{} {} created: the project had none (from the app's pubspec)", landing::ok("✓"), packages::CONFIG);
    }
    if let Some(c) = packages::read(root, &d) {
        packages::load_defs(root, &root.join(&m.out), &c, &loader, &mut d, false)?;
    }
    let out = project::build(root, &m, &d);
    let _ = project::write_metadata(root, &m, &d);
    print_diagnostics(&out, json);
    if out.has_errors() {
        return Err(String::new());
    }
    assets::fonts(&asset_paths(&out))?;
    if !json {
        eprintln!("{} {}: {} files, no errors", landing::ok("✓"), landing::name(&m.name), out.files.len());
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------------
// pub

/// `dragonfly flutter add|remove`: packages through config.nuc; any other `dragonfly flutter <command>` runs
/// flutter inside the app.
fn flutter_command(root: &Path, rest: &[String]) -> Result<(), String> {
    let name = manifest(root)?.name;
    match rest.first().map(String::as_str) {
        Some("add") => {
            let m = manifest(root)?;
            let (d, _) = loader().load(&m.dialect);
            let d = d.ok_or(format!("dialect '{}' has errors (nuclear dialect check {})", m.dialect, m.dialect))?;
            let app = build(root)?;
            packages::add(root, &app, &rest[1..], &name, &d)?;
            build(root).map(|_| ())
        }
        Some("remove") => {
            packages::remove(root, &rest[1..], &name)?;
            build(root).map(|_| ())
        }
        _ => flutter(&app_dir(root)?, &rest.iter().map(String::as_str).collect::<Vec<_>>()),
    }
}

fn pub_command(root: &Path, rest: &[String]) -> Result<(), String> {
    if matches!(rest.first().map(String::as_str), Some("add" | "remove")) {
        return flutter_command(root, rest);
    }
    let app = app_dir(root)?;
    let mut a = vec!["pub"];
    a.extend(rest.iter().map(String::as_str));
    if rest.is_empty() {
        a.push("get");
    }
    flutter(&app, &a)
}

// ---------------------------------------------------------------------------------------------------
// make

/// Long name of a generator from its name or its alias.
fn generator(word: &str) -> Option<&'static str> {
    Some(match word {
        "component" | "c" => "component",
        "screen" | "s" => "screen",
        "stateful" | "sf" => "stateful",
        "statemanager" | "sm" => "statemanager",
        "statemodel" | "smo" => "statemodel",
        "usecase" | "uc" => "usecase",
        "repository" | "r" => "repository",
        "model" | "mo" => "model",
        "service" | "sv" => "service",
        "aggregate" | "ag" => "aggregate",
        "config" | "cf" => "config",
        "test" | "t" => "test",
        "event" | "e" => "event",
        "widget" | "w" => "widget",
        "guard" | "gd" => "guard",
        "global" | "gl" => "global",
        "page" | "pg" => "page",
        "integration" | "it" => "integration",
        _ => return None,
    })
}

fn make(root: &Path, rest: Vec<String>) -> Result<(), String> {
    let mut rest = rest;
    if let Some(m) = non_mobile_manifest(root)? {
        return make_from_layout(root, &m, rest);
    }
    let route = take_option(&mut rest, "--route");
    let manager = take_option(&mut rest, "--manager").or_else(|| take_option(&mut rest, "-m"));
    let state = take_option(&mut rest, "--state").or_else(|| take_option(&mut rest, "-st"));
    let not_found = rest.iter().position(|a| a == "--not-found").map(|i| rest.remove(i)).is_some();
    let usage = || {
        format!(
            "usage:\n{}",
            USAGE.lines().skip_while(|l| !l.starts_with("code generation")).take(21).collect::<Vec<_>>().join("\n")
        )
    };
    let Some(what) = rest.first().and_then(|w| generator(w)) else { return Err(usage()) };
    let names: Vec<String> = rest[1..].iter().map(|w| scaffold::snake(w)).collect();
    let files = match (what, names.as_slice()) {
        ("component", [c]) => scaffold::component(c),
        ("config", [c]) => scaffold::config(c),
        ("test", [c]) => scaffold::test(c),
        ("screen", [c, s]) => {
            let route = route.unwrap_or_else(|| format!("/{}", s.replace('_', "-")));
            let manager = manager.map(|m| scaffold::snake(&m)).unwrap_or_else(|| format!("{s}_manager"));
            let state = state.map(|m| scaffold::snake(&m)).unwrap_or_else(|| format!("{s}_state"));
            scaffold::screen(c, s, &route, &manager, &state)
        }
        ("usecase" | "repository" | "model" | "service" | "aggregate", [c, name]) => {
            scaffold::in_component(what, c, name).unwrap_or_default()
        }
        ("event" | "widget" | "stateful", [name]) => scaffold::shared(what, name).unwrap_or_default(),
        ("statemanager" | "statemodel", [c, s, name]) => {
            scaffold::presentation(what, c, s, name).unwrap_or_default()
        }
        ("guard" | "global" | "page" | "integration", [name]) => {
            let screen = match (not_found, &route) {
                (true, _) => "notFound = true".to_string(),
                (false, Some(r)) => format!("route = \"{r}\""),
                (false, None) => {
                    format!("route = \"/{}\"", name.strip_suffix("_screen").unwrap_or(name).replace('_', "-"))
                }
            };
            scaffold::app_wide(what, name, &screen).unwrap_or_default()
        }
        _ => return Err(usage()),
    };
    let in_component = !matches!(
        what,
        "component" | "event" | "widget" | "stateful" | "test" | "guard" | "global" | "page" | "integration"
    );
    let report = scaffold::write(root, &files).map_err(|e| e.to_string())?;
    eprintln!("{}", report.join("\n"));
    if in_component && !root.join("components").join(&names[0]).join(format!("{}_component.nuc", names[0])).is_file() {
        eprintln!(
            "{} the component '{}' has no facade yet; create it with: {}",
            landing::warn("note:"),
            landing::name(&names[0]),
            landing::cmd(&format!("dragonfly g c {}", names[0]))
        );
    }
    Ok(())
}

/// `dragonfly make <kind> <component> <Name> [--screen <s>]` in a backend or web project: the platform's skeleton of
/// the kind (`[skeletons]` of platform.toml) at its place (`[layout]`), the same for every dialect of the platform.
fn make_from_layout(root: &Path, m: &Manifest, rest: Vec<String>) -> Result<(), String> {
    let mut rest = rest;
    let screen = take_option(&mut rest, "--screen").map(|s| scaffold::snake(&s)).unwrap_or_else(|| "widgets".into());
    let (dialect, _) = loader().load(&m.dialect);
    let d = dialect.ok_or_else(|| format!("dialect '{}' could not be loaded", m.dialect))?;
    let platform = m.platform.clone().unwrap_or_default();
    let kinds: Vec<&String> = d.skeletons.keys().collect();
    let usage = || {
        format!(
            "usage: dragonfly make <kind> <component> <Name> [--screen <screen>]\nkinds of the {platform} platform: {}",
            kinds.iter().map(|k| k.as_str()).collect::<Vec<_>>().join(", ")
        )
    };
    let [kind, component, name] = rest.as_slice() else { return Err(usage()) };
    let kind = generator(kind).filter(|k| d.skeletons.contains_key(*k)).map(str::to_string).unwrap_or_else(|| kind.clone());
    let (Some(skeleton), Some(place)) = (d.skeletons.get(&kind), d.layout.get(&kind)) else { return Err(usage()) };
    let name = scaffold::pascal(&scaffold::snake(name));
    let component = scaffold::snake(component);
    let file = scaffold::snake(&name);
    let camel = format!("{}{}", name[..1].to_lowercase(), &name[1..]);
    let rel = place.replace("{component}", &component).replace("{screen}", &screen).replace("{file}", &file);
    let text = skeleton
        .replace("{{Name}}", &name)
        .replace("{{name}}", &camel)
        .replace("{{Component}}", &scaffold::pascal(&component))
        .replace("{{component}}", &component)
        .replace("{{table}}", &format!("{file}s"));
    let report = scaffold::write(root, &vec![(PathBuf::from(&rel), text)]).map_err(|e| e.to_string())?;
    eprintln!("{}", report.join("\n"));
    Ok(())
}

// ---------------------------------------------------------------------------------------------------
// gen (ADR-0103): finished code from a resource's data, from the platform's [[generators]]

const GEN_USAGE: &str = "usage:
  dragonfly gen inspect                                   what the project has: components and declarations (JSON)
  dragonfly gen component <component>                     a component: its facade and its config
  dragonfly gen resource <component> <Name> [options]     models, repository, a use case per action, the facade's
                                                          aliases, the controller (backend) or the screen (mobile,
                                                          web), the tests
options of gen resource:
  --from-json <file|url>     the fields from an example answer of the API (a url also gives --source rest)
  --from-spec <spec.md>      the fields from the spec's Key | Type | Description table
  --fields \"id:int name:string origin:Origin?\"
  --source rest:<base url> | rest | db     another API (an HTTP repository) or the database (default: db; rest with a url)
  --path <path>              the API's path of the resource (default: the url's last segment, or the name)
  --actions list,get,search,create,update,delete   (default: list,get for rest; every one for db)
  --screen <screen>          mobile and web: a screen that lists the resource
  --plural <Names>           when the English plural is not the right one
  --dry-run                  show the files, write nothing
  dragonfly gen usecase <component> <Name> [options]      a use case wired into its component: its alias in the facade
                                                          and, with --route, its route in the controller (backend)
options of gen usecase:
  --calls <Repository>.<method>   write the use case too: it only calls that method, which a repository of the project
                             declares (without it <Name> is a use case the project has: its rules are written by hand)
  --route \"<VERB> <prefix>[/<path>]\"   backend: the route that calls it (\"POST orders/{id}/cancel\": the controller of
                             `orders` gets it, created when missing; a {name} of the path is a parameter of the use case)
  --dry-run                  show the files, write nothing";

fn fetch_text(source: &str) -> Result<String, String> {
    if source.starts_with("http://") || source.starts_with("https://") {
        let out = Command::new("curl").args(["-fsSL", "--max-time", "20", source]).output().map_err(|e| format!("curl: {e}"))?;
        if !out.status.success() {
            return Err(format!("{source} answered {}", String::from_utf8_lossy(&out.stderr).trim()));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        std::fs::read_to_string(source).map_err(|e| format!("{source}: {e}"))
    }
}

/// The `Http` connection a REST resource calls, written in the app's connections when it is missing with the
/// platform's `[gen] http` form (or `http_named` when the app has another one). Answers the place it changed and the
/// connection's name for `@repository(connection = ...)` ("" for the default one).
fn ensure_http(root: &Path, d: &nuclear_dialect::Dialect, base: &str, name: &str, dry: bool) -> Result<(Option<String>, String), String> {
    let Some(place) = d.layout.get("appConfig") else { return Ok((None, String::new())) };
    let file = root.join(place);
    let Ok(text) = std::fs::read_to_string(&file) else { return Ok((None, String::new())) };
    // already there: its name, if it has one
    let quoted = format!("\"{}", base.trim_end_matches('/'));
    if let Some(at) = text.find(&quoted) {
        let line = text[..at].rsplit('\n').next().unwrap_or("");
        let named = ["connectionName = \"", "name = \""].iter().find_map(|k| line.split(k).nth(1).and_then(|r| r.split('"').next()));
        return Ok((None, named.unwrap_or("").to_string()));
    }
    let has_http = text.contains("Http(");
    let (form, connection) = match (has_http, d.gen_values.get("http"), d.gen_values.get("http_named")) {
        (false, Some(f), _) => (f, String::new()),
        (true, _, Some(f)) => (f, name.to_string()),
        _ => return Ok((None, String::new())),
    };
    let line = form.replace("{{baseUrl}}", base).replace("{{name}}", name);
    let Some(at) = text.find("connections: Connection[] = [") else { return Ok((None, connection)) };
    let line_end = at + text[at..].find('\n').unwrap_or(text.len() - at);
    if !dry {
        let new = format!("{}\n    {line},{}", &text[..line_end], &text[line_end..]);
        std::fs::write(&file, new).map_err(|e| e.to_string())?;
    }
    Ok((Some(place.clone()), connection))
}

fn gen_command(root: &Path, rest: Vec<String>) -> Result<(), String> {
    let mut rest = rest;
    let dry = rest.iter().any(|a| a == "--dry-run");
    rest.retain(|a| a != "--dry-run");
    let m = any_manifest(root)?;
    let (dialect, _) = loader().load(&m.dialect);
    let d = dialect.ok_or_else(|| format!("dialect '{}' could not be loaded", m.dialect))?;
    let what = rest.first().cloned().ok_or(GEN_USAGE)?;
    // a component that does not exist is created first (after the checks below)
    let mut new_component: Option<String> = None;
    let mut http_base: Option<(String, String)> = None;
    let ctx = match what.as_str() {
        "inspect" => {
            let all = nuclear_transpile::map::inventory(root, &m, &d);
            let mut components: std::collections::BTreeMap<String, Vec<serde_json::Value>> = Default::default();
            for x in all {
                components.entry(x.component.clone()).or_default().push(serde_json::json!({ "kind": x.kind, "name": x.name, "file": x.file }));
            }
            let json = serde_json::json!({
                "platform": m.platform, "dialect": m.dialect,
                "components": components.iter().filter(|(c, _)| !c.is_empty()).map(|(c, decls)| serde_json::json!({ "name": c, "declarations": decls })).collect::<Vec<_>>(),
                "outside": components.get("").cloned().unwrap_or_default(),
                "generators": d.generators.keys().collect::<Vec<_>>(),
            });
            println!("{}", serde_json::to_string_pretty(&json).unwrap_or_default());
            return Ok(());
        }
        "component" => {
            let c = rest.get(1).ok_or(GEN_USAGE)?;
            let c = scaffold::snake(c);
            nuclear_dialect::template::Value::Map(std::collections::BTreeMap::from([
                ("component".into(), nuclear_dialect::template::Value::Str(c.clone())),
                ("Component".into(), nuclear_dialect::template::Value::Str(scaffold::pascal(&c))),
            ]))
        }
        "resource" => {
            let from_json = take_option(&mut rest, "--from-json");
            let from_spec = take_option(&mut rest, "--from-spec");
            let fields = take_option(&mut rest, "--fields");
            let source = take_option(&mut rest, "--source");
            let path = take_option(&mut rest, "--path");
            let actions = take_option(&mut rest, "--actions");
            let screen = take_option(&mut rest, "--screen");
            let plural = take_option(&mut rest, "--plural");
            let [_, component, name] = rest.as_slice() else { return Err(GEN_USAGE.into()) };
            let name = scaffold::pascal(&scaffold::snake(name));
            let component = scaffold::snake(component);
            let (models, page) = match (&from_json, &fields, &from_spec) {
                (Some(src), _, _) => {
                    let text = fetch_text(src)?;
                    let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("{src} is not JSON: {e}"))?;
                    generate::from_json(&json, &name)?
                }
                (None, Some(f), _) => (vec![generate::Model { name: name.clone(), fields: generate::parse_fields(f)? }], None),
                // a spec's example answer gives the nested models and the page; else its Key | Type table
                (None, None, Some(spec)) => {
                    let text = fetch_text(spec)?;
                    match (generate::spec_json(&text), generate::from_spec(&text, &name)) {
                        (Some(json), _) => generate::from_json(&json, &name)?,
                        (None, Ok(model)) => (vec![model], None),
                        // objects the table does not detail: the API's own answer, from the spec's GET line
                        (None, Err(e)) => match generate::spec_url(&text) {
                            Some(url) => {
                                let answer = fetch_text(&url)?;
                                generate::from_json(&serde_json::from_str(&answer).map_err(|e| format!("{url} is not JSON: {e}"))?, &name)?
                            }
                            None => return Err(e),
                        },
                    }
                }
                _ => return Err(format!("the fields of {name}: --from-json <an example answer of the API, a file or its url>, --from-spec <spec.md with a Key | Type table> or --fields \"id:int name:string\" (an agent finds them in the spec, the API's answer or its documentation, or asks)")),
            };
            // every type a field names must be known: basic, a model of this answer, or declared in the project
            let declared: Vec<String> = nuclear_transpile::map::inventory(root, &m, &d).into_iter().map(|x| x.name).collect();
            for f in models.iter().flat_map(|x| &x.fields) {
                let ty = f.ty.trim_end_matches("[]");
                if !generate::BASIC.contains(&ty) && !models.iter().any(|x| x.name == ty) && !declared.iter().any(|x| x == ty) {
                    return Err(format!("{}: the type {ty} is not declared (declare the model first, e.g. `dragonfly gen resource ... --fields` for it, or take the fields from --from-json, which makes nested models)", f.name));
                }
            }
            // a url example answer is also where the resource lives: its base and its path
            let spec_url = from_spec.as_ref().and_then(|f| fetch_text(f).ok()).and_then(|t| generate::spec_url(&t));
            let url = from_json.as_ref().filter(|u| u.starts_with("http")).or(spec_url.as_ref()).map(|u| u.split('?').next().unwrap_or(u).trim_end_matches('/').to_string());
            let source = match source.as_deref() {
                Some("db") => generate::Source::Db,
                Some(s) if s.starts_with("rest") => {
                    let base = s.strip_prefix("rest:").map(str::to_string).or_else(|| url.as_ref().and_then(|u| u.rsplit_once('/').map(|(b, _)| b.to_string())));
                    let p = path.clone().or_else(|| url.as_ref().and_then(|u| u.rsplit_once('/').map(|(_, p)| p.to_string()))).unwrap_or_else(|| scaffold::snake(&name));
                    generate::Source::Rest { base, path: p }
                }
                Some(other) => return Err(format!("--source {other}: rest:<base url>, rest or db")),
                None => match &url {
                    Some(u) => {
                        let (b, p) = u.rsplit_once('/').unwrap_or((u, ""));
                        generate::Source::Rest { base: Some(b.to_string()), path: path.clone().unwrap_or_else(|| p.to_string()) }
                    }
                    None => generate::Source::Db,
                },
            };
            if matches!(source, generate::Source::Db) && let Some(f) = models[0].fields.iter().find(|f| !generate::BASIC.contains(&f.ty.trim_end_matches("[]"))) {
                return Err(format!("{}: a database resource takes columns of basic types; a relation to {} is written by hand (`@manyToOne`, spec/backend.md)", f.name, f.ty));
            }
            let actions: Vec<String> = match actions {
                Some(a) => a.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect(),
                None if matches!(source, generate::Source::Rest { .. }) => vec!["list".into(), "get".into()],
                None => generate::ACTIONS.iter().filter(|a| **a != "search").map(|a| a.to_string()).collect(),
            };
            if let Some(bad) = actions.iter().find(|a| !generate::ACTIONS.contains(&a.as_str())) {
                return Err(format!("unknown action '{bad}' (actions: {})", generate::ACTIONS.join(", ")));
            }
            // which column a stored resource is searched by is a decision: a query of its repository, then wired
            if matches!(source, generate::Source::Db) && actions.iter().any(|a| a == "search") {
                return Err(format!(
                    "a database resource has no generated search: which column it searches is yours to say. Write the query in {name}Repository, then `dragonfly gen usecase {component} Search{} --calls {name}Repository.<method>`",
                    plural.clone().unwrap_or_else(|| generate::plural(&name))
                ));
            }
            let r = generate::Resource {
                plural: plural.unwrap_or_else(|| generate::plural(&name)),
                name,
                component: component.clone(),
                models,
                page,
                source: source.clone(),
                actions,
                screen: screen.map(|s| scaffold::snake(&s)),
            };
            // a source the platform has no pieces for (mobile and web keep no database of resources) is refused
            let wanted = if matches!(source, generate::Source::Db) { "db" } else { "rest" };
            if !d.generators.get("resource").is_some_and(|ps| ps.iter().any(|p| p.when.as_deref() == Some(wanted))) {
                return Err(format!("the {} platform generates no `{wanted}` resources: {}", m.platform.as_deref().unwrap_or("?"), if wanted == "db" { "give the API: --source rest:<base url> or --from-json <url>" } else { "use --source db" }));
            }
            let mut connection = String::new();
            if let generate::Source::Rest { base: Some(b), .. } = &source {
                // only its name now: the connection is written after the checks
                connection = ensure_http(root, &d, b, &component, true)?.1;
                http_base = Some((b.clone(), component.clone()));
            }
            new_component = (!root.join("components").join(&component).is_dir() && d.generators.contains_key("component")).then(|| component.clone());
            let mut ctx = generate::context(&r);
            if let nuclear_dialect::template::Value::Map(m) = &mut ctx && !connection.is_empty() {
                m.insert("connection".into(), nuclear_dialect::template::Value::Str(connection));
            }
            ctx
        }
        "usecase" => {
            let calls = take_option(&mut rest, "--calls");
            let route = take_option(&mut rest, "--route");
            let [_, component, name] = rest.as_slice() else { return Err(GEN_USAGE.into()) };
            let name = scaffold::pascal(&scaffold::snake(name));
            let component = scaffold::snake(component);
            let shape = |n: &str| nuclear_transpile::map::shape(root, &m, &d, n);
            let platform = m.platform.as_deref().unwrap_or("?");
            let has = |when: &str| d.generators.get("usecase").is_some_and(|ps| ps.iter().any(|p| p.when.as_deref() == Some(when)));
            // a use case the project declares is wired as it is; else it is written over the repository method it calls
            let (sig, calls) = match (shape(&name), &calls) {
                (Some(found), _) => {
                    if !found.file.starts_with(&format!("components/{component}/")) {
                        return Err(format!("{name} is declared in {}, not in the component {component}", found.file));
                    }
                    let exec = found.methods.into_iter().find(|x| x.name == "exec").ok_or_else(|| format!("{name} ({}) has no exec", found.file))?;
                    (exec, None)
                }
                (None, Some(c)) => {
                    let (repository, method) = c.split_once('.').ok_or("--calls <Repository>.<method> (--calls OrderRepository.pending)")?;
                    let found = shape(repository).ok_or_else(|| format!("{repository} is not declared in this project (`dragonfly gen inspect` lists what is)"))?;
                    let declared: Vec<&str> = found.methods.iter().map(|x| x.name.as_str()).collect();
                    let sig = found.methods.iter().find(|x| x.name == method).cloned().ok_or_else(|| {
                        format!("{repository} ({}) declares no method {method} (it declares: {}); a use case over a method that comes with the repository is written by hand", found.file, declared.join(", "))
                    })?;
                    (sig, Some((repository.to_string(), method.to_string())))
                }
                (None, None) => {
                    let place = d.layout.get("usecase").map(|p| p.replace("{component}", &component).replace("{file}", &scaffold::snake(&name))).unwrap_or_default();
                    return Err(format!(
                        "{name} is not declared. A use case with rules of its own is written first ({place}: `usecase {name}(...)` with `exec(...) Result<Error, T>`), then this command wires it; one that only calls a repository is written here: --calls <Repository>.<method>"
                    ));
                }
            };
            let answers = sig.ret.clone().unwrap_or_else(|| "nothing".into());
            let (error, value, calls) = match (generate::result_parts(&answers), calls) {
                (Some((e, v)), None) => (e, v, None),
                (None, None) => return Err(format!("{name}.exec answers {answers}: a use case answers Result<E, T>")),
                (parts, Some((repository, method))) => {
                    if sig.ret.is_none() {
                        return Err(format!("{repository}.{method} answers nothing: a use case over it is written by hand"));
                    }
                    let result = parts.is_some();
                    if !has(if result { "result" } else { "plain" }) {
                        return Err(format!("the {platform} platform writes no use case over a method that answers {answers}: write it by hand, then run this again to wire it"));
                    }
                    let (e, v) = parts.unwrap_or_else(|| ("Error".into(), answers.clone()));
                    (e, v, Some((repository, method, result)))
                }
            };
            let route = match route {
                Some(_) if !has("route") => return Err(format!("the {platform} platform has no routes: there a screen calls the facade's alias")),
                Some(text) => {
                    let mut r = generate::parse_route(&text, &sig.args)?;
                    // a controller the project has names the facade its own way
                    if let Some((app, _)) = shape(&r.controller()).and_then(|c| c.params.into_iter().next()) {
                        r.app = app;
                    }
                    Some(r)
                }
                None => None,
            };
            // the use case `gen` writes awaits its repository; one of the project says so itself
            let awaits = calls.is_some() || sig.awaits;
            generate::usecase_context(&generate::UseCase { name, component, params: sig.params, args: sig.args, error, value, awaits, calls, route })
        }
        _ => return Err(GEN_USAGE.into()),
    };
    let files = generate::plan(&d, &what, &ctx)?;
    // a name a new file declares that another file of the project already declares (N0303): stop before writing
    let declared = nuclear_transpile::map::inventory(root, &m, &d);
    for (path, _, _) in files.iter().filter(|(p, _, _)| !root.join(p).exists()) {
        let stem = path.file_stem().and_then(|x| x.to_str()).unwrap_or("");
        if let Some(x) = declared.iter().find(|x| scaffold::snake(&x.name) == stem) {
            return Err(format!("{} is already declared in {}: remove or rename it, or generate another name", x.name, x.file));
        }
    }
    if dry {
        for (p, text, _) in &files {
            println!("── {}\n{text}", p.display());
        }
        return Ok(());
    }
    if let Some((base, name)) = http_base
        && let (Some(place), _) = ensure_http(root, &d, &base, &name, false)?
    {
        eprintln!("  {}  {}", landing::dim("added to"), place);
    }
    if let Some(c) = new_component {
        gen_command(root, vec!["component".into(), c])?;
    }
    for (file, how) in generate::write(root, &files).map_err(|e| e.to_string())? {
        eprintln!("  {:>9}  {file}", if how == "created" { landing::ok(how) } else { landing::dim(how) });
    }
    Ok(())
}

#[cfg(test)]
mod tests {

    /// ADR-0103: what `dragonfly gen` writes compiles with every dialect of every platform (a REST resource with a
    /// nested model and a screen; a database resource in the backend). No network: the fields come from --fields.
    #[test]
    fn a_generated_resource_compiles_with_every_dialect() {
        let l = loader();
        for (dialect, files, db) in [
            ("flutter", scaffold::project("demo_app"), false),
            ("fastapi", scaffold::backend_project("demo_api", "fastapi"), true),
            ("nestjs", scaffold::backend_project("demo_api", "nestjs"), true),
            ("react", scaffold::web_project("demo_web", "react", "http://localhost:8000"), false),
        ] {
            let Some(d) = l.load(dialect).0 else {
                eprintln!("skipped {dialect}: its platform repository is not in this working tree");
                continue;
            };
            let root = std::env::temp_dir().join(format!("dragonfly-gen-{dialect}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            for (path, text) in &files {
                let f = root.join(path);
                std::fs::create_dir_all(f.parent().unwrap()).unwrap();
                std::fs::write(f, text).unwrap();
            }
            let args = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
            // a model the fields name must exist first: a type nobody declared is refused
            let origin = root.join(d.layout["model"].replace("{component}", "characters").replace("{file}", "character_origin"));
            let refused = gen_command(&root, args(&["resource", "characters", "Character", "--fields", "id:int origin:CharacterOrigin", "--source", "rest:https://example.com/api"]));
            assert!(refused.is_err_and(|e| e.contains("CharacterOrigin")), "{dialect}");
            std::fs::create_dir_all(origin.parent().unwrap()).unwrap();
            std::fs::write(&origin, "CharacterOrigin\n  name: string\n").unwrap();
            gen_command(&root, args(&["resource", "characters", "Character", "--fields", "id:int name:string origin:CharacterOrigin tags:string[] note?:string", "--source", "rest:https://example.com/api", "--actions", "list,get,search,create,update,delete", "--screen", "characters"])).unwrap();
            // a use case that only calls a repository method is written and gets its alias in the facade
            gen_command(&root, args(&["usecase", "characters", "FindCharacter", "--calls", "CharacterRepository.search"])).unwrap();
            let facade = std::fs::read_to_string(root.join(d.layout["appcomponent"].replace("{component}", "characters"))).unwrap();
            assert!(facade.contains("  findCharacter() = FindCharacter.exec\n"), "{dialect}: {facade}");
            assert!(gen_command(&root, args(&["usecase", "characters", "Missing"])).is_err_and(|e| e.contains("not declared")), "{dialect}");
            assert!(gen_command(&root, args(&["usecase", "characters", "Missing", "--calls", "CharacterRepository.nothing"])).is_err_and(|e| e.contains("declares no method")), "{dialect}");
            // an action added to a resource that exists: its repository gets the method it lacks (and the controller
            // its route), so what is written still compiles
            let planet = ["resource", "planets", "Planet", "--fields", "id:int name:string", "--source", "rest:https://example.com/api", "--actions"];
            gen_command(&root, args(&[&planet[..], &["list"]].concat())).unwrap();
            gen_command(&root, args(&[&planet[..], &["list,search"]].concat())).unwrap();
            let planets = std::fs::read_to_string(root.join(d.layout["repository"].replace("{component}", "planets").replace("{file}", "planet_repository"))).unwrap();
            assert!(planets.contains("  search(name: string)"), "{dialect}: {planets}");
            if db {
                gen_command(&root, args(&["resource", "catalog", "Book", "--fields", "title:string year:int", "--source", "db"])).unwrap();
                // which column a stored resource is searched by is not the generator's to choose
                let search = gen_command(&root, args(&["resource", "catalog", "Book", "--fields", "title:string year:int", "--source", "db", "--actions", "search"]));
                assert!(search.is_err_and(|e| e.contains("no generated search") && e.contains("gen usecase catalog SearchBooks --calls BookRepository.<method>")), "{dialect}");
                // a query that answers no Result goes inside an Ok; its route makes the controller of its prefix
                gen_command(&root, args(&["usecase", "catalog", "PageBooks", "--calls", "BookRepository.page", "--route", "GET shelves/{page}"])).unwrap();
                let shelves = std::fs::read_to_string(root.join("components/catalog/api/shelves_controller.nuc")).unwrap();
                assert!(shelves.contains("  @get(\"{page}\")\n  pageBooks(page: int, size: int) Result<Error, Page<Book>>\n    ~app.pageBooks(page, size)\n"), "{dialect}: {shelves}");
                // a use case written by hand is wired as it is: its route joins the controller the resource has, and
                // calls it without `~` because it does not await
                std::fs::write(root.join("components/catalog/domain/usecases/tidy_title.nuc"), "// The title, as it is kept.\nusecase TidyTitle()\n  exec(title: string) Result<Error, string>\n    Ok(title)\n").unwrap();
                gen_command(&root, args(&["usecase", "catalog", "TidyTitle", "--route", "GET books/tidy"])).unwrap();
                let books = std::fs::read_to_string(root.join("components/catalog/api/books_controller.nuc")).unwrap();
                assert!(books.ends_with("\n  @get(\"tidy\")\n  tidyTitle(title: string) Result<Error, string>\n    app.tidyTitle(title)\n"), "{dialect}: {books}");
                assert!(gen_command(&root, args(&["usecase", "catalog", "TidyTitle", "--route", "GET books/{id}"])).is_err_and(|e| e.contains("{id}")), "{dialect}");
            } else {
                assert!(gen_command(&root, args(&["usecase", "characters", "FindCharacter", "--route", "GET characters/find"])).is_err_and(|e| e.contains("no routes")), "{dialect}");
            }
            let m = nuclear_transpile::project::Manifest::read(&root).unwrap();
            let out = nuclear_transpile::project::build(&root, &m, &d);
            let errors: Vec<String> = out
                .files
                .iter()
                .flat_map(|f| f.diagnostics.iter().filter(|x| x.is_error()).map(move |x| format!("{}: {}", f.source_path.display(), x.message)))
                .collect();
            assert!(errors.is_empty(), "{dialect}: {errors:?}");
            assert!(out.files.iter().any(|f| f.source_path.ends_with("character_repository.nuc")), "{dialect}");
            let _ = std::fs::remove_dir_all(&root);
        }
    }

    #[test]
    fn new_reads_the_platform_from_the_dialect_and_refuses_typos() {
        let s = |x: &str| Some(x.to_string());
        assert_eq!(platform_and_dialect(None, s("nestjs")).unwrap(), ("backend".into(), "nestjs".into()));
        assert_eq!(platform_and_dialect(s("web"), None).unwrap().0, "web");
        assert!(platform_and_dialect(None, None).unwrap().0 == MOBILE);
        assert!(new_name("nuclear_rick_and_morty-p").unwrap_err().contains("-p glued"));
        assert!(new_name("2shop").is_err());
        assert_eq!(new_name("ShopApp").unwrap(), "shop_app");
        assert!(hint_for("backend").contains("-p backend"));
        assert!(hint_for("nestjs").contains("-d nestjs"));
    }

    #[test]
    fn a_placeholder_is_one_shell_word() {
        if cfg!(windows) {
            assert_eq!(super::shell_quote(r#"say "hi""#), r#""say \"hi\"""#);
        } else {
            assert_eq!(super::shell_quote("it's done"), r"'it'\''s done'");
            let out = std::process::Command::new("sh").args(["-c", &format!("printf %s {}", super::shell_quote("a 'b' $c"))]).output().unwrap();
            assert_eq!(String::from_utf8_lossy(&out.stdout), "a 'b' $c");
        }
    }

    use super::*;

    /// `dragonfly make` writes every kind of the backend and web platforms at its layout place, and the project
    /// compiles (a kind of a plugin the project does not enable is N0304, which says the line to add).
    #[test]
    fn make_writes_every_kind_where_the_platform_says() {
        for (platform, dialect) in [("backend", "fastapi"), ("backend", "nestjs"), ("web", "react")] {
            let Some(d) = loader().load(dialect).0 else { continue };
            let root = std::env::temp_dir().join(format!("dragonfly-make-{dialect}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            let files = if platform == "web" { scaffold::web_project("demo", dialect, "http://localhost:8000") } else { scaffold::backend_project("demo", dialect) };
            scaffold::write(&root, &files).unwrap();
            let m = Manifest::read(&root).unwrap();
            for kind in d.skeletons.keys() {
                let name = format!("Demo{}", scaffold::pascal(kind));
                make_from_layout(&root, &m, vec![kind.clone(), "shelf".into(), name, "--screen".into(), "list".into()]).unwrap();
            }
            let out = project::build(&root, &m, &d);
            let errors: Vec<String> = out
                .files
                .iter()
                .flat_map(|f| f.diagnostics.iter().filter(|x| x.is_error() && x.code.id != "N0304").map(move |x| format!("{}: {}", f.source_path.display(), x.message)))
                .collect();
            assert!(errors.is_empty(), "{dialect}: {errors:?}");
            assert!(root.join("components/shelf/domain/usecases/demo_usecase.nuc").is_file(), "{dialect}");
            let _ = std::fs::remove_dir_all(&root);
        }
    }

    #[test]
    fn pub_get_after_clean() {
        let app = std::env::temp_dir().join(format!("dragonfly_pub_{}", std::process::id()));
        std::fs::create_dir_all(app.join(".dart_tool")).unwrap();
        std::fs::write(app.join("pubspec.yaml"), "").unwrap();
        assert!(needs_pub_get(&app));
        std::fs::write(app.join("pubspec.lock"), "").unwrap();
        std::fs::write(app.join(".dart_tool/package_config.json"), "").unwrap();
        assert!(!needs_pub_get(&app));
        // pub get keeps an unchanged lock: the mark says it already ran after the pubspec changed
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(app.join("pubspec.yaml"), "").unwrap();
        assert!(needs_pub_get(&app));
        std::fs::write(app.join(PUB_GET_MARK), "").unwrap();
        assert!(!needs_pub_get(&app));
        std::fs::remove_dir_all(app.join(".dart_tool")).unwrap(); // what flutter clean does
        assert!(needs_pub_get(&app));
        std::fs::remove_dir_all(&app).unwrap();
    }
}
