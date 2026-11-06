// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//! Character creation and selection: 501 `login_create_character` (decoded,
//! `d1_bap::character_lifecycle::CreateCharacterRequest`), 502 `login_delete_character` (u64), 504
//! `login_character` (select).
//!
//! A new Guardian: race, gender and class from the request (wire order {race, gender, class}; the D1
//! API's enums: Human 0 / Awoken 1 / Exo 2, Male 0 / Female 1, Titan 0 / Hunter 1 / Warlock 2), up to
//! three per account, level 1, the starting kit in its buckets and equipped, and the introduction:
//! the prologue, then A Guardian Rises (activity 164).
//!
//! The 56578 default-gear asset loader (`0x576150`, re-found through RTTI) preloads the two class
//! lists in `characters/starting-kit-lists.json`. Their contents are verified; granting exactly
//! the first preload list as a new character's inventory is still an unverified policy. The
//! `D1_STARTING_KIT` table (`class<TAB>bucket:content,...`) stays empty by default. Set it to
//! `content` to choose the exported first list, or to a file to choose another policy.

use std::collections::BTreeMap;

use crate::store::{self, Character, CharacterId, Item, Owner, PlayerStore};
use crate::tables::Tables;

pub const MAX_CHARACTERS: usize = 3;
/// A Guardian Rises: the first activity after the prologue.
pub const FIRST_ACTIVITY: u16 = 164;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Created {
    pub character: Character,
    pub kit: Vec<(u8, u16, u64)>,
    pub first_activity: u16,
}

/// A class's starting kit: `(bucket, content)`.
pub type Kit = BTreeMap<u8, Vec<(u8, u16)>>;

/// Parse `D1_STARTING_KIT`.
pub fn parse_kit(text: &str) -> Kit {
    text.lines().map(|l| l.split('#').next().unwrap_or("").trim()).filter(|l| !l.is_empty()).filter_map(|l| {
        let (class, rest) = l.split_once(char::is_whitespace)?;
        let items = rest.split(',').filter_map(|p| {
            let (b, c) = p.trim().split_once(':')?;
            Some((u8::from_str_radix(b.trim_start_matches("0x"), 16).ok()?, c.parse().ok()?))
        }).collect();
        Some((class.parse().ok()?, items))
    }).collect()
}

pub fn kit_from_env() -> Kit {
    match std::env::var_os("D1_STARTING_KIT") {
        Some(p) if p == "content" => exported_default_gear(),
        Some(p) => std::fs::read_to_string(p).ok().map(|t| parse_kit(&t)).unwrap_or_default(),
        None => Kit::new(),
    }
}

pub fn exported_default_gear() -> Kit {
    parse_kit(include_str!("../../../../fixtures/56578/characters/starting-kit.tsv"))
}

/// Create a Guardian.
pub fn create<S: PlayerStore + Clone>(s: &mut S, t: &Tables, kit: &Kit, race: u8, gender: u8, class: u8, customization: Vec<u8>) -> Result<Created, String> {
    if race > 2 || gender > 1 || class > 2 {
        return Err(format!("race {race}, gender {gender}, class {class} is not a Guardian"));
    }
    if s.characters().len() >= MAX_CHARACTERS {
        return Err(format!("the account already has {MAX_CHARACTERS} characters"));
    }
    let mut work = s.clone();
    let id = work.mint_soid();
    let c = Character { id, race, gender, class, level: 1, customization };
    work.insert_character(c.clone()).map_err(|e| e.to_string())?;
    let mut placed = Vec::new();
    for &(bucket, content) in kit.get(&class).map(|v| v.as_slice()).unwrap_or(&[]) {
        let row = t.items.get(&content).copied();
        let owned = crate::content::default_gear_item(content,class);
        if let Some(item) = owned {
            if item.bucket != bucket { return Err(format!("kit item {content} belongs in bucket {}",item.bucket)); }
        }
        if let Some(r) = row {
            if r.class != 3 && r.class != class {
                return Err(format!("kit item {content} is for class {}", r.class));
            }
        }
        // The manifest-derived social table omits some client starter definitions and carries
        // stale storage for the horn. The owned definition is authoritative where exported.
        let instanced = owned.map(|i|i.instanced).or_else(||row.map(|r|r.instanced())).unwrap_or(true);
        let inst = if instanced { work.mint_soid() } else { 0 };
        let item = if instanced { Item::instanced(content, inst, 0) } else { Item::stack(content, 1) };
        store::give(&mut work, store::owner_of(bucket, id), bucket, item, 0)?;
        if let (true, Some(index)) = (instanced, crate::vendor::equipped_index(bucket)) {
            if work.equipped(id)[index] == 0 {
                work.set_equipped(id, index, inst).map_err(|e| e.to_string())?;
            }
        }
        placed.push((bucket, content, inst));
    }
    *s = work;
    Ok(Created { character: c, kit: placed, first_activity: FIRST_ACTIVITY })
}

/// 502: delete a Guardian (its inventory goes with it; the vault is the account's and stays).
pub fn delete<S: PlayerStore + Clone>(s: &mut S, id: CharacterId) -> Result<(), String> {
    let mut work = s.clone();
    work.remove_character(id).map_err(|e| e.to_string())?;
    *s = work;
    Ok(())
}

/// 504: select. The character must exist.
pub fn select<S: PlayerStore + ?Sized>(s: &S, id: CharacterId) -> Result<Character, String> {
    s.characters().into_iter().find(|c| c.id == id).ok_or_else(|| format!("no character {id:#x}"))
}

/// Is the character still owed its introduction (A Guardian Rises not completed)?
pub fn needs_introduction<S: PlayerStore + ?Sized>(s: &S, t: &Tables, id: CharacterId) -> bool {
    !crate::director::completed(s, t, id, FIRST_ACTIVITY)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::MemoryStore;

    #[test]
    fn three_guardians_with_their_kit_and_the_introduction() {
        let t = crate::tables::tables();
        let titan_helm = *t.items.iter().filter(|(_, r)| r.bucket == 0x07 && r.class == 0 && r.equippable() && r.instanced()).map(|(c, _)| c).min().unwrap();
        let kit = parse_kit(&format!("0\t07:{titan_helm}\n"));
        let mut s = MemoryStore::new();
        let c = create(&mut s, t, &kit, 0, 1, 0, vec![1, 2, 3]).unwrap();
        assert_eq!(c.character.level, 1);
        assert_eq!(c.first_activity, 164);
        assert_eq!(c.kit.len(), 1);
        assert_eq!(s.equipped(c.character.id)[1], c.kit[0].2, "the helmet is worn");
        assert!(needs_introduction(&s, t, c.character.id));
        create(&mut s, t, &kit, 2, 0, 1, vec![]).unwrap();
        create(&mut s, t, &kit, 1, 1, 2, vec![]).unwrap();
        assert!(create(&mut s, t, &kit, 0, 0, 0, vec![]).is_err(), "three at most");
        delete(&mut s, c.character.id).unwrap();
        assert!(select(&s, c.character.id).is_err());
        assert_eq!(s.characters().len(), 2);
        assert!(store::find_content(&s, Owner::Character(c.character.id), 0x07, titan_helm).is_none());
    }

    #[test]
    fn a_bad_identity_or_kit_is_refused() {
        let t = crate::tables::tables();
        let mut s = MemoryStore::new();
        assert!(create(&mut s, t, &Kit::new(), 3, 0, 0, vec![]).is_err());
        assert!(create(&mut s, t, &Kit::new(), 0, 0, 3, vec![]).is_err());
        let hunter_helm = *t.items.iter().filter(|(_, r)| r.bucket == 0x07 && r.class == 1 && r.equippable()).map(|(c, _)| c).min().unwrap();
        assert!(create(&mut s, t, &parse_kit(&format!("0 07:{hunter_helm}")), 0, 0, 0, vec![]).is_err());
        assert!(s.characters().is_empty(), "nothing half-made");
    }
    #[test]
    fn exported_class_lists_create_with_the_owned_storage_metadata() {
        let kit = exported_default_gear();
        assert_eq!(kit.len(),3);
        for (class,items) in &kit {
            assert_eq!(items.len(),12);
            for &(bucket,index) in items {
                let row = crate::content::default_gear_item(index,*class).unwrap();
                assert_eq!(row.bucket,bucket);
                assert!(crate::content::item_hash(index).is_some());
            }
            let mut s = crate::store::MemoryStore::new();
            let made = create(&mut s,crate::tables::tables(),&kit,0,0,*class,vec![]).unwrap();
            assert_eq!(made.kit.len(),12);
            assert!(made.kit.iter().all(|(_,_,soid)|*soid!=0));
        }
    }
}
