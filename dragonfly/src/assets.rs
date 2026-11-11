//! The app's assets in its pubspec, by convention (nothing is written in config.nuc): every folder under
//! `assets/` → `assets:`, `assets/fonts/<Family>/<Family>-<Weight>[Italic].ttf` → `fonts:` and
//! `assets/shaders/*.frag` → `shaders:`. The app's pubspec is generated: these three keys of `flutter:` are
//! rewritten on every build (a removed file leaves the pubspec too); the rest of `flutter:` is kept.

use std::path::Path;

const FONTS: &str = "assets/fonts/";
const SHADERS: &str = "assets/shaders/";
const FONT_FILES: &[&str] = &["ttf", "otf", "ttc"];
/// The keys of `flutter:` this module owns.
const OWNED: &[&str] = &["assets:", "fonts:", "shaders:"];

#[derive(Debug, PartialEq)]
pub struct Font {
    pub path: String,
    pub weight: u16,
    pub italic: bool,
}

/// The families of `assets/fonts/`: each folder is a family and each font file in it gets its weight and
/// style from the end of its name (`Roboto-BoldItalic.ttf` → 700 italic; no known word → 400). Other files
/// (`OFL.txt`) are ignored. Two files with the same weight and style are an error: the name decides.
pub fn fonts(rels: &[String]) -> Result<Vec<(String, Vec<Font>)>, String> {
    let mut families: Vec<(String, Vec<Font>)> = Vec::new();
    for rel in rels.iter().filter(|r| r.starts_with(FONTS)) {
        let path = Path::new(rel);
        if !path.extension().is_some_and(|x| FONT_FILES.contains(&x.to_string_lossy().to_lowercase().as_str())) {
            continue;
        }
        let Some((family, file)) = rel[FONTS.len()..].split_once('/').filter(|(_, f)| !f.contains('/')) else {
            return Err(format!("{rel}: a font goes in the folder of its family: {FONTS}<Family>/<Family>-Bold.ttf"));
        };
        let (weight, italic) = style(&path.file_stem().unwrap_or_default().to_string_lossy());
        let i = match families.iter().position(|(f, _)| f == family) {
            Some(i) => i,
            None => {
                families.push((family.to_string(), Vec::new()));
                families.len() - 1
            }
        };
        if let Some(other) = families[i].1.iter().find(|f| f.weight == weight && f.italic == italic) {
            let kind = if italic { "italic" } else { "normal" };
            return Err(format!(
                "{FONTS}{family}: {} and {file} are both {weight} {kind}: end each name with its weight and style \
                 ({family}-Regular.ttf, {family}-Bold.ttf, {family}-BoldItalic.ttf...)",
                other.path.rsplit('/').next().unwrap_or_default()
            ));
        }
        families[i].1.push(Font { path: rel.clone(), weight, italic });
    }
    families.sort_by(|a, b| a.0.cmp(&b.0));
    for (_, list) in &mut families {
        list.sort_by_key(|f| (f.weight, f.italic));
    }
    Ok(families)
}

/// `Roboto-SemiBoldItalic` → (600, true): the word after the last `-` or `_` (`Oblique` is italic too).
fn style(stem: &str) -> (u16, bool) {
    let mut word = stem.rsplit(['-', '_']).next().unwrap_or_default().to_lowercase();
    let slant = ["italic", "oblique"].into_iter().find(|s| word.ends_with(s));
    if let Some(s) = slant {
        word.truncate(word.len() - s.len());
    }
    let italic = slant.is_some();
    let weight = match word.as_str() {
        "thin" | "hairline" => 100,
        "extralight" | "ultralight" => 200,
        "light" => 300,
        "medium" => 500,
        "semibold" | "demibold" => 600,
        "bold" => 700,
        "extrabold" | "ultrabold" => 800,
        "black" | "heavy" => 900,
        _ => 400,
    };
    (weight, italic)
}

/// The `assets:`, `fonts:` and `shaders:` lines of `flutter:` for the files the build wrote.
fn block(written: &[String]) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut dirs: Vec<String> = written
        .iter()
        .filter(|r| r.starts_with("assets/") && !r.starts_with(FONTS) && !r.starts_with(SHADERS))
        .filter_map(|r| Some(format!("    - {}/", Path::new(r).parent()?.display())))
        .collect();
    dirs.sort();
    dirs.dedup();
    if !dirs.is_empty() {
        out.push("  assets:".to_string());
        out.extend(dirs);
    }
    let families = fonts(written)?;
    if !families.is_empty() {
        out.push("  fonts:".to_string());
    }
    for (family, list) in families {
        out.push(format!("    - family: \"{family}\""));
        out.push("      fonts:".to_string());
        for f in list {
            out.push(format!("        - asset: {}", f.path));
            // Flutter's defaults (400, normal) are not written
            if f.weight != 400 {
                out.push(format!("          weight: {}", f.weight));
            }
            if f.italic {
                out.push("          style: italic".to_string());
            }
        }
    }
    let mut shaders: Vec<&String> = written.iter().filter(|r| r.starts_with(SHADERS) && r.ends_with(".frag")).collect();
    shaders.sort();
    if !shaders.is_empty() {
        out.push("  shaders:".to_string());
        out.extend(shaders.into_iter().map(|s| format!("    - {s}")));
    }
    Ok(out)
}

/// `pubspec` with the owned keys of `flutter:` replaced by the ones of `written`.
fn rewrite(pubspec: &str, written: &[String]) -> Result<String, String> {
    let mut block = block(written)?;
    let mut out: Vec<String> = Vec::new();
    let (mut in_flutter, mut skip) = (false, false);
    for line in pubspec.lines() {
        if !line.is_empty() && !line.starts_with([' ', '#']) {
            in_flutter = line.trim_end() == "flutter:";
            skip = false;
            out.push(line.to_string());
            if in_flutter {
                out.append(&mut block);
            }
            continue;
        }
        if in_flutter && line.starts_with("  ") && !line.starts_with("   ") {
            skip = OWNED.contains(&line.trim());
        }
        if !(in_flutter && skip) {
            out.push(line.to_string());
        }
    }
    if !block.is_empty() {
        out.push("flutter:".to_string());
        out.append(&mut block);
    }
    Ok(out.join("\n") + "\n")
}

/// Writes the assets of the build into the app's pubspec (Flutter only bundles what it lists).
pub fn write(app: &Path, written: &[String]) -> Result<(), String> {
    let path = app.join("pubspec.yaml");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let new = rewrite(&text, written)?;
    if new != text {
        std::fs::write(&path, new).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rels(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn font_weight_and_style_come_from_the_name() {
        assert_eq!(style("Roboto-Regular"), (400, false));
        assert_eq!(style("Roboto-BoldItalic"), (700, true));
        assert_eq!(style("open_sans_SemiBold"), (600, false));
        assert_eq!(style("Inter-Italic"), (400, true));
        assert_eq!(style("Inter[wght]"), (400, false));
        assert_eq!(style("DejaVuSans-BoldOblique"), (700, true));
    }

    #[test]
    fn fonts_are_grouped_by_family_and_ambiguous_names_fail() {
        let f = fonts(&rels(&[
            "assets/fonts/Roboto/Roboto-Bold.ttf",
            "assets/fonts/Roboto/Roboto-Regular.ttf",
            "assets/fonts/Roboto/OFL.txt",
            "assets/images/logo.png",
        ]))
        .unwrap();
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].1.iter().map(|x| x.weight).collect::<Vec<_>>(), [400, 700]);
        let e = fonts(&rels(&["assets/fonts/A/A.ttf", "assets/fonts/A/A-Regular.ttf"])).unwrap_err();
        assert!(e.contains("both 400 normal"), "{e}");
        assert!(fonts(&rels(&["assets/fonts/Loose.ttf"])).is_err());
    }

    #[test]
    fn the_owned_keys_of_flutter_are_rewritten_and_the_rest_kept() {
        let before = "name: x\n\nflutter:\n  uses-material-design: true\n  assets:\n    - assets/old/\n  fonts:\n    \
                      - family: Gone\n      fonts:\n        - asset: assets/fonts/Gone/Gone.ttf\n";
        let written = rels(&[
            "assets/i18n/en/shared.json",
            "assets/fonts/Roboto/Roboto-BoldItalic.ttf",
            "assets/shaders/wave.frag",
            "lib/main.dart",
        ]);
        let after = rewrite(before, &written).unwrap();
        assert_eq!(
            after,
            "name: x\n\nflutter:\n  assets:\n    - assets/i18n/en/\n  fonts:\n    - family: \"Roboto\"\n      fonts:\n        \
             - asset: assets/fonts/Roboto/Roboto-BoldItalic.ttf\n          weight: 700\n          style: italic\n  \
             shaders:\n    - assets/shaders/wave.frag\n  uses-material-design: true\n"
        );
        // nothing under assets/: the keys leave the pubspec
        assert_eq!(rewrite(&after, &rels(&["lib/main.dart"])).unwrap(), "name: x\n\nflutter:\n  uses-material-design: true\n");
        assert_eq!(rewrite("name: x\n", &rels(&["assets/a/b.png"])).unwrap(), "name: x\nflutter:\n  assets:\n    - assets/a/\n");
    }
}
