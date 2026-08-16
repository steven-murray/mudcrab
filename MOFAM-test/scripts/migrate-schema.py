#!/usr/bin/env python3
"""Migrate a modlist TOML from nested `[modlist.A.B]` tables to flat `[[mods]]`.

Line-based on purpose: mofam.full.toml carries source URLs, per-mod notes and
TODOs as comments, and a parse-and-reemit migration would silently drop all of
them. This rewrites only table headers and leaves every other line untouched.

    [modlist."SECTION"."Mod Name"]                 ->  [[mods]]
                                                       id = "Mod Name"
                                                       section = ["SECTION"]
    [[modlist."SECTION"."Mod Name".archives]]      ->  [[mods.archives]]
    [[modlist."SECTION"."Mod Name".archives.build]] -> [[mods.archives.build]]
    [[modlist."SECTION"."Mod Name".actions]]       ->  [[mods.actions]]

Usage: migrate-schema.py <in.toml> [-o out.toml]
"""

from __future__ import annotations

import argparse
import pathlib
import sys


def split_key_path(path: str) -> list[str]:
    """Split a TOML key path on unquoted dots, honouring quoted segments.

    Mod names contain dots, spaces, brackets and apostrophes, so a naive
    split('.') corrupts them.
    """
    parts, buf, in_quotes, quote = [], "", False, ""
    for ch in path:
        if in_quotes:
            buf += ch
            if ch == quote:
                in_quotes = False
        elif ch in "\"'":
            in_quotes, quote = True, ch
            buf += ch
        elif ch == ".":
            parts.append(buf)
            buf = ""
        else:
            buf += ch
    parts.append(buf)
    return [p.strip() for p in parts]


def unquote(seg: str) -> str:
    if len(seg) >= 2 and seg[0] == seg[-1] and seg[0] in "\"'":
        return seg[1:-1]
    return seg


def toml_str(value: str) -> str:
    return '"' + value.replace("\\", "\\\\").replace('"', '\\"') + '"'


def migrate(text: str) -> tuple[str, dict]:
    out: list[str] = []
    emitted: set[tuple[str, ...]] = set()
    stats = {"mods": 0, "sections": set(), "headers_rewritten": 0, "max_depth": 0}

    for line in text.split("\n"):
        stripped = line.strip()

        is_aot = stripped.startswith("[[") and stripped.endswith("]]")
        is_tbl = (
            not is_aot and stripped.startswith("[") and stripped.endswith("]")
        )
        inner = stripped[2:-2] if is_aot else stripped[1:-1] if is_tbl else None

        if inner is None or not (inner == "modlist" or inner.startswith("modlist.")):
            out.append(line)
            continue

        segments = split_key_path(inner)[1:]  # drop leading "modlist"
        if not segments:
            out.append(line)
            continue

        # Trailing known sub-tables (archives, actions, ...) are not part of the
        # section/mod path.
        SUBS = {"archives", "actions", "build", "fomod_selections"}
        head, tail = [], []
        for seg in segments:
            if tail or unquote(seg) in SUBS:
                tail.append(unquote(seg))
            else:
                head.append(seg)

        if not head:
            out.append(line)
            continue

        mod_name = unquote(head[-1])
        section_path = [unquote(s) for s in head[:-1]]
        key = tuple(head)
        stats["max_depth"] = max(stats["max_depth"], len(head))

        if key not in emitted:
            emitted.add(key)
            stats["mods"] += 1
            if section_path:
                stats["sections"].add(" - ".join(section_path))
            out.append("[[mods]]")
            out.append(f"id = {toml_str(mod_name)}")
            if section_path:
                # Always a list, including the single-level case: one shape to
                # read, write and test.
                joined = ", ".join(toml_str(s) for s in section_path)
                out.append(f"section = [{joined}]")
            if tail:
                # header was e.g. [[modlist.A.B.archives]] with no bare mod
                # table; emit the mod header then fall through to the sub-table
                out.append("")
                out.append(("[[mods." if is_aot else "[mods.") + ".".join(tail) + ("]]" if is_aot else "]"))
            stats["headers_rewritten"] += 1
            continue

        if not tail:
            # duplicate bare mod header; should not happen
            out.append(f"# WARNING: duplicate mod table for {mod_name}")
            continue

        out.append(("[[mods." if is_aot else "[mods.") + ".".join(tail) + ("]]" if is_aot else "]"))
        stats["headers_rewritten"] += 1

    return "\n".join(out), stats


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("input")
    ap.add_argument("-o", "--output")
    args = ap.parse_args()

    text = pathlib.Path(args.input).read_text()
    migrated, stats = migrate(text)

    if args.output:
        pathlib.Path(args.output).write_text(migrated)
    else:
        sys.stdout.write(migrated)

    print(
        f"# mods={stats['mods']} sections={len(stats['sections'])} "
        f"headers_rewritten={stats['headers_rewritten']} max_depth={stats['max_depth']}",
        file=sys.stderr,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
