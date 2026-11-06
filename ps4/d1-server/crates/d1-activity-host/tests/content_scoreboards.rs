// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! The exported rule programs must satisfy the existing scoreboard consumer's contract.

use d1_activity_host::pvp::ScoreRule;
use serde_json::Value;

#[test]
fn owned_content_rules_have_the_existing_scoreboard_shape() {
    let doc: Value = serde_json::from_str(include_str!(
        "../../../../fixtures/56578/activities/scoreboard-definitions.json"
    ))
    .unwrap();
    let definitions = doc["data"].as_array().unwrap();
    assert_eq!(definitions.len(), 23);
    let mut count = 0;
    for definition in definitions {
        for row in definition["rules"].as_array().unwrap() {
            let rule: ScoreRule = serde_json::from_value(row.clone()).unwrap();
            assert!(!rule.bytecode.is_empty());
            assert!(!rule.consts.is_empty());
            assert!(rule.consts.iter().all(|n| n.is_finite()));
            assert_eq!(rule.flags & !7, 0, "scope mask");
            count += 1;
        }
    }
    assert_eq!(count, 1064);
}
