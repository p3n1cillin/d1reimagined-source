// SPDX-License-Identifier: GPL-2.0-or-later
// Print d1-bap RSAT/framing vectors as "name hex" lines for the Blam rsat cross-check.
use d1_bap::*;

fn hex(b: &[u8]) -> String { b.iter().map(|x| format!("{x:02x}")).collect() }

fn main() {
    let acct = 0xD100_0000_0000_0001u64;
    let chr = 0xD100_0000_0000_0101u64;
    let chr2 = 0xD100_0000_0000_0102u64;
    let chr3 = 0xD100_0000_0000_0103u64;
    let prof = 0xD100_0000_0000_0201u64;
    let mut v: Vec<(&str, Vec<u8>)> = vec![];
    v.push(("character", character_rsat(chr, Identity::TITAN_HUMAN_FEMALE)));
    v.push(("character_warlock_exo_male", character_rsat(chr, Identity { race: 2, gender: 0, class: 2 })));
    v.push(("roster0", roster_rsat(acct, &[])));
    v.push(("roster1", roster_rsat(acct, &[chr])));
    v.push(("roster3", roster_rsat(acct, &[chr, chr2, chr3])));
    v.push(("account", account_rsat(acct, &[chr], prof)));
    v.push(("account_serial7", account_rsat_with_serial(acct, &[chr], prof, 7)));
    v.push(("account_noprof", account_rsat_no_profile(acct, &[])));
    // field 6: 3-bit blob 101, field 20: 2-bit blob 11 (the unit test's shapes)
    v.push(("account_fields", account_rsat_with_fields(acct, &[chr], prof, &[(6, &[0b1010_0000], 3), (20, &[0b1100_0000], 2)])));
    v.push(("profile", profile_rsat(prof)));
    v.push(("profile_dest", profile_rsat_with_destination(prof, DESTINATION_OLD_RUSSIA)));
    v.push(("inv503_soid", investment_503_account_soid_body(acct)));
    v.push(("resp206", investment_206_response(6, 200)));
    let f3 = queuez_family3_empty_roster_body(acct);
    v.push(("f3_empty_roster", f3.clone()));
    v.push(("body206_span", fetch_family_response_body(Some(&f3))));
    v.push(("f4_baseline", queuez_family4_baseline_body(acct, &[chr], prof)));
    v.push(("f4_delta_ping", queuez_family4_delta_ping(acct, 1)));
    v.push(("f3_character", queuez_character_body(acct, chr, Identity::TITAN_HUMAN_FEMALE)));
    v.push(("create_resp", character_lifecycle::create_character_response_body(chr)));
    v.push(("result_ok", character_lifecycle::RESULT_OK_BODY.to_vec()));
    if let Some((b503, b205)) = unlock_flag_bodies(&[98, 834, 5015, 6018]) {
        v.push(("unlock503", b503));
        v.push(("unlock205", b205));
    }
    v.push(("unlock205_pinned", investment_205_response_body()));
    for (n, b) in v { println!("{n} {}", hex(&b)); }
}
