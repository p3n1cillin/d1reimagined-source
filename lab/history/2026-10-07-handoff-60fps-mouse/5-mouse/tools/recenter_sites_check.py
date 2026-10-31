"""Offline check of the register facts behind the KBM recentering gates in
src/core/d1_native_mouse.cpp (GateRecenter at 0x91FE67, GatePitchSpring at 0x9202CF), read from the
CUSA00219 01.29 eboot.bin (decrypted ELF). Static only: nothing runs, nothing attaches.

It disassembles the camera-angle applier 0x91EEF0 with capstone, runs a small forward data-flow
over its control-flow graph and asserts:
  1. the pinned spans at both sites;
  2. at 0x91FE67, on every path: r13 = player * 0x1A0 and r14 = the look-state array;
  3. at 0x9202CF, on every path: r13 = &P[player].pitch (NOT the player offset), r14 = the
     look-state array and [rbp-0x248] = player * 0x1A0;
  4. the other 0x78B50 call (0x920599) is dead: its store at 0x92059E is overwritten at 0x9205F6
     on a straight line.
The look-state array is *(u64*)(eboot + 0x2725798). Callees are assumed to honour the SysV ABI
(rbx, rbp, r12-r15 preserved) and not to write the caller's frame slots tracked here.

Usage: py recenter_sites_check.py <eboot.bin>      (needs: pip install capstone)
Exit 0 when every fact holds; otherwise the failing facts are printed and the exit code is 1."""
import struct, sys
import capstone
from capstone import x86

FUNC, FUNC_END = 0x91EEF0, 0x9207E8
LOOK_STATE_PTR = 0x2725798
RECENTER_SITE, RECENTER_SPAN = 0x91FE67, bytes.fromhex("c5f857c0c5fa108d80fdffff")
SPRING_SITE, SPRING_SPAN = 0x9202CF, bytes.fromhex("c5fa109da0fdffff")
TRACKED_SLOTS = (-0x248, -0x260)        # frame slots the applier uses to park r13 / r14

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

PARENT = {}
for name in ("r13", "r14", "rbx"):
    for sub in {"r13": ("r13", "r13d", "r13w", "r13b"), "r14": ("r14", "r14d", "r14w", "r14b"),
                "rbx": ("rbx", "ebx", "bx", "bl", "bh")}[name]:
        PARENT[sub] = name

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

    check(rd(RECENTER_SITE, len(RECENTER_SPAN)) == RECENTER_SPAN, "pinned span at 0x91FE67")
    check(rd(SPRING_SITE, len(SPRING_SPAN)) == SPRING_SPAN, "pinned span at 0x9202CF")
    check(RECENTER_SITE in insns and SPRING_SITE in insns, "both sites are instruction boundaries")

    def rip_target(ins):
        for op in ins.operands:
            if op.type == x86.X86_OP_MEM and op.mem.base == x86.X86_REG_RIP:
                return ins.address + ins.size + op.mem.disp
        return None

    def frame_slot(op):
        if op.type == x86.X86_OP_MEM and op.mem.base == x86.X86_REG_RBP and op.mem.index == 0:
            return op.mem.disp
        return None

    # Abstract values: PLAYER (player * 0x1A0), P (look-state array), PITCH (&P[player].pitch),
    # LOOKPTR (address of the global pointer), OTHER. A state maps tracked names to frozensets.
    def transfer(ins, st):
        st = dict(st)
        m, ops = ins.mnemonic, ins.operands
        regs_written = {ins.reg_name(r) for r in ins.regs_access()[1]}
        written = {PARENT[r] for r in regs_written if r in PARENT}
        # memory stores to tracked frame slots
        for op in ops:
            slot = frame_slot(op)
            if slot in TRACKED_SLOTS and op.access & capstone.CS_AC_WRITE:
                src = ops[1] if len(ops) == 2 else None
                val = frozenset({"OTHER"})
                if m == "mov" and op.size == 8 and src is not None and src.type == x86.X86_OP_REG:
                    val = st.get(ins.reg_name(src.reg), frozenset({"OTHER"}))
                st[f"slot{slot:#x}"] = val
        if m == "lea" and ops[0].type == x86.X86_OP_REG and rip_target(ins) == LOOK_STATE_PTR:
            st[ins.reg_name(ops[0].reg)] = frozenset({"LOOKPTR"})
        for reg in written:
            dst = ops[0] if ops else None
            full = dst is not None and dst.type == x86.X86_OP_REG and ins.reg_name(dst.reg) == reg
            val = frozenset({"OTHER"})
            if full and m == "imul" and reg == "r13" and len(ops) == 3 and ops[2].imm == 0x1A0:
                val = frozenset({"PLAYER"})
            elif full and m == "mov" and ops[1].type == x86.X86_OP_MEM:
                slot = frame_slot(ops[1])
                if slot in TRACKED_SLOTS:
                    val = st.get(f"slot{slot:#x}", frozenset({"OTHER"}))
                elif ops[1].mem.disp == 0 and ops[1].mem.index == 0 and \
                        st.get(ins.reg_name(ops[1].mem.base)) == frozenset({"LOOKPTR"}):
                    val = frozenset({"P"})
            elif full and m == "mov" and ops[1].type == x86.X86_OP_REG:
                val = st.get(ins.reg_name(ops[1].reg), frozenset({"OTHER"}))
            elif full and m == "lea" and ops[1].mem.disp == 0x3C and ops[1].mem.scale == 1:
                base, index = ins.reg_name(ops[1].mem.base), ins.reg_name(ops[1].mem.index)
                if {base, index} == {"r14", "r13"} and st.get("r14") == frozenset({"P"}) and \
                        st.get("r13") == frozenset({"PLAYER"}):
                    val = frozenset({"PITCH"})
            st[reg] = val
        # any other register written, or a caller-saved one across a call, loses a LOOKPTR mark
        clobbered = set(regs_written)
        if m == "call":
            clobbered |= {"rax", "rcx", "rdx", "rsi", "rdi", "r8", "r9", "r10", "r11"}
        for r in clobbered:
            if r not in PARENT and st.get(r) == frozenset({"LOOKPTR"}) and not (
                    m == "lea" and rip_target(ins) == LOOK_STATE_PTR):
                st[r] = frozenset({"OTHER"})
        return st

    def successors(ins):
        m = ins.mnemonic
        out = []
        if m == "ret":
            return out
        if m.startswith("j"):
            op = ins.operands[0]
            if op.type == x86.X86_OP_IMM and FUNC <= op.imm < FUNC_END:
                out.append(op.imm)
            if m == "jmp":
                return out
        if ins.address in nxt:
            out.append(nxt[ins.address])
        return out

    def join(a, b):
        keys = set(a) | set(b)
        return {k: a.get(k, frozenset({"OTHER"})) | b.get(k, frozenset({"OTHER"})) for k in keys}

    states = {FUNC: {}}
    work = [FUNC]
    while work:
        addr = work.pop()
        out = transfer(insns[addr], states[addr])
        for succ in successors(insns[addr]):
            if succ not in insns:
                failures.append(f"jump into the middle of an instruction at {succ:#x}")
                continue
            merged = out if succ not in states else join(states[succ], out)
            if merged != states.get(succ):
                states[succ] = merged
                work.append(succ)

    def at(site, name):
        return set(states.get(site, {}).get(name, {"UNREACHED"}))

    check(at(RECENTER_SITE, "r13") == {"PLAYER"}, f"0x91FE67 r13 = player*0x1A0 (got {at(RECENTER_SITE, 'r13')})")
    check(at(RECENTER_SITE, "r14") == {"P"}, f"0x91FE67 r14 = look-state array (got {at(RECENTER_SITE, 'r14')})")
    check(at(SPRING_SITE, "r13") == {"PITCH"}, f"0x9202CF r13 = &P[player].pitch (got {at(SPRING_SITE, 'r13')})")
    check(at(SPRING_SITE, "r14") == {"P"}, f"0x9202CF r14 = look-state array (got {at(SPRING_SITE, 'r14')})")
    check(at(SPRING_SITE, "slot-0x248") == {"PLAYER"},
          f"0x9202CF [rbp-0x248] = player*0x1A0 (got {at(SPRING_SITE, 'slot-0x248')})")
    rbp_writes = [hex(a) for a in order if "rbp" in {insns[a].reg_name(r) for r in insns[a].regs_access()[1]}
                  and a not in (FUNC, FUNC + 1) and insns[a].mnemonic not in ("pop",)]
    check(not rbp_writes, f"rbp is the frame pointer throughout (other writes: {rbp_writes})")
    dead = [insns[a] for a in order if 0x92059E <= a < 0x9205F6]
    check(insns[0x92059E].op_str == insns[0x9205F6].op_str == "dword ptr [r13], xmm0" and
          not any(i.mnemonic.startswith("j") or i.mnemonic == "ret" for i in dead),
          "0x920599 spring result (0x92059E) is overwritten at 0x9205F6 on a straight line")
    if failures:
        print(f"{len(failures)} fact(s) failed")
        sys.exit(1)
    print("all facts hold")

if __name__ == "__main__":
    main()
