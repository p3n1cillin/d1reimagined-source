// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The Director and orbit: which activities are open, completing them, launching.
//!
//! **The client decides what the Director lists**, from the unlock state the server serves. Each
//! activity row names requirement ids; each id is an entry of content `0x80a5ffb0` holding two lists
//! of expressions, all of which must be true; the expressions run on the client's stack machine
//! `0x28e590` over the flag array (indexed by master flag-table row) and the unlock values
//! (`notes/ps4-56578-tower-social-vendors-2026-10-30.md`). Its opcodes are the D1 API's
//! `DestinyUnlockFlagOperator`, which this module evaluates in [`eval`]. Op 1 pushes `flag[i] == 2`.
//!
//! So the server's part is the state: the completion record (profile field [11], one byte per sub,
//! 2 = completed) and the 50 served flag pairs (503/205 `0x80801941`). [`complete`] marks an
//! activity's own completion flag (manifest `completionFlagHash`, resolved through the client's flag
//! table to row/kind/sub: `data/social/activities.tsv`). [`available`] predicts the client's verdict
//! where the rule is known, so the server can check a launch and report what a completion opened.
//!
//! Rules known (decoded live, 30 October): activity 0 (the Tower) and 164/165 (A Guardian Rises and
//! its cinematic). ⚠ The others' rules are content, not wire: dump `0x80a5ffb0` (28 entries) and each
//! activity row's requirement ids (content root `+0x48`, 16-byte rows) with a `d1-oracle` reader and
//! load them through `D1_ACTIVITY_RULES` ([`parse_rules`]). Until then [`available`] answers
//! [`Availability::Unknown`] for them and launching them is not refused.

use std::collections::BTreeMap;

use crate::store::{CharacterId, PlayerStore};
use crate::tables::Tables;

/// One op of the client's expression machine: `{u8 op, u32 operand}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Op {
    pub op: u8,
    pub operand: u32,
}

pub const OP_FLAG: u8 = 1;
pub const OP_NOT: u8 = 2;
pub const OP_OR: u8 = 3;
pub const OP_AND: u8 = 4;
pub const OP_UNLOCK_VALUE: u8 = 10;
pub const OP_CONSTANT: u8 = 11;

/// Evaluate one expression (`DestinyUnlockFlagOperator`): 1 Flag, 2 Not, 3 Or, 4 And, 5 Nor, 6 Xor,
/// 7 Nand, 8 Equal, 9 NotEqual, 10 UnlockValue, 11 Constant, 12–15 comparisons, 16–20 arithmetic,
/// 21 Negate. `None` for a malformed expression (stack underflow, unknown op, division by zero).
pub fn eval(ops: &[Op], flag: &dyn Fn(u16) -> u8, value: &dyn Fn(u16) -> i64) -> Option<bool> {
    let mut st: Vec<i64> = Vec::new();
    for o in ops {
        match o.op {
            OP_FLAG => st.push(i64::from(flag(o.operand as u16) == 2)),
            OP_UNLOCK_VALUE => st.push(value(o.operand as u16)),
            OP_CONSTANT => st.push(i64::from(o.operand as i32)),
            OP_NOT => {
                let a = st.pop()?;
                st.push(i64::from(a == 0));
            }
            21 => {
                let a = st.pop()?;
                st.push(-a);
            }
            3..=9 | 12..=20 => {
                let b = st.pop()?;
                let a = st.pop()?;
                let (x, y) = (a != 0, b != 0);
                st.push(match o.op {
                    3 => i64::from(x || y),
                    4 => i64::from(x && y),
                    5 => i64::from(!(x || y)),
                    6 => i64::from(x ^ y),
                    7 => i64::from(!(x && y)),
                    8 => i64::from(a == b),
                    9 => i64::from(a != b),
                    12 => i64::from(a > b),
                    13 => i64::from(a >= b),
                    14 => i64::from(a < b),
                    15 => i64::from(a <= b),
                    16 => a.wrapping_add(b),
                    17 => a.wrapping_sub(b),
                    18 => a.wrapping_mul(b),
                    19 => a.checked_div(b)?,
                    _ => a.checked_rem(b)?,
                });
            }
            _ => return None,
        }
    }
    (st.len() == 1).then(|| st[0] != 0)
}

/// An activity's rule: every expression must be true.
pub type Rule = Vec<Vec<Op>>;

const fn f(row: u32) -> Op {
    Op { op: OP_FLAG, operand: row }
}
const NOT: Op = Op { op: OP_NOT, operand: 0 };
const OR: Op = Op { op: OP_OR, operand: 0 };

/// The rules decoded live (`notes/ps4-56578-tower-social-vendors-2026-10-30.md`):
/// - 0, the Tower: `NOT f[308]`, `NOT f[3391]`, `(f[419] OR f[440])`, `(f[2668] OR f[440])`;
/// - 164 and 165: `NOT f[309]`, `NOT f[3392]`, and the same shared pair.
pub fn known_rules() -> BTreeMap<u16, Rule> {
    let shared = vec![vec![f(419), f(440), OR], vec![f(2668), f(440), OR]];
    let mut m = BTreeMap::new();
    let mut tower = vec![vec![f(308), NOT], vec![f(3391), NOT]];
    tower.extend(shared.clone());
    m.insert(0, tower);
    for a in [164u16, 165] {
        let mut r = vec![vec![f(309), NOT], vec![f(3392), NOT]];
        r.extend(shared.clone());
        m.insert(a, r);
    }
    m
}

/// Parse `D1_ACTIVITY_RULES`: one activity per line, `index<TAB>expr;expr;...`, each expression a
/// comma list of `op:operand` (e.g. `0\t1:308,2:0;1:419,1:440,3:0`). `#` starts a comment.
pub fn parse_rules(text: &str) -> BTreeMap<u16, Rule> {
    text.lines().map(|l| l.split('#').next().unwrap_or("").trim()).filter(|l| !l.is_empty()).filter_map(|l| {
        let (idx, rest) = l.split_once(char::is_whitespace)?;
        let rule = rest.trim().split(';').map(|e| e.split(',').filter_map(|o| {
            let (a, b) = o.trim().split_once(':')?;
            Some(Op { op: a.parse().ok()?, operand: b.parse().ok()? })
        }).collect()).collect();
        Some((idx.parse().ok()?, rule))
    }).collect()
}

/// The rules in force: the known ones, overridden by `D1_ACTIVITY_RULES` when set.
pub fn rules() -> BTreeMap<u16, Rule> {
    let mut r = known_rules();
    if let Some(p) = std::env::var_os("D1_ACTIVITY_RULES") {
        if let Ok(text) = std::fs::read_to_string(p) {
            r.extend(parse_rules(&text));
        }
    }
    r
}

/// The flag the client's machine reads for a row: a completion-record flag (kind 3) from the
/// completion byte, anything else from the served pairs.
pub fn flag_value<S: PlayerStore + ?Sized>(s: &S, t: &Tables, chr: CharacterId, row: u16) -> u8 {
    if let Some((_, _, sub)) = t.activities.values().filter_map(|a| a.completion).find(|c| c.0 == row && c.1 == 3) {
        return s.completion(chr, sub);
    }
    s.flag(chr, row)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    Open,
    Locked,
    /// No rule known for this activity.
    Unknown,
}

pub fn available<S: PlayerStore + ?Sized>(s: &S, t: &Tables, rules: &BTreeMap<u16, Rule>, chr: CharacterId, activity: u16) -> Availability {
    let Some(rule) = rules.get(&activity) else { return Availability::Unknown };
    let flag = |r: u16| flag_value(s, t, chr, r);
    let value = |_: u16| 0i64; // unlock values are not served yet (all 0)
    if rule.iter().all(|e| eval(e, &flag, &value) == Some(true)) { Availability::Open } else { Availability::Locked }
}

/// Is the activity completed (its completion byte is 2)?
pub fn completed<S: PlayerStore + ?Sized>(s: &S, t: &Tables, chr: CharacterId, activity: u16) -> bool {
    match t.activities.get(&activity).and_then(|a| a.completion) {
        Some((row, 3, sub)) => { let _ = row; s.completion(chr, sub) == 2 }
        Some((row, _, _)) => s.flag(chr, row) == 2,
        None => false,
    }
}

/// Record an activity's completion. Returns the activities whose verdict changed to Open.
pub fn complete<S: PlayerStore + ?Sized>(s: &mut S, t: &Tables, rules: &BTreeMap<u16, Rule>, chr: CharacterId, activity: u16) -> Result<Vec<u16>, String> {
    let a = t.activities.get(&activity).ok_or_else(|| format!("no activity {activity}"))?;
    let (row, kind, sub) = a.completion.ok_or_else(|| format!("activity {activity} ({}) has no completion flag", a.name))?;
    let before: Vec<u16> = rules.keys().copied().filter(|x| available(s, t, rules, chr, *x) == Availability::Open).collect();
    if kind == 3 {
        s.set_completion(chr, sub, 2);
    } else {
        s.set_flag(chr, row, 2);
    }
    Ok(rules.keys().copied().filter(|x| !before.contains(x) && available(s, t, rules, chr, *x) == Availability::Open).collect())
}

/// Check a launch (LAUNCH sends the activity hash; the host resolves the index). Refused only when a
/// known rule says Locked: the client would not have offered it.
pub fn can_launch<S: PlayerStore + ?Sized>(s: &S, t: &Tables, rules: &BTreeMap<u16, Rule>, chr: CharacterId, activity_hash: u32) -> Result<u16, String> {
    let a = t.activities.values().find(|a| a.hash == activity_hash).ok_or_else(|| format!("no activity with hash {activity_hash:#010x}"))?;
    match available(s, t, rules, chr, a.index) {
        Availability::Locked => Err(format!("{} ({}) is locked for this Guardian", a.index, a.name)),
        _ => Ok(a.index),
    }
}

/// The served flag rows set to 2 (for `D1_UNLOCK_FLAGS`, at most 50 pairs), from the store.
pub fn served_flags<S: PlayerStore + ?Sized>(s: &S, chr: CharacterId, candidates: &[u16]) -> Vec<u16> {
    candidates.iter().copied().filter(|r| s.flag(chr, *r) == 2).take(50).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::tests::{memory, CHR};

    fn t() -> &'static Tables {
        crate::tables::tables()
    }

    #[test]
    fn the_machine_evaluates_the_api_operators() {
        let flags = |r: u16| if r == 7 { 2 } else { 0 };
        let vals = |r: u16| i64::from(r) * 10;
        assert_eq!(eval(&[f(7)], &flags, &vals), Some(true));
        assert_eq!(eval(&[f(8)], &flags, &vals), Some(false));
        assert_eq!(eval(&[f(8), NOT], &flags, &vals), Some(true));
        assert_eq!(eval(&[f(8), f(7), OR], &flags, &vals), Some(true));
        assert_eq!(eval(&[f(8), f(7), Op { op: OP_AND, operand: 0 }], &flags, &vals), Some(false));
        let ge = [Op { op: OP_UNLOCK_VALUE, operand: 3 }, Op { op: OP_CONSTANT, operand: 25 }, Op { op: 13, operand: 0 }];
        assert_eq!(eval(&ge, &flags, &vals), Some(true), "value[3] = 30 >= 25");
        assert_eq!(eval(&[OR], &flags, &vals), None, "underflow");
        assert_eq!(eval(&[Op { op: OP_CONSTANT, operand: 1 }, Op { op: OP_CONSTANT, operand: 0 }, Op { op: 19, operand: 0 }], &flags, &vals), None);
    }

    /// The Tower opens once 308 and 3391 are clear and the shared pair holds (run sense66).
    #[test]
    fn the_tower_rule_matches_the_live_reading() {
        let r = known_rules();
        let mut s = memory();
        assert_eq!(available(&s, t(), &r, CHR, 0), Availability::Locked, "the shared pair needs 440 or 419+2668");
        s.set_flag(CHR, 440, 2);
        assert_eq!(available(&s, t(), &r, CHR, 0), Availability::Open);
        s.set_flag(CHR, 308, 2);
        assert_eq!(available(&s, t(), &r, CHR, 0), Availability::Locked, "serving 308 refused the Tower (sense64)");
        assert_eq!(available(&s, t(), &r, CHR, 166), Availability::Unknown);
    }

    #[test]
    fn completing_restoration_sets_its_completion_byte() {
        let r = known_rules();
        let mut s = memory();
        assert!(!completed(&s, t(), CHR, 166));
        complete(&mut s, t(), &r, CHR, 166).unwrap();
        assert_eq!(s.completion(CHR, 42), 2, "Restoration's completion is sub 42");
        assert!(completed(&s, t(), CHR, 166));
        // A rule over its completion row sees it.
        let mut rr = r.clone();
        rr.insert(167, vec![vec![f(132)]]);
        assert_eq!(available(&s, t(), &rr, CHR, 167), Availability::Open);
    }

    #[test]
    fn a_completion_reports_what_it_opened() {
        let mut s = memory();
        s.set_flag(CHR, 440, 2);
        let mut rr = known_rules();
        rr.insert(166, vec![vec![f(130)]]); // Restoration after A Guardian Rises (sub 40)
        let opened = complete(&mut s, t(), &rr, CHR, 164).unwrap();
        assert_eq!(opened, vec![166]);
        assert_eq!(can_launch(&s, t(), &rr, CHR, 0xb9fccac3), Ok(166));
        let fresh = memory();
        assert!(can_launch(&fresh, t(), &rr, CHR, 0xb9fccac3).is_err(), "locked before A Guardian Rises");
    }

    #[test]
    fn rules_parse_from_text() {
        let r = parse_rules("# x\n0\t1:308,2:0;1:419,1:440,3:0\n");
        assert_eq!(r[&0], vec![vec![f(308), NOT], vec![f(419), f(440), OR]]);
    }
}
