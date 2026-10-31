/* SPDX-License-Identifier: GPL-2.0-or-later
 * Frame-rate independence, phases 0 and 1 (FRAME-RATE-INDEPENDENCE-20261006.md sections 4.2,
 * 4.3, 4.4, 4.6, 4.7, 4.9; implementation record FRAME-RATE-PHASE1-IMPL-20261006.md, whose
 * section 10 holds the review fixes).
 *
 * Everything here is opt-in and leaves the simulation alone: Havok's step h, the tick count per
 * wall second and the action sweep run exactly as the clock manifest runs them. Two switches:
 *
 *   D1_FRAME_UNITS=1 (Phase 0, the four unit fixes of section 4.2; a comma list of director,
 *   cloth, fades, ui selects some of them, for testing one at a time). Which fixes apply is read
 *   from the play configuration at install time, the fixed-clock override byte 0x18c4aa4 (01
 *   shipped, 00 under the clock manifest): override off (wall-time ticks, uncapped) all four;
 *   override on (one tick per rendered frame, the Windows locked-60 configuration) cloth and ui
 *   only, the director and fades fixes are skipped with a notice even when named, because the
 *   director's h and F10's +0x08 are already the frame's time there. "1" means all applicable.
 *     director  F11 -> 0x4cba50 -> 0x8ede20 is given h every presented frame. It now gets the
 *               wall time since it last ran (clamped), so its smoothing and transitions run at
 *               one speed at any frame rate. A hook at the director's entry rewrites XMM0.
 *     cloth     0x7f0905 / 0x7f090f load a constant 1/30 s every frame. Hooks at both loads
 *               give the frame's own share of the catch-up group's time: the published frame
 *               record's +0x08 (measured frame: accepted - (n-1) x period, 0x590def; a
 *               continuation frame: one period, 0x590d61; a zero-tick frame: accepted,
 *               0x590e2b), which sums to the group's accepted time. The packet's +0x192e8 is
 *               the whole group's accepted time on every frame of the group (the continuation
 *               path copies it, 0x590d92), so it would integrate cloth twice in a 2-tick group.
 *               The share is stashed per packet by the view-build wrap (rdi = the packet at
 *               0x7ea1fc / 0x7eb209) and looked up by the packet in RDX at the cloth site. The
 *               engine's guard (dt under 1 ms: dt 0 and a pause byte, 0x7f08ee) is kept.
 *     fades     F10 passes rec+0x08 (the frame's elapsed time in native mode) to 0x7ccca0,
 *               0x7d6fb0 and 0x8722c0 on tick frames only. Hooks at their entries rewrite XMM0
 *               to h, one tick per call.
 *     UI clock  0x94b160 adds trunc(dt x 1000) ms every presented frame and drops the fraction.
 *               Hooks at its two dt loads (0x94b1c2 game time, 0x94b1cc wall time) carry the
 *               fraction per path and hand the engine the integer in EAX; the lock add stays.
 *
 *   D1_INTERP=1 (Phase 1, sections 4.3 and 4.4). With the fixed-clock override byte 01 there is
 *   no fraction to blend by: 0x5905c0 writes G2+8 at 0x5908c4, before the override test at
 *   0x5908cb, and the override replaces only due (ebx = 1) and accepted (xmm1 = G+8), so G2+8 is
 *   the discarded accumulator's phase against its own grid, not the phase of any tick the game
 *   ran. There D1_INTERP uses alpha 1 (no camera blend, no shader-time add) and its director
 *   gating is not installed (the Phase 0 wall-time dt of its stall branch must not apply with
 *   the byte 01); a notice says so. Everything below is the override-off behaviour.
 *     alpha     the engine's kept fractional tick G2+8 (G2 = *0x26a3c08), read on the extract
 *               path of the same frame (0x5905c0 wrote it at this frame's start; on
 *               continuation frames it is not called, so the measured frame's fraction
 *               stands). Non-final frames of a catch-up group (record +0x2b clear) use alpha 1.
 *     director  gated to ticks: it runs only when the tick count G+0xc (incremented once per
 *               tick at 0x590a70, F1 0x74a97f) advanced since its last run, with dt = h x
 *               (ticks advanced); on zero-tick frames the hook returns to the caller before the
 *               function body. A count that moves backwards or by more than the catch-up
 *               allowance (2) was set from outside the tick (0x5905b0, the engine interface):
 *               dt h, the camera snaps, counted as a resync. When the tick stops (paused,
 *               G+0 set and G2+0 bit 1, as 0x5909e0 reads it, or no tick for 3 tick periods)
 *               the director runs every frame with the wall time since its last run (the
 *               Phase 0 rule) and nothing is blended until ticks resume. A run is published
 *               when the director returns (a wrap of its only call, 0x4cba8c in F11's
 *               0x4cba50), so a view build never pairs a new tick total with the old pose.
 *     camera    snapshots are keyed by the director's tick runs, not by a change of pose: each
 *               run publishes the ticks it consumed; the view build shifts previous = current,
 *               current = live whenever that total advanced, even if the pose is unchanged.
 *               It blends with t = (span - 1 + alpha) / span, span = ticks between the two
 *               snapshots, so a 2-tick span shows tick T-1+alpha, not T-2+2 alpha.
 *     order     on the final frame of a tick (+0x2a and +0x2b set) the build checks that the
 *               director's last run was published in this outer frame: the F0 frame serial
 *               (the byte *0x26ae448, incremented by F0 at 0x74a44b every outer frame) stamped
 *               with the run equals the serial now (on time). If not, and the tick count is
 *               ahead of the run's, the build came before the director (late): alpha is then
 *               keyed to the newest snapshot's own tick, t = (span - 1 + alpha + lead) / span,
 *               lead = ticks the count is ahead of that snapshot, so the view holds the newest
 *               pose rather than stepping back. Counted as on time, late (held), no tick.
 *     serve     nothing writes the camera globals. A thread-local flag is set by the wraps of
 *               the extract-path view build (0x7ea1fc / 0x7eb209 -> 0x813640 -> 0x812d90) and
 *               cleared right before the first-person view (0x812e5d) and after the build; the
 *               view callback's position/forward/up getter call (0x52be45 -> 0x8f2e80) is
 *               wrapped and, while the flag is set, its outputs are replaced by the blend
 *               (only when they equal the live pose read at the build's start, so the getter's
 *               own fallbacks and any concurrent write stay untouched). The game-graph build
 *               (0x812d10 -> 0x52c360), the engine interface (0x6e639f) and 0x5b71aa never see
 *               a blend. fov is served live (0x8f2b00 reads +0x4ac itself).
 *     snaps     first pose, a position jump over 2.5 units, a fov change over 20 degrees, a
 *               turn over 90 degrees together with a position step over 0.5 units (all per
 *               tick of span; a turn alone never snaps, so a fast flick is blended), a resync:
 *               no blend, and none until the next tick (one-tick quarantine).
 *     shader    0x82d7e0 (wrapped at 0x7ea22a / 0x7eb237) writes the tick-stepped wrapped times
 *               into the packet's time block; alpha x h is added to them afterwards.
 *
 * Mechanism: a 16 KiB code page within rel32 reach of the image holds one trampoline per site.
 * An entry hook replaces the pinned six-byte prologue with a five-byte jump; a mid-function
 * hook replaces one position-independent instruction and resumes at a chosen address; a
 * call-site wrap replaces a `call rel32` with a jump to a trampoline that calls the handler,
 * then the original target, then the handler again. Every trampoline saves rbp, the argument
 * and scratch registers and XMM0-15, calls ft_handler(frame, id) with the stack 16-aligned and
 * restores everything; the handler may rewrite XMM0 or RAX in the saved frame. Pinned bytes are
 * compared before anything is written; a site that differs is left out with a notice. Both hosts:
 * on Windows the handler is sysv_abi (the trampolines call it as the guest's SysV code would) and
 * the page and site writes go through VirtualAlloc/VirtualProtect (runtime_low_map first). Per
 * frame: at most about seventeen handler calls, no I/O on the hot path except one report line
 * every five seconds. */
#ifndef _WIN32
#ifndef _GNU_SOURCE
#define _GNU_SOURCE /* MAP_ANONYMOUS, MAP_FIXED_NOREPLACE */
#endif
#endif
#include "d1_frame_time.h"
#include <inttypes.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#ifdef _WIN32
/* Windows host (MSYS2 CLANG64): the guest's code is SysV, so the handler the trampolines call is
 * declared sysv_abi (runtime.h's ABI); the clock is the runtime's host clock; the trampoline page
 * is host-owned memory next to the image (runtime_low_map, else VirtualAlloc at 64 KiB hints
 * below or above it), as probe.c's install_engine_hooks takes its own page there. */
#include "runtime.h"
#include <windows.h>
#define FT_ABI ABI
#else
#include <time.h>
#include <sys/mman.h>
#define FT_ABI
#ifndef MAP_FIXED_NOREPLACE
#define MAP_FIXED_NOREPLACE 0x100000
#endif
#endif

/* Guest addresses (eboot virtual addresses; the image maps address 0 at offset 0). */
enum {
    VA_GAME_TIME_PTR = 0x26a3c00, /* -> G: +0 active byte (0x590450), +8 h, +0xc tick count */
    VA_CONTROL_PTR = 0x26a3c08,   /* -> G2: +0 flags (bit 1: paused, 0x5909e0), +8 kept fractional tick */
    VA_FRAME_RECORD = 0x26ae348,  /* two 0x38-byte slots, published index at +0x70 */
    VA_CAMERA_PTR = 0x2723060,    /* -> camera globals */
    VA_FP_VIEW = 0x26d83f0,       /* the first-person render view (lea r14 at 0x812e5d) */
    VA_FRAME_SERIAL_PTR = 0x26ae448, /* -> a byte F0 increments at the start of every outer frame (0x74a44b) */
    VA_F0_SERIAL_INC = 0x74a444,  /* mov rax,[rip+..] (= 0x26ae448); inc byte [rax], pinned at install */
    CAM_POS = 0x46c, CAM_FWD = 0x494, CAM_UP = 0x4a0, CAM_FOV = 0x4ac,
    REC_SHARE = 0x08, REC_ACCEPTED = 0x0c, REC_DUE = 0x14, REC_FRESH = 0x29, REC_TICK = 0x2a, REC_LAST = 0x2b,
    PKT_ACCEPTED = 0x192e8,       /* the packet's accepted time, written by X5 at 0x7eaf7c */
    TIME_WRAP_A = 0x0c, TIME_WRAP_B = 0x10, TIME_WRAP_C = 0x14, TIME_WRAP_D = 0x18
};

/* The saved frame a trampoline hands the handler (lowest address first). */
typedef struct {
    unsigned char xmm[16][16];
    uint64_t r11, r10, rax, r9, r8, rcx, rdx, rsi, rdi, rbp;
} FtFrame;

/* K_WRAP sites call id before the target and id + 1 after it. */
enum HandlerId {
    H_DIRECTOR, H_FADE0, H_FADE1, H_FADE2, H_UI_GAME, H_UI_WALL, H_CLOTH,
    H_VIEW_BEFORE, H_VIEW_AFTER, H_VIEW_FP, H_GETTER_BEFORE, H_GETTER_AFTER,
    H_TIME_BEFORE, H_TIME_AFTER, H_DIRCALL_BEFORE, H_DIRCALL_AFTER
};
enum Kind { K_ENTRY, K_ENTRY_SKIP, K_MID, K_WRAP };
/* Which switch installs a site: the four D1_FRAME_UNITS fixes one bit each, D1_INTERP one bit,
 * and D1_INTERP's director gating its own bit, since the fixed-clock override leaves it out. */
enum Group { G_DIRECTOR = 1, G_CLOTH = 2, G_FADES = 4, G_UI = 8, G_UNITS = 15, G_INTERP = 16, G_INTERP_DIR = 32 };
enum Pre { PRE_NONE, PRE_COPY, PRE_MOV_R14_FP_VIEW };

typedef struct {
    const char *name;
    uint64_t vaddr;
    enum Kind kind;
    int group;
    int id;                       /* handler id (K_WRAP: the before id, after = id + 1) */
    unsigned pinned_len;
    unsigned char pinned[8];
    unsigned char alt[8];         /* also accepted (all zero: none); the replaced instruction is not re-emitted */
    enum Pre pre;                 /* K_MID: how the displaced instruction is re-emitted */
    uint64_t resume;              /* K_MID: where the trampoline jumps back to */
} Site;

/* Every address below was read in native/out/eboot.elf with capstone (the implementation record
 * lists the disassembly); the pinned bytes are the instruction(s) each write displaces. */
static const Site sites[] = {
    {"camera director 0x8ede20 (entry, dt in XMM0)", 0x8ede20, K_ENTRY_SKIP, G_DIRECTOR | G_INTERP_DIR, H_DIRECTOR,
     6, {0x55, 0x48, 0x89, 0xe5, 0x41, 0x57}, {0}, PRE_NONE, 0},
    /* call 0x8ede20 in F11's 0x4cba50, the director's only caller (RSP 0 mod 16 there: four pushes
     * and sub 0x38): D1_INTERP publishes the run after the body has written the pose. */
    {"camera director call 0x4cba8c (F11 0x4cba50)", 0x4cba8c, K_WRAP, G_INTERP_DIR, H_DIRCALL_BEFORE,
     5, {0xe8, 0x8f, 0x23, 0x42, 0x00}, {0}, PRE_NONE, 0},
    {"fade/age 0x7ccca0 (entry, dt in XMM0)", 0x7ccca0, K_ENTRY, G_FADES, H_FADE0,
     6, {0x55, 0x48, 0x89, 0xe5, 0x41, 0x56}, {0}, PRE_NONE, 0},
    {"fade/age 0x7d6fb0 (entry, dt in XMM0)", 0x7d6fb0, K_ENTRY, G_FADES, H_FADE1,
     6, {0x55, 0x48, 0x89, 0xe5, 0x41, 0x56}, {0}, PRE_NONE, 0},
    {"fade/age 0x8722c0 (entry, dt in XMM0)", 0x8722c0, K_ENTRY, G_FADES, H_FADE2,
     6, {0x55, 0x48, 0x89, 0xe5, 0x41, 0x57}, {0}, PRE_NONE, 0},
    /* vmovss xmm0,[rbp-0x30] (game-time dt) and vmovss xmm0,[rbp-0x34] (wall dt): both resume
     * at the lock add 0x94b1dd, skipping the engine's own vmulss/vcvttss2si at 0x94b1d1. */
    {"UI clock game-time load 0x94b1c2", 0x94b1c2, K_MID, G_UI, H_UI_GAME,
     5, {0xc5, 0xfa, 0x10, 0x45, 0xd0}, {0}, PRE_COPY, 0x94b1dd},
    {"UI clock wall-time load 0x94b1cc", 0x94b1cc, K_MID, G_UI, H_UI_WALL,
     5, {0xc5, 0xfa, 0x10, 0x45, 0xcc}, {0}, PRE_COPY, 0x94b1dd},
    /* vmovss xmm0,[rip+disp] (0x1652170, 0.0333): the handler sets XMM0 to the frame's share and
     * the trampoline resumes at the store 0x7f0917. The alternative bytes are the frame-units
     * manifest's vmovss xmm0,[rdx+0x192e8], which the hook replaces just the same. */
    {"cloth dt load 0x7f0905 (flag clear path)", 0x7f0905, K_MID, G_CLOTH, H_CLOTH,
     8, {0xc5, 0xfa, 0x10, 0x05, 0x63, 0x18, 0xe6, 0x00}, {0xc5, 0xfa, 0x10, 0x82, 0xe8, 0x92, 0x01, 0x00}, PRE_NONE, 0x7f0917},
    {"cloth dt load 0x7f090f (dt over 1 ms path)", 0x7f090f, K_MID, G_CLOTH, H_CLOTH,
     8, {0xc5, 0xfa, 0x10, 0x05, 0x59, 0x18, 0xe6, 0x00}, {0xc5, 0xfa, 0x10, 0x82, 0xe8, 0x92, 0x01, 0x00}, PRE_NONE, 0x7f0917},
    /* call 0x813640 (jmp 0x812d90, the extract-path view build) from the two extract fixup jobs;
     * rdi is the frame packet at both. */
    {"view build call 0x7ea1fc (extract fixup job)", 0x7ea1fc, K_WRAP, G_UNITS | G_INTERP, H_VIEW_BEFORE,
     5, {0xe8, 0x3f, 0x94, 0x02, 0x00}, {0}, PRE_NONE, 0},
    {"view build call 0x7eb209 (second extract job)", 0x7eb209, K_WRAP, G_UNITS | G_INTERP, H_VIEW_BEFORE,
     5, {0xe8, 0x32, 0x84, 0x02, 0x00}, {0}, PRE_NONE, 0},
    /* lea r14,[rip+0x1ec558c] (= 0x26d83f0) right before the first-person view callback. */
    {"first-person view switch 0x812e5d", 0x812e5d, K_MID, G_INTERP, H_VIEW_FP,
     7, {0x4c, 0x8d, 0x35, 0x8c, 0x55, 0xec, 0x01}, {0}, PRE_MOV_R14_FP_VIEW, 0x812e64},
    /* call 0x8f2e80 (position, forward, up into rdi, rsi, rdx) inside the view callback 0x52be00. */
    {"camera getter call 0x52be45 (view callback)", 0x52be45, K_WRAP, G_INTERP, H_GETTER_BEFORE,
     5, {0xe8, 0x36, 0x70, 0x3c, 0x00}, {0}, PRE_NONE, 0},
    /* call 0x82d7e0 (the packet's time block) from the same two jobs. */
    {"shader time call 0x7ea22a", 0x7ea22a, K_WRAP, G_INTERP, H_TIME_BEFORE,
     5, {0xe8, 0xb1, 0x35, 0x04, 0x00}, {0}, PRE_NONE, 0},
    {"shader time call 0x7eb237", 0x7eb237, K_WRAP, G_INTERP, H_TIME_BEFORE,
     5, {0xe8, 0xa4, 0x25, 0x04, 0x00}, {0}, PRE_NONE, 0},
};
enum { SITE_COUNT = sizeof sites / sizeof *sites, SLOT = 1024, PAGE_BYTES = 16384 };
_Static_assert(SITE_COUNT * SLOT <= PAGE_BYTES, "frame-time trampolines exceed the page");

static unsigned char *g_image;
static int g_units, g_interp; /* g_units: the G_DIRECTOR..G_UI bits D1_FRAME_UNITS selected */
static int g_alpha_one;       /* D1_INTERP under the fixed-clock override: alpha 1, nothing blended or added */
static int g_interp_dir;      /* D1_INTERP's director gating is installed (the override off) */
static int g_dir_exit;        /* the director's call is wrapped: runs are published when it returns */
static int g_serial_ok;       /* F0's serial increment holds its pinned bytes: the extract order is measured */

/* Counters, printed every five seconds (relaxed atomics; counts only on the hot path). */
static uint64_t c_frames, c_due[4], c_continuation,
                c_director_run, c_director_skip, c_director_stall, c_director_dt_ns, c_resyncs, c_span[3], c_span_ticks,
                c_fade, c_ui, c_cloth, c_cloth_us, c_cloth_fallback,
                c_snapshots, c_snaps, c_blends, c_t_milli, c_still, c_drift, c_stall_frames,
                c_served, c_mismatch, c_fp, c_time_adds, c_build_conflicts, c_order[3], c_order_held,
                c_snap_cause[5], c_snap_lines, /* snaps by cause (SNAP_FIRST..SNAP_FOV), lines printed */
                c_cloth_groups_exact, c_cloth_groups_off, c_cloth_group_lines; /* cloth group check */
static uint64_t order_total[3]; /* the same three since install, never reset (d1_frame_time_extract_order) */
static uint64_t report_last_ns;

static uint64_t now_ns(void) {
#ifdef _WIN32
    return host_monotonic_ns();
#else
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return (uint64_t)t.tv_sec * 1000000000u + (uint64_t)t.tv_nsec;
#endif
}
static void count(uint64_t *c, uint64_t n) { __atomic_fetch_add(c, n, __ATOMIC_RELAXED); }
static uint64_t take(uint64_t *c) { return __atomic_exchange_n(c, 0, __ATOMIC_RELAXED); }

static uintptr_t guest_ptr(uint64_t vaddr) {
    uintptr_t p;
    memcpy(&p, g_image + vaddr, sizeof p);
    return p;
}
/* h and the tick count from the game-time block; h falls back to 1/60 when the block is absent. */
static float tick_h(uint32_t *ticks) {
    const uintptr_t g = guest_ptr(VA_GAME_TIME_PTR);
    float h = 1.0f / 60.0f;
    if (ticks) *ticks = 0;
    if (!g) return h;
    float v;
    memcpy(&v, (const void *)(g + 8), sizeof v);
    if (v > 0.001f && v < 0.1f) h = v;
    if (ticks) memcpy(ticks, (const void *)(g + 0xc), sizeof *ticks);
    return h;
}
/* The engine's own paused test (0x5909e0): G+0 set and G2+0 bit 1. */
static int guest_paused(void) {
    const uintptr_t g = guest_ptr(VA_GAME_TIME_PTR), g2 = guest_ptr(VA_CONTROL_PTR);
    if (!g || !g2) return 0;
    return *(const volatile unsigned char *)g != 0 && (*(const volatile unsigned char *)g2 & 2) != 0;
}
static float kept_fraction(void) {
    const uintptr_t g2 = guest_ptr(VA_CONTROL_PTR);
    float f = 0.0f;
    if (!g2) return 0.0f;
    memcpy(&f, (const void *)(g2 + 8), sizeof f);
    if (!(f >= 0.0f)) f = 0.0f; /* NaN included */
    if (f > 1.0f) f = 1.0f;
    return f;
}
static const unsigned char *frame_record(void) {
    int32_t idx;
    memcpy(&idx, g_image + VA_FRAME_RECORD + 0x70, sizeof idx);
    return g_image + VA_FRAME_RECORD + (idx & 1) * 0x38;
}
/* The outer frame's serial: the byte F0 (0x74a420, the first game phase) increments at 0x74a44b,
 * at its start and unconditionally (0x74bd40 allocates it and sets 1; 0x749c10 reads its low bit). */
static int frame_serial(uint32_t *serial) {
    if (!g_serial_ok) return 0;
    const uintptr_t p = guest_ptr(VA_FRAME_SERIAL_PTR);
    if (!p) return 0;
    *serial = *(const volatile unsigned char *)p;
    return 1;
}
/* A scalar into XMM0 with the upper lanes cleared, as a vmovss load leaves them. */
static void set_xmm0(FtFrame *f, float v) { memset(f->xmm[0], 0, 16); memcpy(f->xmm[0], &v, sizeof v); }
static float get_xmm0(const FtFrame *f) { float v; memcpy(&v, f->xmm[0], sizeof v); return v; }

static void report(uint64_t now) {
    const uint64_t last = __atomic_load_n(&report_last_ns, __ATOMIC_RELAXED);
    if (!last) { __atomic_store_n(&report_last_ns, now, __ATOMIC_RELAXED); return; }
    if (now - last < 5000000000u) return;
    if (!__atomic_compare_exchange_n(&report_last_ns, (uint64_t[]){last}, now, 0, __ATOMIC_RELAXED, __ATOMIC_RELAXED)) return;
    const uint64_t frames = take(&c_frames), d0 = take(&c_due[0]), d1 = take(&c_due[1]), d2 = take(&c_due[2]),
                   d3 = take(&c_due[3]), cont = take(&c_continuation), runs = take(&c_director_run),
                   skips = take(&c_director_skip), stall = take(&c_director_stall), dt_ns = take(&c_director_dt_ns),
                   resyncs = take(&c_resyncs), s1 = take(&c_span[0]), s2 = take(&c_span[1]), s3 = take(&c_span[2]),
                   sticks = take(&c_span_ticks), fades = take(&c_fade), ui = take(&c_ui), cloth = take(&c_cloth),
                   cloth_us = take(&c_cloth_us), cloth_fb = take(&c_cloth_fallback), snapshots = take(&c_snapshots),
                   snaps = take(&c_snaps), blends = take(&c_blends), t_milli = take(&c_t_milli), still = take(&c_still),
                   drift = take(&c_drift), stall_frames = take(&c_stall_frames), served = take(&c_served),
                   mismatch = take(&c_mismatch), fp = take(&c_fp), adds = take(&c_time_adds),
                   conflicts = take(&c_build_conflicts), on_time = take(&c_order[0]), late = take(&c_order[1]),
                   no_tick = take(&c_order[2]), held = take(&c_order_held), sc_first = take(&c_snap_cause[0]),
                   sc_resync = take(&c_snap_cause[1]), sc_pos = take(&c_snap_cause[2]), sc_turn = take(&c_snap_cause[3]),
                   sc_fov = take(&c_snap_cause[4]), cg_exact = take(&c_cloth_groups_exact),
                   cg_off = take(&c_cloth_groups_off);
    take(&c_snap_lines);
    take(&c_cloth_group_lines);
    printf("Frame time [%.1f s]: frames %" PRIu64 " (due 0/1/2/3+: %" PRIu64 "/%" PRIu64 "/%" PRIu64 "/%" PRIu64
           "; continuation %" PRIu64 "); director runs %" PRIu64 " (span 1/2/3+: %" PRIu64 "/%" PRIu64 "/%" PRIu64
           ", ticks %" PRIu64 ") skips %" PRIu64 " stall runs %" PRIu64 " resyncs %" PRIu64 " dt mean %.2f ms; fades %"
           PRIu64 "; ui %" PRIu64 "; cloth calls %" PRIu64 " dt sum %.3f s fallback %" PRIu64 "; camera snapshots %" PRIu64
           " snaps %" PRIu64 " blends %" PRIu64 " t mean %.3f still %" PRIu64 " drift %" PRIu64 " stall frames %" PRIu64
           " served %" PRIu64 " mismatched %" PRIu64 " fp switches %" PRIu64 "; shader time adds %" PRIu64
           "; build conflicts %" PRIu64 "; extract order on time %" PRIu64 " late %" PRIu64 " (held %" PRIu64
           ") no tick %" PRIu64 "; snaps by cause first %" PRIu64 " resync %" PRIu64 " position %" PRIu64
           " turn %" PRIu64 " fov %" PRIu64 "; cloth groups exact %" PRIu64 " off %" PRIu64 "\n",
           (double)(now - last) * 1e-9, frames, d0, d1, d2, d3, cont, runs, s1, s2, s3, sticks, skips, stall, resyncs,
           runs + stall ? (double)dt_ns / (double)(runs + stall) * 1e-6 : 0.0, fades, ui, cloth, (double)cloth_us * 1e-6,
           cloth_fb, snapshots, snaps, blends, blends ? (double)t_milli / (double)blends * 1e-3 : 0.0, still, drift,
           stall_frames, served, mismatch, fp, adds, conflicts, on_time, late, held, no_tick, sc_first, sc_resync, sc_pos,
           sc_turn, sc_fov, cg_exact, cg_off);
}

/* --- Director (H_DIRECTOR): dt in XMM0; return 1 to skip the function. ---
 * F11 runs it on the final frame of a catch-up group only. Published to the view build (the
 * extract graph's X2 is expected to start after F11, on another thread; the extract order
 * counter checks it): one word, so a build reads a consistent set, bits 0-23 the total of ticks
 * the director has consumed (mod 2^24), 24-47 the tick count G+0xc of its last tick run (mod
 * 2^24), 48-55 the F0 frame serial when that run was published, bit 56 set when the serial was
 * read, bit 57 once anything is published; beside it a snap counter and the stall flag. Totals,
 * not per-run values, so a view build that misses a run still sees the whole span. With the
 * director's call wrapped (H_DIRCALL_AFTER) a run is published when the director returns, after
 * its body wrote the pose; without the wrap, at the entry. */
static uint64_t director_last_wall_ns, tick_advance_ns;
static uint32_t director_last_tick;
static int director_seen;
static uint32_t dir_snap_count, dir_stalled;
static uint64_t dir_pub;
enum { PUB_MASK = 0xffffff };
/* The run waiting for the director to return. F11's thread only: frames are serialized. */
static struct { int pending, snap; uint32_t span, tick; } dir_run;

static uint32_t pub_total(uint64_t p) { return (uint32_t)p & PUB_MASK; }
static uint32_t pub_tick(uint64_t p) { return (uint32_t)(p >> 24) & PUB_MASK; }
static int pub_serial(uint64_t p, uint32_t *serial) { *serial = (uint32_t)(p >> 48) & 0xff; return (int)(p >> 56) & 1; }
static int pub_any(uint64_t p) { return (int)(p >> 57) & 1; }
/* a - b for counts kept mod 2^24, in -2^23 .. 2^23 - 1. */
static int32_t diff24(uint32_t a, uint32_t b) {
    const uint32_t d = (a - b) & PUB_MASK;
    return d & 0x800000u ? (int32_t)d - 0x1000000 : (int32_t)d;
}
static void publish_pending(void) {
    if (!dir_run.pending) return;
    dir_run.pending = 0;
    uint32_t serial = 0;
    const int have_serial = frame_serial(&serial);
    if (dir_run.snap) __atomic_add_fetch(&dir_snap_count, 1, __ATOMIC_RELAXED);
    const uint64_t old = __atomic_load_n(&dir_pub, __ATOMIC_RELAXED); /* one writer: F11's thread */
    const uint64_t p = (uint64_t)((pub_total(old) + dir_run.span) & PUB_MASK) | (uint64_t)(dir_run.tick & PUB_MASK) << 24 |
                       (uint64_t)(serial & 0xff) << 48 | (uint64_t)(have_serial != 0) << 56 | (uint64_t)1 << 57;
    __atomic_store_n(&dir_pub, p, __ATOMIC_RELEASE);
}
/* A tick run of `span` ticks at tick count `tick`; published now, or when the director returns. */
static void director_run(uint32_t span, int snap, uint32_t tick) {
    if (dir_run.pending) { span += dir_run.span; snap |= dir_run.snap; } /* a return not seen: keep the ticks */
    dir_run.pending = 1; dir_run.span = span; dir_run.snap = snap; dir_run.tick = tick;
    if (!g_dir_exit) publish_pending();
}
static uint64_t clamp_gap(uint64_t gap) {
    if (gap < 500000u) gap = 500000u;
    if (gap > 100000000u) gap = 100000000u;
    return gap;
}
static int on_director(FtFrame *f) {
    uint32_t ticks = 0;
    const float h = tick_h(&ticks);
    const uint64_t now = now_ns();
    report(now);
    if (!guest_ptr(VA_GAME_TIME_PTR)) return 0; /* no game-time block yet: the engine's own dt */
    if (!director_seen) {
        director_seen = 1;
        director_last_tick = ticks;
        director_last_wall_ns = tick_advance_ns = now;
        count(&c_director_run, 1);
        count(&c_director_dt_ns, (uint64_t)((double)get_xmm0(f) * 1e9));
        if (g_interp_dir) director_run(1, 1, ticks);
        return 0;
    }
    if (g_interp_dir) {
        const int32_t delta = (int32_t)(ticks - director_last_tick);
        if (delta == 0) {
            const uint64_t stall_after = (uint64_t)(3.0 * (double)h * 1e9);
            if (!guest_paused() && now - tick_advance_ns < stall_after) { count(&c_director_skip, 1); return 1; }
            /* The tick has stopped: the Phase 0 rule, and no blending until it resumes. */
            const uint64_t gap = clamp_gap(now - director_last_wall_ns);
            director_last_wall_ns = now;
            __atomic_store_n(&dir_stalled, 1, __ATOMIC_RELEASE);
            set_xmm0(f, (float)((double)gap * 1e-9));
            count(&c_director_stall, 1);
            count(&c_director_dt_ns, gap);
            return 0;
        }
        tick_advance_ns = now;
        __atomic_store_n(&dir_stalled, 0, __ATOMIC_RELEASE);
        uint32_t span = (uint32_t)delta;
        int snap = 0;
        if (delta < 0 || delta > 2) { /* set from outside the tick (0x5905b0), or a long gap */
            span = 1;
            snap = 1;
            count(&c_resyncs, 1);
        }
        director_last_tick = ticks;
        director_last_wall_ns = now;
        set_xmm0(f, h * (float)span);
        count(&c_director_run, 1);
        count(&c_span[span >= 3 ? 2 : span - 1], 1);
        count(&c_span_ticks, span);
        count(&c_director_dt_ns, (uint64_t)((double)h * (double)span * 1e9));
        director_run(span, snap, ticks);
        return 0;
    }
    /* Units only: the wall time since the director last ran, clamped to 0.5..100 ms. */
    const uint64_t gap = clamp_gap(now - director_last_wall_ns);
    director_last_wall_ns = now;
    director_last_tick = ticks;
    set_xmm0(f, (float)((double)gap * 1e-9));
    count(&c_director_run, 1);
    count(&c_director_dt_ns, gap);
    return 0;
}

/* --- UI clock (H_UI_GAME, H_UI_WALL): XMM0 holds dt after the displaced load; EAX gets the
 * integer milliseconds with the fraction carried per path. 0x94b160 holds no lock here: the
 * cmpxchg loop at 0x94b190 only copies the counter 0x272b5c8 into 0x272b5cc. Plain statics are
 * safe because 0x94b160 has one caller, F11 at 0x74bb4f, which runs once per frame, and frames
 * are serialized. A second caller would need these to become atomics. --- */
static float ui_fraction[2];
static void on_ui(FtFrame *f, int path) {
    const float dt = get_xmm0(f);
    float v = dt * 1000.0f + ui_fraction[path];
    int32_t ms = 0;
    if (fabsf(v) < 1e9f) {
        ms = (int32_t)v; /* truncation toward zero, as vcvttss2si */
        ui_fraction[path] = v - (float)ms;
    } else ui_fraction[path] = 0.0f;
    f->rax = (uint64_t)(uint32_t)ms;
    count(&c_ui, 1);
}

/* --- Cloth (H_CLOTH): RDX is the packet. The view-build wrap stashes the frame's share per
 * packet; the accepted time is stored with it, so a stash from another frame does not match. --- */
typedef struct { uintptr_t packet; uint32_t accepted_bits, share_bits; } ClothStash;
static ClothStash cloth_stash[8];
static uint32_t cloth_next;
static void cloth_stash_put(uintptr_t packet, const unsigned char *rec) {
    const uint32_t i = __atomic_fetch_add(&cloth_next, 1, __ATOMIC_RELAXED) & 7;
    ClothStash *s = &cloth_stash[i];
    uint32_t accepted_bits, share_bits;
    memcpy(&accepted_bits, rec + REC_ACCEPTED, 4);
    memcpy(&share_bits, rec + REC_SHARE, 4);
    __atomic_store_n(&s->packet, (uintptr_t)0, __ATOMIC_RELAXED);
    __atomic_store_n(&s->accepted_bits, accepted_bits, __ATOMIC_RELAXED);
    __atomic_store_n(&s->share_bits, share_bits, __ATOMIC_RELAXED);
    __atomic_store_n(&s->packet, packet, __ATOMIC_RELEASE);
}
static int cloth_stash_find(uintptr_t packet, uint32_t accepted_bits, float *share) {
    const uint32_t next = __atomic_load_n(&cloth_next, __ATOMIC_RELAXED);
    for (uint32_t k = 1; k <= 8; ++k) {
        const ClothStash *s = &cloth_stash[(next - k) & 7];
        if (__atomic_load_n(&s->packet, __ATOMIC_ACQUIRE) != packet) continue;
        if (__atomic_load_n(&s->accepted_bits, __ATOMIC_RELAXED) != accepted_bits) continue;
        const uint32_t share_bits = __atomic_load_n(&s->share_bits, __ATOMIC_RELAXED);
        memcpy(share, &share_bits, 4);
        return 1;
    }
    return 0;
}
/* Group check (asked after B2 summed 4.5-4.75 s of cloth time per 5 s window below 60 FPS): the
 * calls of one catch-up group carry the same packet accepted time, so consecutive calls with
 * equal accepted bits are one group, and the shares used should sum to that accepted time. A
 * group whose sum is off by more than 0.1 ms is counted, and up to eight a window are printed
 * with each share. Diagnostic only: the shares handed to the engine are unchanged. */
static int cloth_group_lock;
static struct { uint32_t bits, n; float accepted, sum, share[4]; } cloth_group;
static void cloth_group_close(void) {
    if (!cloth_group.n) return;
    if (fabsf(cloth_group.sum - cloth_group.accepted) <= 1e-4f) { count(&c_cloth_groups_exact, 1); return; }
    count(&c_cloth_groups_off, 1);
    if (__atomic_add_fetch(&c_cloth_group_lines, 1, __ATOMIC_RELAXED) > 8) return;
    printf("Frame time: cloth group off: accepted %.3f ms, %u call(s), shares %.3f %.3f %.3f %.3f ms, sum %.3f ms\n",
           cloth_group.accepted * 1e3, cloth_group.n, cloth_group.share[0] * 1e3, cloth_group.share[1] * 1e3,
           cloth_group.share[2] * 1e3, cloth_group.share[3] * 1e3, cloth_group.sum * 1e3);
}
static void cloth_group_add(uint32_t bits, float accepted, float dt) {
    if (__atomic_exchange_n(&cloth_group_lock, 1, __ATOMIC_ACQUIRE)) return;
    if (bits != cloth_group.bits) {
        cloth_group_close();
        memset(&cloth_group, 0, sizeof cloth_group);
        cloth_group.bits = bits; cloth_group.accepted = accepted;
    }
    if (cloth_group.n < 4) cloth_group.share[cloth_group.n] = dt;
    cloth_group.n++;
    cloth_group.sum += dt;
    __atomic_store_n(&cloth_group_lock, 0, __ATOMIC_RELEASE);
}
static void on_cloth(FtFrame *f) {
    const uintptr_t packet = (uintptr_t)f->rdx;
    float accepted;
    memcpy(&accepted, (const void *)(packet + PKT_ACCEPTED), sizeof accepted);
    uint32_t bits;
    memcpy(&bits, &accepted, 4);
    float dt;
    if (!cloth_stash_find(packet, bits, &dt)) { dt = accepted; count(&c_cloth_fallback, 1); }
    if (!(dt >= 0.0f)) dt = 0.0f;
    if (dt > 0.1f) dt = 0.1f;
    set_xmm0(f, dt);
    count(&c_cloth, 1);
    count(&c_cloth_us, (uint64_t)((double)dt * 1e6));
    cloth_group_add(bits, accepted, dt);
}

/* --- Camera (H_VIEW_BEFORE / H_VIEW_AFTER / H_VIEW_FP / H_GETTER_BEFORE / H_GETTER_AFTER) --- */
typedef struct { float pos[3], fwd[3], up[3], fov; } Pose;
static Pose cam_prev, cam_cur;           /* guarded by build_lock */
static uint32_t cam_ticks, cam_snaps_seen, cam_span = 1;
static uint32_t cam_tick;                /* the tick count G+0xc (mod 2^24) of the run cam_cur came from */
static int cam_have, cam_quarantine;
static int build_lock;
static __thread Pose tl_blend, tl_live;  /* the blend this thread serves, and the live pose it replaces */
static __thread int tl_serve;            /* set inside the extract-path main view build */
static __thread uintptr_t tl_out[3];     /* the getter's output pointers (rdi, rsi, rdx) */

static void read_pose(uintptr_t cam, Pose *p) {
    memcpy(p->pos, (const void *)(cam + CAM_POS), sizeof p->pos);
    memcpy(p->fwd, (const void *)(cam + CAM_FWD), sizeof p->fwd);
    memcpy(p->up, (const void *)(cam + CAM_UP), sizeof p->up);
    memcpy(&p->fov, (const void *)(cam + CAM_FOV), sizeof p->fov);
}
static float dot3(const float a[3], const float b[3]) { return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]; }
static void normalize3(float v[3]) {
    const float l = sqrtf(dot3(v, v));
    if (l > 1e-12f) { v[0] /= l; v[1] /= l; v[2] /= l; }
}
static int pose_finite(const Pose *p) {
    const float *v = (const float *)p;
    for (unsigned i = 0; i < sizeof *p / sizeof(float); ++i) if (!isfinite(v[i])) return 0;
    return 1;
}
/* A discontinuity between two consecutive snapshots `span` ticks apart: snap instead of blending.
 * The per-tick thresholds scale with the span. A turn alone never snaps: a flick on a
 * high-sensitivity mouse turns well past 45 degrees a tick and must stay blended. A turn over
 * 90 degrees a tick snaps only together with a position step over 0.5 units a tick, a cut to a
 * nearby viewpoint (a third-person camera, a respawn close by) that the 2.5-unit jump misses; on
 * foot the camera moves well under 0.5 units a tick. At a span of 2 or more, 90 degrees a tick
 * is 180 or more, which no turn exceeds. */
/* Returns the cause: 0 none, SNAP_POSITION, SNAP_TURN or SNAP_FOV. */
enum { SNAP_FIRST, SNAP_RESYNC, SNAP_POSITION, SNAP_TURN, SNAP_FOV, SNAP_CAUSES };
static int pose_jump(const Pose *a, const Pose *b, uint32_t span) {
    const float s = (float)(span ? span : 1);
    float d[3] = {b->pos[0] - a->pos[0], b->pos[1] - a->pos[1], b->pos[2] - a->pos[2]};
    const float step2 = dot3(d, d);
    if (step2 > (2.5f * s) * (2.5f * s)) return SNAP_POSITION;
    if (span < 2 && step2 > (0.5f * s) * (0.5f * s)) {
        float fa[3] = {a->fwd[0], a->fwd[1], a->fwd[2]}, fb[3] = {b->fwd[0], b->fwd[1], b->fwd[2]};
        normalize3(fa); normalize3(fb);
        if (dot3(fa, fb) < 0.0f) return SNAP_TURN; /* cos 90 degrees */
    }
    /* fov: radians when the value is small, degrees otherwise (the unit is not pinned). */
    const float fov_jump = (fabsf(a->fov) < 4.0f ? 0.349f : 20.0f) * s;
    if (fabsf(b->fov - a->fov) > fov_jump) return SNAP_FOV;
    return 0;
}
static void blend_pose(const Pose *a, const Pose *b, float t, Pose *out) {
    for (int i = 0; i < 3; ++i) {
        out->pos[i] = a->pos[i] + (b->pos[i] - a->pos[i]) * t;
        out->fwd[i] = a->fwd[i] + (b->fwd[i] - a->fwd[i]) * t;
        out->up[i] = a->up[i] + (b->up[i] - a->up[i]) * t;
    }
    out->fov = a->fov + (b->fov - a->fov) * t;
    /* A turn near 180 degrees within one span (no longer a snap) lerps through zero: the newer axis. */
    if (dot3(out->fwd, out->fwd) < 1e-6f) memcpy(out->fwd, b->fwd, sizeof out->fwd);
    if (dot3(out->up, out->up) < 1e-6f) memcpy(out->up, b->up, sizeof out->up);
    normalize3(out->fwd);
    const float k = dot3(out->up, out->fwd);
    for (int i = 0; i < 3; ++i) out->up[i] -= out->fwd[i] * k;
    normalize3(out->up);
}

/* Where this build stands against the director (FRAME-RATE-PHASE1-IMPL section 4.3, "Extract
 * order"). Only the final frame of a tick (+0x2a set by F0, 0x74a73d / 0x74a74b; +0x2b) is
 * classified, since only there must the director have run: on time when its last run was
 * published in this outer frame (the F0 serials match); late when it was not and the tick
 * count is ahead of that run's (the build came first: the run is still to come, or will not
 * come); no tick when it was not and the count is not ahead (nothing was pending: a tick the
 * count did not record, or a build before F1's increment). */
enum { ORDER_NONE, ORDER_ON_TIME, ORDER_LATE, ORDER_NO_TICK };
static int extract_order(uint64_t p, const unsigned char *rec, uint32_t ticks_now) {
    uint32_t at_run, now;
    if (!rec[REC_TICK] || !rec[REC_LAST] || !pub_any(p) || !pub_serial(p, &at_run) || !frame_serial(&now)) return ORDER_NONE;
    if ((now & 0xff) == at_run) return ORDER_ON_TIME;
    return diff24(ticks_now & PUB_MASK, pub_tick(p)) != 0 ? ORDER_LATE : ORDER_NO_TICK;
}

/* One snap: counted by cause, and up to eight a report window printed with the time since the
 * first snapshot and what triggered it (the per-tick step, the turn as the cosine between the
 * two forward axes, the fov before and after), so a steady cadence can be told from cuts. */
_Static_assert(SNAP_CAUSES == sizeof c_snap_cause / sizeof *c_snap_cause, "one counter per snap cause");
static void snap_note(int cause, uint32_t span, const Pose *a, const Pose *b) {
    static const char *const names[SNAP_CAUSES] = {"first pose", "resync", "position", "turn", "fov"};
    static uint64_t t0;
    const uint64_t now = now_ns();
    if (!t0) t0 = now;
    count(&c_snap_cause[cause], 1);
    if (__atomic_add_fetch(&c_snap_lines, 1, __ATOMIC_RELAXED) > 8) return;
    float d[3] = {b->pos[0] - a->pos[0], b->pos[1] - a->pos[1], b->pos[2] - a->pos[2]};
    float fa[3] = {a->fwd[0], a->fwd[1], a->fwd[2]}, fb[3] = {b->fwd[0], b->fwd[1], b->fwd[2]};
    normalize3(fa); normalize3(fb);
    printf("Frame time: snap (%s) at %.3f s: span %u, step %.3f, forward cos %.3f, fov %.4f -> %.4f\n",
           names[cause], (double)(now - t0) * 1e-9, span, sqrtf(dot3(d, d)), dot3(fa, fb), a->fov, b->fov);
}

/* The snapshot rule and the blend for this build; called with build_lock held. */
static void camera_update(uintptr_t cam, const unsigned char *rec) {
    Pose now;
    read_pose(cam, &now);
    if (!pose_finite(&now)) return;
    const uint64_t p = __atomic_load_n(&dir_pub, __ATOMIC_ACQUIRE);
    const uint32_t snaps = __atomic_load_n(&dir_snap_count, __ATOMIC_RELAXED);
    uint32_t ticks_now = 0;
    tick_h(&ticks_now);
    const int order = extract_order(p, rec, ticks_now);
    if (order != ORDER_NONE) { count(&c_order[order - 1], 1); count(&order_total[order - 1], 1); }
    if (__atomic_load_n(&dir_stalled, __ATOMIC_ACQUIRE)) { cam_have = 0; count(&c_stall_frames, 1); return; }
    const uint32_t total = pub_total(p);
    if (!cam_have) {
        snap_note(SNAP_FIRST, 1, &now, &now);
        cam_prev = cam_cur = now;
        cam_have = cam_quarantine = 1;
        cam_ticks = total; cam_tick = pub_tick(p); cam_snaps_seen = snaps; cam_span = 1;
        count(&c_snaps, 1);
    } else if (total != cam_ticks) {
        /* The director consumed ticks since the last snapshot: shift, even if the pose is unchanged. */
        const uint32_t span = (total - cam_ticks) & PUB_MASK;
        count(&c_snapshots, 1);
        const int cause = snaps != cam_snaps_seen ? SNAP_RESYNC : pose_jump(&cam_cur, &now, span);
        if (cause) {
            snap_note(cause, span, &cam_cur, &now);
            cam_prev = cam_cur = now; cam_quarantine = 1; count(&c_snaps, 1);
        } else if (cam_quarantine) {
            cam_prev = cam_cur = now; cam_quarantine = 0;
        } else {
            cam_prev = cam_cur; cam_cur = now;
        }
        cam_ticks = total; cam_tick = pub_tick(p); cam_snaps_seen = snaps; cam_span = span ? span : 1;
    } else if (memcmp(&now, &cam_cur, sizeof now) != 0) {
        count(&c_drift, 1); /* the pose moved without a director tick run */
    }
    const float alpha = rec[REC_LAST] ? kept_fraction() : 1.0f;
    /* A late build: alpha belongs to the tick count now, and the newest snapshot is `lead` ticks
     * older, so the view time T-1+alpha lies past it: t >= 1 holds the newest pose instead of
     * blending an older pair by this frame's alpha, which would step the view back. On time
     * lead is 0 and this is the ordinary blend. */
    float lead = 0.0f;
    if (order == ORDER_LATE) {
        const int32_t d = diff24(ticks_now & PUB_MASK, cam_tick);
        if (d > 0) { lead = (float)(d > 2 ? 2 : d); count(&c_order_held, 1); }
    }
    float t = ((float)cam_span - 1.0f + alpha + lead) / (float)cam_span;
    if (!(t >= 0.0f)) t = 0.0f;
    if (t >= 1.0f) return;
    if (memcmp(&cam_prev, &cam_cur, sizeof cam_prev) == 0) { count(&c_still, 1); return; }
    blend_pose(&cam_prev, &cam_cur, t, &tl_blend);
    tl_live = now;
    tl_serve = 1;
    count(&c_blends, 1);
    count(&c_t_milli, (uint64_t)(t * 1000.0f));
}
static void on_view_before(FtFrame *f) {
    const unsigned char *rec = frame_record();
    int32_t due;
    memcpy(&due, rec + REC_DUE, sizeof due);
    count(&c_frames, 1);
    count(&c_due[due < 0 ? 0 : due > 3 ? 3 : due], 1);
    if (!rec[REC_FRESH]) count(&c_continuation, 1);
    report(now_ns());
    if (g_units & G_CLOTH) cloth_stash_put((uintptr_t)f->rdi, rec);
    tl_serve = 0;
    /* Under the fixed-clock override alpha is 1: the live pose is the newest tick, nothing to blend. */
    if (!g_interp || g_alpha_one) return;
    const uintptr_t cam = guest_ptr(VA_CAMERA_PTR);
    if (!cam) return;
    if (__atomic_exchange_n(&build_lock, 1, __ATOMIC_ACQUIRE)) { count(&c_build_conflicts, 1); return; }
    camera_update(cam, rec);
    __atomic_store_n(&build_lock, 0, __ATOMIC_RELEASE);
}
static void on_getter_after(void) {
    if (!tl_serve || !tl_out[0] || !tl_out[1] || !tl_out[2]) return;
    float *pos = (float *)tl_out[0], *fwd = (float *)tl_out[1], *up = (float *)tl_out[2];
    /* Only when the getter returned the live pose this build started from (no fallback, no write since). */
    if (memcmp(pos, tl_live.pos, 12) || memcmp(fwd, tl_live.fwd, 12) || memcmp(up, tl_live.up, 12)) {
        count(&c_mismatch, 1);
        return;
    }
    memcpy(pos, tl_blend.pos, 12);
    memcpy(fwd, tl_blend.fwd, 12);
    memcpy(up, tl_blend.up, 12);
    count(&c_served, 1);
}

/* --- Shader time (H_TIME_BEFORE / H_TIME_AFTER): rdi is the packet's time block. --- */
static __thread uintptr_t time_block;
static void on_time_after(void) {
    if (!time_block) return;
    const unsigned char *rec = frame_record();
    const float alpha = rec[REC_LAST] ? kept_fraction() : 1.0f;
    const float add = alpha * tick_h(NULL);
    float a, b, c, d;
    unsigned char *t = (unsigned char *)time_block;
    memcpy(&a, t + TIME_WRAP_A, 4); memcpy(&b, t + TIME_WRAP_B, 4); memcpy(&c, t + TIME_WRAP_C, 4); memcpy(&d, t + TIME_WRAP_D, 4);
    const int d_follows_a = memcmp(&a, &d, 4) == 0;
    a += add; b += add; c += add;
    memcpy(t + TIME_WRAP_A, &a, 4); memcpy(t + TIME_WRAP_B, &b, 4); memcpy(t + TIME_WRAP_C, &c, 4);
    if (d_follows_a) memcpy(t + TIME_WRAP_D, &a, 4);
    time_block = 0;
    count(&c_time_adds, 1);
}

/* The handler every trampoline calls: SysV, rdi = the saved frame, esi = the id. */
static FT_ABI int ft_handler(FtFrame *f, uint32_t id) {
    switch (id) {
    case H_DIRECTOR: return on_director(f);
    case H_FADE0: case H_FADE1: case H_FADE2: set_xmm0(f, tick_h(NULL)); count(&c_fade, 1); return 0;
    case H_UI_GAME: on_ui(f, 0); return 0;
    case H_UI_WALL: on_ui(f, 1); return 0;
    case H_CLOTH: on_cloth(f); return 0;
    case H_VIEW_BEFORE: on_view_before(f); return 0;
    case H_VIEW_AFTER: tl_serve = 0; return 0;
    case H_VIEW_FP: tl_serve = 0; count(&c_fp, 1); return 0; /* the first-person view is built on the live pose */
    case H_GETTER_BEFORE: tl_out[0] = (uintptr_t)f->rdi; tl_out[1] = (uintptr_t)f->rsi; tl_out[2] = (uintptr_t)f->rdx; return 0;
    case H_GETTER_AFTER: on_getter_after(); return 0;
    case H_TIME_BEFORE: time_block = g_interp && !g_alpha_one ? (uintptr_t)f->rdi : 0; return 0; /* override: add 0 */
    case H_TIME_AFTER: on_time_after(); return 0;
    case H_DIRCALL_BEFORE: return 0;
    case H_DIRCALL_AFTER: publish_pending(); return 0; /* the director returned (or was skipped) */
    default: return 0;
    }
}

/* --- Trampoline emission --- */
static unsigned char *emit_u32(unsigned char *w, uint32_t v) { memcpy(w, &v, 4); return w + 4; }
/* movdqu [rsp+disp32], xmmN (store) or xmmN, [rsp+disp32] (load). */
static unsigned char *emit_xmm(unsigned char *w, int n, uint32_t disp, int load) {
    *w++ = 0xf3;
    if (n >= 8) *w++ = 0x44;
    *w++ = 0x0f; *w++ = load ? 0x6f : 0x7f;
    *w++ = (unsigned char)(0x84 | ((n & 7) << 3)); *w++ = 0x24;
    return emit_u32(w, disp);
}
/* Save the frame and call ft_handler(frame, id). `pad8`: RSP is 8 mod 16 here (function entry).
 * After this the handler's return value is in EAX and nothing has been restored yet. */
static unsigned char *emit_enter(unsigned char *w, int pad8, uint32_t id) {
    static const unsigned char pushes[] = {0x55, 0x57, 0x56, 0x52, 0x51, 0x41, 0x50, 0x41, 0x51, 0x50, 0x41, 0x52, 0x41, 0x53};
    memcpy(w, pushes, sizeof pushes); w += sizeof pushes;
    *w++ = 0x48; *w++ = 0x81; *w++ = 0xec; w = emit_u32(w, pad8 ? 0x108 : 0x100); /* sub rsp, imm32 */
    const uint32_t base = pad8 ? 8 : 0;
    for (int x = 0; x < 16; ++x) w = emit_xmm(w, x, base + 16u * (uint32_t)x, 0);
    if (pad8) { *w++ = 0x48; *w++ = 0x8d; *w++ = 0x7c; *w++ = 0x24; *w++ = 0x08; } /* lea rdi,[rsp+8] */
    else { *w++ = 0x48; *w++ = 0x89; *w++ = 0xe7; }                               /* mov rdi,rsp */
    *w++ = 0xbe; w = emit_u32(w, id);                                              /* mov esi, id */
    *w++ = 0x48; *w++ = 0xb8; { uintptr_t h = (uintptr_t)ft_handler; memcpy(w, &h, 8); w += 8; }
    *w++ = 0xff; *w++ = 0xd0;                                                      /* call rax */
    return w;
}
static unsigned char *emit_leave(unsigned char *w, int pad8) {
    static const unsigned char pops[] = {0x41, 0x5b, 0x41, 0x5a, 0x58, 0x41, 0x59, 0x41, 0x58, 0x59, 0x5a, 0x5e, 0x5f, 0x5d};
    const uint32_t base = pad8 ? 8 : 0;
    for (int x = 0; x < 16; ++x) w = emit_xmm(w, x, base + 16u * (uint32_t)x, 1);
    *w++ = 0x48; *w++ = 0x81; *w++ = 0xc4; w = emit_u32(w, pad8 ? 0x108 : 0x100); /* add rsp, imm32 */
    memcpy(w, pops, sizeof pops); w += sizeof pops;
    return w;
}
/* Write `opcode rel32` at w for an instruction that will execute at `at` (w itself for
 * trampoline code; the site for the jump written into the image). */
static int emit_rel32(unsigned char *w, const unsigned char *at, unsigned char opcode, const unsigned char *target) {
    const int64_t rel = (int64_t)((intptr_t)target - (intptr_t)(at + 5));
    if (rel < INT32_MIN || rel > INT32_MAX) return 0;
    w[0] = opcode;
    int32_t r = (int32_t)rel; memcpy(w + 1, &r, 4);
    return 1;
}
static int has_alt(const Site *s) {
    for (unsigned i = 0; i < s->pinned_len; ++i) if (s->alt[i]) return 1;
    return 0;
}

/* D1_FRAME_UNITS: "1" selects every fix that applies to the play configuration (all four with
 * the fixed-clock override off, cloth and ui with it on; see d1_frame_time_install); a comma
 * list of director, cloth, fades and ui selects those (for testing them one at a time), subject
 * to the same rule; unset, empty or "0" selects none. */
static int parse_units(const char *v) {
    if (!v || !v[0] || !strcmp(v, "0")) return 0;
    if (!strcmp(v, "1")) return G_UNITS;
    static const struct { const char *name; int bit; } names[] = {
        {"director", G_DIRECTOR}, {"cloth", G_CLOTH}, {"fades", G_FADES}, {"ui", G_UI}};
    int mask = 0;
    const char *p = v;
    while (*p) {
        const size_t n = strcspn(p, ",");
        int known = 0;
        for (unsigned i = 0; i < sizeof names / sizeof *names; ++i)
            if (strlen(names[i].name) == n && !strncmp(p, names[i].name, n)) { mask |= names[i].bit; known = 1; }
        if (!known && n) printf("Frame time: D1_FRAME_UNITS names an unknown fix '%.*s' (director, cloth, fades, ui or 1)\n", (int)n, p);
        p += n;
        if (*p == ',') ++p;
    }
    return mask;
}

/* The fixed-clock override byte (FRAME-RATE-INDEPENDENCE-20261006.md section 5, "Two play
 * configurations"): 0x5908cb reads it (`lea rax,[0x18c4aa4]; cmp byte [rax],0`) and, while it is
 * set, gives one tick of the fixed step per rendered frame. The shipped value is 01; the clock
 * manifest (guest/CUSA00219-native-clock.xml) writes 00, and guest patches are applied before
 * this runs, so the byte read here is the configuration this run plays. 1 on, 0 off, -1 the
 * byte is outside the image. */
enum { VA_CLOCK_OVERRIDE = 0x18c4aa4 };
static int clock_override(const unsigned char *image, uint64_t image_size) {
    if (VA_CLOCK_OVERRIDE >= image_size) return -1;
    return image[VA_CLOCK_OVERRIDE] != 0;
}

/* The trampoline page: PAGE_BYTES, read/write until the trampolines are written, every byte
 * within rel32 reach of the whole image (and back). */
static int page_reaches(uintptr_t page, uintptr_t base, uint64_t size) {
    const uintptr_t lo = page < base ? page : base;
    const uintptr_t hi = page + PAGE_BYTES > base + size ? page + PAGE_BYTES : base + size;
    return hi - lo < (uintptr_t)INT32_MAX - 0x1000;
}
static unsigned char *trampoline_page(unsigned char *image, uint64_t image_size) {
    const uintptr_t base = (uintptr_t)image;
#ifdef _WIN32
    /* The image lives in the placeholder-reserved guest space, where a VirtualAlloc hint cannot
     * land: runtime_low_map takes the next host-owned piece (just past the image, its traps and
     * install_engine_hooks's page). Else 64 KiB granules below and above the image. */
    unsigned char *low = runtime_low_map(PAGE_BYTES, PROT_READ | PROT_WRITE);
    if (low && page_reaches((uintptr_t)low, base, image_size)) return low;
    enum { GRANULE = 0x10000, TRIES = 48 };
    const uintptr_t below = (base & ~(uintptr_t)(GRANULE - 1)) - GRANULE;
    const uintptr_t above = (base + image_size + 2 * GRANULE - 1) & ~(uintptr_t)(GRANULE - 1);
    for (int k = 0; k < TRIES; ++k) {
        const uintptr_t hints[2] = {below - (uintptr_t)k * GRANULE, above + (uintptr_t)k * GRANULE};
        for (int h = 0; h < 2; ++h) {
            if (!page_reaches(hints[h], base, image_size)) continue;
            void *p = VirtualAlloc((void *)hints[h], PAGE_BYTES, MEM_RESERVE | MEM_COMMIT, PAGE_READWRITE);
            if (p && page_reaches((uintptr_t)p, base, image_size)) return p;
            if (p) VirtualFree(p, 0, MEM_RELEASE);
        }
    }
    return NULL;
#else
    /* Away from install_engine_hooks's page (image - 0x10000 or image + size + 0x10000). */
    const uintptr_t end = base + ((image_size + 4095) & ~(uintptr_t)4095);
    const uintptr_t candidates[4] = {base - 0x20000 - PAGE_BYTES, end + 0x20000, base - 0x40000 - PAGE_BYTES, end + 0x40000};
    for (int c = 0; c < 4; ++c) {
        void *p = mmap((void *)candidates[c], PAGE_BYTES, PROT_READ | PROT_WRITE | PROT_EXEC,
                       MAP_PRIVATE | MAP_ANONYMOUS | MAP_FIXED_NOREPLACE, -1, 0);
        if (p == MAP_FAILED) continue;
        if (page_reaches((uintptr_t)p, base, image_size)) return p;
        munmap(p, PAGE_BYTES);
    }
    return NULL;
#endif
}
/* The jump into the image. The image is still read/write here (probe.c protects the segments
 * afterwards); on Windows the write does not rely on it, as install_engine_hooks's does not. */
static int write_site(unsigned char *site, const unsigned char *bytes, unsigned len) {
#ifdef _WIN32
    DWORD old_mode = 0, unused = 0;
    if (!VirtualProtect(site, len, PAGE_EXECUTE_READWRITE, &old_mode)) return 0;
    memcpy(site, bytes, len);
    VirtualProtect(site, len, old_mode, &unused);
    FlushInstructionCache(GetCurrentProcess(), site, len);
#else
    memcpy(site, bytes, len);
#endif
    return 1;
}
static void seal_page(unsigned char *page) {
    __builtin___clear_cache((char *)page, (char *)page + PAGE_BYTES);
#ifdef _WIN32
    DWORD old_mode = 0;
    if (!VirtualProtect(page, PAGE_BYTES, PAGE_EXECUTE_READ, &old_mode)) puts("Frame time: cannot protect the trampoline page");
    FlushInstructionCache(GetCurrentProcess(), page, PAGE_BYTES);
#else
    if (mprotect(page, PAGE_BYTES, PROT_READ | PROT_EXEC)) puts("Frame time: cannot protect the trampoline page");
#endif
}

void d1_frame_time_install(unsigned char *image, uint64_t image_size) {
    const char *units_env = getenv("D1_FRAME_UNITS"), *interp_env = getenv("D1_INTERP");
    g_units = parse_units(units_env);
    g_interp = interp_env && interp_env[0] == '1';
    g_image = image;
    /* Which unit fixes apply depends on the play configuration, read here, not assumed. Override
     * off (the clock manifest, uncapped): all four. Override on (one tick per rendered frame, the
     * Windows locked-60 configuration): the director's h and F10's +0x08 are already the frame's
     * time there, so the director and fades fixes would be wrong and are not installed; cloth
     * (a constant 1/30 s) and the UI clock (truncated ms) are wrong in both and are installed.
     * A byte outside the image is treated as on: the director and fades fixes are left out. */
    const int override = clock_override(image, image_size);
    const char *config = override == 0 ? "override off: wall-time ticks"
                       : override > 0 ? "override on: one tick per frame"
                                      : "override byte outside the image, taken as on: one tick per frame";
    if (!g_units && !g_interp) {
        printf("Frame time: D1_FRAME_UNITS off, D1_INTERP off (both default off; the engine's stock per-frame units and no render interpolation); fixed-clock override byte 0x%x: %s\n",
               (unsigned)VA_CLOCK_OVERRIDE, config);
        return;
    }
    const int skipped = override != 0 ? g_units & (G_DIRECTOR | G_FADES) : 0;
    g_units &= ~skipped;
    /* D1_INTERP under the override: alpha 1 everywhere and no director gating (see the header). */
    g_alpha_one = g_interp && override != 0;
    printf("Frame time: fixed-clock override byte 0x%x: %s; D1_FRAME_UNITS %s%s%s%s%s, D1_INTERP %s\n",
           (unsigned)VA_CLOCK_OVERRIDE, config, g_units ? "on (" : units_env && units_env[0] && strcmp(units_env, "0") ? "on (none applicable)" : "off",
           g_units & G_DIRECTOR ? " director wall dt" : "", g_units & G_CLOTH ? " cloth frame share" : "",
           g_units & G_FADES ? " fades h" : "", g_units & G_UI ? " UI ms carry )" : g_units ? " )" : "",
           !g_interp ? "off"
           : g_alpha_one ? "on (alpha 1 under the override: no camera blend, no shader-time add, no director gating)"
                         : "on (director per tick, camera blended by the kept tick fraction, shader time fraction)");
    if (skipped)
        printf("Frame time: D1_FRAME_UNITS%s%s skipped: the fixed-clock override is on (one tick per rendered frame), "
               "so the director's h and F10's +0x08 are already the frame's time; these fixes apply only with the override off\n",
               skipped & G_DIRECTOR ? " director" : "", skipped & G_FADES ? " fades" : "");
    if (g_alpha_one)
        printf("Frame time: D1_INTERP blending skipped: the fixed-clock override is on (one tick per rendered frame); "
               "0x5905c0 writes the kept fraction G2+8 at 0x5908c4 before the override test at 0x5908cb, so it is the "
               "discarded accumulator's phase, not that of a tick the game ran: alpha 1 (camera blend off, shader-time add 0), "
               "and the director gating is not installed (the director keeps the engine's h; the wall-time dt of its stall "
               "rule applies only with the override off)\n");
    const int want = g_units | (g_interp ? G_INTERP : 0) | (g_interp && !g_alpha_one ? G_INTERP_DIR : 0);
    g_interp_dir = (want & G_INTERP_DIR) != 0;
    if (g_interp_dir) {
        /* The extract order counter reads F0's frame serial: mov rax,[rip+0x1f63ffd] (= 0x26ae448); inc byte [rax]. */
        static const unsigned char f0_serial_inc[9] = {0x48, 0x8b, 0x05, 0xfd, 0x3f, 0xf6, 0x01, 0xfe, 0x00};
        g_serial_ok = VA_F0_SERIAL_INC + sizeof f0_serial_inc <= image_size &&
                      memcmp(image + VA_F0_SERIAL_INC, f0_serial_inc, sizeof f0_serial_inc) == 0;
        if (!g_serial_ok)
            puts("Frame time: F0's frame serial increment 0x74a444 does not hold the pinned bytes; the extract order is not "
                 "measured and its late rule is off");
    }
    if (!want) { puts("Frame time: 0 of 0 applicable sites installed"); return; }
    unsigned char *page = trampoline_page(image, image_size);
    if (!page) { puts("Frame time: no trampoline page within reach of the image; every hook left out"); return; }
    unsigned installed = 0, applicable = 0;
    for (unsigned i = 0; i < SITE_COUNT; ++i) {
        const Site *s = &sites[i];
        if (!(s->group & want)) continue;
        ++applicable;
        unsigned char *site = image + s->vaddr;
        if (s->vaddr + s->pinned_len > image_size) { printf("Frame time: %s is outside the image; left out\n", s->name); continue; }
        if (memcmp(site, s->pinned, s->pinned_len) != 0) {
            if (has_alt(s) && memcmp(site, s->alt, s->pinned_len) == 0) {
                printf("Frame time: %s holds the frame-units manifest's load; hooked over it\n", s->name);
            } else {
                printf("Frame time: %s does not hold the pinned bytes; left out\n", s->name);
                continue;
            }
        }
        unsigned char *t = page + i * SLOT, *w = t;
        int ok = 1;
        if (s->kind == K_ENTRY || s->kind == K_ENTRY_SKIP) {
            w = emit_enter(w, 1, (uint32_t)s->id);
            unsigned char *branch = NULL;
            if (s->kind == K_ENTRY_SKIP) {
                *w++ = 0x85; *w++ = 0xc0;             /* test eax, eax */
                *w++ = 0x0f; *w++ = 0x85; branch = w; w += 4; /* jnz rel32 (patched below) */
            }
            w = emit_leave(w, 1);
            memcpy(w, s->pinned, 6); w += 6;          /* the displaced prologue */
            ok = emit_rel32(w, w, 0xe9, site + 6); w += 5;
            if (branch) {
                emit_u32(branch, (uint32_t)(w - (branch + 4)));
                w = emit_leave(w, 1);
                *w++ = 0xc3;                           /* ret: the caller continues as if the function returned */
            }
        } else if (s->kind == K_MID) {
            if (s->pre == PRE_COPY) { memcpy(w, s->pinned, s->pinned_len); w += s->pinned_len; }
            else if (s->pre == PRE_MOV_R14_FP_VIEW) {
                *w++ = 0x49; *w++ = 0xbe;              /* mov r14, imm64 (REX.W+B, B8+6) */
                uintptr_t v = (uintptr_t)image + VA_FP_VIEW; memcpy(w, &v, 8); w += 8;
            }
            w = emit_enter(w, 0, (uint32_t)s->id);
            w = emit_leave(w, 0);
            ok = emit_rel32(w, w, 0xe9, image + s->resume); w += 5;
        } else { /* K_WRAP: handler(before); call original target; handler(after); back */
            int32_t rel; memcpy(&rel, s->pinned + 1, 4);
            unsigned char *target = site + 5 + rel;
            w = emit_enter(w, 0, (uint32_t)s->id);
            w = emit_leave(w, 0);
            ok = emit_rel32(w, w, 0xe8, target); w += 5;
            w = emit_enter(w, 0, (uint32_t)s->id + 1);
            w = emit_leave(w, 0);
            ok = ok && emit_rel32(w, w, 0xe9, site + 5); w += 5;
        }
        if (!ok || (size_t)(w - t) > SLOT) { printf("Frame time: %s trampoline out of reach or too large; left out\n", s->name); continue; }
        unsigned char jump[8] = {0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90, 0x90};
        if (!emit_rel32(jump, site, 0xe9, t)) { printf("Frame time: %s is out of reach of the trampoline page; left out\n", s->name); continue; }
        if (!write_site(site, jump, s->pinned_len > 5 ? s->pinned_len : 5)) { printf("Frame time: %s cannot be made writable; left out\n", s->name); continue; }
        printf("Frame time: %s -> trampoline %p (%u bytes)\n", s->name, (void *)t, (unsigned)(w - t));
        ++installed;
        if (s->id == H_DIRCALL_BEFORE) g_dir_exit = 1;
    }
    seal_page(page);
    if (g_interp_dir && !g_dir_exit)
        puts("Frame time: the director's call is not wrapped; its runs are published at its entry, before the body writes the pose");
    printf("Frame time: %u of %u applicable sites installed\n", installed, applicable);
}

void d1_frame_time_extract_order(uint64_t *on_time, uint64_t *late, uint64_t *no_tick) {
    *on_time = __atomic_load_n(&order_total[0], __ATOMIC_RELAXED);
    *late = __atomic_load_n(&order_total[1], __ATOMIC_RELAXED);
    *no_tick = __atomic_load_n(&order_total[2], __ATOMIC_RELAXED);
}
