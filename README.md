# Vast Tweaks

KubeJS-style recipe tweaks for [Horizon](https://playhorizon.dev) (Minecraft 26.3 rewritten in Rust), without
running any code in the game: you write a small script, and `vasttweaks.py` turns it into a **content mod** (data
only) that Horizon loads like any other.

```
python3 tools/vasttweaks.py build my_pack.vt --install     # build it and put it in Horizon's mods
python3 tools/vasttweaks.py build scripts/ --hmod          # every .vt of a folder, packed as dist/<id>-<version>.hmod
```

Then turn the pack on in the launcher (the first time) and restart Horizon. Python 3.9 or newer, nothing else.
`--install` only ever replaces a folder this tool made.

## The language

One statement a line; `//` starts a comment, and so does a line starting with `# `. An item is `mod:item`
(`minecraft:` may be left out: `stick`) and a tag is `#mod:tag`.

| Statement | What it does |
| --- | --- |
| `pack <id> "<name>" <version> ["description"]` | the mod the scripts become (default `vasttweaks`) |
| `author <name>...` | its authors |
| `remove output <item>` · `remove input <item/#tag>` · `remove id <recipe id>` · `remove type <type>` | takes recipes out; the keys combine (`remove id minecraft:*_bed where type crafting_shaped`) and `*` matches any run of characters |
| `replace input <from> with <to> [where ...]` | swaps an ingredient, in the recipes picked (`where id`, `type`, `input`, `output`) or in every recipe |
| `replace output <from> with <to> [where ...]` | swaps a result the same way |
| `unify <theirs> -> <ours> [as #tag]` | one mod's item for another's everywhere: what made theirs makes ours, and what asked for theirs takes the tag (or ours) |
| `shaped <result> [xN] [as <id>]` … `"row"` … `K = <item/#tag>` … `end` | a shaped crafting recipe, 1 to 3 rows |
| `shapeless <result> [xN] = a, b, #tag x3 [if mod <id>] [as <id>]` | a shapeless one; `x3` after an ingredient repeats it |
| `smelting` / `blasting` / `smoking` / `campfire <input> -> <result> [xN] [xp F] [time T]` | cooking (time in ticks; 200, 100, 100 and 600 by default) |
| `stonecutting <input> -> <result> [xN]` | the stonecutter |
| `override <recipe id> <shaped/shapeless/...>` | replaces another mod's recipe: the original is filtered out by id and the new one is added in the pack |
| `tag item <tag> add <item>...` | adds to an item tag (`block` too); tags only ever add |
| `name <item> "English" ["Português"]` · `name <item> pt "Texto"` | renames an item or block in English and Brazilian Portuguese |

`if mod <id>` keeps a new recipe only while that mod is installed (`neoforge:mod_loaded`). `as <id>` names a recipe
(else it gets `<pack>:tweaks/NNN_<result>`).

See [`examples/example.vt`](examples/example.vt) for every statement.

## How it works, and its limits

Removing and swapping use Horizon's own **recipe filters** (`data/<pack>/recipe_filter/`): they run after every
mod's recipes are loaded and reach all of them, other mods' machines included, in crafting, furnaces, stations and
recipe viewers. New recipes, tags and names are ordinary data in the pack's own namespace.

- A content mod can't write into another mod's (or the game's) folders, so `override` is a filter plus a new
  recipe, not a file over the original.
- Filters see recipes written as data. A machine that works out its results in code, and loot (mob drops, ore
  drops), are out of their reach.
- Changes need a restart of Horizon (there is no `/reload` of a pack yet).

## Tests

`python3 -m unittest discover -s tests`

## License

MIT, see [LICENSE](LICENSE).
