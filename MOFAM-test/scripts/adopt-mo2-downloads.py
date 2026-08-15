#!/usr/bin/env python3

import argparse
import json
import os
import subprocess
from urllib.parse import parse_qs, unquote, urlparse
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Rename/link MO2 downloads into mudcrab cache names using a personalized plan."
    )
    parser.add_argument("plan", type=Path, help="Path to personalized plan JSON")
    parser.add_argument("downloads_dir", type=Path, help="MO2 downloads directory")
    parser.add_argument(
        "--apply",
        action="store_true",
        help="Apply changes. Without this flag, only print the planned operations.",
    )
    parser.add_argument(
        "--open-missing-urls",
        action="store_true",
        help="Open unresolved non-Nexus HTTP(S) source URLs in browser via xdg-open.",
    )
    return parser.parse_args()


def sanitize_tail(source: str) -> str:
    tail = source.rsplit("/", 1)[-1]
    tail = tail.split("?", 1)[0]
    out = []
    for char in tail:
        if char.isascii() and (char.isalnum() or char in ".-_"):
            out.append(char)
        else:
            out.append("_")
    return "".join(out)


def cache_file_name(mod_id: str, archive_index: int, source: str) -> str:
    return f"{mod_id}_{archive_index}_{sanitize_tail(source)}"


def extension_suffix(filename: str) -> str:
    """Return full archive suffix, preserving multi-part suffixes like .tar.gz."""
    suffixes = Path(filename).suffixes
    if not suffixes:
        return ""
    return "".join(suffixes)


def cache_file_name_for_source(base_cache_name: str, source_filename: str) -> str:
    """
    Keep an existing cache suffix if present; otherwise append suffix from the source file.
    This mirrors mudcrab's extension-aware cache behavior.
    """
    if Path(base_cache_name).suffixes:
        return base_cache_name

    source_suffix = extension_suffix(source_filename)
    if not source_suffix:
        return base_cache_name
    return f"{base_cache_name}{source_suffix}"


def parse_nexus_spec(source: str):
    if not source.startswith("nexus:"):
        return None
    value = source[len("nexus:") :]
    parts = value.split("/")
    if len(parts) != 3:
        return None
    game, mod_id, file_id = parts
    return (game.lower(), mod_id, file_id)


def source_filename_hint(source: str):
    tail = source.rsplit("/", 1)[-1]
    tail = tail.split("?", 1)[0]
    if not tail or "." not in tail:
        return None
    return tail


def tokenise(value: str):
    out = []
    current = []
    for ch in value.lower():
        if ch.isalnum():
            current.append(ch)
        else:
            if current:
                out.append("".join(current))
                current = []
    if current:
        out.append("".join(current))
    return out


def non_nexus_url_token_hints(source: str):
    """Extract useful token hints from non-Nexus URLs (including ModDB referer slugs)."""
    hints = []
    try:
        parsed = urlparse(source)
    except Exception:
        return hints

    # Tokens from path itself
    path_tail = parsed.path.rsplit("/", 1)[-1]
    path_tokens = [t for t in tokenise(path_tail) if len(t) >= 4]
    if path_tokens:
        hints.append(path_tokens)

    # Tokens from common nested URL params (e.g., ModDB `referer=.../mods/<slug>/downloads`)
    query = parse_qs(parsed.query)
    for key in ["referer", "ref", "url"]:
        for raw in query.get(key, []):
            decoded = unquote(raw)
            nested = urlparse(decoded)
            parts = [p for p in nested.path.split("/") if p]
            if "mods" in parts:
                i = parts.index("mods")
                if i + 1 < len(parts):
                    slug = parts[i + 1]
                    slug_tokens = [t for t in tokenise(slug) if len(t) >= 4]
                    if slug_tokens:
                        hints.append(slug_tokens)

    return hints


def parse_meta(path: Path):
    data = {}
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        if "=" not in line:
            continue
        key, value = line.split("=", 1)
        data[key.strip()] = value.strip().strip('"')

    game = data.get("gameName", "").lower()
    mod_id = data.get("modID", "")
    file_id = data.get("fileID", "")
    if not game or not mod_id or not file_id:
        return None

    # MO2 placeholder metadata for manually added files commonly uses id 0/1.
    # Treat these as invalid for Nexus-key matching so non-Nexus fallbacks can run.
    try:
        if int(mod_id) <= 1 or int(file_id) <= 0:
            return None
    except ValueError:
        return None

    return (game, mod_id, file_id)


def resolve_non_nexus_targets(entry: Path, targets, fulfilled):
    target_names = targets["filename"].get(entry.name.lower())
    if target_names:
        return target_names

    entry_tokens = set(tokenise(entry.name))
    fuzzy_candidates = []
    for base_name in targets["all_base"]:
        if base_name in fulfilled:
            continue
        hint_groups = targets["non_nexus_url_hints"].get(base_name, [])
        if not hint_groups:
            continue
        for group in hint_groups:
            if group and all(token in entry_tokens for token in group):
                fuzzy_candidates.append(base_name)
                break

    if len(fuzzy_candidates) == 1:
        return fuzzy_candidates
    return None


def build_targets(plan_path: Path):
    plan = json.loads(plan_path.read_text(encoding="utf-8"))
    nexus_targets = {}
    filename_targets = {}
    all_base_targets = set()
    non_nexus_http_sources = {}
    non_nexus_url_hints = {}

    for mod in plan.get("mods", []):
        mod_id = mod["id"]
        for archive_index, archive in enumerate(mod.get("archives", [])):
            source = archive["path"]
            target_base = cache_file_name(mod_id, archive_index, source)
            all_base_targets.add(target_base)

            nexus_key = parse_nexus_spec(source)
            if nexus_key is not None:
                nexus_targets.setdefault(nexus_key, []).append(target_base)
                continue

            filename_hint = source_filename_hint(source)
            if filename_hint is not None:
                filename_targets.setdefault(filename_hint.lower(), []).append(target_base)

            if source.startswith("http://") or source.startswith("https://"):
                non_nexus_http_sources[target_base] = source
                non_nexus_url_hints[target_base] = non_nexus_url_token_hints(source)

    return {
        "nexus": nexus_targets,
        "filename": filename_targets,
        "all_base": all_base_targets,
        "non_nexus_http_sources": non_nexus_http_sources,
        "non_nexus_url_hints": non_nexus_url_hints,
    }


def ensure_text_file(path: Path, content: str, apply: bool):
    if path.exists() and path.read_text(encoding="utf-8", errors="replace") == content:
        return
    print(f"WRITE {path.name}")
    if apply:
        path.write_text(content, encoding="utf-8")


def ensure_hardlink(source: Path, destination: Path, apply: bool):
    if destination.exists():
        return
    print(f"LINK  {destination.name} -> {source.name}")
    if apply:
        os.link(source, destination)


def rename_file(source: Path, destination: Path, apply: bool):
    if source == destination:
        return
    print(f"MOVE  {source.name} -> {destination.name}")
    if apply:
        source.rename(destination)


def adopt_downloads(targets, downloads_dir: Path, apply: bool, open_missing_urls: bool):
    stats = {
        "matched": 0,
        "renamed": 0,
        "linked": 0,
        "matched_non_nexus": 0,
        "skipped_no_meta": 0,
        "skipped_bad_meta": 0,
        "skipped_unmapped": 0,
        "skipped_duplicate_source": 0,
    }
    fulfilled = set()

    entries = sorted(downloads_dir.iterdir(), key=lambda entry: entry.name.lower())
    for entry in entries:
        if not entry.is_file():
            continue
        if entry.name.endswith(".meta") or entry.name.endswith(".orig-name"):
            continue

        meta_path = entry.with_name(entry.name + ".meta")
        target_names = None

        if meta_path.exists():
            nexus_key = parse_meta(meta_path)
            if nexus_key is not None:
                target_names = targets["nexus"].get(nexus_key)

            if not target_names:
                non_nexus = resolve_non_nexus_targets(entry, targets, fulfilled)
                if non_nexus:
                    target_names = non_nexus
                    stats["matched_non_nexus"] += 1
                else:
                    if nexus_key is None:
                        stats["skipped_bad_meta"] += 1
                    else:
                        stats["skipped_unmapped"] += 1
                    continue
        else:
            # Non-Nexus/manual sources can still be adopted by filename match.
            target_names = resolve_non_nexus_targets(entry, targets, fulfilled)

            if not target_names:
                stats["skipped_no_meta"] += 1
                continue
            stats["matched_non_nexus"] += 1
            # For non-Nexus entries, keep existing sidecar/meta behavior symmetrical.
            # If no .meta exists we can still adopt archive and .orig-name sidecars.
            meta_path = entry.with_name(entry.name + ".meta")

        remaining = [name for name in target_names if name not in fulfilled]
        if not remaining:
            stats["skipped_duplicate_source"] += 1
            continue

        stats["matched"] += 1
        original_name = entry.name

        primary_name_base = remaining[0]
        primary_name = cache_file_name_for_source(primary_name_base, entry.name)
        primary_path = downloads_dir / primary_name
        primary_meta = downloads_dir / f"{primary_name}.meta"
        primary_orig_name = downloads_dir / f"{primary_name}.orig-name"

        if entry.name != primary_name and not primary_path.exists():
            rename_file(entry, primary_path, apply)
            rename_file(meta_path, primary_meta, apply)
            entry = primary_path
            meta_path = primary_meta
            stats["renamed"] += 1
        else:
            if entry.name == primary_name and meta_path.name != primary_meta.name and not primary_meta.exists():
                rename_file(meta_path, primary_meta, apply)
                meta_path = primary_meta
            elif entry.name != primary_name and primary_path.exists():
                entry = primary_path
                meta_path = primary_meta if primary_meta.exists() else meta_path

        ensure_text_file(primary_orig_name, original_name + "\n", apply)
        fulfilled.add(primary_name_base)

        for alias_name_base in remaining[1:]:
            alias_name = cache_file_name_for_source(alias_name_base, entry.name)
            alias_path = downloads_dir / alias_name
            alias_meta = downloads_dir / f"{alias_name}.meta"
            alias_orig_name = downloads_dir / f"{alias_name}.orig-name"

            if not alias_path.exists():
                ensure_hardlink(entry, alias_path, apply)
                stats["linked"] += 1
            if meta_path.exists() and not alias_meta.exists():
                ensure_hardlink(meta_path, alias_meta, apply)
            ensure_text_file(alias_orig_name, original_name + "\n", apply)
            fulfilled.add(alias_name_base)

    print()
    print("Summary:")
    for key in [
        "matched",
        "renamed",
        "linked",
        "matched_non_nexus",
        "skipped_no_meta",
        "skipped_bad_meta",
        "skipped_unmapped",
        "skipped_duplicate_source",
    ]:
        print(f"  {key}: {stats[key]}")

    expected = len(targets["all_base"])
    print(f"  target_cache_entries: {expected}")
    print(f"  fulfilled_cache_entries: {len(fulfilled)}")

    missing = []
    for name in targets["all_base"]:
        if name not in fulfilled:
            missing.append(name)

    if missing:
        print()
        print("Missing target cache entries (base cache keys):")
        for name in sorted(missing):
            print(f"  {name} [extension resolved from matched source archive]")

    if open_missing_urls:
        unresolved_urls = []
        for base_key in sorted(missing):
            source = targets["non_nexus_http_sources"].get(base_key)
            if source:
                unresolved_urls.append(source)

        if unresolved_urls:
            print()
            print("Opening unresolved non-Nexus URLs in browser:")
            for url in unresolved_urls:
                print(f"  {url}")
                if apply:
                    subprocess.run(["xdg-open", url], check=False)


def main():
    args = parse_args()
    targets = build_targets(args.plan)
    adopt_downloads(targets, args.downloads_dir, args.apply, args.open_missing_urls)


if __name__ == "__main__":
    main()