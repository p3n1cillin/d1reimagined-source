"""What changed between two Destiny PS3 builds: runtime types, web services, the type handles the
stand-in uses, and executable strings, as one Markdown report.

Type handles are renumbered between builds (the investment globals of 01.24 share only 3,322 of
their tag classes with 01.00), so types are matched by structure: a fingerprint of the size and
every field (offset, kind, optional flag, width, bias, array mode, nested type's fingerprint).
Unmatched types are paired by the closest field layout of the same size.

  python scripts/d1_builddiff.py --old-elf OLD.elf --new-elf NEW.elf --out report.md
      [--old-packages DIR --new-packages DIR --oodle oo2core_3_win64.dll]
Either ELF can be replaced by a schema JSON from `d1_types.py schema` (--old-types / --new-types).
"""

import argparse
import difflib
import hashlib
import json
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import d1_types  # noqa: E402

REPO = Path(__file__).resolve().parents[1]
NONE = 0xFFFFFFFF


def load_types(elf=None, types=None):
    if types:
        data = json.loads(Path(types).read_text(encoding="utf-8"))
        return {k: v for k, v in data.items() if k.startswith("0x")}, data.get("services")
    return d1_types.scan_types(Path(elf).read_bytes()), None


def fingerprints(types):
    """{handle: hex digest} of each type's structure (nested types included, cycles cut)."""
    memo = {}

    def fp(handle, stack=()):
        if handle in memo:
            return memo[handle]
        entry = types.get(handle)
        if entry is None or handle in stack:
            return "?"
        parts = [str(entry["size"])]
        for f in entry["fields"]:
            nested = f["nested"] & NONE
            inner = fp(f"{nested:#x}", stack + (handle,)) if f["kind"] == 1 and nested != NONE else "-"
            parts.append(f"{f['off']}:{f['kind']}:{f['opt']}:{f['width']}:{f['min']}:{f['arr']}:{inner}")
        memo[handle] = hashlib.sha1("|".join(parts).encode()).hexdigest()[:16]
        return memo[handle]

    return {h: fp(h) for h in types}


def layout(entry):
    return [(f["off"], f["kind"], f["opt"], f["width"]) for f in entry["fields"]]


def match_types(old, new):
    """(mapping old->new, how {old: 'same'|'changed'}, removed old handles, added new handles)."""
    fo, fn = fingerprints(old), fingerprints(new)
    by_fp = {}
    for h, digest in fn.items():
        by_fp.setdefault(digest, []).append(h)
    mapping, how = {}, {}
    for h, digest in fo.items():
        candidates = by_fp.get(digest, [])
        if len(candidates) == 1 or h in candidates:
            mapping[h] = h if h in candidates else candidates[0]
            how[h] = "same"
    taken = set(mapping.values())
    left_new = [h for h in new if h not in taken]
    by_size = {}
    for h in left_new:
        by_size.setdefault(new[h]["size"], []).append(h)
    for h in [h for h in old if h not in mapping]:
        pool = [c for c in by_size.get(old[h]["size"], []) if c not in taken] or \
               [c for c in left_new if c not in taken and abs(new[c]["size"] - old[h]["size"]) <= 16]
        best, score = None, 0.0
        for c in pool[:400]:
            ratio = difflib.SequenceMatcher(None, layout(old[h]), layout(new[c])).ratio()
            if ratio > score:
                best, score = c, ratio
        if best and score >= 0.6:
            mapping[h], how[h] = best, "changed"
            taken.add(best)
    removed = [h for h in old if h not in mapping]
    added = [h for h in new if h not in taken]
    return mapping, how, removed, added


def standin_handles():
    """Type handles written in the stand-in's code (server/*.py), with where they appear."""
    found = {}
    for path in sorted((REPO / "server").glob("*.py")):
        if path.name.startswith("test_"):
            continue
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            for m in re.finditer(r"0x8080[0-9A-Fa-f]{4}", line):
                found.setdefault(f"{int(m.group(0), 16):#x}", f"{path.name}:{number}")
    return found


def strings_of(elf):
    import d1_eboot
    return {text for _, text in d1_eboot.Eboot(elf).strings()}


def services_of(packages, oodle):
    if not packages:
        return None
    import d1_pkg
    ps = d1_pkg.PackageSet([packages], d1_pkg.Oodle(oodle) if oodle else None)
    table = d1_types.service_table(ps)
    return {str(w): [f"{rq:#x}", f"{rs:#x}"] for w, _, rq, rs in table} if table else None


def report(old, new, mapping, how, removed, added, services=(None, None), strings=(None, None), names=("old", "new")):
    a, b = names
    same = sum(1 for v in how.values() if v == "same")
    changed = [h for h, v in how.items() if v == "changed"]
    renumbered = sum(1 for h, n in mapping.items() if h != n)
    out = [f"# Build difference: {a} -> {b}", "",
           f"- Types: {len(old)} in {a}, {len(new)} in {b}; {same} structurally identical, "
           f"{len(changed)} changed, {len(removed)} removed, {len(added)} added.",
           f"- Handles renumbered: {renumbered} of {len(mapping)} matched types.", ""]
    used = standin_handles()
    out += ["## Handles the stand-in uses", "", f"| {a} | {b} | match | first use |", "| --- | --- | --- | --- |"]
    for h, where in sorted(used.items()):
        if h in old:
            out.append(f"| `{h}` | `{mapping.get(h, '-')}` | {how.get(h, 'removed')} | {where} |")
    out += ["", "## Changed types", ""]
    for h in changed[:200]:
        n = mapping[h]
        diff = list(difflib.unified_diff([str(x) for x in layout(old[h])], [str(x) for x in layout(new[n])],
                                         lineterm="", n=0))[2:]
        out.append(f"- `{h}` -> `{n}` (size {old[h]['size']:#x} -> {new[n]['size']:#x}): " +
                   "; ".join(diff[:6]) + (" ..." if len(diff) > 6 else ""))
    out += ["", f"## Removed types ({len(removed)})", "", ", ".join(f"`{h}`" for h in removed[:400]), "",
            f"## Added types ({len(added)})", "", ", ".join(f"`{h}`" for h in added[:400]), ""]
    so, sn = services
    if so is not None or sn is not None:
        out += ["## Web services", ""]
        if so is None or sn is None:
            out.append(f"- Service table found in {a}: {so is not None}; in {b}: {sn is not None}.")
        else:
            fo, fn = fingerprints(old), fingerprints(new)
            for wire in sorted(set(so) | set(sn), key=int):
                if wire not in sn:
                    out.append(f"- {wire}: removed")
                elif wire not in so:
                    out.append(f"- {wire}: added, request {sn[wire][0]}, response {sn[wire][1]}")
                else:
                    same_rq = fo.get(so[wire][0]) == fn.get(sn[wire][0])
                    same_rs = fo.get(so[wire][1]) == fn.get(sn[wire][1])
                    if not (same_rq and same_rs):
                        out.append(f"- {wire}: request {'same' if same_rq else 'changed'}, "
                                   f"response {'same' if same_rs else 'changed'}")
        out.append("")
    to, tn = strings
    if to is not None and tn is not None:
        gone, new_strings = sorted(to - tn), sorted(tn - to)
        interesting = re.compile(r"error\.|bootflow|activity|investment|bap|host|service|entitle|dlc|expansion", re.I)
        out += [f"## Strings: {len(gone)} removed, {len(new_strings)} added", "", "Added (selected):", ""]
        out += [f"- `{s[:120]}`" for s in new_strings if interesting.search(s)][:300]
        out += ["", "Removed (selected):", ""]
        out += [f"- `{s[:120]}`" for s in gone if interesting.search(s)][:300]
    return "\n".join(out) + "\n"


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--old-elf"); ap.add_argument("--new-elf")
    ap.add_argument("--old-types"); ap.add_argument("--new-types")
    ap.add_argument("--old-packages"); ap.add_argument("--new-packages"); ap.add_argument("--oodle")
    ap.add_argument("--names", default="old,new", help="labels for the two builds, e.g. 01.00,01.24")
    ap.add_argument("--map-out", type=Path, help="write the old -> new handle mapping as JSON")
    ap.add_argument("--out", type=Path, required=True)
    a = ap.parse_args(argv)
    if not (a.old_elf or a.old_types) or not (a.new_elf or a.new_types):
        ap.error("give --old-elf/--old-types and --new-elf/--new-types")
    old, old_services = load_types(a.old_elf, a.old_types)
    new, new_services = load_types(a.new_elf, a.new_types)
    mapping, how, removed, added = match_types(old, new)
    services = (services_of(a.old_packages, a.oodle) or old_services,
                services_of(a.new_packages, a.oodle) or new_services)
    strings = (strings_of(a.old_elf), strings_of(a.new_elf)) if a.old_elf and a.new_elf else (None, None)
    a.out.write_text(report(old, new, mapping, how, removed, added, services, strings, tuple(a.names.split(","))),
                     encoding="utf-8")
    if a.map_out:
        a.map_out.write_text(json.dumps({h: [n, how[h]] for h, n in mapping.items()}, indent=0), encoding="utf-8")
    print(f"{len(mapping)} matched ({sum(v == 'same' for v in how.values())} identical), "
          f"{len(removed)} removed, {len(added)} added -> {a.out}")


if __name__ == "__main__":
    main()
