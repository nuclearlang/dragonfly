//! `dragonfly explain`: a Dart/Flutter run-time error (on stdin) told in Nuclear terms: its message, a short
//! explanation and the `.nuc` lines of its stack, from the line map `dragonfly build` writes
//! (`build/app/.dart_tool/nuclear_lines.json`). The editors call it with `--json` to show the error at its
//! `.nuc` line; in a terminal it prints `file.nuc:line` rows.

use serde_json::{Value, json};
use std::path::Path;

/// Short explanations of frequent Flutter/Dart run-time errors (the Dart text is always shown too).
const HINTS: &[(&[&str], &str)] = &[
    (
        &["Null check operator used on a null value"],
        "A value used with `!` (or that cannot be null) was null when this line ran. Check the `!` here, or give \
         the value a default with `??`.",
    ),
    (
        &["LateInitializationError"],
        "A field was read before it got a value. Give it a value in the constructor or check it before reading it.",
    ),
    (&["RangeError"], "An index is outside the list: the list is empty or the index is off by one."),
    (
        &["is not a subtype of type"],
        "A value has another type than the code expects; usually JSON read into a model whose field types do not \
         match the data.",
    ),
    (
        &["RenderFlex overflowed"],
        "The children of a Row/Column need more space than there is. Wrap the big child in Expanded/Flexible, or \
         make the content scrollable.",
    ),
    (
        &["unbounded height", "unbounded width", "Vertical viewport was given unbounded"],
        "A scrollable list is inside a Column/Row without a size. Wrap it in Expanded or give it a height.",
    ),
    (
        &["setState() called after dispose", "Cannot emit new states after calling close"],
        "The screen changed its state after it was closed: an async call finished after leaving the screen.",
    ),
    (
        &["No Material widget found", "No MediaQuery widget", "No Directionality widget", "No Overlay widget"],
        "This widget needs an ancestor (MaterialApp, Scaffold) that is not above it in the tree.",
    ),
    (
        &["does not include a Navigator", "No GoRouter found"],
        "Navigation was called with a context that is outside the app router.",
    ),
    (
        &["ProviderNotFoundException", "BlocProvider.of() called with a context"],
        "The screen asked for a state manager that no provider above it creates. Check the `providers` of its \
         `@screen`.",
    ),
    (
        &["DioException", "SocketException", "Failed host lookup", "Connection refused", "Connection reset"],
        "A network request failed: no connection, a wrong URL or a server error.",
    ),
    (&["FormatException"], "A text could not be read as the expected format (JSON, number or date)."),
    (&["Bad state: No element"], "`first`, `last` or `single` on an empty collection."),
    (&["Stack Overflow"], "A function calls itself without end (or two functions call each other)."),
];

const NO_HINT: &str = "The Dart error happened while running the code generated from this Nuclear line.";

pub fn hint(text: &str) -> &'static str {
    HINTS.iter().find(|(keys, _)| keys.iter().any(|k| text.contains(k))).map_or(NO_HINT, |(_, h)| h)
}

/// The message of the error: after `Unhandled Exception:`, or the first line of a Flutter error block that is
/// not its header or its "The following … was thrown" line.
pub fn title(text: &str) -> String {
    let rows: Vec<&str> =
        text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.chars().all(|c| c == '═')).collect();
    if let Some(r) = rows.iter().find_map(|r| r.split_once("Unhandled Exception:")) {
        return r.1.trim().to_string();
    }
    rows.iter()
        .find(|l| {
            !l.contains("EXCEPTION CAUGHT BY")
                && !l.contains("Exception caught by")
                && !l.starts_with("The following")
                && !l.starts_with("When the exception")
                && !l.starts_with('#')
        })
        .map_or_else(|| "Error".into(), |l| l.to_string())
}

/// Every `x.dart:12` / `x.dart:12:5` / `x.dart 12:5` of the text: (frame name, file reference, line).
pub fn locations(text: &str) -> Vec<(String, String, u32)> {
    let mut out = Vec::new();
    for row in text.lines() {
        let name = row
            .trim()
            .strip_prefix('#')
            .and_then(|r| r.trim_start_matches(|c: char| c.is_ascii_digit()).trim().rsplit_once(" ("))
            .map_or(String::new(), |(n, _)| n.trim().to_string());
        let mut from = 0;
        while let Some(at) = row[from..].find(".dart") {
            let end = from + at + ".dart".len();
            let start = row[..end].rfind(|c: char| c.is_whitespace() || c == '(').map_or(0, |i| i + 1);
            let rest = &row[end..];
            let digits: String = rest.chars().skip(1).take_while(char::is_ascii_digit).collect();
            if rest.starts_with([':', ' ']) && !digits.is_empty() {
                out.push((name.clone(), row[start..end].to_string(), digits.parse().unwrap_or(0)));
            }
            from = end;
        }
    }
    out
}

/// A reference of a stack (`package:app/x.dart`, `file:///…/x.dart`, `lib/x.dart`) as a path relative to the app.
fn app_relative(app: &Path, file: &str) -> Option<String> {
    // `packages/app/x.dart` is how the web (DDC) writes `package:app/x.dart` in its stacks
    if let Some(rest) = file.strip_prefix("package:").or_else(|| file.strip_prefix("packages/")) {
        return rest.split_once('/').map(|(_, p)| format!("lib/{p}"));
    }
    let path = Path::new(file.strip_prefix("file://").unwrap_or(file));
    match path.strip_prefix(app) {
        Ok(rel) => Some(rel.to_string_lossy().replace('\\', "/")),
        Err(_) if path.is_relative() => Some(file.to_string()),
        Err(_) => None,
    }
}

/// The `.nuc` place of a `.dart` line: the closest mapped line at or above it.
fn to_nuc(map: &Value, out: &str, line: u32) -> Option<(String, u32)> {
    let entry = map.get("files")?.get(out)?;
    let pairs = entry.get("lines")?.as_array()?;
    let hit = pairs
        .iter()
        .filter_map(|p| Some((p.get(0)?.as_u64()? as u32, p.get(1)?.as_u64()? as u32)))
        .take_while(|(d, _)| *d <= line)
        .last()?;
    Some((entry.get("source")?.as_str()?.to_string(), hit.1))
}

/// `{title, hint, frames: [{source (absolute .nuc), line, name}]}`; `frames` lists only the `.nuc` code.
pub fn explain(root: &Path, app: &Path, map: &Value, text: &str) -> Value {
    let frames: Vec<Value> = locations(text)
        .into_iter()
        .filter_map(|(name, file, line)| {
            let (source, line) = to_nuc(map, &app_relative(app, &file)?, line)?;
            Some(json!({ "source": root.join(source), "line": line, "name": name }))
        })
        .collect();
    json!({ "title": title(text), "hint": hint(text), "frames": frames })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLUTTER: &str = "══╡ EXCEPTION CAUGHT BY WIDGETS LIBRARY ╞═══
The following _TypeError was thrown building App:
Null check operator used on a null value
#0      App.build.<anonymous closure> (package:shop/main.dart:19:65)
#1      _MaterialAppState._materialBuilder (package:flutter/src/material/app.dart:1037:27)
#2      App.build (file:///p/build/app/lib/main.dart:15:12)
════════════";

    #[test]
    fn a_flutter_error_points_to_its_nuc_lines() {
        let map =
            json!({ "files": { "lib/main.dart": { "source": "main.nuc", "lines": [[5, 1], [15, 7], [19, 11]] } } });
        let got = explain(Path::new("/p"), Path::new("/p/build/app"), &map, FLUTTER);
        assert_eq!(got["title"], "Null check operator used on a null value");
        assert!(got["hint"].as_str().unwrap().contains("`!`"));
        let frames: Vec<(String, u64, String)> = got["frames"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| {
                (f["source"].as_str().unwrap().into(), f["line"].as_u64().unwrap(), f["name"].as_str().unwrap().into())
            })
            .collect();
        assert_eq!(
            frames,
            [
                ("/p/main.nuc".into(), 11, "App.build.<anonymous closure>".into()),
                ("/p/main.nuc".into(), 7, "App.build".into())
            ]
        );
    }

    #[test]
    fn an_async_error_takes_its_message() {
        let text = "[ERROR:flutter/runtime/dart_vm_initializer.cc(40)] Unhandled Exception: Bad state: No element\n#0 f (package:a/x.dart:3:1)";
        assert_eq!(title(text), "Bad state: No element");
        assert!(hint(text).contains("empty collection"));
        assert_eq!(locations(text), [("f".into(), "package:a/x.dart".into(), 3)]);
        let web = "packages/a/x.dart 3:1  f";
        assert_eq!(locations(web), [(String::new(), "packages/a/x.dart".into(), 3)]);
        assert_eq!(app_relative(Path::new("/p/build/app"), "packages/a/x.dart").as_deref(), Some("lib/x.dart"));
    }
}
