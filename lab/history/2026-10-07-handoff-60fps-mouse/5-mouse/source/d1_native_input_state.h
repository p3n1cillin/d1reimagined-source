// SPDX-License-Identifier: GPL-2.0-or-later
#pragma once
#include <algorithm>
#include <array>
#include <cmath>
#include <cstdint>
#include <SDL3/SDL_scancode.h>

namespace Input::D1Native {
// Engine order extracted from Xbox 56578's table at RVA 0x11f2a50.
// PS4 01.29 queries independently verified: WASD=20/2d/2e/2f,
// arrows=4d..50, space=48, modifiers=39/44,45/4c,47/49.
constexpr std::array<SDL_Scancode, 104> Keys = {
    SDL_SCANCODE_ESCAPE,
    SDL_SCANCODE_F1,
    SDL_SCANCODE_F2,
    SDL_SCANCODE_F3,
    SDL_SCANCODE_F4,
    SDL_SCANCODE_F5,
    SDL_SCANCODE_F6,
    SDL_SCANCODE_F7,
    SDL_SCANCODE_F8,
    SDL_SCANCODE_F9,
    SDL_SCANCODE_F10,
    SDL_SCANCODE_F11,
    SDL_SCANCODE_F12,
    SDL_SCANCODE_F13,
    SDL_SCANCODE_SCROLLLOCK,
    SDL_SCANCODE_F15,
    SDL_SCANCODE_GRAVE,
    SDL_SCANCODE_1,
    SDL_SCANCODE_2,
    SDL_SCANCODE_3,
    SDL_SCANCODE_4,
    SDL_SCANCODE_5,
    SDL_SCANCODE_6,
    SDL_SCANCODE_7,
    SDL_SCANCODE_8,
    SDL_SCANCODE_9,
    SDL_SCANCODE_0,
    SDL_SCANCODE_MINUS,
    SDL_SCANCODE_EQUALS,
    SDL_SCANCODE_BACKSPACE,
    SDL_SCANCODE_TAB,
    SDL_SCANCODE_Q,
    SDL_SCANCODE_W,
    SDL_SCANCODE_E,
    SDL_SCANCODE_R,
    SDL_SCANCODE_T,
    SDL_SCANCODE_Y,
    SDL_SCANCODE_U,
    SDL_SCANCODE_I,
    SDL_SCANCODE_O,
    SDL_SCANCODE_P,
    SDL_SCANCODE_LEFTBRACKET,
    SDL_SCANCODE_RIGHTBRACKET,
    SDL_SCANCODE_BACKSLASH,
    SDL_SCANCODE_CAPSLOCK,
    SDL_SCANCODE_A,
    SDL_SCANCODE_S,
    SDL_SCANCODE_D,
    SDL_SCANCODE_F,
    SDL_SCANCODE_G,
    SDL_SCANCODE_H,
    SDL_SCANCODE_J,
    SDL_SCANCODE_K,
    SDL_SCANCODE_L,
    SDL_SCANCODE_SEMICOLON,
    SDL_SCANCODE_APOSTROPHE,
    SDL_SCANCODE_RETURN,
    SDL_SCANCODE_LSHIFT,
    SDL_SCANCODE_Z,
    SDL_SCANCODE_X,
    SDL_SCANCODE_C,
    SDL_SCANCODE_V,
    SDL_SCANCODE_B,
    SDL_SCANCODE_N,
    SDL_SCANCODE_M,
    SDL_SCANCODE_COMMA,
    SDL_SCANCODE_PERIOD,
    SDL_SCANCODE_SLASH,
    SDL_SCANCODE_RSHIFT,
    SDL_SCANCODE_LCTRL,
    SDL_SCANCODE_LGUI,
    SDL_SCANCODE_LALT,
    SDL_SCANCODE_SPACE,
    SDL_SCANCODE_RALT,
    SDL_SCANCODE_RGUI,
    SDL_SCANCODE_APPLICATION,
    SDL_SCANCODE_RCTRL,
    SDL_SCANCODE_UP,
    SDL_SCANCODE_DOWN,
    SDL_SCANCODE_LEFT,
    SDL_SCANCODE_RIGHT,
    SDL_SCANCODE_INSERT,
    SDL_SCANCODE_HOME,
    SDL_SCANCODE_PAGEUP,
    SDL_SCANCODE_DELETE,
    SDL_SCANCODE_END,
    SDL_SCANCODE_PAGEDOWN,
    SDL_SCANCODE_NUMLOCKCLEAR,
    SDL_SCANCODE_KP_DIVIDE,
    SDL_SCANCODE_KP_MULTIPLY,
    SDL_SCANCODE_KP_0,
    SDL_SCANCODE_KP_1,
    SDL_SCANCODE_KP_2,
    SDL_SCANCODE_KP_3,
    SDL_SCANCODE_KP_4,
    SDL_SCANCODE_KP_5,
    SDL_SCANCODE_KP_6,
    SDL_SCANCODE_KP_7,
    SDL_SCANCODE_KP_8,
    SDL_SCANCODE_KP_9,
    SDL_SCANCODE_KP_MINUS,
    SDL_SCANCODE_KP_PLUS,
    SDL_SCANCODE_KP_ENTER,
    SDL_SCANCODE_KP_PERIOD
};
inline int KeyIndex(SDL_Scancode key) {
    for (unsigned i = 0; i < Keys.size(); ++i) if (Keys[i] == key) return static_cast<int>(i);
    return -1;
}
struct Key { uint16_t held_ms{}; uint8_t frames{}, flags{}; };
struct Mouse { int32_t dx{}, dy{}, wheel{}; uint8_t frames[4]{}; uint16_t held_ms[4]{}; };
struct Sample { Mouse mouse; std::array<Key, 104> keys; };
static_assert(sizeof(Key) == 4 && sizeof(Mouse) == 24 && sizeof(Sample) == 440);
struct Control {
    uint64_t edges{};
    unsigned pending{};
    bool physical{}, down{}, overflow{};
    Key last{};
    void Push(bool pressed) {
        if (physical == pressed) return;
        physical = pressed;
        if (overflow) { if (!pressed) overflow = false; return; }
        if (pending == 64) {
            edges = 0; pending = 0; down = false; last = {}; overflow = pressed;
            return;
        }
        edges |= uint64_t(pressed) << pending++;
    }
    Key Read(unsigned elapsed) {
        bool before = down;
        if (pending) { down = (edges & 1) != 0; edges >>= 1; --pending; }
        last.held_ms = down ? static_cast<uint16_t>(std::min(65535u, (before ? unsigned(last.held_ms) : 0u) + elapsed)) : 0;
        last.frames = down ? static_cast<uint8_t>(before ? std::min(255u, unsigned(last.frames) + 1) : 1) : 0;
        last.flags = (down ? 1 : 0) | (!before && down ? 2 : 0) | (before && !down ? 4 : 0);
        return last;
    }
};
struct State {
    std::array<Control, 109> controls{};
    std::array<Key, 5> buttons{};
    uint64_t previous_ms{};
    float remainder_x{}, remainder_y{}, wheel{}, cursor_x{}, cursor_y{};
    static int32_t Counts(float value, float& remainder) {
        if (!std::isfinite(value)) { remainder = 0; return 0; }
        const float total = std::clamp(value + remainder, -8192.0f, 8192.0f);
        const auto integer = static_cast<int32_t>(total);
        remainder = total - integer;
        return integer;
    }
    D1Native::Sample Sample(uint64_t now, bool active, float dx, float dy, float sensitivity) {
        if (!active) { *this = {}; return {}; }
        const unsigned elapsed = previous_ms && now >= previous_ms ? static_cast<unsigned>(std::min(uint64_t(100), now - previous_ms)) : 0;
        previous_ms = now;
        D1Native::Sample result{};
        for (unsigned i = 0; i < 104; ++i) result.keys[i] = controls[i].Read(elapsed);
        for (unsigned i = 0; i < 5; ++i) buttons[i] = controls[104 + i].Read(elapsed);
        // Action publication is merged after the title's exclusive pad/KBM selection.
        // Keep the native mouse buttons neutral here to avoid the developer layout's
        // MMB=ADS/RMB=stick-click mapping and duplicate actions.
        result.mouse.dx = Counts(dx * sensitivity, remainder_x);
        result.mouse.dy = Counts(dy * sensitivity, remainder_y);
        float wheel_remainder = 0;
        result.mouse.wheel = Counts(wheel, wheel_remainder);
        wheel = wheel_remainder;
        return result;
    }
};
}
