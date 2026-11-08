// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! 406: the item's state flags. The item request handler `0xA29D30` reads the item's slot entry
//! (`0x808016E0`) and sends its `+0x18` word with bit 0 set (action 8, lock) or cleared (9), or bit 1
//! set (11) or cleared (12; its name is open). The server keeps the word in the same place, the
//! profile's slot entry, so every later profile push carries it and the infusion check
//! (`0x26F260`, bit 0) sees it.
//!
//! ⚠ An item in the vault is refused: the player store's account slots keep no flags word
//! (`Account.slots` is `(content, quantity)`), so a lock there could not be kept. A move to the vault
//! (405) likewise drops it.

use d1_bap::investment_ops::ItemSelector;

use super::snapshot::{Place, Snapshot, PROFILE_SLOTS};
use super::{Refusal, Status};

pub fn set_flags(s: &mut Snapshot, item: ItemSelector, flags: i32) -> Result<String, Refusal> {
    let place = if item.instanced || item.instance != 0 {
        s.locate(item.instance).ok_or_else(|| Refusal::new(Status::NotFound, format!("instance {:#x} is in no slot", item.instance)))?
    } else {
        // A stack: the first profile slot of that content.
        let content = item.index as u16;
        let slot = (0..PROFILE_SLOTS).find(|sl| { let e = s.profile_slot(*sl); e.0 == content && e.1 == 0 })
            .ok_or_else(|| Refusal::new(Status::NotFound, format!("no stack of content {content}")))?;
        Place::Profile { slot, bucket: super::snapshot::profile_bucket(slot).unwrap_or(0) }
    };
    match place {
        Place::Account { .. } => Err(Refusal::new(Status::Invalid, "the vault's slots keep no flags word in the player store")),
        Place::Profile { slot, .. } => {
            let before = s.profile_slot(slot).3;
            let after = flags as u32;
            if before == after {
                return Ok(format!("slot {slot}: flags already {after:#x}"));
            }
            s.set_profile_flags(slot, after);
            Ok(format!("slot {slot}: flags {before:#x} -> {after:#x}{}", if after & 1 != 0 { " (locked)" } else { "" }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::investment::snapshot::tests::{snapshot, with_instance};
    use crate::investment::snapshot::Touch;

    #[test]
    fn lock_and_unlock_write_the_slot_word() {
        let mut s = snapshot();
        with_instance(&mut s, 3598, 0x401, vec![0; 0xB8]);
        let sel = ItemSelector { instanced: true, instance: 0x401, index: -1 };
        set_flags(&mut s, sel, 1).unwrap();
        assert_eq!(s.flags(0x401), Some(1));
        assert_eq!(s.log, vec![Touch::Profile], "the profile only");
        s.log.clear();
        set_flags(&mut s, sel, 1).unwrap();
        assert!(s.log.is_empty(), "no change, no push");
        set_flags(&mut s, sel, 0).unwrap();
        assert_eq!(s.flags(0x401), Some(0));
        assert_eq!(set_flags(&mut s, ItemSelector { instanced: true, instance: 0x999, index: -1 }, 1).unwrap_err().status, Status::NotFound);
    }
}
