#!/usr/bin/env python3
"""d1-watchwrite - log every guest write to a few dwords of the live client, with hardware breakpoints.

Attaches to shadPS4 as a debugger, puts data-write breakpoints (DR0..DR3, 4 bytes each) on every
thread, and logs RIP plus return-address candidates from the stack for each hit. Every other
exception is passed back unhandled, so the emulator's own handlers still see its first-chance
faults. Detaches cleanly (breakpoints cleared, kill-on-exit off) after --seconds.

    d1-watchwrite.py --seconds 300 --addr 0x37fa385d0 --addr 0x37fa385c8
    d1-watchwrite.py --seconds 300 --player-array 0 --field 0x50 --field 0x48

Read-only on guest memory: it only sets debug registers. Output: a JSON-lines log (--out).
"""
import argparse, ctypes, ctypes.wintypes as wt, json, struct, sys, time, os

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
src = open(os.path.join(HERE, 'd1-winguest.py'), encoding='utf-8').read().split('def main():')[0]
wg = {}
exec(compile(src, 'd1-winguest.py', 'exec'), wg)

k32 = ctypes.WinDLL('kernel32', use_last_error=True)
k32.DebugActiveProcess.argtypes = [wt.DWORD]
k32.DebugActiveProcessStop.argtypes = [wt.DWORD]
k32.DebugSetProcessKillOnExit.argtypes = [wt.BOOL]
k32.WaitForDebugEvent.argtypes = [ctypes.c_void_p, wt.DWORD]
k32.ContinueDebugEvent.argtypes = [wt.DWORD, wt.DWORD, wt.DWORD]
k32.GetThreadContext.argtypes = [wt.HANDLE, ctypes.c_void_p]
k32.SetThreadContext.argtypes = [wt.HANDLE, ctypes.c_void_p]
k32.SuspendThread.argtypes = [wt.HANDLE]
k32.SuspendThread.restype = wt.DWORD
k32.ResumeThread.argtypes = [wt.HANDLE]
k32.ResumeThread.restype = wt.DWORD
k32.GetExitCodeThread.argtypes = [wt.HANDLE, ctypes.POINTER(wt.DWORD)]
k32.CloseHandle.argtypes = [wt.HANDLE]
k32.OpenThread.restype = wt.HANDLE
k32.OpenThread.argtypes = [wt.DWORD, wt.BOOL, wt.DWORD]

DBG_CONTINUE, DBG_EXCEPTION_NOT_HANDLED = 0x00010002, 0x80010001
EXCEPTION_DEBUG_EVENT, CREATE_THREAD, CREATE_PROCESS, EXIT_THREAD, EXIT_PROCESS, LOAD_DLL = 1, 2, 3, 4, 5, 6
SINGLE_STEP, BREAKPOINT = 0x80000004, 0x80000003
CONTEXT_DEBUG_CONTROL = 0x100013          # CONTEXT_AMD64 | CONTROL | INTEGER | DEBUG_REGISTERS
OFF_FLAGS, OFF_DR0, OFF_DR6, OFF_DR7, OFF_RSP, OFF_RIP = 0x30, 0x48, 0x68, 0x70, 0x98, 0xF8
OFF_EFLAGS, OFF_RAX, OFF_RBP, OFF_RDI = 0x44, 0x78, 0xA0, 0xB0
OFF_RDX, OFF_RSI = 0x88, 0xA8
OFF_RCX, OFF_RBX = 0x80, 0x90


class Ctx:
    def __init__(self):
        self.raw = ctypes.create_string_buffer(1232 + 16)
        self.addr = (ctypes.addressof(self.raw) + 15) & ~15

    def u64(self, off):
        return ctypes.c_uint64.from_address(self.addr + off).value

    def set64(self, off, v):
        ctypes.c_uint64.from_address(self.addr + off).value = v

    def get(self, h):
        ctypes.c_uint32.from_address(self.addr + OFF_FLAGS).value = CONTEXT_DEBUG_CONTROL
        return k32.GetThreadContext(h, ctypes.c_void_p(self.addr))

    def put(self, h):
        ctypes.c_uint32.from_address(self.addr + OFF_FLAGS).value = CONTEXT_DEBUG_CONTROL
        return k32.SetThreadContext(h, ctypes.c_void_p(self.addr))


EXEC = set()   # indices of DR slots that are execute breakpoints (RW=00, LEN=00)
READ = set()   # indices of DR slots that break on read or write (RW=11), 8 bytes


def dr7_for(n):
    v = 0
    for i in range(n):
        v |= 1 << (2 * i)                  # L_i
        if i in READ:
            v |= (0b11 | (0b10 << 2)) << (16 + 4 * i)   # RW=read/write, LEN=8 bytes
        elif i not in EXEC:
            v |= (0b01 | (0b11 << 2)) << (16 + 4 * i)   # RW=write, LEN=4 bytes
    return v


def arm(h, addrs, on=True):
    c = Ctx()
    if not c.get(h):
        return False
    for i in range(4):
        c.set64(OFF_DR0 + 8 * i, addrs[i] if on and i < len(addrs) else 0)
    c.set64(OFF_DR7, dr7_for(len(addrs)) if on else 0)
    c.set64(OFF_DR6, 0)
    return bool(c.put(h))


def disarm_threads(threads, addrs):
    """Stop the whole known thread set before clearing it, then release it together.

    Resuming one thread while another still has DR7 armed can queue a fresh trap
    during cleanup. Thread-creation events must also be disarmed in the drain below.
    """
    suspended = []
    cleared = 0
    try:
        for tid, handle in list(threads.items()):
            if isinstance(tid, str):
                continue
            if k32.SuspendThread(handle) == 0xFFFFFFFF:
                exit_code = wt.DWORD()
                if not k32.GetExitCodeThread(handle, ctypes.byref(exit_code)) or exit_code.value == 259:
                    raise RuntimeError(f'Cannot suspend live thread {tid} for debug-register cleanup')
                continue
            suspended.append((tid, handle))
        for tid, handle in suspended:
            if not arm(handle, addrs, on=False):
                raise RuntimeError(f'Failed to clear debug registers on thread {tid}')
            check = Ctx()
            if not check.get(handle) or check.u64(OFF_DR7) != 0:
                raise RuntimeError(f'DR7 clear was not verified on thread {tid}')
            cleared += 1
    finally:
        for _, handle in reversed(suspended):
            k32.ResumeThread(handle)
    return cleared


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--seconds', type=float, default=120)
    ap.add_argument('--addr', action='append', default=[])
    ap.add_argument('--player-array', type=int)
    ap.add_argument('--field', action='append', default=[])
    ap.add_argument('--out', default=os.path.join(HERE, 'd1-watchwrite.jsonl'))
    ap.add_argument('--max-hits', type=int, default=200)
    ap.add_argument('--read', action='append', default=[], help='absolute 8-aligned address: break on read or write (8 bytes)')
    ap.add_argument('--exec', action='append', default=[], help='eboot-relative address: execute breakpoint; logs rax/rbp/rdi and 16 bytes at [rbp-0x150]')
    ap.add_argument('--peek', action='append', default=[], help='on an exec hit, log the u32 at this register expression, e.g. "rcx+rdx*4+0xc" (repeatable)')
    ap.add_argument('--log-av', action='store_true', help='log access violations outside the guest image (still passed on)')
    ap.add_argument('--av-lo', type=lambda v: int(v, 0), default=0, help='only log AVs with rip >= this')
    ap.add_argument('--av-skip', type=lambda v: {int(x, 0) for x in v.split(',')}, default=set(), help='comma-separated RIPs never to log (routine faults)')
    ap.add_argument('--av-in-image', action='store_true', help='also log AVs inside the guest image')
    ap.add_argument('--av-hi', type=lambda v: int(v, 0), default=1 << 64, help='only log AVs with rip < this')
    a = ap.parse_args()
    p, base = wg['open_client']()
    addrs = [int(x, 16) for x in a.addr]
    if a.player_array is not None:
        T = base + 0x2689580
        entry = p.u(T + 8, 'Q') + a.player_array * p.u(T + 0x10, 'I')
        addrs += [entry + int(f, 16) for f in a.field]
    for x in a.read:
        READ.add(len(addrs)); addrs.append(int(x, 16))
    for x in a.exec:
        EXEC.add(len(addrs)); addrs.append(base + int(x, 16))
    if not 1 <= len(addrs) <= 4 or any(x & 3 for i, x in enumerate(addrs) if i not in EXEC):
        sys.exit(f'need 1..4 dword-aligned addresses, got {[hex(x) for x in addrs]}')
    pid = int(__import__('subprocess').run(['tasklist', '/FI', 'IMAGENAME eq shadPS4.exe', '/FO', 'CSV', '/NH'],
              capture_output=True, text=True).stdout.split('","')[1])
    out = open(a.out, 'a')
    def log(**kw):
        kw['t'] = time.time(); out.write(json.dumps(kw) + '\n'); out.flush()
        print(json.dumps(kw), flush=True)
    log(event='start', pid=pid, base=hex(base), addrs=[hex(x) for x in addrs])
    if not k32.DebugActiveProcess(pid):
        sys.exit(f'DebugActiveProcess failed ({ctypes.get_last_error()})')
    k32.DebugSetProcessKillOnExit(False)
    threads = {}
    ev = ctypes.create_string_buffer(256)
    end = time.time() + a.seconds
    hits = 0
    try:
        while time.time() < end and hits < a.max_hits:
            if not k32.WaitForDebugEvent(ev, 200):
                continue
            code, epid, tid = struct.unpack_from('<III', ev.raw, 0)
            cont = DBG_CONTINUE
            if code == CREATE_PROCESS:
                hfile, hproc, hthr = struct.unpack_from('<QQQ', ev.raw, 16)
                if hfile: k32.CloseHandle(hfile)
                threads[tid] = hthr; arm(hthr, addrs)
            elif code == CREATE_THREAD:
                hthr, = struct.unpack_from('<Q', ev.raw, 16)
                threads[tid] = hthr; arm(hthr, addrs)
            elif code == EXIT_THREAD:
                threads.pop(tid, None)
            elif code == LOAD_DLL:
                hfile, = struct.unpack_from('<Q', ev.raw, 16)
                if hfile: k32.CloseHandle(hfile)
            elif code == EXIT_PROCESS:
                log(event='process exited'); return 1
            elif code == EXCEPTION_DEBUG_EVENT:
                exc, = struct.unpack_from('<I', ev.raw, 16)
                if exc == SINGLE_STEP and tid in threads:
                    c = Ctx(); h = threads[tid]
                    if c.get(h) and c.u64(OFF_DR6) & 0xF:
                        rip, rsp, dr6 = c.u64(OFF_RIP), c.u64(OFF_RSP), c.u64(OFF_DR6)
                        try:
                            stack = struct.unpack('<64Q', p.read(rsp, 512))
                        except OSError:
                            stack = ()
                        img = [hex(q - base) for q in stack if base <= q < base + 0x18D8000][:12]
                        which = [hex(addrs[i]) for i in range(4) if dr6 >> i & 1]
                        if any(i in EXEC for i in range(4) if dr6 >> i & 1):
                            rax, rbp, rdi = c.u64(OFF_RAX), c.u64(OFF_RBP), c.u64(OFF_RDI)
                            try:
                                dp = p.u(rbp - 0x150, 'Q'); defn = p.read(dp, 0x10).hex(); dtype = p.read(dp + 0x8E, 1)[0]
                            except OSError:
                                dp, defn, dtype = 0, '', None
                            regs = dict(rax=rax, rbx=c.u64(OFF_RBX), rcx=c.u64(OFF_RCX), rdx=c.u64(OFF_RDX),
                                        rsi=c.u64(OFF_RSI), rdi=rdi, rbp=rbp, rsp=rsp, **{f'r{n}': c.u64(0xB8 + (n - 8) * 8) for n in range(8, 16)})
                            peeks = {}
                            for expr in a.peek:
                                try:
                                    if expr.endswith('='):
                                        peeks[expr] = hex(eval(expr[:-1], {'__builtins__': {}}, regs) & 0xFFFFFFFFFFFFFFFF)
                                        continue
                                    at = eval(expr.rstrip('!'), {'__builtins__': {}}, regs) & 0xFFFFFFFFFFFFFFFF
                                    peeks[expr] = hex(at) + ':' + p.read(at, 0x18).hex() if expr.endswith('!') else hex(p.u(at, 'I'))
                                except Exception as e:
                                    peeks[expr] = f'unreadable ({e})'
                            log(event='exec', tid=tid, rip_img=hex(rip - base), rax=hex(rax), al=rax & 0xFF,
                                rdi=hex(rdi), rsi=hex(c.u64(OFF_RSI)), rdx=hex(c.u64(OFF_RDX)), rcx=hex(regs['rcx']),
                                defn=hex(dp), defn_head=defn, defn_type=dtype, stack_img=img, peek=peeks)
                            ctypes.c_uint32.from_address(c.addr + OFF_EFLAGS).value |= 0x10000   # RF
                        else:
                            log(event='write', tid=tid, rip=hex(rip), rip_img=hex(rip - base) if base <= rip < base + 0x18D8000 else None,
                                which=which, values=[hex(p.u(x, 'I')) for x in addrs], stack_img=img)
                        hits += 1
                        c.set64(OFF_DR6, 0); c.put(h)
                    else:
                        cont = DBG_EXCEPTION_NOT_HANDLED
                elif exc == BREAKPOINT and not threads.get('_attach_bp'):
                    threads['_attach_bp'] = 1      # the attach break-in
                else:
                    # --log-av: record access violations outside the guest image (the emulator's own
                    # handlers still get every one, first chance included - nothing is swallowed).
                    if a.log_av and exc == 0xC0000005 and tid in threads and threads.get('_avs', 0) < 50:
                        c = Ctx()
                        if c.get(threads[tid]):
                            rip, rsp = c.u64(OFF_RIP), c.u64(OFF_RSP)
                            if (a.av_in_image or not base <= rip < base + 0x18D8000) and (a.av_lo <= rip < a.av_hi) and rip not in a.av_skip:
                                try:
                                    stack = struct.unpack('<128Q', p.read(rsp, 1024))
                                except OSError:
                                    stack = ()
                                img = [hex(q - base) for q in stack if base <= q < base + 0x18D8000][:16]
                                threads['_avs'] = threads.get('_avs', 0) + 1   # only logged faults count
                                log(event='av', tid=tid, rip=hex(rip), rax=hex(c.u64(OFF_RAX)),
                                    rcx=hex(c.u64(0x80)), rdx=hex(c.u64(OFF_RDX)), rbx=hex(c.u64(0x90)),
                                    rdi=hex(c.u64(OFF_RDI)), rsi=hex(c.u64(OFF_RSI)), stack_img=img,
                                    stack_raw=[hex(q) for q in stack[:24]])
                    cont = DBG_EXCEPTION_NOT_HANDLED
            k32.ContinueDebugEvent(epid, tid, cont)
    finally:
        cleared = disarm_threads(threads, addrs)
        # Drain before detaching: a hit already raised on another thread is still queued, and once
        # detached it reaches the emulator as an unhandled 0x80000004 and kills it (13 Oct, run
        # bundle7d, at the watched store 0x409CAD). Swallow single-steps until the queue is quiet.
        # 1 s was not always enough on a hot execute breakpoint (26 Oct: 0x28CFCA, max-hits 1).
        quiet_until = time.time() + 3.0
        while time.time() < quiet_until:
            if not k32.WaitForDebugEvent(ev, 100):
                continue
            code, epid, tid = struct.unpack_from('<III', ev.raw, 0)
            cont = DBG_CONTINUE
            if code == EXCEPTION_DEBUG_EVENT:
                exc, = struct.unpack_from('<I', ev.raw, 16)
                if exc == SINGLE_STEP:
                    h = threads.get(tid)
                    if h:
                        arm(h, addrs, on=False)
                    quiet_until = time.time() + 3.0
                else:
                    cont = DBG_EXCEPTION_NOT_HANDLED
            elif code == CREATE_THREAD:
                hthr, = struct.unpack_from('<Q', ev.raw, 16); threads[tid] = hthr
                # This event can have been queued before the first disarm pass. A thread
                # inherited with DR7 set might not call the watched function until minutes
                # after detach (sense32, 28 Oct). Merely remembering its handle is unsafe.
                if not arm(hthr, addrs, on=False):
                    raise RuntimeError(f'Failed to disarm teardown-created thread {tid}')
                quiet_until = time.time() + 3.0
            elif code == EXIT_THREAD:
                threads.pop(tid, None)
            elif code in (CREATE_PROCESS, LOAD_DLL):
                hfile, = struct.unpack_from('<Q', ev.raw, 16)
                if hfile: k32.CloseHandle(hfile)
            k32.ContinueDebugEvent(epid, tid, cont)
        cleared = disarm_threads(threads, addrs)
        if not k32.DebugActiveProcessStop(pid):
            raise RuntimeError(f'DebugActiveProcessStop failed ({ctypes.get_last_error()})')
        log(event='detached', hits=hits, cleared_threads=cleared, dr7_verified_zero=True)
    return 0


if __name__ == '__main__':
    sys.exit(main())
