"""Read-only scan of the owned emulator's guest memory while the urgent dialog is on screen.
Finds the urgent definition record (library 0x80a5843f, title 0x84cc7b33, body 0x40e5f178), live
objects holding {library, title}, then words pointing at either (two levels), with context.
usage: memscan.py RUN TAG"""
import json, re, struct, sys, time
import numpy as np
from pathlib import Path
sys.path.insert(0, r"F:\d1\d1xenia\scripts")
from rpcs3_owned_memory import OwnedMemory

run, tag = sys.argv[1], sys.argv[2]
rt = Path(r"F:\d1\<pc>-import\d1xenia\local")
manifest = rt / f"SIGNON_LOCAL/logs/FINAL124_{run}-processes.json"
out = rt / f"RESEARCH/{run}-memscan-{tag}.txt"
DEF = bytes.fromhex("80a5843f811c9dc584cc7b3340e5f178")
TITLE = bytes.fromhex("84cc7b33")
CHUNK, SUB = 1 << 20, 1 << 16


def chunks(memory):
    for base in range(0x10000, 0xE0000000, CHUNK):
        try:
            yield base, memory.read(base, CHUNK)
        except OSError:
            for sub in range(base, base + CHUNK, SUB):
                try:
                    yield sub, memory.read(sub, SUB)
                except OSError:
                    pass


def find_all(data, pat):
    i = data.find(pat)
    while i >= 0:
        yield i
        i = data.find(pat, i + 1)


def ctx(memory, addr, before=0x20, after=0x40):
    start = max(addr - before, 0) & ~3
    try:
        d = memory.read(start, before + after)
    except OSError:
        return "unreadable"
    return " ".join(d[i:i + 4].hex() for i in range(0, len(d), 4))


lines = []
def say(*a):
    s = " ".join(str(x) for x in a); lines.append(s); print(s, flush=True)

t0 = time.time()
with OwnedMemory(manifest) as memory:
    defs, titles, mapped = [], [], 0
    for base, data in chunks(memory):
        mapped += len(data)
        defs += [base + i for i in find_all(data, DEF) if i % 4 == 0]
        titles += [base + i for i in find_all(data, TITLE) if i % 4 == 0]
    say(f"{tag}: mapped {mapped >> 20} MiB in {time.time() - t0:.0f}s; definition {[hex(a) for a in defs]}; title words {len(titles)}")
    for a in titles[:60]:
        say(f"  title@{a:#x}: {ctx(memory, a)}")
    # Pointer targets: each definition record (starts 16 bytes before the pattern) and each
    # non-content title holder (any object start within 0x40 before the title word).
    targets = {}
    for d in defs:
        for k in range(-0x20, 0x20, 4):
            targets[d + k] = f"def{d:#x}{k:+#x}"
    for a in titles:
        if not 0x70000000 <= a < 0x80000000:
            for k in range(-0x40, 4, 4):
                targets.setdefault(a + k, f"title{a:#x}{k:+#x}")
    def pointers(table):
        keys = np.array(sorted(table), dtype=np.uint32); hits = []
        for base, data in chunks(memory):
            words = np.frombuffer(data, dtype=">u4")
            for i in np.nonzero(np.isin(words, keys))[0]:
                hits.append((base + 4 * int(i), int(words[i])))
        return hits
    level1 = pointers(targets)
    say(f"level1 pointers: {len(level1)}")
    for at, w in level1[:80]:
        say(f"  {at:#x} -> {w:#x} ({targets[w]}): {ctx(memory, at)}")
    t2 = {}
    for at, _ in level1:
        if not 0x70000000 <= at < 0x80000000:
            for k in range(-0x40, 4, 4):
                t2.setdefault(at + k, f"holder{at:#x}{k:+#x}")
    level2 = pointers(t2) if t2 else []
    say(f"level2 pointers: {len(level2)}")
    for at, w in level2[:80]:
        say(f"  {at:#x} -> {w:#x} ({t2[w]}): {ctx(memory, at)}")
    tagwords = {0x80a58505: "urgent def tag", 0x80a581e5: "urgent group tag", 0x80a5802e: "dialog table",
                0x80a5e9fc: "second-record ref", 0x80a5851c: "motd important", 0x80a581e6: "motd list"}
    hits = [h for h in pointers(tagwords) if not 0x70000000 <= h[0] < 0x80000000]
    say(f"tag-hash words outside content: {len(hits)}")
    for at, w in hits[:60]:
        say(f"  {at:#x} = {w:#x} ({tagwords[w]}): {ctx(memory, at)}")
    for text in (b"Destiny servers will go offline", "Destiny servers will go offline".encode("utf-16-be")):
        found = []
        for base, data in chunks(memory):
            found += [base + i for i in find_all(data, text)]
        say(f"text {text[:12]!r}: {[hex(a) for a in found[:20]]}")
        for a in found[:6]:
            say(f"  text@{a:#x}: {ctx(memory, a)}")
            tp = pointers({a + k: f"text{a:#x}{k:+#x}" for k in range(-0x10, 4, 4)})
            for at, w in tp[:10]:
                say(f"    ptr {at:#x} -> {w:#x}: {ctx(memory, at)}")
out.write_text("\n".join(lines) + "\n", encoding="utf-8")
say(f"done {time.time() - t0:.0f}s")
