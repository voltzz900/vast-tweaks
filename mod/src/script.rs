// Vast Tweaks' script language, compiled into data pack files: recipe filters, recipes and tags. No game here (it
// is tested on its own); `client.rs` hands it the scripts and gives the game what it makes. The same language as
// `tools/vasttweaks.py`, which writes a content mod instead.

use mod_api::serde_json::{Map, Value, json};
use std::collections::BTreeMap;

/// What the scripts make: the data pack files, by the key Horizon's overlay takes (`recipe/<ns>:<path>`,
/// `recipe_filter/<ns>:<path>`, `tags/<ns>:<registry>/<path>`), and the counts for the status line.
#[derive(Debug, Default)]
pub struct Compiled {
    pub files: Vec<(String, Value)>,
    pub filters: usize,
    pub recipes: usize,
    pub tags: usize,
}

/// A mistake in a script: where (`file.vt:12`) and what.
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptError {
    pub at: String,
    pub message: String,
}

impl std::fmt::Display for ScriptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.at, self.message)
    }
}

fn err<T>(at: &str, message: impl Into<String>) -> Result<T, ScriptError> {
    Err(ScriptError { at: at.to_owned(), message: message.into() })
}

const NS: &str = "vasttweaks";

#[derive(Default)]
struct Pack {
    filters: Vec<Value>,
    recipes: BTreeMap<String, Value>,
    tags: BTreeMap<(String, String), Vec<String>>,
    counter: usize,
    /// Lines left out that don't stop the script (a `name`, which only the content-mod tool does).
    notes: Vec<ScriptError>,
}

/// Compiles every script (name, text), in order. A script with a mistake is left out whole (the others still
/// count); its mistakes come back with the rest.
pub fn compile(scripts: &[(String, String)]) -> (Compiled, Vec<ScriptError>) {
    let mut pack = Pack::default();
    let mut errors = Vec::new();
    for (name, text) in scripts {
        let mut one = Pack { counter: pack.counter, ..Pack::default() };
        let stem = name.trim_end_matches(".vt").to_lowercase().replace(|c: char| !c.is_ascii_alphanumeric() && c != '_', "_");
        match parse(text, name, &stem, &mut one) {
            Ok(()) => {
                errors.extend(one.notes.drain(..));
                pack.counter = one.counter;
                pack.filters.extend(one.filters);
                pack.recipes.extend(one.recipes);
                for (k, v) in one.tags {
                    pack.tags.entry(k).or_default().extend(v);
                }
            }
            Err(e) => errors.push(e),
        }
    }
    let mut out = Compiled { filters: pack.filters.len(), recipes: pack.recipes.len(), tags: pack.tags.len(), ..Compiled::default() };
    if !pack.filters.is_empty() {
        out.files.push((format!("recipe_filter/{NS}:scripts"), json!({ "filters": pack.filters })));
    }
    for (id, recipe) in pack.recipes {
        out.files.push((format!("recipe/{id}"), recipe));
    }
    for ((registry, tag), mut values) in pack.tags {
        values.sort();
        values.dedup();
        let (ns, path) = tag.split_once(':').unwrap_or(("minecraft", &tag));
        out.files.push((format!("tags/{ns}:{registry}/{path}"), json!({ "replace": false, "values": values })));
    }
    (out, errors)
}

// ---- reading -------------------------------------------------------------------------------------------------

/// `//` starts a comment anywhere outside quotes; a line whose first word is `#` (a hash and a space) is one.
fn strip_comment(line: &str) -> &str {
    let t = line.trim_start();
    if t.starts_with("# ") || t == "#" {
        return "";
    }
    let mut quote = None;
    let bytes = line.as_bytes();
    for (i, c) in line.char_indices() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == '/' && bytes.get(i + 1) == Some(&b'/') => return &line[..i],
            None => {}
        }
    }
    line
}

/// Words, with "quoted text" kept whole (its quotes taken off).
fn words(line: &str, at: &str) -> Result<Vec<String>, ScriptError> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote = None;
    let mut started = false;
    for c in line.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => cur.push(c),
            None if c == '"' || c == '\'' => {
                quote = Some(c);
                started = true;
            }
            None if c.is_whitespace() => {
                if started {
                    out.push(std::mem::take(&mut cur));
                    started = false;
                }
            }
            None => {
                cur.push(c);
                started = true;
            }
        }
    }
    if quote.is_some() {
        return err(at, "a quote is not closed");
    }
    if started {
        out.push(cur);
    }
    Ok(out)
}

/// An item or `#tag`; a bare name is the game's (`stick` = `minecraft:stick`).
fn item(token: &str, at: &str) -> Result<String, ScriptError> {
    if token.is_empty() || matches!(token, "->" | "with" | "=") {
        return err(at, "an item or #tag is missing");
    }
    let (tag, name) = match token.strip_prefix('#') {
        Some(n) => (true, n),
        None => (false, token),
    };
    let name = if name.contains(':') { name.to_owned() } else { format!("minecraft:{name}") };
    Ok(if tag { format!("#{name}") } else { name })
}

/// `xN` (or `*N`).
fn times(t: &str) -> Option<u32> {
    let mut c = t.chars();
    match c.next() {
        Some('x' | '*') if t.len() > 1 => c.as_str().parse().ok(),
        _ => None,
    }
}

/// A trailing `xN` among `tokens`: (tokens without it, N).
fn count(tokens: &[String]) -> (Vec<String>, u32) {
    for (i, t) in tokens.iter().enumerate() {
        if let Some(n) = times(t) {
            let mut rest = tokens.to_vec();
            rest.remove(i);
            return (rest, n);
        }
    }
    (tokens.to_vec(), 1)
}

#[derive(Default)]
struct Opts {
    map: BTreeMap<String, String>,
    mods: Vec<String>,
}

/// `key value` pairs after a statement: `xp 0.7`, `as <id>`, `if mod <id>`, `where id ...`.
fn options(tokens: &[String], at: &str, allowed: &[&str]) -> Result<Opts, ScriptError> {
    let mut o = Opts::default();
    let mut i = 0;
    while i < tokens.len() {
        let mut key = tokens[i].as_str();
        if key == "if" && tokens.get(i + 1).map(String::as_str) == Some("mod") {
            match tokens.get(i + 2) {
                Some(m) => o.mods.push(m.clone()),
                None => return err(at, "`if mod` needs a mod id"),
            }
            i += 3;
            continue;
        }
        let value;
        if key == "where" && i + 2 < tokens.len() {
            key = tokens[i + 1].as_str();
            value = tokens[i + 2].clone();
            i += 3;
        } else if i + 1 < tokens.len() {
            value = tokens[i + 1].clone();
            i += 2;
        } else {
            return err(at, format!("`{key}` needs a value"));
        }
        if !allowed.contains(&key) {
            return err(at, format!("unknown option `{key}` (allowed: {})", allowed.join(", ")));
        }
        o.map.insert(key.to_owned(), value);
    }
    Ok(o)
}

fn conditions(recipe: &mut Value, o: &Opts) {
    if !o.mods.is_empty() {
        recipe["neoforge:conditions"] = Value::Array(
            o.mods.iter().map(|m| json!({ "type": "neoforge:mod_loaded", "modid": m })).collect(),
        );
    }
}

fn result(id: &str, n: u32) -> Value {
    if n == 1 { json!({ "id": id }) } else { json!({ "id": id, "count": n }) }
}

/// `remove`'s and `replace`'s selection: `output X`, `input X`, `id P`, `type P`, each optionally after `where`.
/// None selects every recipe: allowed for a swap (`every`), never for `remove`.
fn select(tokens: &[String], at: &str, every: bool) -> Result<Map<String, Value>, ScriptError> {
    let mut f = Map::new();
    let mut i = 0;
    while i < tokens.len() {
        let key = tokens[i].as_str();
        if key == "where" || key == "and" {
            i += 1;
            continue;
        }
        let Some(value) = tokens.get(i + 1).filter(|_| matches!(key, "id" | "type" | "input" | "output")) else {
            return err(at, format!("expected id|type|input|output <value>, got `{key}`"));
        };
        let v = if matches!(key, "input" | "output") {
            item(value, at)?
        } else if value.contains(':') || value.starts_with('*') {
            value.clone()
        } else {
            format!("minecraft:{value}")
        };
        f.insert(key.to_owned(), Value::String(v));
        i += 2;
    }
    if f.is_empty() && !every {
        return err(at, "say which recipes: output|input|id|type <value>");
    }
    Ok(f)
}

fn parse(text: &str, source: &str, stem: &str, pack: &mut Pack) -> Result<(), ScriptError> {
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let at = format!("{source}:{}", i + 1);
        let raw = strip_comment(lines[i]).trim();
        i += 1;
        if raw.is_empty() {
            continue;
        }
        let w = words(raw, &at)?;
        let mut verb = w[0].to_lowercase();
        let mut rest: Vec<String> = w[1..].to_vec();
        let mut fixed_id: Option<String> = None;
        if verb == "override" {
            // Native: the new recipe takes the original's id, so it stands in its place.
            if rest.len() < 2 {
                return err(&at, "override <recipe id> shaped|shapeless|smelting|... (a recipe follows)");
            }
            let id = if rest[0].contains(':') { rest[0].clone() } else { format!("minecraft:{}", rest[0]) };
            fixed_id = Some(id);
            verb = rest[1].to_lowercase();
            rest = rest[2..].to_vec();
            if !matches!(verb.as_str(), "shaped" | "shapeless" | "stonecutting" | "smelting" | "blasting" | "smoking" | "campfire") {
                return err(&at, "override <recipe id> shaped|shapeless|smelting|... (a recipe follows)");
            }
        }
        let name_recipe = |pack: &mut Pack, out: &str, given: Option<&String>| -> String {
            if let Some(id) = &fixed_id {
                return id.clone();
            }
            if let Some(g) = given {
                return if g.contains(':') { g.clone() } else { format!("{NS}:{g}") };
            }
            pack.counter += 1;
            let tail = out.rsplit(':').next().unwrap_or(out).replace('/', "_");
            format!("{NS}:{stem}/{:03}_{tail}", pack.counter)
        };
        match verb.as_str() {
            // The content-mod tool's header and authors; the native mod has no use for them.
            "pack" | "author" => {}
            "remove" => {
                let mut f = select(&rest, &at, false)?;
                f.insert("remove".into(), Value::Bool(true));
                pack.filters.push(Value::Object(f));
            }
            "replace" => {
                if rest.len() < 4 || !matches!(rest[0].as_str(), "input" | "output") || rest[2] != "with" {
                    return err(&at, "replace input|output <from> with <to> [where ...]");
                }
                let mut f = select(&rest[4..], &at, true)?;
                f.insert(format!("replace_{}", rest[0]), json!({ "from": item(&rest[1], &at)?, "to": item(&rest[3], &at)? }));
                pack.filters.push(Value::Object(f));
            }
            "unify" => {
                if rest.len() < 3 || rest[1] != "->" {
                    return err(&at, "unify <theirs> -> <ours> [as #tag]");
                }
                let (theirs, ours) = (item(&rest[0], &at)?, item(&rest[2], &at)?);
                let accept = if rest.len() > 4 && rest[3] == "as" { item(&rest[4], &at)? } else { ours.clone() };
                pack.filters.push(json!({ "replace_output": { "from": theirs, "to": ours } }));
                pack.filters.push(json!({ "replace_input": { "from": theirs, "to": accept } }));
            }
            "shaped" => {
                let (rest, n) = count(&rest);
                let Some(first) = rest.first() else { return err(&at, "shaped <result> [xN]") };
                let out = item(first, &at)?;
                let o = options(&rest[1..], &at, &["as", "category"])?;
                let mut pattern: Vec<String> = Vec::new();
                let mut key = Map::new();
                let mut closed = false;
                let mut at_body = at.clone();
                while i < lines.len() {
                    at_body = format!("{source}:{}", i + 1);
                    let body = strip_comment(lines[i]).trim();
                    i += 1;
                    if body.is_empty() {
                        continue;
                    }
                    if body == "end" {
                        closed = true;
                        break;
                    }
                    if body.starts_with('"') || body.starts_with('\'') {
                        pattern.push(words(body, &at_body)?.into_iter().next().unwrap_or_default());
                    } else if let Some((k, v)) = body.split_once('=') {
                        let k = k.trim();
                        if k.chars().count() != 1 {
                            return err(&at_body, "a key is one character");
                        }
                        key.insert(k.to_owned(), Value::String(item(v.trim(), &at_body)?));
                    } else {
                        return err(&at_body, "inside shaped: \"row\", K = item, or end");
                    }
                }
                if !closed {
                    return err(&at_body, "shaped without end");
                }
                let width = pattern.first().map(|r| r.chars().count()).unwrap_or(0);
                if pattern.is_empty() || pattern.len() > 3 || width == 0 || width > 3 || pattern.iter().any(|r| r.chars().count() != width) {
                    return err(&at_body, "the pattern is 1 to 3 rows of the same width (1 to 3)");
                }
                let mut missing: Vec<String> = pattern.iter().flat_map(|r| r.chars()).filter(|c| *c != ' ' && !key.contains_key(&c.to_string())).map(|c| c.to_string()).collect();
                missing.sort();
                missing.dedup();
                if !missing.is_empty() {
                    return err(&at_body, format!("no item for {}", missing.join(", ")));
                }
                let id = name_recipe(pack, &out, o.map.get("as"));
                let mut r = json!({ "type": "minecraft:crafting_shaped", "category": o.map.get("category").cloned().unwrap_or("misc".into()),
                    "key": key, "pattern": pattern, "result": result(&out, n) });
                conditions(&mut r, &o);
                pack.recipes.insert(id, r);
            }
            "shapeless" => {
                let Some(eq) = rest.iter().position(|t| t == "=") else {
                    return err(&at, "shapeless <result> [xN] = item, item, ...");
                };
                let (head, n) = count(&rest[..eq]);
                let tail = &rest[eq + 1..];
                let stop = tail.iter().position(|t| t == "if" || t == "as").unwrap_or(tail.len());
                let o = options(&tail[stop..], &at, &["as"])?;
                let mut ingredients: Vec<String> = Vec::new();
                for w in tail[..stop].iter().flat_map(|t| t.split(',')).map(str::trim).filter(|w| !w.is_empty()) {
                    if let Some(k) = times(w) {
                        let Some(last) = ingredients.last().cloned() else {
                            return err(&at, format!("`{w}` repeats the ingredient before it; there is none"));
                        };
                        // A recipe holds 9 at most: refuse a repeat past that before making the copies.
                        if k == 0 || ingredients.len() + (k as usize - 1) > 9 {
                            return err(&at, "a result and 1 to 9 ingredients");
                        }
                        for _ in 1..k {
                            ingredients.push(last.clone());
                        }
                    } else {
                        ingredients.push(item(w, &at)?);
                    }
                }
                if head.is_empty() || ingredients.is_empty() || ingredients.len() > 9 {
                    return err(&at, "a result and 1 to 9 ingredients");
                }
                let out = item(&head[0], &at)?;
                let id = name_recipe(pack, &out, o.map.get("as"));
                let mut r = json!({ "type": "minecraft:crafting_shapeless", "category": "misc", "ingredients": ingredients, "result": result(&out, n) });
                conditions(&mut r, &o);
                pack.recipes.insert(id, r);
            }
            "smelting" | "blasting" | "smoking" | "campfire" | "stonecutting" => {
                let (rest, n) = count(&rest);
                if rest.len() < 3 || rest[1] != "->" {
                    return err(&at, format!("{verb} <input> -> <result> [xN] [xp F] [time T]"));
                }
                let (src, out) = (item(&rest[0], &at)?, item(&rest[2], &at)?);
                let o = options(&rest[3..], &at, &["xp", "time", "as"])?;
                let id = name_recipe(pack, &out, o.map.get("as"));
                let mut r = if verb == "stonecutting" {
                    json!({ "type": "minecraft:stonecutting", "ingredient": src, "result": result(&out, n) })
                } else {
                    let (kind, default) = match verb.as_str() {
                        "smelting" => ("smelting", 200),
                        "blasting" => ("blasting", 100),
                        "smoking" => ("smoking", 100),
                        _ => ("campfire_cooking", 600),
                    };
                    let xp: f64 = match o.map.get("xp").map(|v| v.parse()) {
                        None => 0.1,
                        Some(Ok(v)) => v,
                        Some(Err(_)) => return err(&at, "xp is a number"),
                    };
                    let time: u32 = match o.map.get("time").map(|v| v.parse()) {
                        None => default,
                        Some(Ok(v)) => v,
                        Some(Err(_)) => return err(&at, "time is a whole number of ticks"),
                    };
                    json!({ "type": format!("minecraft:{kind}"), "category": "misc", "ingredient": src, "result": result(&out, n),
                        "experience": xp, "cookingtime": time })
                };
                conditions(&mut r, &o);
                pack.recipes.insert(id, r);
            }
            "tag" => {
                if rest.len() < 4 || !matches!(rest[0].as_str(), "item" | "block" | "fluid" | "entity_type") || rest[2] != "add" {
                    return err(&at, "tag item|block <tag> add <id> [<id>...]");
                }
                let tag = rest[1].trim_start_matches('#');
                let tag = if tag.contains(':') { tag.to_owned() } else { format!("minecraft:{tag}") };
                let values = rest[3..].iter().map(|t| item(t, &at)).collect::<Result<Vec<_>, _>>()?;
                pack.tags.entry((rest[0].clone(), tag)).or_default().extend(values);
            }
            "name" => pack.notes.push(ScriptError { at: at.clone(), message: "left out: `name` is for the content-mod tool (vasttweaks.py) for now".into() }),
            _ => return err(&at, format!("unknown statement `{}`", w[0])),
        }
    }
    Ok(())
}
