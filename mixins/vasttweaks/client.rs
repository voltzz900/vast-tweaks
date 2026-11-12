// Vast Tweaks, the game's side: the scripts in `config/vasttweaks/` compiled into data pack files each time a world
// loads its data (opening it, `/reload`). Injected into horizon-client's game.rs as `vasttweaks_client`.

use super::vasttweaks_script as script;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

/// Whether this load's scripts changed anything: then the server sends its clients the recipes of the game's own
/// types, so their recipe book and viewers (JEI) show the scripts' changes too.
static CHANGED: AtomicBool = AtomicBool::new(false);

/// The recipe types a script most often touches, sent to Horizon clients when there are scripts.
const SYNCED_TYPES: [&str; 8] = [
    "minecraft:crafting_shaped",
    "minecraft:crafting_shapeless",
    "minecraft:smelting",
    "minecraft:blasting",
    "minecraft:smoking",
    "minecraft:campfire_cooking",
    "minecraft:stonecutting",
    "minecraft:smithing_transform",
];

/// At `native_datapack_sync`: with scripts in play, the clients get the server's recipes of those types.
pub(super) fn datapack_sync(recipe_types: &mut Vec<String>) {
    if CHANGED.load(Ordering::Relaxed) {
        for t in SYNCED_TYPES {
            if !recipe_types.iter().any(|r| r == t) {
                recipe_types.push(t.to_owned());
            }
        }
    }
}

const README: &str = "Vast Tweaks: put your scripts here, as <name>.vt files.\n\
They are read when a world opens and on /reload; last-load.txt says what was loaded and any mistakes.\n\
\n\
remove output <item>                         take out every recipe that makes it (also: input, id, type; `*` matches)\n\
replace input <from> with <to> [where id <recipe id with *>]\n\
replace output <from> with <to> [where ...]\n\
unify <theirs> -> <ours> [as #tag]           made as ours everywhere, asked for as the tag\n\
shaped <result> [xN] [as <id>]               then \"rows\", K = <item>, and end\n\
shapeless <result> [xN] = a, b, #tag x3 [if mod <id>]\n\
smelting|blasting|smoking|campfire <in> -> <out> [xN] [xp 0.5] [time 200]\n\
stonecutting <in> -> <out> [xN]\n\
override <recipe id> shaped|shapeless|...     a new recipe in the original's place\n\
tag item <tag> add <item>...\n\
\n\
An item is mod:item (minecraft: may be left out); a tag is #mod:tag. // starts a comment.\n\
Rename this folder's example.vt.off to example.vt to try it. More: https://github.com/voltzz900/vast-tweaks\n";

const EXAMPLE: &str = "// Rename this file to example.vt to load it.\n\
remove output minecraft:iron_pickaxe\n\
replace input stick with minecraft:bamboo where id minecraft:*_pickaxe\n\
shaped minecraft:saddle as saddle\n  \"LLL\"\n  \"S S\"\n  L = leather\n  S = string\nend\n\
shapeless minecraft:string x4 = #minecraft:wool\n\
smelting minecraft:rotten_flesh -> minecraft:leather xp 0.1\n\
override minecraft:furnace shapeless minecraft:furnace = #minecraft:stone_crafting_materials x8\n";

/// At `native_data_pack_files`: the scripts' files join this load.
pub(super) fn data_pack_files(files: &mut Vec<(String, Vec<u8>)>) {
    let dir = match super::Game::native_config_dir() {
        Ok(d) => d.join("vasttweaks"),
        Err(e) => {
            log::warn!("Vast Tweaks: no config folder: {e}");
            return;
        }
    };
    if !dir.exists() {
        // The first time: a folder with how to write scripts and an example that isn't loaded.
        let _ = fs::create_dir_all(&dir)
            .and_then(|()| fs::write(dir.join("README.txt"), README))
            .and_then(|()| fs::write(dir.join("example.vt.off"), EXAMPLE));
    }
    let scripts = read_scripts(&dir);
    let (compiled, errors) = script::compile(&scripts);
    CHANGED.store(!compiled.files.is_empty(), Ordering::Relaxed);
    for (key, value) in &compiled.files {
        match serde_json::to_vec(value) {
            Ok(bytes) => files.push((key.clone(), bytes)),
            Err(e) => log::warn!("Vast Tweaks: {key} not written: {e}"),
        }
    }
    let mut status = format!(
        "{} script(s): {} recipe filter(s), {} recipe(s), {} tag(s)\n",
        scripts.len(),
        compiled.filters,
        compiled.recipes,
        compiled.tags
    );
    for e in &errors {
        log::warn!("Vast Tweaks: {e}");
        status.push_str(&format!("{e}\n"));
    }
    if errors.iter().any(|e| !e.message.starts_with("left out")) {
        status.push_str("A script with a mistake is left out whole; fix it and /reload.\n");
    }
    log::info!("Vast Tweaks: {}", status.lines().next().unwrap_or_default());
    let _ = fs::write(dir.join("last-load.txt"), status);
}

/// Every `*.vt` in the folder, by name.
fn read_scripts(dir: &Path) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "vt"))
        .filter_map(|e| Some((e.file_name().to_string_lossy().into_owned(), fs::read_to_string(e.path()).ok()?)))
        .collect();
    out.sort();
    out
}
