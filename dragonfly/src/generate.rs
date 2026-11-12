//! `dragonfly gen` (ADR-0103): finished code from the data of a resource, never placeholders. The pieces and their
//! templates are the platform's (`[[generators.<what>]]` of `platform.toml`); this file reads the fields (written,
//! from an example JSON answer or from a spec's table), names everything, and writes each piece at its `[layout]`
//! path. A file that exists is kept; a facade (`merge = true`) gets the aliases it lacks.

use crate::scaffold;
use nuclear_dialect::Dialect;
use nuclear_dialect::template::Value;
use serde_json::Value as Json;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A field of a model: its Nuclear name (camelCase), type and whether it may be missing.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub name: String,
    pub ty: String,
    pub optional: bool,
    /// The name in the JSON when it is not the field's (`air_date` for `airDate`).
    pub key: Option<String>,
}

/// A model and its fields (the resource's first, then the nested ones).
#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    pub name: String,
    pub fields: Vec<Field>,
}

/// Where a resource's data comes from: another API (an HTTP repository) or the project's database.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    Rest { base: Option<String>, path: String },
    Db,
}

/// Everything `gen resource` writes from.
#[derive(Debug, Clone)]
pub struct Resource {
    pub name: String,
    pub plural: String,
    pub component: String,
    pub models: Vec<Model>,
    /// The API answers a list in pages: the page model (`CharacterPage`: `info` + `results`) and the field of the items.
    pub page: Option<(Model, String)>,
    pub source: Source,
    pub actions: Vec<String>,
    pub screen: Option<String>,
}

pub const ACTIONS: &[&str] = &["list", "get", "search", "create", "update", "delete"];

/// `character` → `characters`, `category` → `categories`, `box` → `boxes` (English; `--plural` overrides).
pub fn plural(word: &str) -> String {
    let lower = word.to_lowercase();
    if lower.ends_with('y') && !lower.ends_with("ay") && !lower.ends_with("ey") && !lower.ends_with("oy") && !lower.ends_with("uy") {
        format!("{}ies", &word[..word.len() - 1])
    } else if ["s", "x", "z", "ch", "sh"].iter().any(|e| lower.ends_with(e)) {
        format!("{word}es")
    } else {
        format!("{word}s")
    }
}

fn camel(name: &str) -> String {
    let p = scaffold::pascal(&scaffold::snake(name));
    p[..1].to_lowercase() + &p[1..]
}

/// `id:int name:string origin:Origin species:string?` → fields (a trailing `?` is optional).
pub fn parse_fields(text: &str) -> Result<Vec<Field>, String> {
    text.split([' ', ',']).filter(|s| !s.is_empty()).map(|f| {
        let (name, ty) = f.split_once(':').ok_or_else(|| format!("'{f}' is not name:type (id:int, name:string, tags:string[], origin:Origin?)"))?;
        let optional = ty.ends_with('?');
        Ok(Field { name: camel(name), ty: ty.trim_end_matches('?').to_string(), optional, key: None })
    }).collect()
}

/// The models an example JSON answer implies: `name` for the item; a nested object is a model named after its key
/// (`origin` → `Origin`, or `<Name><Key>` when two would clash); a list under `results` / `items` / `data` next to other
/// fields is a paged answer.
/// The models of an answer, and its page model with the field that holds the items when it is paged.
pub type Answer = (Vec<Model>, Option<(Model, String)>);

pub fn from_json(json: &Json, name: &str) -> Result<Answer, String> {
    let mut models = Vec::new();
    let (item, page) = match json {
        Json::Array(items) => (items.first().cloned().ok_or("the example is an empty list: give one with an item")?, None),
        Json::Object(map) => {
            let list = ["results", "items", "data", "content"].into_iter().find(|k| map.get(*k).is_some_and(|v| v.as_array().is_some_and(|a| a.first().is_some_and(Json::is_object))));
            match list {
                Some(key) => {
                    let first = map[key].as_array().and_then(|a| a.first()).cloned().unwrap_or(Json::Null);
                    let mut wrapper = Json::Object(map.clone());
                    if let Json::Object(w) = &mut wrapper {
                        w.remove(key);
                    }
                    (first, Some((wrapper, key.to_string())))
                }
                None => (json.clone(), None),
            }
        }
        _ => return Err("the example is not a JSON object or list".into()),
    };
    let main = object_model(&item, name, name, &mut models)?;
    models.insert(0, main);
    let page = match page {
        Some((wrapper, key)) => {
            let page_name = format!("{name}Page");
            let mut m = object_model(&wrapper, &page_name, &page_name, &mut models)?;
            m.fields.push(Field { name: camel(&key), ty: format!("{name}[]"), optional: false, key: json_key(&key) });
            Some((m, camel(&key)))
        }
        None => None,
    };
    Ok((models, page))
}

/// The JSON name of a field, when Nuclear's camelCase name is another (`air_date` → `airDate`).
fn json_key(key: &str) -> Option<String> {
    (camel(key) != key).then(|| key.to_string())
}

fn object_model(value: &Json, name: &str, owner: &str, models: &mut Vec<Model>) -> Result<Model, String> {
    let Json::Object(map) = value else { return Err(format!("{name}: an object was expected")) };
    let mut fields = Vec::new();
    // the key first, then the others in alphabetical order (the JSON reader keeps the keys sorted)
    let mut entries: Vec<(&String, &Json)> = map.iter().collect();
    entries.sort_by_key(|(k, _)| *k != "id");
    for (key, v) in entries {
        let field = camel(key);
        let (ty, optional) = match v {
            Json::Null => ("string".to_string(), true),
            Json::Bool(_) => ("boolean".into(), false),
            Json::Number(n) => (if n.is_i64() || n.is_u64() { "int" } else { "double" }.into(), false),
            Json::String(_) => ("string".into(), false),
            Json::Array(items) => match items.first() {
                Some(Json::Object(_)) => (format!("{}[]", nested(&items[0], key, owner, models)?), false),
                Some(Json::Number(n)) => (format!("{}[]", if n.is_i64() { "int" } else { "double" }), false),
                Some(Json::Bool(_)) => ("boolean[]".into(), false),
                _ => ("string[]".into(), false),
            },
            Json::Object(_) => (nested(v, key, owner, models)?, false),
        };
        fields.push(Field { name: field, ty, optional, key: json_key(key) });
    }
    Ok(Model { name: name.to_string(), fields })
}

fn nested(v: &Json, key: &str, owner: &str, models: &mut Vec<Model>) -> Result<String, String> {
    // named after its owner (`CharacterOrigin`): a project's names are global, two resources may nest an `origin`
    let name = format!("{owner}{}", scaffold::pascal(&scaffold::snake(key)));
    if models.iter().any(|m| m.name == name) {
        return Ok(name);
    }
    let m = object_model(v, &name, owner, models)?;
    models.push(m);
    Ok(name)
}

/// The fields of a spec's table (`Key | Type | Description`, or the same separated by tabs): `int`, `string`,
/// `string (url)`, `array`/`array (urls)` (a list of texts), `boolean`, `number`; an `object` needs its fields, which a
/// table does not give: an error that says to pass the API's example answer (`--from-json`).
/// The example answer a spec shows: the first block that starts with `{` or `[` at the start of a line and ends with
/// its closing line; `// ...` lines and the commas they leave before `]` or `}` are dropped.
pub fn spec_json(text: &str) -> Option<Json> {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.iter().position(|l| *l == "{" || *l == "[")?;
    let close = if lines[start] == "{" { "}" } else { "]" };
    let end = start + lines[start..].iter().position(|l| *l == close)?;
    let kept: Vec<&str> = lines[start..=end].iter().filter(|l| !l.trim_start().starts_with("//")).copied().collect();
    // a comma left before a closing `]` or `}` (the next kept line) is dropped
    let mut json = String::new();
    for (i, line) in kept.iter().enumerate() {
        let next = kept.get(i + 1).map(|l| l.trim_start()).unwrap_or("");
        let line = if next.starts_with(']') || next.starts_with('}') { line.trim_end().trim_end_matches(',') } else { line };
        json.push_str(line);
        json.push('\n');
    }
    serde_json::from_str(&json).ok()
}

/// The resource's address a spec gives: `GET https://...` on a line of its own.
pub fn spec_url(text: &str) -> Option<String> {
    text.lines().find_map(|l| l.trim().strip_prefix("GET ").map(str::trim).filter(|u| u.starts_with("http")).map(str::to_string))
}

pub fn from_spec(text: &str, name: &str) -> Result<Model, String> {
    let mut fields = Vec::new();
    let mut objects = Vec::new();
    for line in text.lines() {
        let cells: Vec<&str> = if line.contains('|') { line.split('|').map(str::trim).filter(|c| !c.is_empty()).collect() } else { line.split('\t').map(str::trim).collect() };
        if cells.len() < 2 || cells[0].eq_ignore_ascii_case("key") || cells[0].starts_with('-') || cells[0].contains(' ') {
            continue;
        }
        let ty = cells[1].to_lowercase();
        let nuclear = match ty.split_whitespace().next().unwrap_or("") {
            "int" | "integer" => "int",
            "string" | "text" | "date" => "string",
            "number" | "float" | "double" => "double",
            "boolean" | "bool" => "boolean",
            "array" | "list" => "string[]",
            "object" => {
                objects.push(cells[0].to_string());
                continue;
            }
            _ => continue,
        };
        fields.push(Field { name: camel(cells[0]), ty: nuclear.into(), optional: false, key: json_key(cells[0]) });
    }
    if fields.is_empty() {
        return Err("the spec has no Key | Type table".into());
    }
    if !objects.is_empty() {
        return Err(format!(
            "{} {} an object in the spec's table, whose fields the table does not give: pass an example answer of the API too (--from-json <file or url>)",
            objects.join(", "),
            if objects.len() == 1 { "is" } else { "are" }
        ));
    }
    Ok(Model { name: name.to_string(), fields })
}

fn s(v: impl Into<String>) -> Value {
    Value::Str(v.into())
}

fn fields_value(fields: &[Field]) -> Value {
    let n = fields.len();
    Value::List(
        fields
            .iter()
            .enumerate()
            .map(|(i, f)| {
                Value::Map(BTreeMap::from([
                    ("name".into(), s(&f.name)),
                    ("type".into(), s(&f.ty)),
                    ("optional".into(), Value::Bool(f.optional)),
                    ("last".into(), Value::Bool(i + 1 == n)),
                    ("id".into(), Value::Bool(f.name == "id")),
                    ("key".into(), f.key.clone().map(Value::Str).unwrap_or(Value::Bool(false))),
                    // a value of the type for the tests (`1`, `"Rick"`, `[]`, a nested model is left out: optional)
                    ("sample".into(), s(sample(&f.ty, &f.name))),
                ]))
            })
            .collect(),
    )
}

/// A value of a model for the tests: `Character(id = 1, name = "name", origin = CharacterOrigin(...), episode = [])`;
/// an optional field is left out.
pub fn sample_of(model: &str, models: &[&Model], depth: usize) -> String {
    let Some(m) = models.iter().find(|m| m.name == model) else { return String::new() };
    let args: Vec<String> = m
        .fields
        .iter()
        .filter(|f| !f.optional)
        .map(|f| {
            let v = sample(&f.ty, &f.name);
            let v = if v.is_empty() && depth < 4 { sample_of(&f.ty, models, depth + 1) } else { v };
            format!("{} = {v}", f.name)
        })
        .collect();
    format!("{model}({})", args.join(", "))
}

/// The types a field can name without a model: `gen` checks every other one is declared.
pub const BASIC: [&str; 6] = ["int", "double", "boolean", "string", "any", "DateTime"];

fn sample(ty: &str, name: &str) -> String {
    match ty {
        "int" => "1".into(),
        "double" => "1.5".into(),
        "boolean" => "true".into(),
        "string" => format!("\"{name}\""),
        t if t.ends_with("[]") => "[]".into(),
        _ => String::new(),
    }
}

/// The values every template of `gen resource` reads (spec/dialects.md §7, `dragonfly help gen`).
pub fn context(r: &Resource) -> Value {
    let name = &r.name;
    let plural = &r.plural;
    let main = &r.models[0];
    let id = main.fields.iter().find(|f| f.name == "id").map(|f| f.ty.clone()).unwrap_or_else(|| "int".into());
    let mut m = BTreeMap::from([
        ("Name".into(), s(name)),
        ("name".into(), s(camel(name))),
        ("Names".into(), s(plural)),
        ("names".into(), s(camel(plural))),
        ("component".into(), s(&r.component)),
        ("Component".into(), s(scaffold::pascal(&r.component))),
        ("idType".into(), s(id)),
        ("fields".into(), fields_value(&main.fields)),
        // the fields without the key: what a new one takes
        ("data".into(), fields_value(&main.fields.iter().filter(|f| f.name != "id").cloned().collect::<Vec<_>>())),
        ("rest".into(), Value::Bool(matches!(r.source, Source::Rest { .. }))),
        ("db".into(), Value::Bool(r.source == Source::Db)),
        ("paged".into(), Value::Bool(r.page.is_some())),
        // what listing takes: a page, unless the API answers the whole list at once
        ("listParam".into(), s(if r.page.is_some() || r.source == Source::Db { "page: int = 1" } else { "" })),
        ("listArg".into(), s(if r.page.is_some() || r.source == Source::Db { "page" } else { "" })),
    ]);
    if let Source::Rest { base, path } = &r.source {
        m.insert("path".into(), s(path));
        if let Some(b) = base {
            m.insert("baseUrl".into(), s(b));
        }
    }
    if let Some((page, items)) = &r.page {
        m.insert("Page".into(), s(&page.name));
        m.insert("items".into(), s(items));
    }
    let mut models: Vec<&Model> = r.models.iter().collect();
    if let Some((page, _)) = &r.page {
        models.push(page);
    }
    // what a list shows of each item: its `name`, else its first text, else its key
    let label = main.fields.iter().find(|f| f.name == "name").or_else(|| main.fields.iter().find(|f| f.ty == "string" && !f.optional)).map(|f| f.name.clone()).unwrap_or_else(|| "id".into());
    let text = main.fields.iter().any(|f| f.name == label && f.ty == "string" && !f.optional);
    // a view writes a text label as it is, anything else inside a text
    m.insert("labelText".into(), Value::Bool(text));
    m.insert("label".into(), s(label));
    let sample = sample_of(name, &models, 0);
    m.insert("sample".into(), s(&sample));
    if let Some((page, _)) = &r.page {
        // an empty page: what a test's mock answers
        m.insert("pageSample".into(), s(sample_of(&page.name, &models, 0)));
    }
    m.insert(
        "models".into(),
        Value::List(
            models
                .iter()
                .enumerate()
                .map(|(i, md)| Value::Map(BTreeMap::from([("Model".into(), s(&md.name)), ("fields".into(), fields_value(&md.fields)), ("main".into(), Value::Bool(i == 0))])))
                .collect(),
        ),
    );
    let mut acts = BTreeMap::new();
    let mut list = Vec::new();
    for a in &r.actions {
        acts.insert(a.clone(), Value::Bool(true));
        let usecase = match a.as_str() {
            "list" => format!("List{plural}"),
            "get" => format!("Get{name}"),
            "search" => format!("Search{plural}"),
            "create" => format!("Create{name}"),
            "update" => format!("Update{name}"),
            _ => format!("Delete{name}"),
        };
        let alias = camel(&usecase);
        list.push(Value::Map(BTreeMap::from([
            ("action".into(), s(a)),
            ("UseCase".into(), s(&usecase)),
            ("alias".into(), s(alias)),
            ("is".into(), Value::Map(BTreeMap::from([(a.clone(), Value::Bool(true))]))),
        ])));
    }
    m.insert("actions".into(), Value::Map(acts));
    m.insert("each_action".into(), Value::List(list));
    if let Some(screen) = &r.screen {
        m.insert("screen".into(), s(screen));
        m.insert("Screen".into(), s(scaffold::pascal(screen)));
    }
    Value::Map(m)
}

/// The value at a dotted path of the context (`actions.list`).
fn lookup<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(v, |cur, key| match cur {
        Value::Map(m) => m.get(key),
        _ => None,
    })
}

fn merged(base: &Value, item: &Value) -> Value {
    match (base, item) {
        (Value::Map(a), Value::Map(b)) => {
            let mut m = a.clone();
            m.extend(b.clone());
            Value::Map(m)
        }
        _ => base.clone(),
    }
}

/// The files the platform's `[[generators.<what>]]` write for this context: (path, text, merge).
pub fn plan(d: &Dialect, what: &str, ctx: &Value) -> Result<Vec<(PathBuf, String, bool)>, String> {
    let pieces = d.generators.get(what).ok_or_else(|| format!("the {} platform has no `gen {what}` ([[generators.{what}]] in its platform.toml)", d.platform.as_deref().unwrap_or("?")))?;
    let mut out = Vec::new();
    for p in pieces {
        if let Some(w) = &p.when
            && !lookup(ctx, w).is_some_and(Value::truthy)
        {
            continue;
        }
        let items: Vec<Value> = match &p.each {
            Some(list) => match lookup(ctx, list) {
                Some(Value::List(items)) => items.iter().map(|i| merged(ctx, i)).collect(),
                _ => Vec::new(),
            },
            None => vec![ctx.clone()],
        };
        for item in items {
            let name = p.name.render(&item).trim().to_string();
            let place = d.layout.get(&p.kind).ok_or_else(|| format!("no [layout] place for '{}'", p.kind))?;
            let screen = lookup(&item, "screen").map(|v| match v { Value::Str(s) => s.clone(), _ => String::new() }).unwrap_or_else(|| "widgets".into());
            let component = lookup(&item, "component").map(|v| match v { Value::Str(s) => s.clone(), _ => String::new() }).unwrap_or_default();
            let rel = place.replace("{component}", &component).replace("{screen}", &screen).replace("{file}", &scaffold::snake(&name));
            // the canonical layout of `nuclear fmt` (a template's sections leave blank lines behind)
            let text = p.template.render(&item);
            let text = nuclear_syntax::fmt::format(&text).unwrap_or(text);
            out.push((PathBuf::from(rel), text, p.merge));
        }
    }
    Ok(out)
}

/// Writes the planned files: a new file as it is; an existing one is kept, except a facade (`merge`) that gets the
/// indented lines (its aliases) it does not have yet. Returns what happened to each file.
pub fn write(root: &Path, files: &[(PathBuf, String, bool)]) -> std::io::Result<Vec<(String, &'static str)>> {
    let mut report = Vec::new();
    for (rel, text, merge) in files {
        let p = root.join(rel);
        let shown = rel.to_string_lossy().replace('\\', "/");
        if p.exists() {
            if !merge {
                report.push((shown, "kept"));
                continue;
            }
            let mut have = std::fs::read_to_string(&p)?;
            let missing: Vec<&str> = text.lines().filter(|l| l.starts_with("  ") && !l.trim().is_empty() && !have.contains(l.trim())).collect();
            if missing.is_empty() {
                report.push((shown, "kept"));
                continue;
            }
            if !have.ends_with('\n') {
                have.push('\n');
            }
            for l in missing {
                have.push_str(l);
                have.push('\n');
            }
            std::fs::write(&p, have)?;
            report.push((shown, "added to"));
            continue;
        }
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&p, text)?;
        report.push((shown, "created"));
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_spec_gives_its_example_answer_and_its_url() {
        let spec = "The model:\n\nGET https://example.com/api/character\n\n{\n  \"results\": [\n    {\n      \"id\": 1,\n      \"episode\": [\n        \"a\",\n        // ...\n      ]\n    },\n    // ...\n  ]\n}\n";
        assert_eq!(spec_url(spec).as_deref(), Some("https://example.com/api/character"));
        let json = spec_json(spec).expect("the example parses");
        assert_eq!(json["results"][0]["episode"][0], "a");
    }


    #[test]
    fn plurals_and_fields() {
        assert_eq!(plural("Character"), "Characters");
        assert_eq!(plural("Category"), "Categories");
        assert_eq!(plural("Box"), "Boxes");
        let f = parse_fields("id:int air_date:string origin:Origin? tags:string[]").unwrap();
        assert_eq!(f[1].name, "airDate");
        assert!(f[2].optional && f[2].ty == "Origin");
        assert!(parse_fields("id").is_err());
    }

    #[test]
    fn an_example_answer_gives_the_models_and_the_page() {
        let json: Json = serde_json::from_str(r#"{"info":{"count":826,"pages":42,"next":"x","prev":null},"results":[{"id":1,"name":"Rick","origin":{"name":"Earth","url":""},"location":{"name":"Citadel","url":""},"episode":["a"],"air_date":"x"}]}"#).unwrap();
        let (models, page) = from_json(&json, "Character").unwrap();
        assert_eq!(models[0].name, "Character");
        let names: Vec<&str> = models.iter().map(|m| m.name.as_str()).collect();
        assert!(names.contains(&"CharacterOrigin") && names.contains(&"CharacterLocation"), "{names:?}");
        assert!(models[0].fields.iter().any(|f| f.name == "airDate" && f.ty == "string"));
        assert!(models[0].fields.iter().any(|f| f.name == "episode" && f.ty == "string[]"));
        let (page, key) = page.unwrap();
        assert_eq!((page.name.as_str(), key.as_str()), ("CharacterPage", "results"));
        assert!(page.fields.iter().any(|f| f.name == "info" && f.ty == "CharacterPageInfo"));
        assert!(page.fields.iter().any(|f| f.name == "results" && f.ty == "Character[]"));
    }

    #[test]
    fn a_spec_table_gives_the_fields_or_says_what_is_missing() {
        let spec = "Key\tType\tDescription\nid\tint\tThe id.\nname\tstring\tThe name.\nepisode\tarray (urls)\tEpisodes.\n";
        let m = from_spec(spec, "Character").unwrap();
        assert_eq!(m.fields.len(), 3);
        assert_eq!(m.fields[2].ty, "string[]");
        let with_object = format!("{spec}origin\tobject\tName and link.\n");
        assert!(from_spec(&with_object, "Character").unwrap_err().contains("--from-json"));
    }
}
