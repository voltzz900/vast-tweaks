#!/usr/bin/env python3
"""Vast Tweaks: a small script language for a Horizon modpack's recipes, tags and names, compiled into a content
mod (data only, nothing to build in the game).

    python3 vasttweaks.py build my_pack.vt            writes build/<id>/ (a content mod folder)
    python3 vasttweaks.py build scripts/ --install    every *.vt of a folder, installed into Horizon's mods
    python3 vasttweaks.py build my_pack.vt --hmod     also packs dist/<id>-<version>.hmod

Removing and swapping recipes uses Horizon's own recipe filters (`data/<ns>/recipe_filter/`), which reach every
recipe of every mod after they are all loaded. New recipes, tags and names are ordinary data. See README.md for the
language.
"""
import argparse
import json
import shlex
import shutil
import sys
import zipfile
from pathlib import Path

HORIZON_MODS = Path.home() / "Library/Application Support/horizon/horizon-mods"
MARKER = "vasttweaks.json"  # in every mod this tool writes: --install replaces only a folder that has it
COOKING = {"smelting": 200, "blasting": 100, "smoking": 100, "campfire": 600}
LANGS = {"en": "en_us", "pt": "pt_br"}


class ScriptError(Exception):
    def __init__(self, where, message):
        super().__init__(f"{where}: {message}")


class Pack:
    """What the scripts say, before it is written as files."""

    def __init__(self):
        self.id, self.name, self.version = "vasttweaks", "Vast Tweaks", "1.0.0"
        self.description = "A modpack's recipe, tag and name tweaks, made with Vast Tweaks."
        self.authors = []
        self.filters = []          # recipe filters, in order
        self.recipes = {}          # recipe id -> json
        self.tags = {}             # (registry, tag) -> [ids]
        self.lang = {}             # lang code -> {key: text}
        self.counter = 0

    def recipe_id(self, result, given):
        if given:
            return given if ":" in given else f"{self.id}:{given}"
        self.counter += 1
        return f"{self.id}:tweaks/{self.counter:03d}_{result.split(':')[-1].replace('/', '_')}"


# ---- reading ---------------------------------------------------------------------------------

def strip_comment(line):
    """`//` starts a comment anywhere outside quotes; a line whose first word is `#` (a hash and a space) is one."""
    if line.lstrip().startswith("# ") or line.strip() == "#":
        return ""
    out, quote = [], None
    i = 0
    while i < len(line):
        c = line[i]
        if quote:
            if c == quote:
                quote = None
        elif c in "\"'":
            quote = c
        elif line.startswith("//", i):
            break
        out.append(c)
        i += 1
    return "".join(out)


def item(token, where):
    """An item or `#tag`; a bare name is the game's (`stick` = `minecraft:stick`)."""
    if not token or token in ("->", "with", "="):
        raise ScriptError(where, "an item or #tag is missing")
    tag = token.startswith("#")
    name = token[1:] if tag else token
    if ":" not in name:
        name = "minecraft:" + name
    return ("#" if tag else "") + name


def count(tokens, where):
    """A trailing `xN` (or `*N`) among `tokens`: (tokens without it, N)."""
    for i, t in enumerate(tokens):
        if len(t) > 1 and t[0] in "x*" and t[1:].isdigit():
            return tokens[:i] + tokens[i + 1:], int(t[1:])
    return tokens, 1


def options(tokens, where, allowed):
    """`key value` pairs after a statement: `where id ...`, `xp 0.7`, `as <id>`, `if mod <id>`."""
    out, i = {}, 0
    while i < len(tokens):
        key = tokens[i]
        if key == "where" and i + 2 < len(tokens):
            key, value, i = tokens[i + 1], tokens[i + 2], i + 3
        elif key == "if" and i + 2 < len(tokens) and tokens[i + 1] == "mod":
            out.setdefault("mods", []).append(tokens[i + 2])
            i += 3
            continue
        elif i + 1 < len(tokens):
            value, i = tokens[i + 1], i + 2
        else:
            raise ScriptError(where, f"`{key}` needs a value")
        if key not in allowed:
            raise ScriptError(where, f"unknown option `{key}` (allowed: {', '.join(sorted(allowed))})")
        out[key] = value
    return out


def conditions(opts):
    mods = opts.get("mods")
    if not mods:
        return {}
    conds = [{"type": "neoforge:mod_loaded", "modid": m} for m in mods]
    return {"neoforge:conditions": conds}


def result(id_, n):
    r = {"id": id_}
    if n != 1:
        r["count"] = n
    return r


def parse(text, pack, source="script"):
    lines = text.splitlines()
    i = 0
    while i < len(lines):
        where = f"{source}:{i + 1}"
        raw = strip_comment(lines[i]).strip()
        i += 1
        if not raw:
            continue
        try:
            words = shlex.split(raw)
        except ValueError as e:
            raise ScriptError(where, str(e)) from None
        verb, rest = words[0].lower(), words[1:]
        override = None
        if verb == "override":
            if not rest:
                raise ScriptError(where, "override needs a recipe id")
            override, verb, rest = rest[0], rest[1].lower() if len(rest) > 1 else "", rest[2:]
            if ":" not in override:
                override = "minecraft:" + override
            if verb not in ("shaped", "shapeless", "stonecutting", *COOKING):
                raise ScriptError(where, "override <recipe id> shaped|shapeless|smelting|... (a recipe follows)")
            # A content mod writes only in its own namespace: the original goes (a filter on its id) and the
            # new one comes in the pack's, as override/<namespace>/<path>.
            pack.filters.append({"id": override, "remove": True})
            ons, opath = override.split(":", 1)
            override = f"{pack.id}:override/{ons}/{opath}"
        if verb == "pack":
            # pack <id> "<name>" <version> ["description"]
            if len(rest) < 3:
                raise ScriptError(where, 'pack <id> "<name>" <version> ["description"]')
            pack.id, pack.name, pack.version = rest[0], rest[1], rest[2]
            if len(rest) > 3:
                pack.description = rest[3]
        elif verb == "author":
            pack.authors.extend(rest)
        elif verb == "remove":
            pack.filters.append(match(rest, where) | {"remove": True})
        elif verb == "replace":
            # replace input|output <from> with <to> [where id|type|input|output <x>]...
            if len(rest) < 4 or rest[0] not in ("input", "output") or rest[2] != "with":
                raise ScriptError(where, "replace input|output <from> with <to> [where ...]")
            f = match(rest[4:], where, every=True)
            f[f"replace_{rest[0]}"] = {"from": item(rest[1], where), "to": item(rest[3], where)}
            pack.filters.append(f)
        elif verb == "unify":
            # unify <theirs> -> <ours> [as #tag]: everything that made theirs makes ours; recipes that asked for
            # theirs take the tag (or ours).
            if len(rest) < 3 or rest[1] != "->":
                raise ScriptError(where, "unify <theirs> -> <ours> [as #tag]")
            theirs, ours = item(rest[0], where), item(rest[2], where)
            accept = item(rest[4], where) if len(rest) > 4 and rest[3] == "as" else ours
            pack.filters.append({"replace_output": {"from": theirs, "to": ours}})
            pack.filters.append({"replace_input": {"from": theirs, "to": accept}})
        elif verb == "shaped":
            rest, n = count(rest, where)
            if not rest:
                raise ScriptError(where, "shaped <result> [xN]")
            out, opts = item(rest[0], where), options(rest[1:], where, {"as", "category"})
            pattern, key = [], {}
            while i < len(lines):
                body = strip_comment(lines[i]).strip()
                where = f"{source}:{i + 1}"
                i += 1
                if not body:
                    continue
                if body == "end":
                    break
                if body.startswith('"') or body.startswith("'"):
                    pattern.append(shlex.split(body)[0])
                elif "=" in body:
                    k, v = (s.strip() for s in body.split("=", 1))
                    if len(k) != 1:
                        raise ScriptError(where, "a key is one character")
                    key[k] = item(v, where)
                else:
                    raise ScriptError(where, 'inside shaped: "row", K = item, or end')
            else:
                raise ScriptError(where, "shaped without end")
            if not pattern or len({len(r) for r in pattern}) != 1 or len(pattern) > 3 or len(pattern[0]) > 3:
                raise ScriptError(where, "the pattern is 1 to 3 rows of the same width (1 to 3)")
            missing = {c for r in pattern for c in r if c != " "} - set(key)
            if missing:
                raise ScriptError(where, f"no item for {', '.join(sorted(missing))}")
            rid = override or pack.recipe_id(out, opts.get("as"))
            pack.recipes[rid] = {"type": "minecraft:crafting_shaped", "category": opts.get("category", "misc"),
                                 "key": key, "pattern": pattern, "result": result(out, n)} | conditions(opts)
        elif verb == "shapeless":
            # shapeless <result> [xN] = a, b, #tag [if mod x]
            if "=" not in rest:
                raise ScriptError(where, "shapeless <result> [xN] = item, item, ...")
            at = rest.index("=")
            head, n = count(rest[:at], where)
            tail = rest[at + 1:]
            # The ingredients run up to `if` or `as`; commas between them are optional.
            stop = next((k for k, t in enumerate(tail) if t in ("if", "as")), len(tail))
            opts = options(tail[stop:], where, {"as"})
            words = [w.strip() for t in tail[:stop] for w in t.split(",") if w.strip()]
            ingredients = []
            for w in words:
                # `item x8`: the ingredient before it, eight times.
                if len(w) > 1 and w[0] == "x" and w[1:].isdigit():
                    if not ingredients:
                        raise ScriptError(where, f"`{w}` repeats the ingredient before it; there is none")
                    ingredients += [ingredients[-1]] * (int(w[1:]) - 1)
                else:
                    ingredients.append(item(w, where))
            if not head or not 1 <= len(ingredients) <= 9:
                raise ScriptError(where, "a result and 1 to 9 ingredients")
            out = item(head[0], where)
            rid = override or pack.recipe_id(out, opts.get("as"))
            pack.recipes[rid] = {"type": "minecraft:crafting_shapeless", "category": "misc",
                                 "ingredients": ingredients, "result": result(out, n)} | conditions(opts)
        elif verb in COOKING or verb == "stonecutting":
            # smelting <input> -> <result> [xN] [xp F] [time T] [if mod x]
            rest, n = count(rest, where)
            if len(rest) < 3 or rest[1] != "->":
                raise ScriptError(where, f"{verb} <input> -> <result> [xN] [xp F] [time T]")
            src, out = item(rest[0], where), item(rest[2], where)
            opts = options(rest[3:], where, {"xp", "time", "as"})
            rid = override or pack.recipe_id(out, opts.get("as"))
            if verb == "stonecutting":
                pack.recipes[rid] = {"type": "minecraft:stonecutting", "ingredient": src, "result": result(out, n)}
            else:
                kind = "campfire_cooking" if verb == "campfire" else verb
                pack.recipes[rid] = {"type": f"minecraft:{kind}", "category": "misc", "ingredient": src,
                                     "result": result(out, n), "experience": float(opts.get("xp", 0.1)),
                                     "cookingtime": int(opts.get("time", COOKING[verb]))}
            pack.recipes[rid] |= conditions(opts)
        elif verb == "tag":
            # tag item|block <tag> add <ids...>
            if len(rest) < 4 or rest[0] not in ("item", "block", "fluid", "entity_type") or rest[2] != "add":
                raise ScriptError(where, "tag item|block <tag> add <id> [<id>...]")
            tag = rest[1].lstrip("#")
            if ":" not in tag:
                tag = "minecraft:" + tag
            pack.tags.setdefault((rest[0], tag), []).extend(item(t, where) for t in rest[3:])
        elif verb == "name":
            # name <item> "English" ["Português"]  or  name <item> pt "Texto"
            if len(rest) < 2:
                raise ScriptError(where, 'name <item> "English" ["Português"]')
            target = item(rest[0], where)
            if target.startswith("#"):
                raise ScriptError(where, "a name is for an item, not a tag")
            texts = rest[1:]
            pairs = []
            if texts[0] in LANGS and len(texts) >= 2:
                pairs = [(LANGS[texts[0]], texts[1])]
            else:
                pairs = list(zip(("en_us", "pt_br"), texts))
            ns, path = target.split(":", 1)
            for lang, text_ in pairs:
                for kind in ("item", "block"):
                    pack.lang.setdefault(lang, {})[f"{kind}.{ns}.{path.replace('/', '.')}"] = text_
        else:
            raise ScriptError(where, f"unknown statement `{words[0]}`")
    return pack


def match(tokens, where, every=False):
    """`remove`'s and `replace`'s selection: `output X`, `input X`, `id P`, `type P`, each optionally after `where`.
    None selects every recipe: allowed for a swap (`every`), never for `remove`."""
    f, i = {}, 0
    while i < len(tokens):
        key = tokens[i]
        if key in ("where", "and"):
            i += 1
            continue
        if key not in ("id", "type", "input", "output") or i + 1 >= len(tokens):
            raise ScriptError(where, f"expected id|type|input|output <value>, got `{key}`")
        value = tokens[i + 1]
        f[key] = item(value, where) if key in ("input", "output") else (value if ":" in value or "*" == value[0] else "minecraft:" + value)
        i += 2
    if not f and not every:
        raise ScriptError(where, "say which recipes: output|input|id|type <value>")
    return f


# ---- writing ---------------------------------------------------------------------------------

def write(pack, out_root):
    out = Path(out_root) / pack.id
    if out.exists():
        if not (out / MARKER).exists():
            raise ScriptError(str(out), "is not a folder this tool made; it is left alone")
        shutil.rmtree(out)
    files = {}
    if pack.filters:
        files[f"data/{pack.id}/recipe_filter/tweaks.json"] = {"filters": pack.filters}
    for rid, recipe in pack.recipes.items():
        ns, path = rid.split(":", 1)
        files[f"data/{ns}/recipe/{path}.json"] = recipe
    for (registry, tag), values in pack.tags.items():
        ns, path = tag.split(":", 1)
        files[f"data/{ns}/tags/{registry}/{path}.json"] = {"replace": False, "values": sorted(set(values))}
    for lang, entries in pack.lang.items():
        files[f"assets/{pack.id}/lang/{lang}.json"] = dict(sorted(entries.items()))
    files[MARKER] = {"made_by": "Vast Tweaks", "note": "rebuilt from scripts; edits here are lost"}
    for path, value in files.items():
        target = out / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n")
    authors = ", ".join(json.dumps(a) for a in pack.authors)
    caps = '["registries"]'
    (out / "horizon-mod.toml").write_text(
        f'id = "{pack.id}"\nname = {json.dumps(pack.name)}\nversion = "{pack.version}"\n'
        f"description = {json.dumps(pack.description)}\nauthors = [{authors}]\n"
        f'api = "0.2"\nside = "both"\ncapabilities = {caps}\n')
    return out, files


def pack_hmod(folder, dist, extra=()):
    dist.mkdir(parents=True, exist_ok=True)
    version = next(l.split('"')[1] for l in (folder / "horizon-mod.toml").read_text().splitlines() if l.startswith("version"))
    target = dist / f"{folder.name}-{version}.hmod"
    with zipfile.ZipFile(target, "w", zipfile.ZIP_DEFLATED) as z:
        for f in sorted(p for p in folder.rglob("*") if p.is_file()):
            info = zipfile.ZipInfo(f.relative_to(folder).as_posix(), (2026, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            z.writestr(info, f.read_bytes())
        for name, path in extra:
            info = zipfile.ZipInfo(name, (2026, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            z.writestr(info, Path(path).read_bytes())
    return target


def main(argv=None):
    ap = argparse.ArgumentParser(description="Compile Vast Tweaks scripts (*.vt) into a Horizon content mod.")
    sub = ap.add_subparsers(dest="cmd", required=True)
    b = sub.add_parser("build", help="compile scripts into a content mod")
    b.add_argument("scripts", nargs="+", help=".vt files or folders of them")
    b.add_argument("--out", default="build", help="where the mod folder goes (default: build)")
    b.add_argument("--install", action="store_true", help="copy it into Horizon's mods folder")
    b.add_argument("--hmod", action="store_true", help="also pack dist/<id>-<version>.hmod")
    args = ap.parse_args(argv)
    pack = Pack()
    sources = []
    for s in args.scripts:
        p = Path(s)
        sources += sorted(p.glob("*.vt")) if p.is_dir() else [p]
    try:
        for p in sources:
            parse(p.read_text(encoding="utf-8"), pack, p.name)
    except ScriptError as e:
        print(f"error: {e}", file=sys.stderr)
        return 1
    try:
        folder, files = write(pack, args.out)
    except ScriptError as e:
        print(f"error: {e}", file=sys.stderr)
        return 1
    print(f"{pack.id} {pack.version}: {len(pack.filters)} filters, {len(pack.recipes)} recipes, "
          f"{len(pack.tags)} tags, {sum(len(v) for v in pack.lang.values())} names -> {folder}")
    if args.hmod:
        print(pack_hmod(folder, Path("dist")))
    if args.install:
        dest = HORIZON_MODS / pack.id
        if dest.exists():
            if not (dest / MARKER).exists():
                print(f"error: {dest} is a mod this tool didn't make; pick another pack id", file=sys.stderr)
                return 1
            shutil.rmtree(dest)
        shutil.copytree(folder, dest)
        print(f"installed in {dest} (enable it in the launcher if it is new, then restart Horizon)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
