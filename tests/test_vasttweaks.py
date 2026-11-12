import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "tools"))
import vasttweaks as vt  # noqa: E402


def build(text):
    return vt.parse(text, vt.Pack(), "test.vt")


class Language(unittest.TestCase):
    def test_pack_header_and_comments(self):
        p = build('pack mytweaks "My Tweaks" 2.0.0 "Mine"\n# a comment\n// another\nauthor me you\n')
        self.assertEqual((p.id, p.name, p.version, p.description, p.authors), ("mytweaks", "My Tweaks", "2.0.0", "Mine", ["me", "you"]))

    def test_remove_and_replace_make_filters(self):
        p = build("remove output iron_pickaxe\n"
                  "remove id minecraft:*_bed where type crafting_shaped\n"
                  "replace input stick with vastores:steel_rod where id minecraft:*_pickaxe // swap\n"
                  "replace output voltz:steel_ingot with vastores:steel_ingot\n")
        self.assertEqual(p.filters, [
            {"output": "minecraft:iron_pickaxe", "remove": True},
            {"id": "minecraft:*_bed", "type": "minecraft:crafting_shaped", "remove": True},
            {"id": "minecraft:*_pickaxe", "replace_input": {"from": "minecraft:stick", "to": "vastores:steel_rod"}},
            {"replace_output": {"from": "voltz:steel_ingot", "to": "vastores:steel_ingot"}},
        ])

    def test_unify_swaps_outputs_and_inputs(self):
        p = build("unify voltz:steel_ingot -> vastores:steel_ingot as #c:ingots/steel\n")
        self.assertEqual(p.filters, [
            {"replace_output": {"from": "voltz:steel_ingot", "to": "vastores:steel_ingot"}},
            {"replace_input": {"from": "voltz:steel_ingot", "to": "#c:ingots/steel"}},
        ])

    def test_shaped(self):
        p = build('shaped vastores:ore_hammer x2 as hammer\n  " I "\n  " SI"\n  "S  "\n  I = iron_ingot\n  S = #c:rods/wooden\nend\n')
        r = p.recipes["vasttweaks:hammer"]
        self.assertEqual(r["pattern"], [" I ", " SI", "S  "])
        self.assertEqual(r["key"], {"I": "minecraft:iron_ingot", "S": "#c:rods/wooden"})
        self.assertEqual(r["result"], {"id": "vastores:ore_hammer", "count": 2})

    def test_shaped_errors(self):
        with self.assertRaises(vt.ScriptError):
            build('shaped stone\n  "AB"\n  A = dirt\nend\n')  # B has no item
        with self.assertRaises(vt.ScriptError):
            build('shaped stone\n  "A"\n  A = dirt\n')  # no end

    def test_shapeless_with_condition(self):
        p = build("shapeless string x4 = #minecraft:wool, #minecraft:wool if mod vastores\n")
        (rid, r), = p.recipes.items()
        self.assertTrue(rid.startswith("vasttweaks:tweaks/001_string"))
        self.assertEqual(r["ingredients"], ["#minecraft:wool", "#minecraft:wool"])
        self.assertEqual(r["result"], {"id": "minecraft:string", "count": 4})
        self.assertEqual(r["neoforge:conditions"], [{"type": "neoforge:mod_loaded", "modid": "vastores"}])

    def test_shapeless_repeats(self):
        p = build("shapeless furnace = #minecraft:stone_crafting_materials x8\n")
        (r,) = p.recipes.values()
        self.assertEqual(r["ingredients"], ["#minecraft:stone_crafting_materials"] * 8)
        with self.assertRaises(vt.ScriptError):
            build("shapeless furnace = cobblestone x10\n")

    def test_cooking_and_stonecutting(self):
        p = build("smelting vastores:tin_dust -> vastores:tin_ingot xp 0.3\n"
                  "blasting raw_iron -> iron_ingot x2 time 50\n"
                  "stonecutting stone -> stone_bricks x1\n")
        a, b, c = p.recipes.values()
        self.assertEqual((a["type"], a["cookingtime"], a["experience"]), ("minecraft:smelting", 200, 0.3))
        self.assertEqual((b["type"], b["cookingtime"], b["result"]["count"]), ("minecraft:blasting", 50, 2))
        self.assertEqual(c["type"], "minecraft:stonecutting")

    def test_override_writes_over_the_mods_recipe(self):
        p = build("override ironfurnaces:iron_furnace shapeless ironfurnaces:iron_furnace = furnace, iron_block\n")
        # The original is filtered out by id and the new one lives in the pack's own namespace.
        self.assertEqual(p.filters, [{"id": "ironfurnaces:iron_furnace", "remove": True}])
        self.assertIn("vasttweaks:override/ironfurnaces/iron_furnace", p.recipes)

    def test_tags_and_names(self):
        p = build('tag item c:ingots/tin add voltz:tin_ingot othermod:tin\nname stick "Twig" "Graveto"\nname vastores:tin_ingot pt "Lingote"\n')
        self.assertEqual(p.tags[("item", "c:ingots/tin")], ["voltz:tin_ingot", "othermod:tin"])
        self.assertEqual(p.lang["en_us"]["item.minecraft.stick"], "Twig")
        self.assertEqual(p.lang["pt_br"]["block.minecraft.stick"], "Graveto")
        self.assertEqual(p.lang["pt_br"]["item.vastores.tin_ingot"], "Lingote")

    def test_unknown_statement_says_where(self):
        with self.assertRaisesRegex(vt.ScriptError, "test.vt:2"):
            build("remove output stick\nfrobnicate stick\n")


class Writing(unittest.TestCase):
    def test_writes_a_content_mod_and_keeps_foreign_folders(self):
        p = build('pack t "T" 1.0.0\nremove output stick\nshapeless dirt = sand\ntag item c:x add stick\nname stick "S"\n')
        with tempfile.TemporaryDirectory() as d:
            out, files = vt.write(p, d)
            self.assertTrue((out / "horizon-mod.toml").exists())
            f = json.loads((out / "data/t/recipe_filter/tweaks.json").read_text())
            self.assertEqual(f["filters"][0]["remove"], True)
            self.assertTrue((out / "data/c/tags/item/x.json").exists())
            self.assertTrue((out / "assets/t/lang/en_us.json").exists())
            vt.write(p, d)  # its own folder is rebuilt
            other = Path(d) / "other"
            other.mkdir()
            q = vt.Pack()
            q.id = "other"
            with self.assertRaises(vt.ScriptError):
                vt.write(q, d)  # a folder it didn't make is left alone


if __name__ == "__main__":
    unittest.main()
