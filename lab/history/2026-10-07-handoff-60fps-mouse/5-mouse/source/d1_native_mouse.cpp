// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

#include "core/d1_native_mouse.h"

#include <algorithm>
#include <array>
#include <atomic>
#include <cmath>
#include <cstring>
#include <chrono>
#include <cstdlib>
#include <mutex>
#include "input/d1_native_input_state.h"
#include "input/d1_native_look.h"
#include "input/d1_native_actions.h"
#include <string_view>

#include <SDL3/SDL_events.h>

#ifdef _WIN32
#include <windows.h>

#include <SDL3/SDL_system.h>
#endif

#include "common/elf_info.h"
#include "common/logging/log.h"
#include "common/memory_patcher.h"
#include "core/cpu_patches.h"
#include "core/emulator_settings.h"
#include "input/input_handler.h"
#include "input/native_mouse_mailbox.h"

namespace Core::D1NativeMouse {
namespace {

constexpr std::string_view ExpectedSerial = "CUSA00219";
constexpr std::string_view ExpectedAppVersion = "01.29";
constexpr u64 InjectionSiteVaddr = 0x924CC5;
constexpr u64 CursorSiteVaddr = 0x984060;
constexpr u64 LookSiteVaddr = 0x91AB3C;
constexpr std::array<u8, 7> LookSpan = {0x48, 0x89, 0x9D, 0xE0, 0xFC, 0xFF, 0xFF};
constexpr u64 MouseStateVaddr = 0x1ABC3E0;
constexpr u64 FlagsVaddr = 0x1ABBC50;
constexpr u64 PlayerZeroInputStateVaddr = 0x2725C50;
constexpr u64 DeltaTimeRbpOffset = 0x2DC;
constexpr u64 LookXOffset = 0x2CC;
constexpr u64 LookYOffset = 0x2D4;
constexpr float BaseStickSecondsPerCount = 0.00025f;
constexpr float MinimumDeltaTime = 0.0001f;
constexpr float MaximumDeltaTime = 0.25f;
constexpr float MaximumCountsPerSample = 8192.0f;

// Publish under the native input lock, before the builder consumes the platform tables.
// The complete RIP-relative LEA is relocated by the existing detour machinery.
constexpr std::array<u8, 7> PinnedSpan = {0x48, 0x8D, 0x05, 0xE4, 0x0C, 0xE0, 0x01};
constexpr std::array<u8, 7> CursorSpan = {0x41, 0x8B, 0x9F, 0x10, 0x02, 0x00, 0x00};

Input::NativeMouseMailbox g_motion{};
Input::NativeMouseMailbox g_fallback_look{};
std::atomic<bool> g_armed{};
std::atomic<bool> g_capture_requested{};
std::atomic<bool> g_capture_active{};
std::atomic<float> g_sensitivity{1.0f};
std::atomic<bool> g_invert_x{};
std::atomic<bool> g_invert_y{};
std::atomic<u64> g_player_zero_input_state{};
std::array<D1DetourHandle, 9> g_handles{};
constexpr u64 ActionsSite = 0x92684F;
constexpr std::array<u8, 5> ActionsSpan = {0xE8,0xEC,0x68,0x78,0xFF};
Input::D1Native::ActionMerge g_actions;
std::array<Input::D1Native::Key, 5> g_last_buttons{};
std::atomic<bool> g_actions_logged{};
constexpr u64 RestoreLookSite = 0x91B7A7;
constexpr std::array<u8, 10> RestoreLookSpan = {0x4D,0x89,0xF5,0x4C,0x89,0xBD,0xF8,0xFC,0xFF,0xFF};
constexpr u64 AssistSite = 0x91BB3E;
constexpr std::array<u8, 9> AssistSpan = {0xC5,0xF8,0x57,0xC0,0xC5,0xFA,0x10,0x4D,0xA8};
// Pitch recentering in the camera-angle applier 0x91EEF0 (documents/d1/vertical-recenter-
// research.md). The look-state array P is *(u64*)(eboot + 0x2725798), stride 0x1A0 per player,
// pitch at +0x3C; the applier holds it in r14. Player identification per site is in
// input/d1_native_look.h (RecenterSiteIsPlayerZero, SpringSiteIsPlayerZero).
constexpr u64 LookStatePointerVaddr = 0x2725798;
// RecenterSite: vxorps xmm0,xmm0,xmm0 ; vmovss xmm1,[rbp-0x280]. r13 = player * 0x1A0 here.
// [rbp-0x280] is the content pitch target (tuning block +0x18) and P+0xE8 the auto-look-centering
// latch; with both zero the game's own je at 0x91FE84 skips the target pull and the centering
// decay. [rbp-0x290]/[rbp-0x260] hold the state's pitch limits (tuning +0xB0/+0xB4, default
// +-1.4922565) that the hard clamp at 0x92014C chases; they are only reported, never changed.
constexpr u64 RecenterSite = 0x91FE67;
constexpr std::array<u8, 12> RecenterSpan = {0xC5,0xF8,0x57,0xC0,0xC5,0xFA,0x10,0x8D,0x80,0xFD,0xFF,0xFF};
constexpr u64 PitchTargetRbpOffset = 0x280;
constexpr u64 CenteringLatchOffset = 0xE8;
constexpr u64 PitchLimitMinRbpOffset = 0x290;
constexpr u64 PitchLimitMaxRbpOffset = 0x260;
constexpr float DefaultPitchLimit = 1.4922565f;
// PitchSpringSite: vmovss xmm3,[rbp-0x260], the soft pitch-limit spring rate, read on frames
// whose pitch input is exactly 0 (every frame without vertical mouse motion). The spring eases
// pitch back into the band [rbp-0x270]/[rbp-0x26C] (the limits minus the tuning margins). A rate
// of 0 makes 0x78B50 return the pitch unchanged; the hard clamp at 0x92014C still runs before it.
// r13 is &P[player].pitch here (0x920143 mov r13,rbx), not the player offset: the 8d8d4196 gate
// tested r13 == 0 and so never fired. The player offset is saved at [rbp-0x248] (0x92013C).
// The other 0x78B50 call (0x920599, root-frame clamp) needs nothing: 0x9205F6 overwrites its result.
constexpr u64 PitchSpringSite = 0x9202CF;
constexpr std::array<u8, 8> PitchSpringSpan = {0xC5,0xFA,0x10,0x9D,0xA0,0xFD,0xFF,0xFF};
constexpr u64 PitchSpringRateRbpOffset = 0x260;
constexpr u64 PitchSpringPlayerOffsetRbpOffset = 0x248;
constexpr u64 PitchSpringBandRbpOffset = 0x270;
// The measured KBM vertical spring (pitch back to its pre-shot value at the weapon's recovery
// rate) is recoil recovery, not the applier sites above. Look function 0x919860 (rdx = input
// record, rcx = &P[player] + 0x20, r8 = look output) keeps the recoil offset at P+0xF0 (pitch) /
// P+0xF4 (yaw), credits look input against it in the block at 0x91AA17 and drains the rest back
// out of the view. RecoilOffsetSite: vmovss xmm0,[r15+0x18], the block's first instruction,
// reached exactly once per call and after GateLook of that call. There [rbp-0x330] = &P[player] +
// 0x20 (0x91A74B), [rbp-0x340] = the input record (0x91A741) and r15 = the look output, whose +0x18
// is the pass's pitch delta (0x91BD0F). Rules: input/d1_native_look.h (MouseClearsRecoilOffset,
// NativeMousePitchPass); register and slot facts: documents/d1/tools/recoil_offset_site_check.py.
constexpr u64 RecoilOffsetSite = 0x91AA17;
constexpr std::array<u8, 6> RecoilOffsetSpan = {0xC4,0xC1,0x7A,0x10,0x47,0x18};
constexpr u64 LookRecordRbpOffset = 0x330;
constexpr u64 InputRecordRbpOffset = 0x340;
constexpr u64 LookRecordBias = 0x20;     // [rbp-0x330] - P[player]
constexpr u64 RecoilOffsetField = 0xD0;  // from the look record: P+0xF0 pitch, P+0xF4 yaw
constexpr u64 LookOutputPitchDelta = 0x18;
std::atomic<bool> g_recenter_logged_target{};
std::atomic<bool> g_recenter_logged_latch{};
std::atomic<bool> g_recenter_logged_limits{};
std::atomic<bool> g_spring_logged{};
std::atomic<bool> g_spring_pull_logged{};
std::atomic<bool> g_recoil_gate_logged{};
std::atomic<bool> g_recoil_clear_logged{};
thread_local Input::D1Native::LookOverride g_look_backup;
Input::D1Native::Mouse g_last_mouse{};
std::atomic<bool> g_hybrid_logged{}, g_assist_logged{};
std::mutex g_input_mutex;
Input::D1Native::State g_input;
bool g_cursor_only{};
std::atomic<u64> g_cursor_tick{};
std::atomic<bool> g_gameplay_route{};
std::atomic<bool> g_native_sample_logged{};
std::atomic<bool> g_cursor_logged{};

void ClearInput() {
    std::lock_guard lock(g_input_mutex);
    g_motion.Clear();
    g_fallback_look.Clear();
    g_input = {};
    g_last_mouse = {};
    g_last_buttons = {};
    g_actions = {};
}

bool g_command_line_enabled{};
std::optional<float> g_command_line_sensitivity;
bool g_command_line_invert_x{};
bool g_command_line_invert_y{};

void RequestFrontendRefresh() noexcept {
    SDL_Event event{};
    event.type = SDL_EVENT_D1_NATIVE_MOUSE_STATE_CHANGED;
    SDL_PushEvent(&event);
}

#ifdef _WIN32
std::atomic<bool> g_win32_raw_active{};
bool g_win32_hook_installed{};

// Runs on SDL's message pump thread for every message, before SDL processes it. Reads the raw
// mouse deltas (hardware counts, no pointer ballistics) and always returns true so SDL still
// sees every message - buttons and wheel stay on the legacy path for the input bindings.
bool RawInputMessageHook(void* /*userdata*/, MSG* msg) {
    if (msg->message != WM_INPUT || !g_win32_raw_active.load(std::memory_order_acquire) ||
        !g_capture_active.load(std::memory_order_acquire)) {
        return true;
    }
    RAWINPUT raw{};
    UINT size = sizeof(raw);
    if (GetRawInputData(reinterpret_cast<HRAWINPUT>(msg->lParam), RID_INPUT, &raw, &size,
                        sizeof(RAWINPUTHEADER)) == static_cast<UINT>(-1)) {
        return true;
    }
    if (raw.header.dwType != RIM_TYPEMOUSE) {
        return true;
    }
    if ((raw.data.mouse.usFlags & MOUSE_MOVE_ABSOLUTE) == 0) {
        const auto dx = static_cast<float>(raw.data.mouse.lLastX);
        const auto dy = static_cast<float>(raw.data.mouse.lLastY);
        if (dx != 0.0f || dy != 0.0f) {
            g_motion.Accumulate(dx, dy);
            if (g_cursor_only) g_fallback_look.Accumulate(dx, dy);
        }
    }
    // Our registration displaces SDL's own raw-input registration, and SDL3 on Windows sources
    // BUTTON state from raw input while relative mode is active - so while captured, SDL sees no
    // mouse buttons at all (measured: every bound mouse button dead in-game, run 231522).
    // The hook owns the raw stream now, so it forwards button and wheel transitions as
    // synthesized SDL events; they flow through the normal binding machinery (r2=leftbutton
    // etc.) with raw-input latency.
    const USHORT flags = raw.data.mouse.usButtonFlags;
    if (flags != 0) {
        const auto PushButton = [](Uint8 button, bool down) {
            SDL_Event e{};
            e.type = down ? SDL_EVENT_MOUSE_BUTTON_DOWN : SDL_EVENT_MOUSE_BUTTON_UP;
            e.button.button = button;
            e.button.down = down;
            SDL_PushEvent(&e);
        };
        if (flags & RI_MOUSE_LEFT_BUTTON_DOWN) PushButton(SDL_BUTTON_LEFT, true);
        if (flags & RI_MOUSE_LEFT_BUTTON_UP) PushButton(SDL_BUTTON_LEFT, false);
        if (flags & RI_MOUSE_RIGHT_BUTTON_DOWN) PushButton(SDL_BUTTON_RIGHT, true);
        if (flags & RI_MOUSE_RIGHT_BUTTON_UP) PushButton(SDL_BUTTON_RIGHT, false);
        if (flags & RI_MOUSE_MIDDLE_BUTTON_DOWN) PushButton(SDL_BUTTON_MIDDLE, true);
        if (flags & RI_MOUSE_MIDDLE_BUTTON_UP) PushButton(SDL_BUTTON_MIDDLE, false);
        if (flags & RI_MOUSE_BUTTON_4_DOWN) PushButton(SDL_BUTTON_X1, true);
        if (flags & RI_MOUSE_BUTTON_4_UP) PushButton(SDL_BUTTON_X1, false);
        if (flags & RI_MOUSE_BUTTON_5_DOWN) PushButton(SDL_BUTTON_X2, true);
        if (flags & RI_MOUSE_BUTTON_5_UP) PushButton(SDL_BUTTON_X2, false);
        if (flags & RI_MOUSE_WHEEL) {
            SDL_Event e{};
            e.type = SDL_EVENT_MOUSE_WHEEL;
            e.wheel.y = static_cast<float>(static_cast<SHORT>(raw.data.mouse.usButtonData)) /
                        WHEEL_DELTA;
            SDL_PushEvent(&e);
        }
    }
    return true;
}
#endif

float LoadFloat(u64 address) noexcept {
    float value{};
    std::memcpy(&value, reinterpret_cast<const void*>(address), sizeof(value));
    return value;
}

void StoreFloat(u64 address, float value) noexcept {
    std::memcpy(reinterpret_cast<void*>(address), &value, sizeof(value));
}

bool CursorActive() noexcept {
    const auto last = g_cursor_tick.load(std::memory_order_acquire);
    const auto now = SDL_GetTicks();
    return last != 0 && now - last < 100;
}

void PS4_SYSV_ABI InjectAtSample(const D1DetourCapturedRegisters*, u64 site_vaddr) {
    if (site_vaddr != InjectionSiteVaddr) return;
    std::lock_guard lock(g_input_mutex);
    const bool active = g_capture_active.load(std::memory_order_acquire);
    const bool gameplay = active && !g_cursor_only && !CursorActive();
    if (g_gameplay_route.exchange(gameplay) != gameplay) RequestFrontendRefresh();
    auto motion = g_motion.Consume();
    auto sample = g_input.Sample(SDL_GetTicks(), active, motion.x, motion.y,
                                g_sensitivity.load(std::memory_order_relaxed));
    const u64 base = MemoryPatcher::g_eboot_address;
    if (!g_cursor_only) {
        // Cursor polarity is screen coordinates. Gameplay inversion belongs to the game's
        // settings; never invert this shared state and make menus run backwards.
        std::memcpy(reinterpret_cast<void*>(base + MouseStateVaddr), &sample, sizeof(sample));
        // +1 suppresses ALL devices, including physical pads. It belongs to the game,
        // not mouse capture. Only our mouse-enable flag changes with capture.
        g_last_mouse = sample.mouse;
        g_fallback_look.Accumulate(float(sample.mouse.dx), float(sample.mouse.dy));
        g_last_buttons = g_input.buttons;
        *reinterpret_cast<u8*>(base + FlagsVaddr + 3) = active ? 1 : 0;
        if (active && !g_native_sample_logged.exchange(true)) {
            LOG_INFO(Input, "D1 native input: first platform sample published; mouse and 104 keys enabled");
        }
    } else {
        // Fallback cursor consumes a separately retained sample; native tables stay disabled.
        if (!CursorActive()) g_input.cursor_x = g_input.cursor_y = 0;
        else {
            g_input.cursor_x += sample.mouse.dx;
            g_input.cursor_y += sample.mouse.dy;
        }
    }
}

void PS4_SYSV_ABI InjectCursor(const D1DetourCapturedRegisters* regs, u64 site_vaddr) {
    if (site_vaddr != CursorSiteVaddr || !g_capture_active.load(std::memory_order_acquire)) return;
    g_cursor_tick.store(SDL_GetTicks(), std::memory_order_release);
    g_fallback_look.Clear();
    if (!g_cursor_logged.exchange(true)) LOG_INFO(Input, "D1 native input: menu cursor consumer reached");
    if (!g_cursor_only) return; // Native getter applies the sample exactly once.
    std::lock_guard lock(g_input_mutex);
    const float x = LoadFloat(regs->r15 + 0x350) + g_input.cursor_x;
    const float y = LoadFloat(regs->r15 + 0x354) + g_input.cursor_y;
    g_input.cursor_x = g_input.cursor_y = 0;
    if (std::isfinite(x) && std::isfinite(y)) {
        StoreFloat(regs->r15 + 0x350, x);
        StoreFloat(regs->r15 + 0x354, y);
    }
}

void PS4_SYSV_ABI GateLook(const D1DetourCapturedRegisters* regs, u64 site_vaddr) {
    if (site_vaddr != LookSiteVaddr || !g_capture_active.load(std::memory_order_acquire) ||
        regs->r13 != g_player_zero_input_state.load(std::memory_order_relaxed)) return;
    if (!g_cursor_only) {
        Input::D1Native::Mouse mouse;
        {
            std::lock_guard lock(g_input_mutex);
            mouse = g_last_mouse;
            const auto counts = g_fallback_look.Consume();
            mouse.dx = static_cast<int32_t>(counts.x);
            mouse.dy = static_cast<int32_t>(counts.y);
        }
        if (g_look_backup.Begin(regs->r13, regs->rbp, mouse.dx, mouse.dy, CursorActive(),
                                g_invert_x.load(), g_invert_y.load()) && !g_hybrid_logged.exchange(true))
            LOG_INFO(Input, "D1 native input: hybrid look reached; pad actions preserved, stick look live while the mouse is idle, pitch corrected");
    } else if (CursorActive()) {
        for (const u64 offset : {0x2C8u, 0x2CCu, 0x2D0u, 0x2D4u}) StoreFloat(regs->r13 + offset, 0);
        g_fallback_look.Clear();
    } else if (g_cursor_only && regs->rbp >= DeltaTimeRbpOffset) {
        const float dt = LoadFloat(regs->rbp - DeltaTimeRbpOffset);
        if (!std::isfinite(dt) || dt <= MinimumDeltaTime || dt > MaximumDeltaTime) return;
        const auto motion = g_fallback_look.Consume();
        const float gain = BaseStickSecondsPerCount * g_sensitivity.load() / dt;
        const float x = LoadFloat(regs->r13 + LookXOffset) +
            std::clamp(motion.x, -MaximumCountsPerSample, MaximumCountsPerSample) * gain * (g_invert_x.load() ? 1 : -1);
        const float y = LoadFloat(regs->r13 + LookYOffset) +
            std::clamp(motion.y, -MaximumCountsPerSample, MaximumCountsPerSample) * gain * (g_invert_y.load() ? -1 : 1);
        if (std::isfinite(x) && std::isfinite(y)) {
            StoreFloat(regs->r13 + LookXOffset, x); StoreFloat(regs->r13 + LookYOffset, y);
        }
    }
}

void PS4_SYSV_ABI MergeActions(const D1DetourCapturedRegisters*, u64 site) {
    if (site != ActionsSite || g_cursor_only || !g_capture_active.load(std::memory_order_acquire)) return;
    std::lock_guard lock(g_input_mutex);
    const u64 state = g_player_zero_input_state.load(std::memory_order_relaxed);
    g_actions.Apply(reinterpret_cast<u8*>(state), g_last_buttons, CursorActive());
    if (!g_actions_logged.exchange(true)) LOG_INFO(Input,
        "D1 native input: mouse actions merged after pad selection; LMB fire, RMB ADS, M4 reload, M5 crouch/slide");
}

void PS4_SYSV_ABI RestoreLook(const D1DetourCapturedRegisters* regs, u64 site) {
    if (site == RestoreLookSite) g_look_backup.Restore(regs->rbp);
}

void PS4_SYSV_ABI GateAssist(const D1DetourCapturedRegisters* regs, u64 site) {
    if (site != AssistSite || g_cursor_only || !g_capture_active.load(std::memory_order_acquire)) return;
    u64 input{};
    std::memcpy(&input, reinterpret_cast<void*>(regs->rbp - 0x348), sizeof(input));
    if (input != g_player_zero_input_state.load(std::memory_order_relaxed)) return;
    // Xbox adapter clears the assistance-active register at its equivalent branch.
    // Here zero the two strengths immediately before PS4 computes that boolean.
    // Original compares then select its own unassisted branch (AL=0), with registers intact.
    StoreFloat(regs->rbp - 0x58, 0);
    StoreFloat(regs->rbp - 0x54, 0);
    if (!g_assist_logged.exchange(true)) LOG_INFO(Input,
        "D1 native input: late target-assist bypass reached (Xbox-equivalent gate)");
}

u64 LookStateBase() noexcept {
    u64 base{};
    std::memcpy(&base, reinterpret_cast<const void*>(MemoryPatcher::g_eboot_address +
                                                     LookStatePointerVaddr), sizeof(base));
    return base;
}

// Player 0 under mouse capture: no content pitch target and no auto-look-centering decay. A
// physical pad (capture off) keeps the game's behaviour.
void PS4_SYSV_ABI GateRecenter(const D1DetourCapturedRegisters* regs, u64 site) {
    if (site != RecenterSite || g_cursor_only ||
        !g_capture_active.load(std::memory_order_acquire) ||
        !Input::D1Native::RecenterSiteIsPlayerZero(regs->r13, regs->r14, LookStateBase())) return;
    const u64 target_slot = regs->rbp - PitchTargetRbpOffset;
    u8* const latch = reinterpret_cast<u8*>(regs->r14 + regs->r13 + CenteringLatchOffset);
    const float target = LoadFloat(target_slot);
    if (target != 0.0f && !g_recenter_logged_target.exchange(true)) LOG_INFO(Input,
        "D1 native input: content pitch target {:.4f} neutralised under mouse capture", target);
    if (*latch != 0 && !g_recenter_logged_latch.exchange(true)) LOG_INFO(Input,
        "D1 native input: auto look centering latch neutralised under mouse capture");
    StoreFloat(target_slot, 0.0f);
    *latch = 0;
    // Report only: a look state with narrower pitch limits makes the hard clamp pull pitch in.
    const float limit_min = LoadFloat(regs->rbp - PitchLimitMinRbpOffset);
    const float limit_max = LoadFloat(regs->rbp - PitchLimitMaxRbpOffset);
    if ((limit_min > -DefaultPitchLimit + 0.01f || limit_max < DefaultPitchLimit - 0.01f) &&
        !g_recenter_logged_limits.exchange(true)) LOG_INFO(Input,
        "D1 native input: look state narrows pitch limits to [{:.3f}, {:.3f}] (default +-{:.3f}); "
        "not changed", limit_min, limit_max, DefaultPitchLimit);
}

// Player 0 under mouse capture: no soft pitch-limit spring. The game eases pitch back into the
// band on every frame without pitch input, which with a mouse is every frame without vertical
// motion; a stick held at the limit keeps its input nonzero and so rarely meets it.
void PS4_SYSV_ABI GatePitchSpring(const D1DetourCapturedRegisters* regs, u64 site) {
    if (site != PitchSpringSite || g_cursor_only ||
        !g_capture_active.load(std::memory_order_acquire)) return;
    u64 saved_player_offset{};
    std::memcpy(&saved_player_offset,
                reinterpret_cast<const void*>(regs->rbp - PitchSpringPlayerOffsetRbpOffset),
                sizeof(saved_player_offset));
    if (!Input::D1Native::SpringSiteIsPlayerZero(regs->r13, regs->r14, saved_player_offset,
                                                 LookStateBase())) return;
    const u64 rate_slot = regs->rbp - PitchSpringRateRbpOffset;
    const float rate = LoadFloat(rate_slot);
    if (rate > 0.0f) {
        const float band_lo = LoadFloat(regs->rbp - PitchSpringBandRbpOffset);
        const float band_hi = LoadFloat(regs->rbp - PitchSpringBandRbpOffset + 4);
        const float pitch = LoadFloat(regs->r13);
        if (!g_spring_logged.exchange(true)) LOG_INFO(Input,
            "D1 native input: soft pitch-limit spring (rate {:.3f}/s, band [{:.3f}, {:.3f}]) "
            "neutralised under mouse capture", rate, band_lo, band_hi);
        if ((pitch < band_lo || pitch > band_hi) && !g_spring_pull_logged.exchange(true))
            LOG_INFO(Input, "D1 native input: soft pitch-limit spring held off at pitch {:.3f} "
                            "(it would ease toward {:.3f})",
                     pitch, pitch < band_lo ? band_lo : band_hi);
    }
    StoreFloat(rate_slot, 0.0f);
}

// Player 0, a pass whose look the mouse drives, under capture: mouse look never grows or flips the
// recoil offset, so recoil recovery drains the weapon kick and nothing else. The game reads "the
// player is looking the offset's way" from the stick pitch axis, which is 0 for a mouse, and so
// added every mouse delta to the offset that recovery then took back out of the pitch. Clearing
// here is the game's own clear (0x91B448, both halves); the block then skips the offset. A smaller
// opposite delta is left to the game (offset += delta), as for a stick. Pad passes, capture off and
// the other players keep the game's rule.
void PS4_SYSV_ABI GateRecoilOffset(const D1DetourCapturedRegisters* regs, u64 site) {
    if (site != RecoilOffsetSite) return;
    // Taken on every pass, so a mark never outlives the call that set it.
    const bool override_pass = g_look_backup.TakeMouseFrame(regs->rbp);
    if (g_cursor_only || !g_capture_active.load(std::memory_order_acquire)) return;
    u64 look_record{}, input_record{};
    std::memcpy(&look_record, reinterpret_cast<const void*>(regs->rbp - LookRecordRbpOffset),
                sizeof(look_record));
    std::memcpy(&input_record, reinterpret_cast<const void*>(regs->rbp - InputRecordRbpOffset),
                sizeof(input_record));
    const u64 look_state = LookStateBase();
    if (look_state == 0 || look_record != look_state + LookRecordBias ||
        input_record != g_player_zero_input_state.load(std::memory_order_relaxed)) return;
    if (!override_pass &&
        !Input::D1Native::NativeMousePitchPass(reinterpret_cast<const u8*>(input_record))) return;
    const float delta = LoadFloat(regs->r15 + LookOutputPitchDelta);
    const float offset = LoadFloat(look_record + RecoilOffsetField);
    if (!g_recoil_gate_logged.exchange(true)) LOG_INFO(Input,
        "D1 native input: recoil-offset gate reached on a mouse look pass (offset {:.4f}, "
        "pitch delta {:.4f})", offset, delta);
    if (!Input::D1Native::MouseClearsRecoilOffset(delta, offset)) return;
    if (!g_recoil_clear_logged.exchange(true)) LOG_INFO(Input,
        "D1 native input: mouse look cleared recoil offset {:.4f} (pitch delta {:.4f}); "
        "recoil recovery drains weapon kicks only", offset, delta);
    const u64 cleared = 0;
    std::memcpy(reinterpret_cast<void*>(look_record + RecoilOffsetField), &cleared,
                sizeof(cleared));
}

const char* StatusName(D1DetourStatus status) {
    switch (status) {
    case D1DetourStatus::Ok:
        return "Ok";
    case D1DetourStatus::ModuleNotFound:
        return "ModuleNotFound";
    case D1DetourStatus::ModuleMismatch:
        return "ModuleMismatch";
    case D1DetourStatus::AlreadyInstalled:
        return "AlreadyInstalled";
    case D1DetourStatus::SpanUnsuitable:
        return "SpanUnsuitable";
    case D1DetourStatus::ShadowStackUnsupported:
        return "ShadowStackUnsupported";
    case D1DetourStatus::TrampolineExhausted:
        return "TrampolineExhausted";
    case D1DetourStatus::AuxiliaryEmissionFailed:
        return "AuxiliaryEmissionFailed";
    case D1DetourStatus::JumpOutOfRange:
        return "JumpOutOfRange";
    case D1DetourStatus::NotInstalled:
        return "NotInstalled";
    }
    return "Unknown";
}

} // namespace

void ConfigureFromCommandLine(bool enabled, std::optional<float> sensitivity, bool invert_x,
                              bool invert_y) {
    g_command_line_enabled = enabled;
    g_command_line_sensitivity = sensitivity;
    g_command_line_invert_x = invert_x;
    g_command_line_invert_y = invert_y;
}

void ArmBeforeGuestThreads() {
    if (!g_command_line_enabled && !EmulatorSettings.IsD1NativeMouseEnabled()) {
        return;
    }

    const auto& elf = Common::ElfInfo::Instance();
    if (elf.GameSerial() != ExpectedSerial || elf.AppVer() != ExpectedAppVersion) {
        LOG_ERROR(Input, "D1 native mouse: refusing unsupported title/build {} {}; expected {} {}",
                  elf.GameSerial(), elf.AppVer(), ExpectedSerial, ExpectedAppVersion);
        return;
    }
    if (MemoryPatcher::g_eboot_address == 0 || MemoryPatcher::g_eboot_image_size == 0 ||
        InjectionSiteVaddr + PinnedSpan.size() > MemoryPatcher::g_eboot_image_size ||
        CursorSiteVaddr + CursorSpan.size() > MemoryPatcher::g_eboot_image_size ||
        MouseStateVaddr + sizeof(Input::D1Native::Sample) > MemoryPatcher::g_eboot_image_size ||
        PlayerZeroInputStateVaddr + LookYOffset + sizeof(float) >
            MemoryPatcher::g_eboot_image_size) {
        LOG_ERROR(Input, "D1 native mouse: mapped eboot range is unavailable; refusing to arm");
        return;
    }

    const float sensitivity = g_command_line_sensitivity.value_or(
        static_cast<float>(EmulatorSettings.GetD1NativeMouseSensitivity()));
    g_sensitivity.store(std::clamp(sensitivity, 0.05f, 10.0f), std::memory_order_relaxed);
    g_invert_x.store(g_command_line_invert_x || EmulatorSettings.IsD1NativeMouseInvertX(),
                     std::memory_order_relaxed);
    g_invert_y.store(g_command_line_invert_y || EmulatorSettings.IsD1NativeMouseInvertY(),
                     std::memory_order_relaxed);
    g_player_zero_input_state.store(MemoryPatcher::g_eboot_address + PlayerZeroInputStateVaddr,
                                    std::memory_order_relaxed);

    g_cursor_only = std::getenv("D1_NATIVE_CURSOR_ONLY") &&
                    std::string_view(std::getenv("D1_NATIVE_CURSOR_ONLY")) == "1";
    D1DetourRequest request{
        .site = reinterpret_cast<void*>(MemoryPatcher::g_eboot_address + InjectionSiteVaddr),
        .site_vaddr = InjectionSiteVaddr,
        .pinned_span = PinnedSpan.data(),
        .pinned_span_len = PinnedSpan.size(),
        .callback = InjectAtSample,
    };
    D1DetourRequest cursor_request{
        .site = reinterpret_cast<void*>(MemoryPatcher::g_eboot_address + CursorSiteVaddr),
        .site_vaddr = CursorSiteVaddr,
        .pinned_span = CursorSpan.data(),
        .pinned_span_len = CursorSpan.size(),
        .callback = InjectCursor,
    };
    D1DetourRequest look_request{
        .site = reinterpret_cast<void*>(MemoryPatcher::g_eboot_address + LookSiteVaddr),
        .site_vaddr = LookSiteVaddr,
        .pinned_span = LookSpan.data(),
        .pinned_span_len = LookSpan.size(),
        .callback = GateLook,
    };
    D1DetourRequest restore_request{
        .site = reinterpret_cast<void*>(MemoryPatcher::g_eboot_address + RestoreLookSite),
        .site_vaddr = RestoreLookSite,
        .pinned_span = RestoreLookSpan.data(), .pinned_span_len = RestoreLookSpan.size(),
        .callback = RestoreLook,
    };
    D1DetourRequest assist_request{
        .site = reinterpret_cast<void*>(MemoryPatcher::g_eboot_address + AssistSite),
        .site_vaddr = AssistSite,
        .pinned_span = AssistSpan.data(), .pinned_span_len = AssistSpan.size(),
        .callback = GateAssist,
    };
    D1DetourRequest actions_request{
        .site = reinterpret_cast<void*>(MemoryPatcher::g_eboot_address + ActionsSite),
        .site_vaddr = ActionsSite,
        .pinned_span = ActionsSpan.data(), .pinned_span_len = ActionsSpan.size(),
        .callback = MergeActions,
    };
    D1DetourRequest recenter_request{
        .site = reinterpret_cast<void*>(MemoryPatcher::g_eboot_address + RecenterSite),
        .site_vaddr = RecenterSite,
        .pinned_span = RecenterSpan.data(), .pinned_span_len = RecenterSpan.size(),
        .callback = GateRecenter,
    };
    D1DetourRequest spring_request{
        .site = reinterpret_cast<void*>(MemoryPatcher::g_eboot_address + PitchSpringSite),
        .site_vaddr = PitchSpringSite,
        .pinned_span = PitchSpringSpan.data(), .pinned_span_len = PitchSpringSpan.size(),
        .callback = GatePitchSpring,
    };
    D1DetourRequest recoil_request{
        .site = reinterpret_cast<void*>(MemoryPatcher::g_eboot_address + RecoilOffsetSite),
        .site_vaddr = RecoilOffsetSite,
        .pinned_span = RecoilOffsetSpan.data(), .pinned_span_len = RecoilOffsetSpan.size(),
        .callback = GateRecoilOffset,
    };
    const std::array requests{request,         cursor_request,   look_request,
                              restore_request, assist_request,   actions_request,
                              recenter_request, spring_request,  recoil_request};
    static_assert(std::tuple_size_v<decltype(requests)> == std::tuple_size_v<decltype(g_handles)>);
    const auto result = InstallPermanentDetourBatch(requests, g_handles);
    if (!result.committed) {
        LOG_ERROR(Input, "D1 native input: platform/cursor detour batch refused");
        return;
    }

    ClearInput();
    g_armed.store(true, std::memory_order_release);
    g_capture_requested.store(true, std::memory_order_release);
    RequestFrontendRefresh();
    LOG_INFO(Input, "D1 native input mode: {}", g_cursor_only ? "cursor fallback + legacy look" : "global native keyboard/mouse");
    LOG_INFO(Input,
#ifdef _WIN32
             "D1 native mouse: armed at {:#x}; Windows raw input (WM_INPUT) will feed the "
             "native mouse/key tables (sensitivity {:.3f}, invert-x {}, invert-y {})",
#else
             "D1 native mouse: armed at {:#x}; SDL relative motion will feed the native mouse/key "
             "tables (sensitivity {:.3f}, invert-x {}, invert-y {})",
#endif
             InjectionSiteVaddr, g_sensitivity.load(std::memory_order_relaxed),
             g_invert_x.load(std::memory_order_relaxed),
             g_invert_y.load(std::memory_order_relaxed));
}

bool PushSDLEvent(const SDL_Event& event) noexcept {
    if (!g_capture_active.load(std::memory_order_acquire)) {
        return false;
    }
    if (event.type == SDL_EVENT_MOUSE_MOTION) {
#ifdef _WIN32
        // WM_INPUT is the Windows delta source (RawInputMessageHook). An SDL relative delta is
        // the same physical motion re-reported through SDL's pipeline - consume it WITHOUT
        // accumulating or the look state gets fed twice per count.
        if (g_win32_raw_active.load(std::memory_order_acquire)) {
            return true;
        }
#endif
        g_motion.Accumulate(event.motion.xrel, event.motion.yrel);
        if (g_cursor_only) g_fallback_look.Accumulate(event.motion.xrel, event.motion.yrel);
        return true;
    }
    if (event.type == SDL_EVENT_WINDOW_FOCUS_LOST) {
        ClearInput();
    }
    if (!g_cursor_only) {
        std::lock_guard lock(g_input_mutex);
        if (event.type == SDL_EVENT_KEY_DOWN || event.type == SDL_EVENT_KEY_UP) {
            const int index = Input::D1Native::KeyIndex(event.key.scancode);
            if (index >= 0) g_input.controls[index].Push(event.type == SDL_EVENT_KEY_DOWN);
        } else if (event.type == SDL_EVENT_MOUSE_BUTTON_DOWN || event.type == SDL_EVENT_MOUSE_BUTTON_UP) {
            const int index = event.button.button == SDL_BUTTON_LEFT ? 0 :
                              event.button.button == SDL_BUTTON_RIGHT ? 1 :
                              event.button.button == SDL_BUTTON_MIDDLE ? 2 :
                              event.button.button == SDL_BUTTON_X1 ? 3 :
                              event.button.button == SDL_BUTTON_X2 ? 4 : -1;
            if (index >= 0) g_input.controls[104 + index].Push(event.type == SDL_EVENT_MOUSE_BUTTON_DOWN);
        } else if (event.type == SDL_EVENT_MOUSE_WHEEL) {
            g_input.wheel += event.wheel.y;
        }
    }
    return false;
}

bool IsArmed() noexcept {
    return g_armed.load(std::memory_order_acquire);
}

bool CaptureRequested() noexcept {
    return g_armed.load(std::memory_order_acquire) &&
           g_capture_requested.load(std::memory_order_acquire);
}

bool CaptureActive() noexcept {
    return g_capture_active.load(std::memory_order_acquire);
}

void SetCaptureActive(bool active) noexcept {
    const bool admitted = active && g_armed.load(std::memory_order_acquire);
    g_capture_active.store(admitted, std::memory_order_release);
    if (!admitted) {
        ClearInput();
        g_cursor_tick.store(0, std::memory_order_release);
    }
}

void ToggleCaptureRequested() noexcept {
    if (!g_armed.load(std::memory_order_acquire)) {
        return;
    }
    const bool requested = !g_capture_requested.load(std::memory_order_relaxed);
    g_capture_requested.store(requested, std::memory_order_release);
    if (!requested) {
        SetCaptureActive(false);
    }
    RequestFrontendRefresh();
}

void ClearMotion() noexcept {
    ClearInput();
}

bool NativeMouseOwnsButtons() noexcept {
    return CaptureActive() && !g_cursor_only;
}

bool NativeGameplayOwnsInput() noexcept {
    return CaptureActive() && g_gameplay_route.load(std::memory_order_acquire);
}

#ifdef _WIN32
bool ApplyWin32RawCapture(void* hwnd, bool active) noexcept {
    if (!g_win32_hook_installed) {
        SDL_SetWindowsMessageHook(&RawInputMessageHook, nullptr);
        g_win32_hook_installed = true;
    }
    RAWINPUTDEVICE device{
        .usUsagePage = 0x01, // HID_USAGE_PAGE_GENERIC
        .usUsage = 0x02,     // HID_USAGE_GENERIC_MOUSE
        .dwFlags = active ? DWORD{0} : DWORD{RIDEV_REMOVE},
        .hwndTarget = active ? static_cast<HWND>(hwnd) : nullptr,
    };
    if (!RegisterRawInputDevices(&device, 1, sizeof(device))) {
        g_win32_raw_active.store(false, std::memory_order_release);
        if (active) {
            // Fail-closed: no raw registration means no capture; never silently fall back to
            // SDL relative deltas ("we can't put relative input in if we're capable of putting
            // raw input in" - operator, 2026-08-30).
            LOG_ERROR(Input, "D1 native mouse: raw input registration failed (error {}); "
                             "refusing capture",
                      GetLastError());
            return false;
        }
        // Removal can fail benignly when registration never happened this session.
        LOG_WARNING(Input, "D1 native mouse: raw input removal reported error {}",
                    GetLastError());
        return true;
    }
    g_win32_raw_active.store(active, std::memory_order_release);
    return true;
}

bool Win32RawCaptureActive() noexcept {
    return g_win32_raw_active.load(std::memory_order_acquire);
}
#endif

} // namespace Core::D1NativeMouse
