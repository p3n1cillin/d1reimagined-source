// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Character progression the server applies: XP and level-ups, reputation and rank progress, vendor
//! reset schedules, the activity-completion rewards table, and the edits they make to the 56578
//! profile object (`0x80801743`, decoded).
//!
//! Data, not code: the curves, vendor schedules and refund policies are derived from Bungie's D1
//! manifest by `tools/ps4_setup/d1-progression-tables.py` (`ps4/fixtures/56578/progression/*.tsv`,
//! compiled in; `D1_PROGRESSION_CURVES`, `D1_VENDOR_RESETS`, `D1_REFUND_POLICIES` and
//! `D1_ACTIVITY_REWARDS` override them). The rewards
//! table is the original servers' rules as far as anyone has them, each value labelled sourced or
//! stand-in.
//!
//! The client derives levels and ranks from progress by walking the definition's steps; the profile
//! stores only progress (`profile.rs` in d1-oracle). So does this module: a level is computed for
//! logging and the store's summary, never written.

use std::collections::BTreeMap;

/// The 56578 profile object (`0x80801743`, 29,560 bytes decoded), at the offsets the repository
/// established. Each is the client codec's layout, read from served and pushed objects.
pub mod layout {
    pub const OBJECT_BYTES: usize = 29_560;
    /// 340 × 32 B `{u16 content @0, u64 instance @8, u32 quantity @0x10}` (d1-oracle profile.rs).
    pub const ITEM_SLOTS: usize = 0x38;
    pub const ITEM_SLOT_COUNT: usize = 340;
    pub const ITEM_SLOT_STRIDE: usize = 32;
    /// 20 × u64, the equipped instances (vendor.rs `EQUIPPED`).
    pub const EQUIPPED: usize = 0x2B80;
    pub const EQUIPPED_COUNT: usize = 20;
    /// Field [6], struct `0x808017B9`: the current activity (u16 at +0) and its completed flag (+0x18).
    /// The client announces a completion when +0x2C38 goes 0 -> 1 (notes/ps4-56578-postgame-2026-10-29.md).
    pub const ACTIVITY: usize = 0x2C20;
    pub const ACTIVITY_COMPLETED: usize = 0x2C38;
    pub const ACTIVITY_RECORD_BYTES: usize = 0x20;
    /// The progression table: a count, then 127 × 16 B `{u32 progress, u32 daily, u32 weekly, u16 index}`
    /// (d1-oracle profile.rs; the post-game XP 0 / 900 objects differ only at +0x3664).
    pub const PROGRESSION_COUNT: usize = 0x3660;
    pub const PROGRESSIONS: usize = 0x3664;
    pub const PROGRESSION_SLOTS: usize = 127;
    pub const PROGRESSION_STRIDE: usize = 16;
    /// Field [11], one byte per completion flag (tools/ps4_setup/d1-profile-completions.py).
    pub const COMPLETIONS: usize = 0x53C0;
    pub const COMPLETION_BYTES: usize = 2200;
}

/// The progressions the client's completion path needs present: it looks up 117 (character_level)
/// and 118 (character_prestige) and crashes on a missing id; a profile with 117, 118, 124, 249 and
/// 250 was accepted (runs sense47/sense48, notes/ps4-56578-postgame-2026-10-29.md). A client fact,
/// not a reward rule.
pub const COMPLETION_PROGRESSIONS: [u16; 5] = [117, 118, 124, 249, 250];
pub const CHARACTER_LEVEL: &str = "character_level";
pub const CHARACTER_PRESTIGE: &str = "character_prestige";

const BUILTIN_CURVES: &str = include_str!("../../../../fixtures/56578/progression/progression-curves.tsv");
const BUILTIN_RESETS: &str = include_str!("../../../../fixtures/56578/progression/vendor-resets.tsv");
const BUILTIN_REWARDS: &str = include_str!("../../../../fixtures/56578/progression/activity-rewards.tsv");
const BUILTIN_REFUNDS: &str = include_str!("../../../../fixtures/56578/progression/refund-policies.tsv");

fn table_from_env(var: &str, builtin: &'static str) -> String {
    match std::env::var(var).ok().filter(|p| !p.is_empty()) {
        Some(path) => std::fs::read_to_string(&path).unwrap_or_else(|e| {
            tracing::warn!("      ⛔ {var}={path}: {e} - using the repository's table");
            builtin.to_string()
        }),
        None => builtin.to_string(),
    }
}

fn rows<'a>(text: &'a str, header: &str) -> impl Iterator<Item = Vec<String>> + 'a {
    let header = header.to_string();
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .filter(move |l| !l.starts_with(header.as_str()))
        .map(|l| l.split('\t').map(|f| f.trim().to_string()).collect())
}

/// One progression definition: `steps[i]` is the progress from level (or rank) `i` to `i + 1`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Curve {
    pub hash: u32,
    pub name: String,
    pub repeat_last: bool,
    /// The 56578 client's u16 ordinal, where known (the profile's progression entries carry it).
    pub runtime_index: Option<u16>,
    pub steps: Vec<u32>,
}

impl Curve {
    /// Levels completed at `progress`: how many steps' cumulative totals it has reached. For
    /// character_level (`steps[0] = 0`) that is the level itself (0 XP = level 1, 900 = level 2);
    /// for a faction it is the rank (0 below the first step). A repeating curve goes on with its
    /// last step.
    pub fn level(&self, progress: u64) -> u32 {
        let mut total = 0u64;
        let mut level = 0u32;
        for &s in &self.steps {
            total += u64::from(s);
            if progress < total {
                return level;
            }
            level += 1;
        }
        match (self.repeat_last, self.steps.last()) {
            (true, Some(&last)) if last > 0 => level + ((progress - total) / u64::from(last)) as u32,
            _ => level,
        }
    }

    /// The most progress the curve holds (None when it repeats).
    pub fn cap(&self) -> Option<u64> {
        (!self.repeat_last).then(|| self.steps.iter().map(|&s| u64::from(s)).sum())
    }
}

#[derive(Debug, Clone, Default)]
pub struct Curves(pub Vec<Curve>);

impl Curves {
    pub fn parse(text: &str) -> Self {
        let num = |s: &str| s.parse::<u32>().ok();
        Curves(rows(text, "hash\t").filter_map(|f| {
            if f.len() < 6 {
                return None;
            }
            Some(Curve {
                hash: num(&f[0])?,
                name: f[1].clone(),
                repeat_last: f[3] == "1",
                runtime_index: f[4].parse().ok(),
                steps: f[5].split(',').filter_map(num).collect(),
            })
        }).collect())
    }

    pub fn load() -> Self {
        Self::parse(&table_from_env("D1_PROGRESSION_CURVES", BUILTIN_CURVES))
    }

    pub fn by_name(&self, name: &str) -> Option<&Curve> {
        self.0.iter().find(|c| c.name == name)
    }
}

/// What one XP grant did: the new character_level and character_prestige progress and the levels
/// before and after.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XpGrant {
    pub level_xp: u32,
    pub prestige_xp: u32,
    pub level_before: u32,
    pub level_after: u32,
    pub prestige_before: u32,
    pub prestige_after: u32,
}

/// Add `amount` XP: into character_level up to its cap (level 40), the rest into character_prestige
/// (which repeats every 25,000 in the manifest).
pub fn grant_xp(curves: &Curves, level_xp: u32, prestige_xp: u32, amount: u32) -> Option<XpGrant> {
    let level = curves.by_name(CHARACTER_LEVEL)?;
    let prestige = curves.by_name(CHARACTER_PRESTIGE)?;
    let cap = level.cap().unwrap_or(u64::MAX);
    let room = cap.saturating_sub(u64::from(level_xp));
    let into_level = u64::from(amount).min(room) as u32;
    let rest = amount - into_level;
    let (new_level, new_prestige) = (level_xp.saturating_add(into_level), prestige_xp.saturating_add(rest));
    Some(XpGrant {
        level_xp: new_level,
        prestige_xp: new_prestige,
        level_before: level.level(level_xp.into()),
        level_after: level.level(new_level.into()),
        prestige_before: prestige.level(prestige_xp.into()),
        prestige_after: prestige.level(new_prestige.into()),
    })
}

/// A vendor's reset schedule: it resets at epoch minute `m` when `(m + offset) mod interval == 0`
/// (daily 1440/-540 is 09:00 UTC, weekly 10080/2340 is Tuesday 09:00 UTC).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VendorSchedule {
    pub vendor_hash: u32,
    pub name: String,
    pub interval: i64,
    pub offset: i64,
    pub progression_hash: u32,
}

impl VendorSchedule {
    /// The latest reset at or before `now` (epoch minutes).
    pub fn last_reset(&self, now: i64) -> i64 {
        now - (now + self.offset).rem_euclid(self.interval)
    }

    pub fn next_reset(&self, now: i64) -> i64 {
        self.last_reset(now) + self.interval
    }
}

#[derive(Debug, Clone, Default)]
pub struct VendorResets(pub Vec<VendorSchedule>);

impl VendorResets {
    pub fn parse(text: &str) -> Self {
        VendorResets(rows(text, "vendor_hash\t").filter_map(|f| {
            let interval: i64 = f.get(2)?.parse().ok()?;
            (interval > 0).then_some(())?;
            Some(VendorSchedule {
                vendor_hash: f[0].parse().ok()?,
                name: f[1].clone(),
                interval,
                offset: f.get(3)?.parse().ok()?,
                progression_hash: f.get(4).and_then(|v| v.parse().ok()).unwrap_or(0),
            })
        }).collect())
    }

    pub fn load() -> Self {
        Self::parse(&table_from_env("D1_VENDOR_RESETS", BUILTIN_RESETS))
    }

    #[allow(dead_code)] // for stream I's vendor stock rotation
    pub fn get(&self, vendor_hash: u32) -> Option<&VendorSchedule> {
        self.0.iter().find(|v| v.vendor_hash == vendor_hash)
    }
}

/// A vendor whose sales can be returned (`DestinyVendorDefinition.sales[]` `refundPolicy` != 0): in
/// the final manifest only Eververse and its Silver Dust store, 15 minutes (Bungie's FAQ of
/// 21 October 2015 says the same: Silver purchases refundable for about 15 minutes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefundPolicy {
    pub vendor_hash: u32,
    pub name: String,
    pub limit_minutes: u32,
    pub refundable_sales: u32,
    pub sales: u32,
    /// The refundable sales' item hashes (the others at that vendor are not refundable).
    pub item_hashes: Vec<u32>,
}

#[derive(Debug, Clone, Default)]
pub struct RefundPolicies(pub Vec<RefundPolicy>);

impl RefundPolicies {
    pub fn parse(text: &str) -> Self {
        RefundPolicies(rows(text, "vendor_hash\t").filter_map(|f| {
            if f.len() < 7 {
                return None;
            }
            Some(RefundPolicy {
                vendor_hash: f[0].parse().ok()?,
                name: f[1].clone(),
                // One limit per vendor in the manifest; a list would take its shortest.
                limit_minutes: f[3].split(',').filter_map(|x| x.parse().ok()).min()?,
                refundable_sales: f[4].parse().ok()?,
                sales: f[5].parse().ok()?,
                item_hashes: f[6].split(',').filter_map(|x| x.parse().ok()).collect(),
            })
        }).collect())
    }

    pub fn load() -> Self {
        Self::parse(&table_from_env("D1_REFUND_POLICIES", BUILTIN_REFUNDS))
    }

    pub fn get(&self, vendor_hash: u32) -> Option<&RefundPolicy> {
        self.0.iter().find(|p| p.vendor_hash == vendor_hash)
    }

    /// How long a purchase from `vendor_hash` stays refundable, in seconds; None when it can't be
    /// returned. `item_hash` narrows it to the refundable sales when the caller knows the item's
    /// manifest hash (vendor.rs knows only the client's content index so far).
    pub fn window_secs(&self, vendor_hash: u32, item_hash: Option<u32>) -> Option<u64> {
        let p = self.get(vendor_hash)?;
        if let Some(h) = item_hash {
            if !p.item_hashes.contains(&h) {
                return None;
            }
        }
        Some(u64::from(p.limit_minutes) * 60)
    }
}

/// One row of the activity-completion rewards table.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Reward {
    pub activity: u16,
    pub name: String,
    pub subs: Vec<(u16, u8)>,
    pub xp: Option<u32>,
    pub reputation: Vec<(String, u32)>,
    pub unlock_add: Vec<u16>,
    pub unlock_clear: Vec<u16>,
    pub status: String,
    pub source: String,
}

#[derive(Debug, Clone, Default)]
pub struct Rewards(pub BTreeMap<u16, Reward>);

impl Rewards {
    pub fn parse(text: &str) -> Self {
        let list = |s: &str| -> Vec<String> {
            if s == "-" || s.is_empty() { Vec::new() } else { s.split(',').map(|x| x.trim().to_string()).collect() }
        };
        Rewards(rows(text, "activity\t").filter_map(|f| {
            if f.len() < 9 {
                return None;
            }
            let r = Reward {
                activity: f[0].parse().ok()?,
                name: f[1].clone(),
                subs: list(&f[2]).iter().filter_map(|p| {
                    let (s, v) = p.split_once('=')?;
                    Some((s.parse().ok()?, v.parse().ok()?))
                }).collect(),
                xp: f[3].parse().ok(),
                reputation: list(&f[4]).iter().filter_map(|p| {
                    let (n, v) = p.split_once(':')?;
                    Some((n.to_string(), v.parse().ok()?))
                }).collect(),
                unlock_add: list(&f[5]).iter().filter_map(|x| x.parse().ok()).collect(),
                unlock_clear: list(&f[6]).iter().filter_map(|x| x.parse().ok()).collect(),
                status: f[7].clone(),
                source: f[8].clone(),
            };
            Some((r.activity, r))
        }).collect())
    }

    pub fn load() -> Self {
        Self::parse(&table_from_env("D1_ACTIVITY_REWARDS", BUILTIN_REWARDS))
    }
}

// ── profile object edits ─────────────────────────────────────────────────────────────────────

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

fn check(obj: &[u8]) -> anyhow::Result<()> {
    anyhow::ensure!(obj.len() == layout::OBJECT_BYTES, "a profile object is {} bytes, not {}", layout::OBJECT_BYTES, obj.len());
    Ok(())
}

/// The profile's progression entries: runtime index -> (progress, daily, weekly).
pub fn progressions(obj: &[u8]) -> BTreeMap<u16, (u32, u32, u32)> {
    let n = (u32_at(obj, layout::PROGRESSION_COUNT) as usize).min(layout::PROGRESSION_SLOTS);
    (0..n).map(|i| {
        let at = layout::PROGRESSIONS + i * layout::PROGRESSION_STRIDE;
        (u16::from_le_bytes([obj[at + 12], obj[at + 13]]), (u32_at(obj, at), u32_at(obj, at + 4), u32_at(obj, at + 8)))
    }).collect()
}

fn progression_slot(obj: &mut [u8], index: u16) -> anyhow::Result<usize> {
    let n = u32_at(obj, layout::PROGRESSION_COUNT) as usize;
    if let Some(i) = (0..n.min(layout::PROGRESSION_SLOTS)).find(|i| {
        let at = layout::PROGRESSIONS + i * layout::PROGRESSION_STRIDE;
        u16::from_le_bytes([obj[at + 12], obj[at + 13]]) == index
    }) {
        return Ok(layout::PROGRESSIONS + i * layout::PROGRESSION_STRIDE);
    }
    anyhow::ensure!(n < layout::PROGRESSION_SLOTS, "the progression table is full ({n})");
    let at = layout::PROGRESSIONS + n * layout::PROGRESSION_STRIDE;
    obj[at..at + layout::PROGRESSION_STRIDE].fill(0);
    obj[at + 12..at + 14].copy_from_slice(&index.to_le_bytes());
    obj[layout::PROGRESSION_COUNT..layout::PROGRESSION_COUNT + 4].copy_from_slice(&((n + 1) as u32).to_le_bytes());
    Ok(at)
}

/// Set a progression's progress (adding the entry when absent); daily and weekly stay as they are.
pub fn set_progress(obj: &mut [u8], index: u16, progress: u32) -> anyhow::Result<()> {
    check(obj)?;
    let at = progression_slot(obj, index)?;
    obj[at..at + 4].copy_from_slice(&progress.to_le_bytes());
    Ok(())
}

pub fn ensure_progressions(obj: &mut [u8], indices: &[u16]) -> anyhow::Result<()> {
    check(obj)?;
    for &i in indices {
        progression_slot(obj, i)?;
    }
    Ok(())
}

/// Field [6]: `(activity, completed)`, or None when the record is all zero (never set).
pub fn activity_record(obj: &[u8]) -> Option<(u16, bool)> {
    let rec = &obj[layout::ACTIVITY..layout::ACTIVITY + layout::ACTIVITY_RECORD_BYTES];
    let index = u16::from_le_bytes([rec[0], rec[1]]);
    // `ffff` is no current activity: the live host's reset after a cinematic's 8 (run fg2j).
    (rec.iter().any(|&b| b != 0) && index != NO_ACTIVITY)
        .then(|| (index, obj[layout::ACTIVITY_COMPLETED] != 0))
}

/// Field [6]'s index for "no current activity".
pub const NO_ACTIVITY: u16 = 0xFFFF;

/// No current activity: field [6] as the live host reset it 4.5 s after the reveal's 8, in the push
/// answering the client's 104 (A Guardian Rises capture, `d1-srcp.py current`): `ffff ffff`, then
/// zeros through `+0x2c2b`, and zeros from `+0x2c30` through the completed flag. `+0x2c2c..+0x2c2f`
/// is left as it is, as the live push left it.
pub fn clear_activity(obj: &mut [u8]) -> anyhow::Result<String> {
    check(obj)?;
    let a = layout::ACTIVITY;
    obj[a..a + 4].copy_from_slice(&[0xFF; 4]);
    obj[a + 4..a + 0xC].fill(0);
    obj[a + 0x10..=layout::ACTIVITY_COMPLETED].fill(0);
    Ok("no current activity (field [6] = ffff)".into())
}

/// [`complete_activity`] for an activity that is not the current one (the lab's remap stand-in,
/// `D1_STORE_SEED_COMPLETED`): its completion subs and XP, with field [6] left as it was.
pub fn complete_aside(obj: &mut [u8], curves: &Curves, activity: u16, reward: Option<&Reward>) -> anyhow::Result<(String, Option<XpGrant>)> {
    check(obj)?;
    let keep = obj[layout::ACTIVITY..layout::ACTIVITY + layout::ACTIVITY_RECORD_BYTES].to_vec();
    let r = complete_activity(obj, curves, activity, reward, None)?;
    obj[layout::ACTIVITY..layout::ACTIVITY + layout::ACTIVITY_RECORD_BYTES].copy_from_slice(&keep);
    Ok((format!("{} (aside: field [6] kept)", r.0), r.1))
}

pub fn completion_subs(obj: &[u8]) -> BTreeMap<u16, u8> {
    (0..layout::COMPLETION_BYTES).filter(|&i| obj[layout::COMPLETIONS + i] != 0)
        .map(|i| (i as u16, obj[layout::COMPLETIONS + i])).collect()
}

/// The character enters `activity`: field [6] names it, not completed, and the progressions the
/// completion path reads are present (what the lab's mid-mission push `profile-active164-xp0` holds).
pub fn start_activity(obj: &mut [u8], activity: u16) -> anyhow::Result<String> {
    check(obj)?;
    obj[layout::ACTIVITY..layout::ACTIVITY + 2].copy_from_slice(&activity.to_le_bytes());
    obj[layout::ACTIVITY_COMPLETED] = 0;
    ensure_progressions(obj, &COMPLETION_PROGRESSIONS)?;
    Ok(format!("activity {activity} started"))
}

/// XP into the profile through the curves (character_level, then character_prestige).
pub fn add_xp(obj: &mut [u8], curves: &Curves, amount: u32) -> anyhow::Result<XpGrant> {
    check(obj)?;
    let level = curves.by_name(CHARACTER_LEVEL).and_then(|c| c.runtime_index)
        .ok_or_else(|| anyhow::anyhow!("no character_level curve with a runtime index"))?;
    let prestige = curves.by_name(CHARACTER_PRESTIGE).and_then(|c| c.runtime_index)
        .ok_or_else(|| anyhow::anyhow!("no character_prestige curve with a runtime index"))?;
    let p = progressions(obj);
    let (lx, px) = (p.get(&level).map(|e| e.0).unwrap_or(0), p.get(&prestige).map(|e| e.0).unwrap_or(0));
    let g = grant_xp(curves, lx, px, amount).ok_or_else(|| anyhow::anyhow!("no XP curves"))?;
    set_progress(obj, level, g.level_xp)?;
    if g.prestige_xp != px {
        set_progress(obj, prestige, g.prestige_xp)?;
    }
    Ok(g)
}

/// Complete `activity`: field [6] completed (0 -> 1 is what the client announces), the reward's
/// completion subs, and its XP topped up so the activity has given `reward.xp` since it started
/// (`xp_at_start`, character_level + prestige progress then), so XP already granted mid-activity is
/// not given twice. Returns what changed, and the XP grant if any.
pub fn complete_activity(obj: &mut [u8], curves: &Curves, activity: u16, reward: Option<&Reward>,
                         xp_at_start: Option<u64>) -> anyhow::Result<(String, Option<XpGrant>)> {
    check(obj)?;
    if activity_record(obj).map(|r| r.0) != Some(activity) {
        start_activity(obj, activity)?;
    }
    ensure_progressions(obj, &COMPLETION_PROGRESSIONS)?;
    obj[layout::ACTIVITY_COMPLETED] = 1;
    let mut what = vec![format!("activity {activity} completed")];
    let mut grant = None;
    if let Some(r) = reward {
        for &(sub, value) in &r.subs {
            anyhow::ensure!((sub as usize) < layout::COMPLETION_BYTES, "completion sub {sub} out of range");
            obj[layout::COMPLETIONS + sub as usize] = value;
        }
        if !r.subs.is_empty() {
            what.push(format!("subs {:?}", r.subs));
        }
        if let Some(xp) = r.xp {
            let now = total_xp(obj, curves);
            let given = now.saturating_sub(xp_at_start.unwrap_or(now));
            let owed = u64::from(xp).saturating_sub(given) as u32;
            if owed > 0 {
                let g = add_xp(obj, curves, owed)?;
                what.push(format!("+{owed} XP (level {} -> {})", g.level_before, g.level_after));
                grant = Some(g);
            }
        }
    }
    Ok((what.join("; "), grant))
}

/// character_level + character_prestige progress.
pub fn total_xp(obj: &[u8], curves: &Curves) -> u64 {
    let p = progressions(obj);
    [CHARACTER_LEVEL, CHARACTER_PRESTIGE].iter()
        .filter_map(|n| curves.by_name(n)?.runtime_index)
        .map(|i| u64::from(p.get(&i).map(|e| e.0).unwrap_or(0)))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obj() -> Vec<u8> {
        vec![0u8; layout::OBJECT_BYTES]
    }

    #[test]
    fn the_live_reset_clears_field_6_and_an_aside_completion_keeps_it() {
        let curves = Curves::load();
        let mut o = obj();
        start_activity(&mut o, 165).unwrap();
        assert_eq!(activity_record(&o), Some((165, false)));
        let before = o.clone();
        let reward = Reward { subs: vec![(40, 2), (673, 1)], xp: Some(900), ..Reward::default() };
        let (what, xp) = complete_aside(&mut o, &curves, 164, Some(&reward)).unwrap();
        assert!(what.contains("aside"), "{what}");
        assert!(xp.is_some(), "164's XP is paid");
        assert_eq!(activity_record(&o), Some((165, false)), "field [6] still names the served activity");
        assert_eq!(&o[layout::ACTIVITY..layout::ACTIVITY + layout::ACTIVITY_RECORD_BYTES], &before[layout::ACTIVITY..layout::ACTIVITY + layout::ACTIVITY_RECORD_BYTES]);
        assert_eq!((o[layout::COMPLETIONS + 40], o[layout::COMPLETIONS + 673]), (2, 1));
        clear_activity(&mut o).unwrap();
        assert_eq!(activity_record(&o), None, "ffff is no current activity");
        assert_eq!(&o[layout::ACTIVITY..layout::ACTIVITY + 4], &[0xFF; 4]);
        assert_eq!(o[layout::ACTIVITY_COMPLETED], 0);
    }

    #[test]
    fn the_repository_tables_parse() {
        let c = Curves::parse(BUILTIN_CURVES);
        let level = c.by_name(CHARACTER_LEVEL).unwrap();
        assert_eq!((level.hash, level.runtime_index, level.steps.len()), (1716568313, Some(117), 40));
        assert_eq!(level.cap(), Some(346_000), "level 40 = 346,000 XP");
        assert_eq!(c.by_name(CHARACTER_PRESTIGE).unwrap().steps, vec![25_000]);
        let v = VendorResets::parse(BUILTIN_RESETS);
        assert_eq!(v.0.len(), 26);
        let f = RefundPolicies::parse(BUILTIN_REFUNDS);
        assert_eq!(f.0.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["Eververse", "Silver Dust Vendor"]);
        let r = Rewards::parse(BUILTIN_REWARDS);
        assert_eq!(r.0[&164].subs, vec![(40, 2), (673, 1)]);
        assert_eq!(r.0[&164].xp, Some(900));
        assert_eq!(r.0[&164].status, "stand-in");
        assert_eq!(r.0[&165].xp, None);
    }

    #[test]
    fn levels_follow_the_character_level_curve() {
        let c = Curves::parse(BUILTIN_CURVES);
        let l = c.by_name(CHARACTER_LEVEL).unwrap();
        assert_eq!([0, 899, 900, 4999, 5000, 345_999, 346_000, 999_999].map(|x| l.level(x)), [1, 1, 2, 2, 3, 39, 40, 40]);
        let v = c.by_name("faction_fotc_vanguard").unwrap();
        assert_eq!([0, 1499, 1500, 3500, 6000, 61_000, 63_499, 63_500].map(|x| v.level(x)), [0, 0, 1, 2, 3, 25, 25, 26],
                   "Vanguard rank: 1500, 2000, then 2500 repeating");
        let p = c.by_name(CHARACTER_PRESTIGE).unwrap();
        assert_eq!([0, 24_999, 25_000, 50_000].map(|x| p.level(x)), [0, 0, 1, 2]);
    }

    #[test]
    fn xp_past_level_40_goes_to_prestige() {
        let c = Curves::parse(BUILTIN_CURVES);
        let g = grant_xp(&c, 0, 0, 900).unwrap();
        assert_eq!((g.level_xp, g.level_before, g.level_after), (900, 1, 2));
        let g = grant_xp(&c, 345_000, 0, 26_000).unwrap();
        assert_eq!((g.level_xp, g.prestige_xp, g.level_after, g.prestige_after), (346_000, 25_000, 40, 1));
    }

    #[test]
    fn a_reset_falls_on_the_manifest_offsets() {
        let v = VendorResets::parse(BUILTIN_RESETS);
        // 2016-09-20 (a Tuesday) 12:00 UTC = 1474372800 s.
        let now = 1_474_372_800 / 60;
        let weekly = v.get(3233510749).or_else(|| v.get(2668878854)).unwrap();
        assert_eq!(weekly.last_reset(now), (1_474_362_000 / 60), "Tuesday 09:00 UTC");
        let daily = v.get(1527174714).unwrap();
        assert_eq!(daily.last_reset(now), 1_474_362_000 / 60, "09:00 UTC today");
        assert_eq!(daily.next_reset(now), (1_474_362_000 + 86_400) / 60);
        let xur = v.get(2796397637).unwrap();
        assert_eq!(xur.last_reset(now), (1_474_362_000 - 4 * 86_400) / 60, "Friday 09:00 UTC");
        assert_eq!(weekly.last_reset(weekly.last_reset(now)), weekly.last_reset(now), "a reset minute is its own last reset");
    }

    /// Every curve's rank thresholds are its steps' running totals; Crucible and Trials are in.
    #[test]
    fn rank_thresholds_follow_the_manifest_steps() {
        let c = Curves::parse(BUILTIN_CURVES);
        for curve in &c.0 {
            let mut total = 0u64;
            for (i, &step) in curve.steps.iter().enumerate() {
                total += u64::from(step);
                if step > 0 {
                    assert_eq!(curve.level(total - 1), i as u32, "{} just below threshold {i}", curve.name);
                }
                assert!(curve.level(total) >= i as u32 + 1, "{} at threshold {i}", curve.name);
            }
        }
        let crucible = c.by_name("faction_pvp").unwrap();
        assert_eq!((crucible.hash, crucible.level(1499), crucible.level(1500), crucible.level(3500)), (1357277120, 0, 1, 2));
        let cryptarch = c.by_name("faction_cryptarch").unwrap();
        assert_eq!(cryptarch.level(1500 + 2500 + 3000), 3);
        let trials = c.by_name("pvp_tournament0.wins").unwrap();
        assert_eq!((trials.cap(), trials.level(9)), (Some(26), 9), "a Trials card: one step per win");
        let iron_banner = c.by_name("faction_event_iron_banner").unwrap();
        assert_eq!((iron_banner.cap(), iron_banner.level(8_500)), (Some(8_500), 5), "Iron Banner rank 5 caps");
        let srl = c.by_name("d15.winter.racing").unwrap();
        assert_eq!(srl.hash, 2576753410, "the Shipwright's progression comes with the vendors");
    }

    #[test]
    fn refunds_are_eververse_only_for_15_minutes() {
        let f = RefundPolicies::parse(BUILTIN_REFUNDS);
        assert_eq!(f.window_secs(3917130357, None), Some(900), "Eververse");
        assert_eq!(f.window_secs(3599585337, None), Some(900), "Silver Dust");
        assert_eq!(f.window_secs(2668878854, None), None, "the Vanguard Quartermaster refunds nothing");
        let ev = f.get(3917130357).unwrap();
        assert_eq!((ev.refundable_sales, ev.sales), (67, 123));
        assert_eq!(f.window_secs(3917130357, Some(ev.item_hashes[0])), Some(900));
        assert_eq!(f.window_secs(3917130357, Some(1)), None, "a sale that is not refundable");
    }

    #[test]
    fn completing_tops_up_the_activitys_xp_once() {
        let c = Curves::parse(BUILTIN_CURVES);
        let r = Rewards::parse(BUILTIN_REWARDS);
        let mut o = obj();
        start_activity(&mut o, 164).unwrap();
        assert_eq!(activity_record(&o), Some((164, false)));
        assert_eq!(progressions(&o).keys().copied().collect::<Vec<_>>(), COMPLETION_PROGRESSIONS.to_vec());
        let start = total_xp(&o, &c);
        add_xp(&mut o, &c, 900).unwrap(); // the mid-mission grant (the lab's Dock 13 push)
        let (_, g) = complete_activity(&mut o, &c, 164, r.0.get(&164), Some(start)).unwrap();
        assert!(g.is_none(), "900 already given");
        assert_eq!(progressions(&o)[&117].0, 900);
        assert_eq!(activity_record(&o), Some((164, true)));
        assert_eq!(completion_subs(&o), BTreeMap::from([(40, 2), (673, 1)]));
        // Without the mid-mission grant, completion gives the whole 900.
        let mut o = obj();
        start_activity(&mut o, 164).unwrap();
        let (_, g) = complete_activity(&mut o, &c, 164, r.0.get(&164), Some(0)).unwrap();
        assert_eq!(g.unwrap().level_after, 2);
        assert!(start_activity(&mut vec![0u8; 10], 1).is_err(), "only a whole profile object is edited");
    }
}
