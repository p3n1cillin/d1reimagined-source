// SPDX-License-Identifier: GPL-2.0-or-later
#pragma once
#include "input/d1_native_input_state.h"
#include <cstring>

namespace Input::D1Native {
// PS4 input records: 21 controls, stride 20; trigger L/R=0/1,
// stick click R=9, cross/circle/square=10/11/12. Movement/look start later.
struct ActionMerge {
    std::array<Key, 21> previous{};
    void Apply(uint8_t* record, const std::array<Key, 5>& buttons, bool menu) {
        std::array<Key, 21> mouse{};
        mouse[menu ? 10 : 1] = buttons[0]; // LMB: confirm / fire
        mouse[menu ? 11 : 0] = buttons[1]; // RMB: back / ADS
        if (!menu) {
            mouse[9] = buttons[2]; // MMB: right stick click (default melee)
            mouse[12] = buttons[3]; // Mouse4: square (reload/interact)
            mouse[11] = buttons[4]; // Mouse5: circle (crouch/slide)
        }
        for (unsigned i : {0u, 1u, 9u, 10u, 11u, 12u}) {
            auto* action = record + i * 20;
            uint16_t held; float value;
            std::memcpy(&held, action, 2); std::memcpy(&value, action + 8, 4);
            const bool down = action[2] != 0 || mouse[i].frames != 0;
            Key merged{};
            merged.frames = down ? static_cast<uint8_t>(std::min(255u, unsigned(previous[i].frames) + 1)) : 0;
            merged.held_ms = down ? std::max(held, mouse[i].held_ms) : 0;
            action[3] = previous[i].frames;
            action[2] = merged.frames;
            std::memcpy(action, &merged.held_ms, 2);
            if (!down) value = 0;
            if (mouse[i].frames) value = std::max(value, 1.f);
            std::memcpy(action + 8, &value, 4);
            previous[i] = merged;
        }
    }
};
}
