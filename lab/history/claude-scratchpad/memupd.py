import sys


def rep(s, old, new):
    assert s.count(old) == 1, (old[:70], s.count(old))
    return s.replace(old, new)


ST = [
("    membership_dirty: bool,\n",
 """    membership_dirty: bool,
    /// ⭐ The membership update number at image `+0x00` (the client's `MEM-%d`). The client's
    /// slice-set transition manager (56578 `0x778D60`) re-reads the PAH-tabulated region record
    /// only when this differs from the last one it processed (`sstm+0x3B0`), so every changed
    /// re-publish advances it. Join and udt-17 refresh re-send the same number.
    membership_update: u32,
"""),
("            membership_dirty: false,\n",
 "            membership_dirty: false,\n            membership_update: 1,\n"),
("""        let mut m = ReplicateMembership::solo(join_request_image).ok()?;
        let first = !self.published_membership;
""",
 """        let mut m = ReplicateMembership::solo(join_request_image).ok()?;
        let first = !self.published_membership;
        m.image[..4].copy_from_slice(&self.membership_update.to_le_bytes());
"""),
("""        let img = self.join_request_image.clone()?;
        let m = self.build_membership(&img)?;
        self.membership_dirty = false;
        self.last_pushed = now;
        tracing::info!(
            "⭐ replicate_membership re-published: teleport request {:?}",
            self.slice_request.map(|(s, i, _)| (s, i))
        );
""",
 """        let img = self.join_request_image.clone()?;
        self.membership_update = self.membership_update.wrapping_add(1).max(1);
        let m = self.build_membership(&img)?;
        self.membership_dirty = false;
        self.last_pushed = now;
        tracing::info!(
            "⭐ replicate_membership re-published as update {}: teleport request {:?}, region record {:?}",
            self.membership_update,
            self.slice_request.map(|(s, i, _)| (s, i)),
            self.pah_region
        );
"""),
("""        assert_eq!(i.pah_region(), Some((24, 0, 3)), "the same target does not advance the token again");
    }
""",
 """        assert_eq!(i.pah_region(), Some((24, 0, 3)), "the same target does not advance the token again");
    }

    /// ⭐ A changed membership carries the next update number, or the client never re-reads the
    /// region record (`0x778D60` skips an update number it already processed).
    #[test]
    fn a_changed_membership_advances_the_update_number() {
        let (mut i, now) = instance();
        let out = i.on_client_message(&join_request([1, 2, 3, 4]), now);
        let number = |m: &ReplicateMembership| u32::from_le_bytes(m.image[..4].try_into().unwrap());
        let joined = out
            .iter()
            .find_map(|p| match &p.message { AhMessage::ReplicateMembership(m) => Some(number(m)), _ => None })
            .expect("the join publishes a membership");
        assert_eq!(joined, 1);
        i.set_pah_region(112, 0, 2);
        let mut img = vec![0u8; 0x220];
        img[0xB8..0xBC].copy_from_slice(&112u32.to_le_bytes());
        img[0x158..0x15C].copy_from_slice(&24u32.to_le_bytes());
        img[0x15C..0x160].copy_from_slice(&0x4415_15C2u32.to_le_bytes());
        i.on_client_message(&ClientMessage::ClientAuthoritativeDataUpdate { struct_image: img }, now);
        let AhMessage::ReplicateMembership(m) = i.membership_due(now).expect("re-published").message else {
            panic!("expected a membership")
        };
        assert_eq!(number(&m), 2, "the next update number");
        assert_eq!(m.pah_region_id(), 24);
    }
"""),
]
MS = [("/// * `MEMBER_COUNT` `+0x00` - what `0x29C730` prints as `MEM-%d`, set to 1;\n",
       "/// * `MEMBER_COUNT` `+0x00` - what `0x29C730` prints as `MEM-%d`: the membership UPDATE NUMBER\n"
       "///   (56578 `0x778D60` logs it as \"membership update '%d'\" and re-reads the PAH region record\n"
       "///   only when it changes). 1 at the join, advanced on every changed re-publish;\n")]

for root in sys.argv[1:]:
    c = root + '/crates/d1-activity-host/src/'
    for fn, pairs in (('state.rs', ST), ('messages.rs', MS)):
        s = open(c + fn, encoding='utf-8').read()
        for o, n in pairs:
            s = rep(s, o, n)
        open(c + fn, 'w', encoding='utf-8', newline='').write(s)
    print('ok', root)
