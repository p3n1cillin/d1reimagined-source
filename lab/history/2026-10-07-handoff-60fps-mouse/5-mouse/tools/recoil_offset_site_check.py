"""Offline check of the facts behind the KBM recoil-offset gate in src/core/d1_native_mouse.cpp
(GateRecoilOffset at 0x91AA17), read from the CUSA00219 01.29 eboot.bin (decrypted ELF). Static
only: nothing runs, nothing attaches.

0x919860 is the per-player look function (rdx = input record, rcx = &P[player] + 0x20, r8 = look
output). P[player]+0xF0/+0xF4 is the recoil pitch/yaw offset that recoil recovery drains back out of
the view. This disassembles 0x919860 with capstone, runs a forward data-flow over its control-flow
graph and asserts:
  1. the pinned span at 0x91AA17 and that no branch lands inside it;
  2. 0x91AA17 dominates the only `ret`, and nothing after it reaches the look site 0x91AB3C or
     0x91AA17 again: the gate runs exactly once per call, after GateLook (0x91AB3C) of that call;
  3. at 0x91AA17 on every path: [rbp-0x330] = rcx (the look record), [rbp-0x340] = rdx (the input
     record), r15 = r8 (the look output); at 0x91AB3C r13 = the result of the input-record getter
     0x9268C0 (the value GateLook compares with the player-0 record);
  4. the block's only stores to [x+0xD0] are 0x91B448 (clear, qword 0) and 0x91B459 (offset +=
     pitch delta), its same-direction test reads the stick axis [r15+0x2C] at 0x91B3C2, and the
     recoil kick store 0x91A6A0 and the recovery store 0x91B555 lie outside it;
  5. the look converter 0x9269D0, called at 0x91AB65 with dl = 0, adds the KBM mouse pitch
     record+0x2D0 / dt to the stick axis record+0x2D4 only when record+0x2A0 != 0.
Callees are assumed to honour the SysV ABI (rbx, rbp, r12-r15 preserved) and not to write the
caller's tracked frame slots.

Usage: py recoil_offset_site_check.py <eboot.bin>      (needs: pip install capstone)
Exit 0 when every fact holds; otherwise the failing facts are printed and the exit code is 1."""
import struct, sys
import capstone
from capstone import x86

FUNC, FUNC_END = 0x919860, 0x91BD90
SITE, SPAN = 0x91AA17, bytes.fromhex("c4c17a104718")          # vmovss xmm0, [r15+0x18]
LOOK_SITE = 0x91AB3C
INPUT_GETTER = 0x9268C0                                       # input record for a user index
STACK_CHK_FAIL = 0x129D0B0                                    # noreturn
BLOCK_STORES = {0x91B448: "qword ptr [rax + 0xd0], 0", 0x91B459: "dword ptr [rax + 0xd0], xmm0"}
STORES = {"mov", "vmovss", "vmovd", "vmovq", "vmovaps", "vmovups"}
CONVERTER, CONVERTER_LEN = 0x9269D0, 0x63                    # through its ret at 0x926A32
CONVERTER_TEXT = [
    "lea r8, [rdi + 0x2d8]", "lea rax, [rdi + 0x2d0]", "test dl, dl", "cmovne rax, r8",
    "vmovss xmm1, dword ptr [rax + 4]", "vmovss xmm2, dword ptr [rdi + 0x2cc]",
    "cmp byte ptr [rdi + 0x2a0], 0", "je 0x926a20", "vucomiss xmm0, dword ptr [rip + 0xd368ae]",
    "jbe 0x926a20", "vmovss xmm3, dword ptr [rdi + 0x2c8]", "vdivss xmm3, xmm3, xmm0",
    "vmovss xmm4, dword ptr [rax]", "vdivss xmm0, xmm4, xmm0", "vaddss xmm1, xmm1, xmm0",
    "vaddss xmm2, xmm2, xmm3", "vmulss xmm0, xmm1, dword ptr [rsi + 4]",
    "vmulss xmm1, xmm2, dword ptr [rsi]", "vmovss dword ptr [rcx], xmm1",
    "vmovss dword ptr [rcx + 4], xmm0", "ret",
]
CALLER_SAVED = {"rax", "rcx", "rdx", "rsi", "rdi", "r8", "r9", "r10", "r11"}
SUBREGS = {
    "rax": ("eax", "ax", "al", "ah"), "rbx": ("ebx", "bx", "bl", "bh"), "rcx": ("ecx", "cx", "cl", "ch"),
    "rdx": ("edx", "dx", "dl", "dh"), "rsi": ("esi", "si", "sil"), "rdi": ("edi", "di", "dil"),
    "rbp": ("ebp", "bp", "bpl"), "rsp": ("esp", "sp", "spl"),
    **{f"r{i}": (f"r{i}d", f"r{i}w", f"r{i}b") for i in range(8, 16)},
}
OTHER = frozenset({"OTHER"})


def load_text(path):
    data = open(path, "rb").read()
    if data[:4] != b"\x7fELF":
        sys.exit("not a decrypted ELF eboot")
    phoff, = struct.unpack_from("<Q", data, 0x20)
    phnum, = struct.unpack_from("<H", data, 0x38)
    for i in range(phnum):
        p_type, _, off, vaddr, _, filesz, _, _ = struct.unpack_from("<IIQQQQQQ", data, phoff + i * 56)
        if p_type == 1 and vaddr == 0:
            return lambda va, n: data[off + va: off + va + n]
    sys.exit("no text segment at vaddr 0")


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    rd = load_text(sys.argv[1])
    md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_64)
    md.detail = True
    insns = {i.address: i for i in md.disasm(rd(FUNC, FUNC_END - FUNC), FUNC)}
    order = sorted(insns)
    nxt = {a: b for a, b in zip(order, order[1:])}
    failures = []

    def check(ok, what):
        print(("ok    " if ok else "FAIL  ") + what)
        if not ok:
            failures.append(what)

    check(order[-1] + insns[order[-1]].size == FUNC_END, "0x919860 decodes linearly to 0x91BD90")
    check(rd(SITE, len(SPAN)) == SPAN and SITE in insns, "pinned span at 0x91AA17 on a boundary")

    def direct_target(ins):
        op = ins.operands[0] if ins.operands else None
        return op.imm if op is not None and op.type == x86.X86_OP_IMM else None

    def successors(ins):
        m, out = ins.mnemonic, []
        if m == "ret" or (m == "call" and direct_target(ins) == STACK_CHK_FAIL):
            return out
        if m.startswith("j"):
            if direct_target(ins) is not None:
                out.append(direct_target(ins))
            if m == "jmp":
                return out
        if ins.address in nxt:
            out.append(nxt[ins.address])
        return out

    succ = {a: successors(insns[a]) for a in order}
    targets = {t for a in order for t in succ[a]}
    stray = sorted(t for t in targets if not FUNC <= t < FUNC_END)
    check(not stray, f"no branch leaves 0x919860 (got {[hex(t) for t in stray]})")
    inside = sorted(t for t in targets if SITE < t < SITE + len(SPAN))
    check(not inside, f"no branch lands inside the span (got {[hex(t) for t in inside]})")
    rets = [a for a in order if insns[a].mnemonic == "ret"]
    check(len(rets) == 1, f"one ret (got {[hex(a) for a in rets]})")

    def reach(start, blocked=()):
        seen, work = set(), [start]
        while work:
            a = work.pop()
            if a in seen or a in blocked or a not in insns:
                continue
            seen.add(a)
            work.extend(succ[a])
        return seen

    check(rets[0] not in reach(FUNC, blocked={SITE}), "0x91AA17 dominates the ret")
    after = reach(succ[SITE][0])
    check(rets[0] in after and LOOK_SITE not in after and SITE not in after,
          "after 0x91AA17 the ret is reachable and 0x91AB3C / 0x91AA17 are not")

    # Abstract values: the three pointer arguments and the getter result; everything else OTHER.
    # State: 64-bit registers plus 8-byte frame slots [rbp-X].
    def frame_slot(op):
        if op.type == x86.X86_OP_MEM and op.mem.base == x86.X86_REG_RBP and op.mem.index == 0:
            return op.mem.disp
        return None

    def reg64(ins, reg):
        name = ins.reg_name(reg)
        return name if name in SUBREGS else None

    def transfer(ins, st):
        st = dict(st)
        m, ops = ins.mnemonic, ins.operands
        if m == "mov" and len(ops) == 2 and ops[0].type == x86.X86_OP_MEM and ops[0].size == 8:
            slot = frame_slot(ops[0])
            if slot is not None:
                val = OTHER
                if ops[1].type == x86.X86_OP_REG and reg64(ins, ops[1].reg):
                    val = st.get(reg64(ins, ops[1].reg), OTHER)
                st[f"slot{slot:#x}"] = val
        dst = ops[0] if ops else None
        for name in {ins.reg_name(r) for r in ins.regs_access()[1]}:
            parent = next((g for g, subs in SUBREGS.items() if name == g or name in subs), None)
            if parent is None:
                continue
            val = OTHER
            if m == "mov" and dst is not None and dst.type == x86.X86_OP_REG and \
                    ins.reg_name(dst.reg) == parent:
                src = ops[1]
                if src.type == x86.X86_OP_REG and reg64(ins, src.reg):
                    val = st.get(reg64(ins, src.reg), OTHER)
                elif src.type == x86.X86_OP_MEM and frame_slot(src) is not None:
                    val = st.get(f"slot{frame_slot(src):#x}", OTHER)
            st[parent] = val
        if m == "call":
            for r in CALLER_SAVED:
                st[r] = OTHER
            if direct_target(ins) == INPUT_GETTER:
                st["rax"] = frozenset({"GETTER"})
        return st

    def join(a, b):
        return {k: a.get(k, OTHER) | b.get(k, OTHER) for k in set(a) | set(b)}

    entry = {"rcx": frozenset({"LOOKREC"}), "rdx": frozenset({"INPUT"}), "r8": frozenset({"OUT"})}
    states, work = {FUNC: entry}, [FUNC]
    while work:
        a = work.pop()
        out = transfer(insns[a], states[a])
        for s in succ[a]:
            if s not in insns:
                failures.append(f"jump into the middle of an instruction at {s:#x}")
                continue
            merged = out if s not in states else join(states[s], out)
            if merged != states.get(s):
                states[s] = merged
                work.append(s)

    def at(site, key):
        return set(states.get(site, {}).get(key, {"UNREACHED"}))

    check(at(SITE, "slot-0x330") == {"LOOKREC"}, f"0x91AA17 [rbp-0x330] = rcx (got {at(SITE, 'slot-0x330')})")
    check(at(SITE, "slot-0x340") == {"INPUT"}, f"0x91AA17 [rbp-0x340] = rdx (got {at(SITE, 'slot-0x340')})")
    check(at(SITE, "r15") == {"OUT"}, f"0x91AA17 r15 = r8 (got {at(SITE, 'r15')})")
    check(at(LOOK_SITE, "r13") == {"GETTER"}, f"0x91AB3C r13 = 0x9268C0(user) (got {at(LOOK_SITE, 'r13')})")
    rbp_writes = [hex(a) for a in order if "rbp" in {insns[a].reg_name(r) for r in insns[a].regs_access()[1]}
                  and insns[a].mnemonic not in ("push", "pop") and a != FUNC + 1]
    check(not rbp_writes, f"rbp is the frame pointer throughout (other writes: {rbp_writes})")

    def stores_d0(addrs):
        found = {}
        for a in addrs:
            ins = insns[a]
            if ins.mnemonic in STORES and len(ins.operands) == 2 and \
                    ins.operands[0].type == x86.X86_OP_MEM and ins.operands[0].mem.disp == 0xD0:
                found[a] = ins.op_str
        return found

    got = stores_d0(after)
    check(got == BLOCK_STORES, "block stores to [x+0xd0] are 0x91B448 and 0x91B459 (got "
          f"{ {hex(a): s for a, s in sorted(got.items())} })")
    check(0x91B3C2 in after and insns[0x91B3C2].op_str == "xmm2, dword ptr [r15 + 0x2c]",
          "the same-direction test reads the stick axis [r15+0x2c] at 0x91B3C2")
    outside = stores_d0([a for a in order if a not in after])
    check({0x91A6A0, 0x91B555} <= set(outside),
          "the kick store 0x91A6A0 and the recovery store 0x91B555 lie outside the block "
          f"(outside: {sorted(hex(a) for a in outside)})")
    call = insns.get(0x91AB65)
    check(call is not None and call.mnemonic == "call" and direct_target(call) == CONVERTER and
          insns[0x91AB63].mnemonic + " " + insns[0x91AB63].op_str == "xor edx, edx",
          "0x91AB65 calls the converter 0x9269D0 with edx = 0")
    conv = [f"{i.mnemonic} {i.op_str}".strip()
            for i in md.disasm(rd(CONVERTER, CONVERTER_LEN), CONVERTER)]
    check(conv == CONVERTER_TEXT, "converter 0x9269D0: +0x2D0 (dl = 0) / dt added to +0x2D4 only "
          "when [rdi+0x2A0] != 0")
    if failures:
        print(f"{len(failures)} fact(s) failed")
        sys.exit(1)
    print("all facts hold")


if __name__ == "__main__":
    main()
