# Vast Tweaks

KubeJS-style recipe tweaks for [Horizon](https://playhorizon.dev) (Minecraft 26.3 rewritten in Rust): write a small
script, and the game's recipes, tags and unification follow it.

## Using it in the game (the native mod)

1. Install **Vast Tweaks** from the launcher's mod browser (or install `vasttweaks-<version>.hmod`) and build
   natives in the launcher. It runs on Horizon's mod API (1.1 was a `.hnmod`, which launchers from 0.2.70 no longer
   load: remove it).
2. Open a world once: Horizon gets a folder **`config/vasttweaks/`** with a `README.txt` and an example
   (`example.vt.off`, which isn't loaded).
3. Write your scripts there as **`<anything>.vt`** files, then run **`/reload`** (or open the world again). The
   recipe book and JEI show the changes too.
4. **`config/vasttweaks/last-load.txt`** says what was loaded and lists any mistake with its file and line; a script
   with a mistake is left out whole, the others still count.

It works on a server too: the server's scripts apply, and Horizon players see its recipes in their recipe book and
JEI. Everything but `name` works in the mod (renaming is for the tool below, for now).

## Making a content mod instead (the tool)

`tools/vasttweaks.py` compiles the same scripts into a **content mod** you can share or publish (that's how
[Vast Unify](https://github.com/voltzz900/vast-unify) is made), and it can also rename items:

```
python3 tools/vasttweaks.py build my_pack.vt --install     # build it and put it in Horizon's mods
python3 tools/vasttweaks.py build scripts/ --hmod          # every .vt of a folder, packed as dist/<id>-<version>.hmod
```

Then turn the pack on in the launcher (the first time) and restart Horizon. Python 3.9 or newer, nothing else.
`--install` only ever replaces a folder this tool made. A content mod can't write into another mod's folders, so
there `override` is a filter on the original plus a new recipe; in the native mod the new recipe takes the
original's id.

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
- The content mod needs a restart of Horizon to change; the native mod follows `/reload`.

## Tests

`python3 -m unittest discover -s tests` (the tool) and `cargo test` in `mod/` (the native mod, a crate on
Horizon's mod API; its script compiler is `mod/src/script.rs`). `tools/package.py` packs the native mod with
`cargo horizon-mod build`.

## License

MIT, see [LICENSE](LICENSE).
