// SPDX-License-Identifier: GPL-2.0-or-later
#pragma once
#include <array>
#include <cmath>
#include <cstdint>
#include <cstring>

namespace Input::D1Native {
// A look-only override, applied only on samples where the mouse moved: movement/actions and the
// global mouse table are never modified, and an idle mouse leaves controller look untouched.
// PS4 01.29 native builder constants: -pi/1000 divided by 2pi/3 yaw, pi/3 pitch.
struct LookOverride {
    uintptr_t state{}, frame{};
    uint8_t mode{};
    std::array<float, 6> axes{};
    // The vertical axis at +0x2d4 is read again after the restore site: 0x91bd1c loads it from
    // the input record and 0x91bd24 copies it to the output record. Restoring it at 0x91b7a7 let
    // the controller's pitch through in mouse mode. It therefore stays at the override's zero
    // until the next Begin, which writes the saved controller value back before saving anew.
    uintptr_t pending_vertical_state{};
    float pending_vertical{};
    // The 0x919860 frame of the pass whose look the mouse drives (set by Begin, outside menus).
    // The recoil-offset gate at 0x91AA17 takes it later in the same call; every call reaches
    // that gate exactly once, so a mark never outlives its pass.
    uintptr_t mouse_frame{};
    bool Begin(uintptr_t record, uintptr_t stack_frame, int32_t dx, int32_t dy,
               bool menu, bool invert_x, bool invert_y) {
        mouse_frame = 0;
        if (state) return false;
        if (pending_vertical_state) {
            std::memcpy(reinterpret_cast<uint8_t*>(pending_vertical_state) + 0x2d4,
                        &pending_vertical, sizeof(float));
            pending_vertical_state = 0;
        }
        // An idle mouse overrides nothing: the record keeps the controller's look axes and its
        // source byte, so the right stick works whenever the mouse is not moving. The old rule
        // replaced the axes on every pass while capture was active, which disabled stick look
        // for the whole session. The menu case still zeroes look behind the cursor.
        if (!menu && dx == 0 && dy == 0) return false;
        state = record; frame = stack_frame;
        auto* bytes = reinterpret_cast<uint8_t*>(record);
        mode = bytes[0x2a0];
        std::memcpy(axes.data(), bytes + 0x2c8, sizeof(axes));
        const float yaw = menu ? 0.f : float(dx) * -0.0015f * (invert_x ? -1.f : 1.f);
        const float pitch = menu ? 0.f : float(dy) * 0.003f * (invert_y ? -1.f : 1.f);
        const std::array<float, 6> mouse_axes{yaw, 0.f, pitch, 0.f, pitch, 0.f};
        std::memcpy(bytes + 0x2c8, mouse_axes.data(), sizeof(mouse_axes));
        bytes[0x2a0] = 1;
        if (!menu) mouse_frame = stack_frame;
        return true;
    }
    // True once when the pass in `stack_frame` took its look from the mouse; always clears.
    bool TakeMouseFrame(uintptr_t stack_frame) {
        const bool mouse = mouse_frame != 0 && mouse_frame == stack_frame;
        mouse_frame = 0;
        return mouse;
    }
    void Restore(uintptr_t stack_frame) {
        if (!state || frame != stack_frame) return;
        auto* bytes = reinterpret_cast<uint8_t*>(state);
        for (size_t i = 0; i < axes.size(); ++i) {
            if (i == 3) continue; // +0x2d4, deferred to the next Begin
            std::memcpy(bytes + 0x2c8 + i * sizeof(float), &axes[i], sizeof(float));
        }
        pending_vertical_state = state;
        pending_vertical = axes[3];
        bytes[0x2a0] = mode;
        state = 0; frame = 0; mode = 0; axes = {};
    }
};

// Player-0 identification at the two recentering sites inside the camera-angle applier 0x91EEF0
// (PS4 01.29). look_state is the look-state array, *(u64*)(eboot + 0x2725798): stride 0x1A0 per
// player, yaw at +0x38, pitch at +0x3C. The applier loads it into r14 at 0x91F184.
constexpr uint64_t LookStateStride = 0x1A0;
constexpr uint64_t LookPitchOffset = 0x3C;
// 0x91FE67 (content pitch target and auto-centering latch): r13 is the player offset
// (player * 0x1A0, 0x91EF37); 0x91F9C0..0x91FBDC clobber r13/r14 and restore both before here.
inline bool RecenterSiteIsPlayerZero(uint64_t r13, uint64_t r14, uint64_t look_state) {
    return look_state != 0 && r14 == look_state && r13 == 0;
}
// 0x9202CF (soft pitch-limit spring rate): r13 is NOT the player offset here. 0x91FFEA
// lea rbx,[r14+r13+0x3C] and 0x920143 mov r13,rbx leave r13 = &P[player].pitch, and 0x92013C
// saves the player offset at [rbp-0x248]. Testing r13 == 0 at this site never matches.
inline bool SpringSiteIsPlayerZero(uint64_t r13, uint64_t r14, uint64_t saved_player_offset,
                                   uint64_t look_state) {
    return look_state != 0 && r14 == look_state && saved_player_offset == 0 &&
           r13 == look_state + LookPitchOffset;
}

// Recoil offset, look function 0x919860. P[player]+0xF0 (pitch) / +0xF4 (yaw) hold the recoil
// that weapon kicks added (0x91A6A0) and recoil recovery has not yet drained back out of the view
// (0x91AA7A..0x91B569, applied through the impulse call at 0x91B82E). When a look pass moves the
// pitch by more than 1e-4 against an offset above 1e-4, the block at 0x91AA17 clears the offset
// if the delta AND the stick pitch axis (out+0x2C, copied from record+0x2D4 at 0x91BD24) point
// the offset's way (0x91B448), and otherwise adds the delta to the offset (0x91B459). A mouse
// leaves the stick axis at 0, so mouse look the offset's way was added to the offset, and recovery
// then drained the mouse motion back out of the pitch. For a mouse pass this answers the stick
// question with the mouse, and also clears when the delta reaches or crosses zero, so mouse input
// can shrink the offset (credited compensation, as with a stick) but never grow or flip it.
constexpr float RecoilOffsetEpsilon = 1e-4f; // 0x165D0AC
// A pass is a mouse pass when LookOverride::Begin drove it, or when the game's own KBM path did:
// in KBM mode (record+0x2A0 != 0) the converter 0x9269D0 adds the builder's mouse pitch
// record+0x2D0 / dt. The latter only happens in a pass the override did not touch.
inline bool NativeMousePitchPass(const uint8_t* record) {
    float pitch{};
    std::memcpy(&pitch, record + 0x2d0, sizeof(pitch));
    return record[0x2a0] != 0 && pitch != 0.f;
}
inline bool MouseClearsRecoilOffset(float pitch_delta, float pitch_offset) {
    if (!(std::fabs(pitch_delta) > RecoilOffsetEpsilon) ||
        !(std::fabs(pitch_offset) > RecoilOffsetEpsilon)) {
        return false; // the game leaves the offset alone (NaN included)
    }
    return (pitch_delta > 0.f) == (pitch_offset > 0.f) ||
           std::fabs(pitch_delta) >= std::fabs(pitch_offset);
}
}
