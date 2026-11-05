// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Vendor resets from the manifest's schedule, and Xûr's weekend.
//!
//! Each vendor's summary gives `resetIntervalMinutes` and `resetOffsetMinutes`. Read as "a reset
//! falls where (Unix minutes + offset) mod interval == 0", every value lands on the reset times
//! players knew:
//! - the weekly vendors (10080 / 2340): Tuesday 09:00 UTC, the weekly reset;
//! - the daily ones (Bounty Tracker, Iron Banner, Queen's Wrath: 1440 / −540): 09:00 UTC daily;
//! - the Gunsmith (10080 / 900): Wednesday 09:00 UTC, Armsday;
//! - Xûr (Agent of the Nine, 10080 / −12060): Friday 09:00 UTC, his arrival;
//! - House of Judgment (10080 / 22500): the same Tuesday as the weekly vendors.
//!
//! Xûr leaves at the Sunday 09:00 UTC reset, 48 hours after he arrives (he visits from Friday to
//! Sunday). The manifest has no field for that: [`XUR_STAY_MINUTES`] is the one constant here.

use crate::tables::Tables;

/// Xûr stays from Friday's reset to Sunday's.
pub const XUR_STAY_MINUTES: i64 = 2 * 1440;
/// Agent of the Nine.
pub const XUR_HASH: u32 = 2796397637;

/// The period number a moment falls in (each reset starts the next one). Interval 0 = never resets.
pub fn period(now_minutes: i64, interval: i64, offset: i64) -> i64 {
    if interval <= 0 {
        return 0;
    }
    (now_minutes + offset).div_euclid(interval)
}

/// The last reset at or before `now_minutes`, in Unix minutes.
pub fn last_reset(now_minutes: i64, interval: i64, offset: i64) -> i64 {
    if interval <= 0 {
        return i64::MIN;
    }
    period(now_minutes, interval, offset) * interval - offset
}

pub fn next_reset(now_minutes: i64, interval: i64, offset: i64) -> i64 {
    if interval <= 0 {
        return i64::MAX;
    }
    last_reset(now_minutes, interval, offset) + interval
}

/// The period a vendor's stock is in.
pub fn vendor_period(t: &Tables, vendor: u8, now_minutes: i64) -> i64 {
    t.vendors.get(&vendor).map(|v| period(now_minutes, v.reset_interval, v.reset_offset)).unwrap_or(0)
}

/// Is Xûr in the Tower?
pub fn xur_present(t: &Tables, now_minutes: i64) -> bool {
    let Some(x) = t.vendor_by_hash(XUR_HASH) else { return false };
    now_minutes - last_reset(now_minutes, x.reset_interval, x.reset_offset) < XUR_STAY_MINUTES
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tables::tables;
    use crate::vendors::testing::{SATURDAY, WEDNESDAY};

    fn utc(y: i32, mo: u32, d: u32, h: u32) -> i64 {
        // Days from civil (Howard Hinnant), enough for tests.
        let y = if mo <= 2 { y - 1 } else { y } as i64;
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let m = mo as i64;
        let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d as i64 - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        ((era * 146_097 + doe - 719_468) * 86_400 + h as i64 * 3600) / 60
    }

    #[test]
    fn the_manifest_offsets_land_on_the_known_resets() {
        // Tuesday 3 November 2026 09:00 UTC is a weekly reset.
        assert_eq!(last_reset(utc(2026, 11, 3, 9), 10080, 2340), utc(2026, 11, 3, 9));
        assert_eq!(last_reset(utc(2026, 11, 3, 8), 10080, 2340), utc(2026, 10, 27, 9));
        // Daily at 09:00 UTC.
        assert_eq!(last_reset(utc(2026, 11, 4, 8), 1440, -540), utc(2026, 11, 3, 9));
        // Armsday: Wednesday 09:00 UTC.
        assert_eq!(last_reset(WEDNESDAY / 60, 10080, 900), utc(2026, 11, 4, 9));
        // Xûr arrives Friday 09:00 UTC.
        assert_eq!(next_reset(WEDNESDAY / 60, 10080, -12060), utc(2026, 11, 6, 9));
        assert_eq!(utc(2026, 11, 4, 12) * 60, WEDNESDAY, "the test clock is what it says");
    }

    #[test]
    fn xur_is_there_from_friday_to_sunday() {
        let t = tables();
        assert!(!xur_present(t, WEDNESDAY / 60));
        assert!(xur_present(t, SATURDAY / 60));
        assert!(xur_present(t, utc(2026, 11, 6, 9)), "from the Friday reset");
        assert!(!xur_present(t, utc(2026, 11, 6, 8)));
        assert!(xur_present(t, utc(2026, 11, 8, 8)));
        assert!(!xur_present(t, utc(2026, 11, 8, 9)), "gone at the Sunday reset");
    }

    #[test]
    fn a_vendor_that_never_resets_has_one_period() {
        assert_eq!(period(123_456, 0, 0), 0);
        assert_eq!(next_reset(5, 0, 0), i64::MAX);
    }
}
