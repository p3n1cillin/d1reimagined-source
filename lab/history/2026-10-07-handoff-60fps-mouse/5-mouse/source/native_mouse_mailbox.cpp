// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

#include "input/native_mouse_mailbox.h"

#include <algorithm>
#include <bit>
#include <cmath>
#include <limits>

namespace Input {
namespace {

constexpr float SubpixelsPerCount = 256.0f;

s32 ToFixed(float value) noexcept {
    if (!std::isfinite(value)) {
        return 0;
    }
    const double scaled = std::round(static_cast<double>(value) * SubpixelsPerCount);
    return static_cast<s32>(std::clamp(scaled, static_cast<double>(std::numeric_limits<s32>::min()),
                                       static_cast<double>(std::numeric_limits<s32>::max())));
}

s32 SaturatingAdd(s32 lhs, s32 rhs) noexcept {
    const s64 sum = static_cast<s64>(lhs) + static_cast<s64>(rhs);
    return static_cast<s32>(std::clamp(sum, static_cast<s64>(std::numeric_limits<s32>::min()),
                                       static_cast<s64>(std::numeric_limits<s32>::max())));
}

u64 Pack(s32 x, s32 y) noexcept {
    return static_cast<u64>(std::bit_cast<u32>(x)) |
           (static_cast<u64>(std::bit_cast<u32>(y)) << 32);
}

s32 UnpackX(u64 packed) noexcept {
    return std::bit_cast<s32>(static_cast<u32>(packed));
}

s32 UnpackY(u64 packed) noexcept {
    return std::bit_cast<s32>(static_cast<u32>(packed >> 32));
}

} // namespace

void NativeMouseMailbox::Accumulate(float x, float y) noexcept {
    const s32 add_x = ToFixed(x);
    const s32 add_y = ToFixed(y);
    u64 observed = packed_motion.load(std::memory_order_relaxed);
    for (;;) {
        const u64 desired =
            Pack(SaturatingAdd(UnpackX(observed), add_x), SaturatingAdd(UnpackY(observed), add_y));
        if (packed_motion.compare_exchange_weak(observed, desired, std::memory_order_relaxed,
                                                std::memory_order_relaxed)) {
            return;
        }
    }
}

NativeMouseMotion NativeMouseMailbox::Consume() noexcept {
    const u64 packed = packed_motion.exchange(0, std::memory_order_acq_rel);
    return {.x = static_cast<float>(UnpackX(packed)) / SubpixelsPerCount,
            .y = static_cast<float>(UnpackY(packed)) / SubpixelsPerCount};
}

void NativeMouseMailbox::Clear() noexcept {
    packed_motion.store(0, std::memory_order_release);
}

} // namespace Input
