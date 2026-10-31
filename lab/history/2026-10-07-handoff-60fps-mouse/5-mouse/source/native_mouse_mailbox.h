// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

#pragma once

#include <atomic>

#include "common/types.h"

namespace Input {

struct NativeMouseMotion {
    float x{};
    float y{};
};

// One cache-line-sized, lock-free producer/consumer mailbox. SDL accumulates relative motion and
// the guest input seam drains the complete x/y pair with one exchange. Coordinates are stored as
// signed 24.8 fixed point so SDL3 high-resolution fractions are retained without allowing x and y
// from one event to land in different guest samples.
class alignas(64) NativeMouseMailbox {
public:
    void Accumulate(float x, float y) noexcept;
    [[nodiscard]] NativeMouseMotion Consume() noexcept;
    void Clear() noexcept;

private:
    std::atomic<u64> packed_motion{};
};

static_assert(std::atomic<u64>::is_always_lock_free,
              "native mouse mailbox requires lock-free 64-bit atomics");
static_assert(sizeof(NativeMouseMailbox) == 64,
              "native mouse mailbox must occupy exactly one cache line");

} // namespace Input
