//! Vast Tweaks: recipes and tags changed by small scripts. The scripts in `config/vasttweaks/` are compiled into
//! data pack files each time a world loads its data (opening it, `/reload`); with scripts in play, Horizon clients
//! get the server's recipes of the game's own types, so their recipe book and viewers (JEI) show the changes.

mod client;
pub mod script;

use mod_api::{self, DataPackFilesEvent, DatapackSyncEvent, Hooks, ServerHost};

#[derive(Default)]
struct VastTweaks;

#[mod_api::horizon_mod]
impl Hooks for VastTweaks {
    fn data_pack_files(&self, host: &mut dyn ServerHost, event: &mut DataPackFilesEvent<'_>) {
        client::data_pack_files(&host.instance_dir().join("config").join("vasttweaks"), event.files);
    }

    fn datapack_sync(&self, _host: &mut dyn ServerHost, event: &mut DatapackSyncEvent<'_>) {
        client::datapack_sync(event.recipe_types);
    }
}

#[cfg(test)]
mod tests {
    use crate::script::compile;
    use mod_api::serde_json::{Value, json};

    fn files(text: &str) -> Vec<(String, Value)> {
        let (c, errors) = compile(&[("pack.vt".into(), text.into())]);
        assert!(errors.is_empty(), "{errors:?}");
        c.files
    }

    fn get<'a>(f: &'a [(String, Value)], key: &str) -> &'a Value {
        &f.iter().find(|(k, _)| k == key).unwrap_or_else(|| panic!("no {key} in {:?}", f.iter().map(|x| &x.0).collect::<Vec<_>>())).1
    }

    #[test]
    fn filters_from_remove_replace_and_unify() {
        let f = files("remove output iron_pickaxe\nremove id minecraft:*_bed where type crafting_shaped\n\
                       replace input stick with vastores:steel_rod where id minecraft:*_pickaxe // swap\n\
                       unify voltz:steel_ingot -> vastores:steel_ingot as #c:ingots/steel\n");
        assert_eq!(get(&f, "recipe_filter/vasttweaks:scripts")["filters"], json!([
            {"output": "minecraft:iron_pickaxe", "remove": true},
            {"id": "minecraft:*_bed", "type": "minecraft:crafting_shaped", "remove": true},
            {"id": "minecraft:*_pickaxe", "replace_input": {"from": "minecraft:stick", "to": "vastores:steel_rod"}},
            {"replace_output": {"from": "voltz:steel_ingot", "to": "vastores:steel_ingot"}},
            {"replace_input": {"from": "voltz:steel_ingot", "to": "#c:ingots/steel"}},
        ]));
    }

    #[test]
    fn recipes() {
        let f = files("shaped saddle as saddle\n  \"LLL\"\n  \"S S\"\n  L = leather\n  S = string\nend\n\
                       shapeless string x4 = #minecraft:wool if mod vastores\n\
                       smelting rotten_flesh -> leather xp 0.3\n\
                       stonecutting stone -> stone_bricks\n\
                       override furnace shapeless furnace = #minecraft:stone_crafting_materials x8\n");
        assert_eq!(get(&f, "recipe/vasttweaks:saddle")["pattern"], json!(["LLL", "S S"]));
        let s = get(&f, "recipe/vasttweaks:pack/001_string");
        assert_eq!(s["result"], json!({"id": "minecraft:string", "count": 4}));
        assert_eq!(s["neoforge:conditions"][0]["modid"], "vastores");
        assert_eq!(get(&f, "recipe/vasttweaks:pack/002_leather")["experience"], json!(0.3));
        assert_eq!(get(&f, "recipe/vasttweaks:pack/003_stone_bricks")["type"], "minecraft:stonecutting");
        // An override takes the original's id, so it stands in its place.
        assert_eq!(get(&f, "recipe/minecraft:furnace")["ingredients"].as_array().unwrap().len(), 8);
    }

    #[test]
    fn tags() {
        let f = files("tag item c:ingots/tin add voltz:tin_ingot othermod:tin\n");
        assert_eq!(get(&f, "tags/c:item/ingots/tin"), &json!({"replace": false, "values": ["othermod:tin", "voltz:tin_ingot"]}));
    }

    #[test]
    fn a_broken_script_is_left_out_and_says_where() {
        let (c, errors) = compile(&[
            ("good.vt".into(), "remove output stick\n".into()),
            ("bad.vt".into(), "remove output dirt\nfrobnicate stick\n".into()),
        ]);
        assert_eq!(c.filters, 1);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].at, "bad.vt:2");
        let (_, errors) = compile(&[("x.vt".into(), "shaped stone\n  \"AB\"\n  A = dirt\nend\n".into())]);
        assert!(errors[0].message.contains("no item for B"));
        let (_, errors) = compile(&[("x.vt".into(), "remove\n".into())]);
        assert!(errors[0].message.contains("say which recipes"));
    }

    #[test]
    fn names_are_noted_not_fatal() {
        let (c, errors) = compile(&[("x.vt".into(), "name stick \"Twig\"\nremove output stick\n".into())]);
        assert_eq!(c.filters, 1);
        assert!(errors[0].message.starts_with("left out"));
    }

    #[test]
    fn same_language_as_the_tool() {
        // The example the content-mod tool ships, less its `name` line, compiles here too.
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../examples/example.vt")).unwrap();
        let (c, errors) = compile(&[("example.vt".into(), text)]);
        assert!(errors.iter().all(|e| e.message.starts_with("left out")), "{errors:?}");
        assert!(c.recipes >= 6 && c.filters >= 5);
    }
}
