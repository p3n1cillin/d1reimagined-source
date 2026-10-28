//! **601** `loot_pickup_item` - the client claiming a world drop it walked onto.
//!
//! The flow on 01.33 (56578, notes `ps4-56578-blockers-2026-10-26.md`): the character's profile owes
//! an item as a *pending-loot* record (`0x80801829`, nonce `{key, id}`); the activity host announces
//! the drop with incident `0x7A0FD954` (catalog 1851) whose recipient is the activity host's own id;
//! the client's loot manager spawns the pickup; walking onto it sends this request with the same
//! nonce.
//!
//! Both bodies are bit-packed, most significant bit first, with the usual biases (a 4-bit leaf is
//! value + 1, an `i32` is value + 2^31, a `u64` is raw):
//!
//! ```text
//! request  0x80801757  [4 source kind + 1][32 key][64 id][32 word]                 132 bits, 17 B
//! response 0x808017AB  [4 status + 1][32 code][32 key][64 id][32 word]  + 4 pad    164 bits, 21 B
//! ```
//!
//! `[C]` both layouts against the client's own encoder (`d1-oracle message --tag 0x808017ab
//! --msgid 601`), including the claim JOIN223 captured on 01.24
//! (`4800000010a3d000000002170800000010`: kind 3, key 1, id `0x0a3d000000002170`, word 1). The
//! response echoes the claimed nonce after the OK result, so it is a bit copy of the request's
//! nonce fields.

/// The investment message id.
pub const LOOT_PICKUP_ITEM: u16 = 601;

/// A decoded 601 claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LootClaim {
    /// The pending-loot record's source kind (3 = a world drop).
    pub source_kind: u8,
    /// The nonce: `{key, id}`.
    pub key: i32,
    pub id: u64,
    /// The trailing word (1 in every claim seen).
    pub word: i32,
}

const REQUEST_BITS: usize = 132;

fn bits(body: &[u8], from: usize, n: usize) -> Option<u128> {
    if (from + n).div_ceil(8) > body.len() {
        return None;
    }
    let mut v = 0u128;
    for k in from..from + n {
        v = (v << 1) | u128::from(body[k / 8] >> (7 - k % 8) & 1);
    }
    Some(v)
}

/// Decode a 601 request body (after the 2-byte message id).
pub fn parse_loot_pickup_request(body: &[u8]) -> Option<LootClaim> {
    let kind = bits(body, 0, 4)? as u8;
    Some(LootClaim {
        source_kind: kind.checked_sub(1)?,
        key: (bits(body, 4, 32)? as u32 ^ 0x8000_0000) as i32,
        id: bits(body, 36, 64)? as u64,
        word: (bits(body, 100, 32)? as u32 ^ 0x8000_0000) as i32,
    })
}

/// The OK response body for a 601 request: result `{0, 0}` and the claimed nonce echoed.
pub fn loot_pickup_response_body(request: &[u8]) -> Option<Vec<u8>> {
    let nonce = bits(request, 4, REQUEST_BITS - 4)?;
    // [0001][0x80000000][128 nonce bits][0000] = 168 bits
    let hi: u128 = (0x1 << 32) | 0x8000_0000; // the 36-bit result
    let mut out = Vec::with_capacity(21);
    // 36 + 128 + 4 = 168 bits: emit as one big-endian bit string.
    let mut acc: Vec<u8> = Vec::new();
    for k in (0..36).rev() {
        acc.push((hi >> k & 1) as u8);
    }
    for k in (0..128).rev() {
        acc.push((nonce >> k & 1) as u8);
    }
    acc.extend([0, 0, 0, 0]);
    for chunk in acc.chunks(8) {
        out.push(chunk.iter().fold(0u8, |b, &bit| b << 1 | bit));
    }
    Some(out)
}

/// The family-4 DELTA that hands the character a new item instance: optionally the character's
/// updated profile (`{soid, payload}`: the pending record gone, the item in its bucket, any slot
/// unlock the pickup earns), then one family-4/3 inventory element (`0x80801732`, encoded by the
/// client's own encoder) keyed by its instance SOID.
///
/// A delta upserts the objects it carries and leaves every other object alone (a repeated
/// baseline would purge what it does not list, see [`crate::queuez_span_as_delta`]). `version`
/// must be the family-4 version the client last accepted, plus one.
pub fn item_grant_delta(
    account_soid: u64,
    version: u32,
    item_soid: u64,
    element: &[u8],
    profile: Option<(u64, &[u8])>,
) -> Vec<u8> {
    let mut b = crate::queuez_family_header_versioned(
        crate::QUEUEZ_FAMILY_ACCOUNT,
        account_soid,
        1 + profile.is_some() as u32,
        version,
        crate::QUEUEZ_FLAG_DELTA,
    );
    if let Some((soid, payload)) = profile {
        b.extend_from_slice(&crate::profile_instance_type().to_be_bytes());
        b.extend_from_slice(&soid.to_be_bytes());
        b.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        b.extend_from_slice(payload);
    }
    b.extend_from_slice(&crate::INVENTORY_INSTANCE_TYPE.to_be_bytes());
    b.extend_from_slice(&item_soid.to_be_bytes());
    b.extend_from_slice(&(element.len() as u32).to_be_bytes());
    b.extend_from_slice(element);
    b
}

/// A family-4 DELTA carrying only the character's profile (`0x80801743`, client-encoded payload).
/// Used to push a profile change on demand, such as an activity completion. `version` as for
/// [`item_grant_delta`].
pub fn profile_delta(account_soid: u64, version: u32, profile_soid: u64, payload: &[u8]) -> Vec<u8> {
    let mut b = crate::queuez_family_header_versioned(
        crate::QUEUEZ_FAMILY_ACCOUNT,
        account_soid,
        1,
        version,
        crate::QUEUEZ_FLAG_DELTA,
    );
    b.extend_from_slice(&crate::profile_instance_type().to_be_bytes());
    b.extend_from_slice(&profile_soid.to_be_bytes());
    b.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    b.extend_from_slice(payload);
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The profile delta walks as one profile object at the given version, flagged DELTA.
    #[test]
    fn the_profile_delta_is_one_profile_object() {
        let d = profile_delta(1, 4, 0x201, &[0xCD; 40]);
        assert_eq!(crate::queuez_span_version(&d), Some(4));
        assert_eq!(crate::queuez_span_flags(&d), Some(crate::QUEUEZ_FLAG_DELTA));
        let objs = crate::queuez_span_objects(&d).unwrap();
        assert_eq!(objs.len(), 1);
        assert_eq!(objs[0].instance_type, crate::profile_instance_type());
        assert_eq!(objs[0].key, 0x201);
        assert_eq!(objs[0].payload.1 - objs[0].payload.0, 40);
    }

    /// The grant delta walks as one 4/3 object at the given version, flagged DELTA.
    #[test]
    fn the_grant_delta_is_one_inventory_object() {
        let d = item_grant_delta(0xD100_0000_0000_0001, 2, 0xD100_0000_0000_0401, &[0xAB; 171], None);
        assert_eq!(crate::queuez_span_version(&d), Some(2));
        assert_eq!(crate::queuez_span_flags(&d), Some(crate::QUEUEZ_FLAG_DELTA));
        let objs = crate::queuez_span_objects(&d).unwrap();
        assert_eq!(objs.len(), 1);
        assert_eq!(objs[0].instance_type, crate::INVENTORY_INSTANCE_TYPE);
        assert_eq!(objs[0].key, 0xD100_0000_0000_0401);
        assert_eq!(objs[0].payload.1 - objs[0].payload.0, 171);
    }

    /// With a profile, the profile comes first and the item second.
    #[test]
    fn the_grant_delta_can_carry_the_profile() {
        let d = item_grant_delta(1, 3, 0x401, &[0xAB; 171], Some((0x201, &[0xCD; 40])));
        let objs = crate::queuez_span_objects(&d).unwrap();
        assert_eq!(objs.len(), 2);
        assert_eq!((objs[0].instance_type, objs[0].key), (crate::profile_instance_type(), 0x201));
        assert_eq!((objs[1].instance_type, objs[1].key), (crate::INVENTORY_INSTANCE_TYPE, 0x401));
    }

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
    }

    /// The 01.24 claim JOIN223 captured, decoded with the measured layout.
    #[test]
    fn the_join223_claim_decodes() {
        let c = parse_loot_pickup_request(&hex("4800000010a3d000000002170800000010")).unwrap();
        assert_eq!(c, LootClaim { source_kind: 3, key: 1, id: 0x0a3d_0000_0000_2170, word: 1 });
    }

    /// The client's own encoder at 0x808017AB for result 0 and that nonce.
    #[test]
    fn the_response_matches_the_client_encoder() {
        let body = loot_pickup_response_body(&hex("4800000010a3d000000002170800000010")).unwrap();
        assert_eq!(body, hex("180000000800000010a3d000000002170800000010"));
        assert_eq!(body.len(), 21);
    }

    /// The Breach rifle's nonce in our served profile (`{1, crc32("starter_rifle")}`).
    #[test]
    fn the_starter_rifle_nonce_round_trips() {
        // encoder at 0x808017AB: key 1, id 0x827dbaf0, word 1
        let rsp = hex("1800000008000000100000000827dbaf0800000010");
        // [kind 3 + 1][key 1][id][word 1][pad]
        let req = hex("48000000100000000827dbaf0800000010");
        assert_eq!(loot_pickup_response_body(&req).unwrap(), rsp);
        assert_eq!(parse_loot_pickup_request(&req).unwrap().id, 0x827d_baf0);
    }

    #[test]
    fn a_short_body_is_refused() {
        assert_eq!(parse_loot_pickup_request(&[0x48, 0, 0]), None);
        assert_eq!(loot_pickup_response_body(&[0x48, 0, 0]), None);
    }
}
