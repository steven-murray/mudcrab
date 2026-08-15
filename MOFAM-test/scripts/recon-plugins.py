#!/usr/bin/env python3
"""M0 recon: survey TES4 plugins to scope the Rust merge implementation.

Throwaway-but-kept reconnaissance tool. Once src/plugin/ exists (M2/M3) the
`plugin-audit` Rust binary supersedes this for the authoritative gate; this
script exists to produce the initial worklist and to independently corroborate
the design's two load-bearing empirical claims:

  1. the (record, subrecord) matrix that the schema table must cover
  2. that Oblivion compiled script bytecode (SCDA) does NOT embed FormIDs
     inline -- it references them by index into the record's SCRO list

Usage:
    recon-plugins.py matrix <dir>...     # emit the (record, field, sizes) matrix
    recon-plugins.py scda   <dir>...     # SCDA inline-FormID detector
    recon-plugins.py types  <dir>...     # record type census
"""

from __future__ import annotations

import collections
import pathlib
import struct
import sys
import zlib

COMPRESSED = 0x0004_0000

# GRUP types whose label is a raw little-endian FormID (must be rewritten on merge).
FORMID_LABEL_GROUPS = {1, 6, 7, 8, 9, 10}


class Record:
    __slots__ = ("sig", "form_id", "flags", "fields")

    def __init__(self, sig: bytes, form_id: int, flags: int, fields: list):
        self.sig = sig
        self.form_id = form_id
        self.flags = flags
        self.fields = fields  # list[(sig, data)]


def parse_fields(body: bytes) -> list[tuple[bytes, bytes]]:
    """Split a record body into subrecords, honouring XXXX size overflow."""
    out, off, override = [], 0, None
    while off + 6 <= len(body):
        sig = body[off : off + 4]
        size = struct.unpack_from("<H", body, off + 4)[0]
        off += 6
        if sig == b"XXXX":
            # XXXX carries the real length of the *next* subrecord.
            override = struct.unpack_from("<I", body, off)[0]
            off += size
            continue
        if override is not None:
            size, override = override, None
        out.append((sig, body[off : off + size]))
        off += size
    return out


def walk(data: bytes, off: int, end: int, on_record, on_group=None) -> None:
    while off < end:
        sig = data[off : off + 4]
        if sig == b"GRUP":
            gsize = struct.unpack_from("<I", data, off + 4)[0]
            if on_group is not None:
                label = data[off + 8 : off + 12]
                gtype = struct.unpack_from("<i", data, off + 12)[0]
                on_group(gtype, label)
            walk(data, off + 20, off + gsize, on_record, on_group)
            off += gsize
            continue

        size, flags, form_id = struct.unpack_from("<IiI", data, off + 4)
        body = data[off + 20 : off + 20 + size]
        if flags & COMPRESSED:
            # u32 decompressed size, then a raw zlib stream
            body = zlib.decompress(body[4:])
        on_record(Record(sig, form_id, flags, parse_fields(body)))
        off += 20 + size


def load(path: pathlib.Path):
    """Yield every record in a plugin. Returns (masters, records)."""
    data = path.read_bytes()
    if data[0:4] != b"TES4":
        raise ValueError(f"{path}: not a TES4 plugin")
    hsize = struct.unpack_from("<I", data, 4)[0]
    masters = [
        d.rstrip(b"\x00").decode("cp1252")
        for s, d in parse_fields(data[20 : 20 + hsize])
        if s == b"MAST"
    ]
    records = []
    walk(data, 20 + hsize, len(data), records.append)
    return masters, records


def plugins(dirs: list[str]):
    for d in dirs:
        root = pathlib.Path(d)
        pats = ("*.esp", "*.esm", "*.esp.mohidden")
        for pat in pats:
            for p in sorted(root.rglob(pat)):
                yield p


def find_plugin(mods_dir: pathlib.Path, filename: str) -> pathlib.Path | None:
    """Locate <mods_dir>/<some mod>/<filename>, or its .mohidden form.

    Deliberately does NOT use glob: Oblivion plugin names routinely contain
    glob metacharacters -- "Harvest [Flora] - DLCFrostcrag.esp" would be read
    as a character class and silently never match. Compare filenames literally
    and case-insensitively (Windows/MO2 semantics).
    """
    want = {filename.lower(), f"{filename.lower()}.mohidden"}
    for mod in sorted(mods_dir.iterdir()):
        if not mod.is_dir():
            continue
        try:
            for entry in mod.iterdir():
                if entry.name.lower() in want and entry.is_file():
                    return entry
        except OSError:
            continue
    return None


def cmd_matrix(dirs):
    """(record, subrecord) -> observed sizes. The schema table's completeness target."""
    seen: dict[tuple[bytes, bytes], set[int]] = collections.defaultdict(set)
    owners: dict[tuple[bytes, bytes], str] = {}
    n = 0
    for p in plugins(dirs):
        try:
            _, records = load(p)
        except Exception as e:  # noqa: BLE001 - recon tool, report and continue
            print(f"# SKIP {p.name}: {e}", file=sys.stderr)
            continue
        n += 1
        for r in records:
            for fsig, fdata in r.fields:
                key = (r.sig, fsig)
                seen[key].add(len(fdata))
                owners.setdefault(key, p.name)

    print(f"# TES4 (record, subrecord) matrix -- {len(seen)} pairs from {n} plugins")
    print("# Generated by MOFAM-test/scripts/recon-plugins.py matrix")
    print("# Every pair here must have an entry in src/plugin/schema/tes4.rs.")
    print("# Format: RECORD FIELD sizes=<comma-separated observed byte sizes>")
    for (rsig, fsig) in sorted(seen):
        sizes = sorted(seen[(rsig, fsig)])
        shown = ",".join(str(s) for s in sizes[:12]) + (",..." if len(sizes) > 12 else "")
        print(f"{rsig.decode('ascii'):4s} {fsig.decode('ascii'):4s} sizes={shown}")
    print(f"# record types: {len({r for r, _ in seen})}", file=sys.stderr)


def cmd_scda(dirs):
    """Does compiled script bytecode embed FormIDs inline?

    For every record carrying both SCRO (referenced form ids) and SCDA
    (compiled bytecode), check whether any SCRO value appears as a raw 4-byte
    little-endian window inside SCDA. If none do, the bytecode references forms
    by SCRO *index*, and renumbering SCRO in place is sufficient -- no bytecode
    patching needed.
    """
    scripted = inline_hits = 0
    per_type = collections.Counter()
    hits = []
    for p in plugins(dirs):
        try:
            _, records = load(p)
        except Exception:  # noqa: BLE001
            continue
        for r in records:
            scro = [
                struct.unpack("<I", d)[0]
                for s, d in r.fields
                if s == b"SCRO" and len(d) == 4
            ]
            scda = next((d for s, d in r.fields if s == b"SCDA"), None)
            if not scro or not scda:
                continue
            scripted += 1
            per_type[r.sig.decode("ascii")] += 1
            for fid in scro:
                needle = struct.pack("<I", fid)
                pos = scda.find(needle)
                if pos != -1:
                    inline_hits += 1
                    hits.append((p.name, r.sig.decode(), r.form_id, fid, pos))
                    break

    print(f"records with both SCRO and SCDA : {scripted}")
    print(f"  by record type                : {dict(per_type)}")
    print(f"records whose SCDA contains a raw SCRO FormID : {inline_hits}")
    if hits:
        print("\n!! SCDA MAY EMBED FORMIDS INLINE -- bytecode patching needed:")
        for name, rsig, rfid, fid, pos in hits[:20]:
            print(f"  {name} {rsig} {rfid:08X} -> SCRO {fid:08X} at SCDA+{pos}")
    else:
        print("\nOK: no SCRO FormID appears as raw bytes in its own SCDA.")
        print("    Bytecode references forms by index into SCRO; renumbering")
        print("    SCRO in place is sufficient. No SCDA patching required.")


def cmd_types(dirs):
    counts = collections.Counter()
    for p in plugins(dirs):
        try:
            _, records = load(p)
        except Exception:  # noqa: BLE001
            continue
        for r in records:
            counts[r.sig.decode("ascii")] += 1
    print(f"{len(counts)} record types, {sum(counts.values())} records")
    for sig, n in counts.most_common():
        print(f"  {sig} {n}")


def main() -> int:
    if len(sys.argv) < 3:
        print(__doc__)
        return 2
    cmd, dirs = sys.argv[1], sys.argv[2:]
    fn = {"matrix": cmd_matrix, "scda": cmd_scda, "types": cmd_types}.get(cmd)
    if fn is None:
        print(f"unknown command {cmd!r}")
        return 2
    fn(dirs)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
