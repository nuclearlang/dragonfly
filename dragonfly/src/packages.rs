//! The app's packages: `config.nuc` (the project's pubspec, written in Nuclear) → `build/app/pubspec.yaml`,
//! and the type definitions of the packages it adds, so the transpiler knows their names (imports,
//! overrides, N0106). Definitions are generated once into the shared cache (`defs_cache_dir`) and the ones
//! a project uses are listed in `build/defs.lock`; removing a package keeps them in the cache.

use crate::{PACKAGES, dart_bin, flutter, landing, run};
use nuclear_dialect::{Dialect, Loader};
use nuclear_transpile::project::{self, Config};
use std::path::{Path, PathBuf};

pub const CONFIG: &str = "config.nuc";
/// In the app: the packages that came from config.nuc, so a package removed from it leaves the pubspec too.
const MARK: &str = ".dragonfly_deps";
/// Packages Dragonfly or `flutter create` put in the app: config.nuc never removes them.
const OWN: &[&str] = &[
    "flutter",
    "flutter_test",
    "flutter_lints",
    "cupertino_icons",
    "dragonfly",
    "flutter_native_splash",
    "integration_test",
];

/// `config.nuc`, read with the sections of the dialect's `[project.manifest]` (errors are shown by
/// `check`/`build`, which check the file with the rest of the project).
pub fn read(root: &Path, d: &Dialect) -> Option<Config> {
    let m = d.project.manifest.as_ref()?;
    let text = std::fs::read_to_string(root.join(&m.file)).ok()?;
    Some(project::read_config(&text, &m.sections).0)
}

/// The value of `section(key = ...)`.
pub fn get<'a>(c: &'a Config, section: &str, key: &str) -> Option<&'a str> {
    c.get(section)?.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

fn hidden(name: &str) -> bool {
    OWN.contains(&name) || PACKAGES.iter().any(|p| p.trim_start_matches("dev:") == name)
}

/// Writes config.nuc into the app's pubspec: name, description, version, environment, uses-material-design,
/// and the packages of `dependencies`/`devDependencies` (added, changed or removed with `flutter pub`).
pub fn sync(app: &Path, c: &Config, d: &Dialect) -> Result<(), String> {
    let path = app.join("pubspec.yaml");
    let before = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut text = before.clone();
    for key in ["name", "description", "version"] {
        if let Some(v) = get(c, "app", key) {
            text = set_yaml(&text, None, key, &yaml(v));
        }
    }
    if let Some(v) = get(c, "environment", "sdk") {
        text = set_yaml(&text, Some("environment"), "sdk", &yaml(v));
    }
    if let Some(v) = get(c, "environment", "flutter") {
        text = set_yaml(&text, Some("environment"), "flutter", &yaml(v));
    }
    if let Some(v) = get(c, "app", "usesMaterialDesign") {
        text = set_yaml(&text, Some("flutter"), "uses-material-design", v);
    }
    if text != before {
        std::fs::write(&path, &text).map_err(|e| e.to_string())?;
    }

    let mark = app.join(MARK);
    let managed: Vec<String> =
        std::fs::read_to_string(&mark).map(|t| t.lines().map(str::to_string).collect()).unwrap_or_default();
    let mut wanted: Vec<String> = Vec::new();
    let mut add: Vec<String> = Vec::new();
    for (section, block, prefix) in
        [("dependencies", "dependencies", ""), ("devDependencies", "dev_dependencies", "dev:")]
    {
        // a package of the framework is an error of config.nuc (N0303, shown by the transpiler): never added
        for (name, value) in c.get(section).into_iter().flatten().filter(|(n, _)| !project::reserved_package(d, n)) {
            wanted.push(name.clone());
            if yaml_get(&text, block, name).as_deref() != Some(value.as_str()) {
                add.push(format!("{prefix}{name}:{value}"));
            }
        }
    }
    let remove: Vec<&str> =
        managed.iter().map(String::as_str).filter(|n| !wanted.iter().any(|w| w == n) && !hidden(n)).collect();
    if !remove.is_empty() {
        flutter(app, &[&["pub", "remove"][..], &remove].concat())?;
    }
    if !add.is_empty() {
        let add: Vec<&str> = add.iter().map(String::as_str).collect();
        flutter(app, &[&["pub", "add"][..], &add].concat())?;
    }
    if managed != wanted {
        std::fs::write(&mark, wanted.join("\n")).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// The definitions of the `dependencies` of config.nuc, with the version the app resolved
/// (pubspec.lock): generated into the cache when they are missing (`generate`), added to the dialect and
/// listed in build/defs.lock. Packages the dialect already knows (dio, go_router...) are skipped.
pub fn load_defs(
    root: &Path,
    app: &Path,
    c: &Config,
    loader: &Loader,
    d: &mut Dialect,
    generate: bool,
) -> Result<(), String> {
    let lock = std::fs::read_to_string(app.join("pubspec.lock")).unwrap_or_default();
    let language = d.chain.last().cloned().unwrap_or_default();
    let cache = nuclear_dialect::defs_cache_dir().ok_or("no cache folder: set HOME or XDG_CACHE_HOME")?;
    let mut refs = Vec::new();
    for (name, _) in c.get("dependencies").into_iter().flatten() {
        if d.typedefs.package(name).is_some() {
            continue;
        }
        let Some(version) = locked_version(&lock, name) else { continue };
        let dir = cache.join(&language).join(name).join(&version);
        if !dir.join("package.toml").is_file() {
            if !generate {
                continue;
            }
            generate_defs(app, name, &version, d, &dir)?;
        }
        refs.push(format!("{name}@{version}"));
    }
    let report = loader.add_defs(d, &refs);
    if report.has_errors() {
        let msgs: Vec<String> = report.diags.iter().map(|(_, x)| x.message.clone()).collect();
        return Err(format!("type definitions of the project: {}", msgs.join("; ")));
    }
    let path = root.join(project::DEFS_LOCK);
    let text = refs.join("\n");
    if std::fs::read_to_string(&path).ok().as_deref() != Some(text.as_str()) {
        std::fs::create_dir_all(path.parent().unwrap_or(root)).map_err(|e| e.to_string())?;
        std::fs::write(&path, text).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Runs the definitions generator (tools/defs, Dart's analyzer) on one package the app resolved.
fn generate_defs(app: &Path, name: &str, version: &str, d: &Dialect, out: &Path) -> Result<(), String> {
    let tool = tool_dir().ok_or("the definitions generator (tools/defs) is not installed: run ./install.sh")?;
    let dart = dart_bin()?;
    if !tool.join(".dart_tool/package_config.json").is_file() {
        run(&dart, &tool, &["pub", "get"])?;
    }
    let root = package_root(app, name)?;
    eprintln!("{} type definitions of {}", landing::dim("·"), landing::name(&format!("{name} {version}")));
    let (app_s, out_s) = (app.display().to_string(), out.display().to_string());
    let description = format!("{name} {version} (pub.dev), generated by dragonfly flutter add");
    let mut args: Vec<String> =
        ["run", "bin/defs.dart", "--project", &app_s, "--out", &out_s, "--name", name].map(String::from).to_vec();
    args.extend(["--version".into(), version.into(), "--description".into(), description]);
    // names of the packages the dialect already has are not repeated (Widget, Future...)
    for p in &d.typedefs.packages {
        args.extend(["--exclude".into(), p.dir.display().to_string()]);
    }
    args.extend(libraries(&root, name));
    let a: Vec<&str> = args.iter().map(String::as_str).collect();
    run(&dart, &tool, &a).inspect_err(|_| {
        let _ = std::fs::remove_dir_all(out);
    })
}

/// tools/defs of the install, or of the repository dragonfly was built from.
fn tool_dir() -> Option<PathBuf> {
    let installed = nuclear_dialect::install_dir().map(|d| d.join("tools/defs"));
    let repo = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tools/defs"));
    installed.into_iter().chain([repo]).find(|d| d.join("bin/defs.dart").is_file())
}

/// The package's folder, from the app's `.dart_tool/package_config.json`.
fn package_root(app: &Path, name: &str) -> Result<PathBuf, String> {
    let file = app.join(".dart_tool/package_config.json");
    let text = std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let uri = json["packages"]
        .as_array()
        .and_then(|ps| ps.iter().find(|p| p["name"] == name))
        .and_then(|p| p["rootUri"].as_str())
        .ok_or(format!("'{name}' is not a package of the app (flutter pub get)"))?;
    Ok(match uri.strip_prefix("file://") {
        Some(abs) => PathBuf::from(abs),
        None => file.parent().unwrap_or(app).join(uri),
    })
}

/// The public libraries to describe: `package:<name>/<name>.dart`, or every file directly in lib/.
fn libraries(root: &Path, name: &str) -> Vec<String> {
    if root.join("lib").join(format!("{name}.dart")).is_file() {
        return vec![format!("package:{name}/{name}.dart")];
    }
    let mut libs: Vec<String> = std::fs::read_dir(root.join("lib"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().to_str().filter(|f| f.ends_with(".dart")).map(|f| format!("package:{name}/{f}")))
        .collect();
    libs.sort();
    libs
}

/// `version: "2.2.3"` of a package in a pubspec.lock.
fn locked_version(lock: &str, name: &str) -> Option<String> {
    let start = lock.lines().position(|l| l == format!("  {name}:"))?;
    lock.lines()
        .skip(start + 1)
        .take_while(|l| l.starts_with("    "))
        .find_map(|l| l.trim().strip_prefix("version: "))
        .map(|v| v.trim_matches('"').to_string())
}

// ---------------------------------------------------------------------------------------------------
// dragonfly flutter add | remove

/// `dragonfly flutter add <package>[:version]... [--dev]`: flutter pub add (it picks the version), then the
/// package goes to config.nuc. The build that follows generates its definitions.
/// Only third-party packages the framework does not use: the dialect's (its `defs`) and the framework's
/// (`[project.manifest] reserved`) are refused before anything changes.
pub fn add(root: &Path, app: &Path, args: &[String], default_name: &str, d: &Dialect) -> Result<(), String> {
    let dev = args.iter().any(|a| a == "--dev");
    let pkgs: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    if pkgs.is_empty() {
        return Err("usage: dragonfly flutter add <package>[:version]... [--dev]".into());
    }
    let reserved: Vec<&str> =
        pkgs.iter().map(|p| p.split(':').next().unwrap_or(p)).filter(|n| project::reserved_package(d, n)).collect();
    if !reserved.is_empty() {
        return Err(format!(
            "{} already part of Dragonfly (the {} dialect uses it): use it directly, without adding it.\n\
             Only third-party packages the framework does not use can be added.",
            reserved.join(", "),
            d.name
        ));
    }
    let prefix = if dev { "dev:" } else { "" };
    let specs: Vec<String> = pkgs.iter().map(|p| format!("{prefix}{p}")).collect();
    flutter(app, &[&["pub", "add"][..], &specs.iter().map(String::as_str).collect::<Vec<_>>()].concat())?;
    let pubspec = std::fs::read_to_string(app.join("pubspec.yaml")).map_err(|e| e.to_string())?;
    let (section, other, block) = if dev {
        ("devDependencies", "dependencies", "dev_dependencies")
    } else {
        ("dependencies", "devDependencies", "dependencies")
    };
    let mut text = config_text(root, default_name);
    for p in pkgs {
        let name = p.split(':').next().unwrap_or(p);
        let version = yaml_get(&pubspec, block, name).ok_or(format!("flutter pub add did not add '{name}'"))?;
        text = remove_entry(&text, other, name)?;
        text = set_entry(&text, section, name, &version)?;
    }
    std::fs::write(root.join(CONFIG), text).map_err(|e| e.to_string())
}

/// `dragonfly flutter remove <package>...`: out of config.nuc; the build that follows removes it from the
/// app. Its definitions stay in the cache.
pub fn remove(root: &Path, args: &[String], default_name: &str) -> Result<(), String> {
    if args.is_empty() {
        return Err("usage: dragonfly flutter remove <package>...".into());
    }
    let mut text = config_text(root, default_name);
    for name in args {
        text = remove_entry(&text, "dependencies", name)?;
        text = remove_entry(&text, "devDependencies", name)?;
    }
    std::fs::write(root.join(CONFIG), text).map_err(|e| e.to_string())?;
    if let Some(cache) = nuclear_dialect::defs_cache_dir() {
        eprintln!("{} type definitions kept in {}", landing::dim("·"), landing::dim(&cache.display().to_string()));
    }
    Ok(())
}

/// A project without config.nuc (made before it existed) gets one: its name, the description and version of
/// the app's pubspec, and the third-party packages the app already has (they stay in the app).
pub fn ensure_config(root: &Path, app: &Path, name: &str, d: &Dialect) -> Result<bool, String> {
    let Some(m) = &d.project.manifest else { return Ok(false) };
    if root.join(&m.file).exists() {
        return Ok(false);
    }
    let mut text = template(name);
    let pubspec = std::fs::read_to_string(app.join("pubspec.yaml")).unwrap_or_default();
    let mut managed = Vec::new();
    for key in ["description", "version"] {
        if let Some(v) = top_value(&pubspec, key) {
            text = set_entry(&text, "app", key, &v)?;
        }
    }
    for (block, section) in [("dependencies", "dependencies"), ("dev_dependencies", "devDependencies")] {
        for (pkg, version) in yaml_entries(&pubspec, block) {
            // shortcut: packages with a map (path:, git:, sdk:) are left out; config.nuc has only versions
            if version.is_empty() || hidden(&pkg) || project::reserved_package(d, &pkg) {
                continue;
            }
            text = set_entry(&text, section, &pkg, &version)?;
            managed.push(pkg);
        }
    }
    std::fs::write(root.join(&m.file), text).map_err(|e| e.to_string())?;
    if !managed.is_empty() && app.is_dir() {
        std::fs::write(app.join(MARK), managed.join("\n")).map_err(|e| e.to_string())?;
    }
    Ok(true)
}

fn config_text(root: &Path, default_name: &str) -> String {
    std::fs::read_to_string(root.join(CONFIG)).unwrap_or_else(|_| template(default_name))
}

/// The config.nuc of a new project.
pub fn template(name: &str) -> String {
    format!(
        r#"// The project's pubspec: dragonfly writes build/app/pubspec.yaml from it.
// Dragonfly adds by itself (not written here): dragonfly, flutter_bloc, fpdart, go_router,
// json_annotation; for code generation and tests: build_runner, json_serializable, mocktail, bloc_test,
// integration_test (and flutter_native_splash with @splash). Full list: build/app/pubspec.yaml.
// Only third-party packages the framework does not use (the ones above cannot be added).
// `dragonfly flutter add <package>` adds a package here and generates its type definitions.
// The platform this project is written for and the dialect it is built with (ADR-0093).
dragonfly(platform = "mobile", dialect = "flutter")

app(
  name = "{name}",
  description = "A Dragonfly app.",
  version = "1.0.0+1",
  usesMaterialDesign = true,
)
"#
    )
}

// ---------------------------------------------------------------------------------------------------
// text editing (config.nuc and pubspec.yaml keep their comments and order)

/// Sets `name = "value",` inside `section(` ... `)` (one entry per line, as the template writes them).
pub fn set_entry(text: &str, section: &str, name: &str, value: &str) -> Result<String, String> {
    let entry = format!("  {name} = \"{value}\",");
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let Some((open, close)) = block(&lines, section)? else {
        let sep = if text.ends_with('\n') || text.is_empty() { "" } else { "\n" };
        return Ok(format!("{text}{sep}\n{section}(\n{entry}\n)\n"));
    };
    match (open + 1..close).find(|&i| is_entry(&lines[i], name)) {
        Some(i) => lines[i] = entry,
        None => lines.insert(close, entry),
    }
    Ok(lines.join("\n") + "\n")
}

pub fn remove_entry(text: &str, section: &str, name: &str) -> Result<String, String> {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    if let Some((open, close)) = block(&lines, section)?
        && let Some(i) = (open + 1..close).find(|&i| is_entry(&lines[i], name))
    {
        lines.remove(i);
        // a section left without entries is not shown: `flutter add` writes it again
        if close - 1 == open + 1 {
            let start = if open > 0 && lines[open - 1].trim().is_empty() { open - 1 } else { open };
            lines.drain(start..=open + 1);
        }
        return Ok(lines.join("\n") + "\n");
    }
    Ok(text.to_string())
}

fn is_entry(line: &str, name: &str) -> bool {
    line.trim_start().strip_prefix(name).is_some_and(|r| r.trim_start().starts_with('='))
}

/// The lines of `section(` and of its closing `)`; `section()` is opened into two lines.
fn block(lines: &[String], section: &str) -> Result<Option<(usize, usize)>, String> {
    let Some(open) = lines.iter().position(|l| l.starts_with(&format!("{section}("))) else { return Ok(None) };
    if lines[open].trim_end().ends_with(')') {
        if lines[open].trim_end() == format!("{section}()") {
            return Err(format!("{CONFIG}: write `{section}(` and `)` on their own lines"));
        }
        // shortcut: a section written in one line is not edited; the template writes one entry per line
        return Err(format!("{CONFIG}: write one entry per line in {section}(...) to edit it"));
    }
    let close = (open + 1..lines.len())
        .find(|&i| lines[i].trim() == ")")
        .ok_or(format!("{CONFIG}: {section}( has no closing `)` on its own line"))?;
    Ok(Some((open, close)))
}

/// A YAML scalar: quoted when it could be read as something else.
fn yaml(v: &str) -> String {
    if v.is_empty() || v.contains([':', '#', ' ', '>', '<', '"', '\'', '{', '[', '*', '&', '!', '|']) {
        format!("\"{}\"", v.replace('"', "\\\""))
    } else {
        v.to_string()
    }
}

/// `key: value` at the top of the pubspec (`block` None) or inside a top-level `block:`.
fn set_yaml(text: &str, block: Option<&str>, key: &str, value: &str) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let (indent, range) = match block {
        None => ("", 0..lines.len()),
        Some(b) => match lines.iter().position(|l| l.trim_end() == format!("{b}:")) {
            Some(start) => {
                let end = (start + 1..lines.len())
                    .find(|&i| !lines[i].is_empty() && !lines[i].starts_with(' ') && !lines[i].starts_with('#'))
                    .unwrap_or(lines.len());
                ("  ", start + 1..end)
            }
            None => {
                lines.push(format!("\n{b}:\n  {key}: {value}"));
                return lines.join("\n") + "\n";
            }
        },
    };
    let prefix = format!("{indent}{key}:");
    let line = format!("{prefix} {value}");
    match range.clone().find(|&i| lines[i].starts_with(&prefix)) {
        Some(i) => lines[i] = line,
        None => lines.insert(if block.is_some() { range.start } else { 0 }, line),
    }
    lines.join("\n") + "\n"
}

/// `key: value` at the top of a pubspec.
fn top_value(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|l| l.strip_prefix(&format!("{key}:"))).map(|v| v.trim().trim_matches(['"', '\'']).to_string())
}

/// Every `  name: value` of a top-level block; a package written as a map (`path:`) has an empty value.
fn yaml_entries(text: &str, block: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = text.lines().collect();
    let Some(start) = lines.iter().position(|l| l.trim_end() == format!("{block}:")) else { return Vec::new() };
    lines[start + 1..]
        .iter()
        .take_while(|l| l.is_empty() || l.starts_with(' ') || l.starts_with('#'))
        .filter(|l| l.starts_with("  ") && !l.starts_with("   ") && !l.trim_start().starts_with('#'))
        .filter_map(|l| l.trim().split_once(':'))
        .map(|(n, v)| (n.trim().to_string(), v.trim().trim_matches('"').to_string()))
        .collect()
}

/// The constraint of a package in a top-level block (`dependencies:`), as written (`^2.2.3`).
fn yaml_get(text: &str, block: &str, name: &str) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.iter().position(|l| l.trim_end() == format!("{block}:"))?;
    lines[start + 1..]
        .iter()
        .take_while(|l| l.is_empty() || l.starts_with(' ') || l.starts_with('#'))
        .find_map(|l| l.strip_prefix(&format!("  {name}:")))
        .map(|v| v.trim().trim_matches('"').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_entries_are_set_and_removed_in_their_section() {
        let t = template("shop");
        let t = set_entry(&t, "dependencies", "hive", "^2.2.3").unwrap();
        let t = set_entry(&t, "dependencies", "hive", "^2.2.4").unwrap();
        let t = set_entry(&t, "devDependencies", "hive_generator", "^2.0.1").unwrap();
        assert!(t.contains("dependencies(\n  hive = \"^2.2.4\",\n)"), "{t}");
        assert!(t.contains("devDependencies(\n  hive_generator = \"^2.0.1\",\n)"), "{t}");
        let (c, diags) = project::read_config(&t, &["dragonfly", "app", "dependencies", "devDependencies"].map(String::from));
        assert!(diags.is_empty(), "{diags:#?}");
        assert_eq!(get(&c, "dependencies", "hive"), Some("^2.2.4"));
        assert!(!template("shop").contains("dependencies"), "empty sections are not written");
        let t = remove_entry(&t, "dependencies", "hive").unwrap();
        assert!(!t.contains("\ndependencies("), "an empty section is removed: {t}");
        assert!(t.contains(")\n\ndevDependencies(\n  hive_generator"), "{t}");
        let t = remove_entry(&t, "devDependencies", "hive_generator").unwrap();
        assert_eq!(t, template("shop"));
        // a section that is missing is added
        let t = set_entry("app(\n  name = \"x\",\n)\n", "dependencies", "dio", "^5.0.0").unwrap();
        assert!(t.ends_with("dependencies(\n  dio = \"^5.0.0\",\n)\n"), "{t}");
        assert!(set_entry("dependencies(hive = \"1\")\n", "dependencies", "dio", "1").is_err());
    }

    #[test]
    fn pubspec_fields_and_packages() {
        let p = "name: app\ndescription: \"A new Flutter project.\"\nversion: 1.0.0+1\n\nenvironment:\n  sdk: ^3.9.0\n\n\
                 dependencies:\n  flutter:\n    sdk: flutter\n  hive: ^2.2.3\n\ndev_dependencies:\n  mocktail: ^1.0.4\n\n\
                 flutter:\n  uses-material-design: true\n";
        let p = set_yaml(p, None, "description", &yaml("Shop: the app"));
        let p = set_yaml(&p, Some("environment"), "sdk", &yaml(">=3.5.0 <4.0.0"));
        let p = set_yaml(&p, Some("flutter"), "uses-material-design", "false");
        assert!(p.contains("description: \"Shop: the app\"\n"), "{p}");
        assert!(p.contains("environment:\n  sdk: \">=3.5.0 <4.0.0\"\n"), "{p}");
        assert!(p.ends_with("flutter:\n  uses-material-design: false\n"), "{p}");
        assert_eq!(yaml_get(&p, "dependencies", "hive").as_deref(), Some("^2.2.3"));
        assert_eq!(yaml_get(&p, "dependencies", "mocktail"), None);
        assert_eq!(yaml_get(&p, "dev_dependencies", "mocktail").as_deref(), Some("^1.0.4"));
        let entries = yaml_entries(&p, "dependencies");
        assert_eq!(entries, [("flutter".to_string(), String::new()), ("hive".to_string(), "^2.2.3".to_string())]);
        assert_eq!(top_value(&p, "version").as_deref(), Some("1.0.0+1"));
        let lock = "packages:\n  hive:\n    dependency: \"direct main\"\n    source: hosted\n    version: \"2.2.3\"\n";
        assert_eq!(locked_version(lock, "hive").as_deref(), Some("2.2.3"));
    }
}
