import sys


def rep(s, old, new, n=1):
    assert s.count(old) == n, (old[:70], s.count(old))
    return s.replace(old, new)


PR = [
("""    pub off_completions: usize,
    pub completion_slots: usize,
}
""",
 """    pub off_completions: usize,
    pub completion_slots: usize,
    /// ⭐ The 64 pending-loot slots (`0x80801829`, 56 B each): field `[10]` `0x80801701` `+0x8`,
    /// `0x80804671`. `None` where unmeasured.
    pub off_pending_loot: Option<usize>,
}

/// Pending-loot slots and their stride.
pub const PENDING_LOOT_SLOTS: usize = 64;
pub const PENDING_LOOT_STRIDE: usize = 0x38;

/// ⭐ A pending world-loot record (`0x80801829`): an item the character is owed and will receive
/// when it picks up the matching world drop. The host announces the drop with incident 1504 (a
/// `0x80800CB6` body carrying the same nonce); walking onto it sends a service-601 claim naming
/// `source_kind` and the nonce. Layout: `+0` i16 content index, `+4` u32 quantity (1), `+8`
/// 4-bit source kind (3 = a world spawn), `+0x10` `{u32 key, u64 id}` nonce, `+0x20` u32 (1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingLoot {
    pub content_index: u16,
    pub source_kind: u8,
    pub key: u32,
    pub nonce: u64,
}
"""),
("""    off_completions: OFF_COMPLETIONS,
    completion_slots: COMPLETION_SLOTS,
};
""",
 """    off_completions: OFF_COMPLETIONS,
    completion_slots: COMPLETION_SLOTS,
    off_pending_loot: None,
};
"""),
("""    off_completions: OFF_COMPLETIONS + 0x100,
    completion_slots: 2560,
};
""",
 """    off_completions: OFF_COMPLETIONS + 0x100,
    completion_slots: 2560,
    off_pending_loot: Some(0x3E90 + 0x8),
};
"""),
("""    pub completions: Vec<u16>,
}
""",
 """    pub completions: Vec<u16>,
    /// Pending world-loot records (see [`PendingLoot`]). Non-empty marks every other slot empty.
    pub pending_loot: Vec<PendingLoot>,
}
""", ),
("""            all_content: false,
            completions: Vec::new(),
        }""",
 """            all_content: false,
            completions: Vec::new(),
            pending_loot: Vec::new(),
        }"""),
("""        if self.empty_item_slots {
            for i in 0..ITEM_SLOTS {""",
 """        if !self.pending_loot.is_empty() {
            let Some(base) = o_pending(&o_offs(&o_struct_size_marker())) else { unreachable!() };
            let _ = base;
        }
        if self.empty_item_slots {
            for i in 0..ITEM_SLOTS {"""),
]

for root in sys.argv[1:]:
    p = root + '/crates/d1-oracle/src/profile.rs'
    s = open(p, encoding='utf-8').read()
    for o, n in PR[:5]:
        s = rep(s, o, n)
    # the writer: right after the item slots block, using the Offs value `o` is shadowed by the
    # object buffer, so take the offset before.
    s = rep(s, """        let Offs { off_stat_override_present, off_stat_override, off_all_content_override_items, off_all_content_override, struct_size, off_completions, completion_slots, .. } = o;
        let too_many""", """        let Offs { off_stat_override_present, off_stat_override, off_all_content_override_items, off_all_content_override, struct_size, off_completions, completion_slots, off_pending_loot, .. } = o;
        let too_many""")
    s = rep(s, """        if self.empty_item_slots {
            for i in 0..ITEM_SLOTS {
                let at = OFF_ITEM_SLOTS + i * ITEM_SLOT_STRIDE;
                o[at..at + 2].copy_from_slice(&EMPTY_INDEX.to_le_bytes());
            }
        }
""", """        if self.empty_item_slots {
            for i in 0..ITEM_SLOTS {
                let at = OFF_ITEM_SLOTS + i * ITEM_SLOT_STRIDE;
                o[at..at + 2].copy_from_slice(&EMPTY_INDEX.to_le_bytes());
            }
        }

        if !self.pending_loot.is_empty() {
            let Some(base) = off_pending_loot else {
                return Err(OracleError::Shape("pending loot: no measured offset on this build".into()));
            };
            if self.pending_loot.len() > PENDING_LOOT_SLOTS {
                return too_many("pending loot records", self.pending_loot.len(), PENDING_LOOT_SLOTS);
            }
            for k in 0..PENDING_LOOT_SLOTS {
                let at = base + k * PENDING_LOOT_STRIDE;
                match self.pending_loot.get(k) {
                    // Zero is item 0, not empty (as for the item slots): mark unused slots -1.
                    None => o[at..at + 2].copy_from_slice(&EMPTY_INDEX.to_le_bytes()),
                    Some(p) => {
                        o[at..at + 2].copy_from_slice(&p.content_index.to_le_bytes());
                        o[at + 4..at + 8].copy_from_slice(&1u32.to_le_bytes());
                        o[at + 8] = p.source_kind;
                        o[at + 0x10..at + 0x14].copy_from_slice(&p.key.to_le_bytes());
                        o[at + 0x18..at + 0x20].copy_from_slice(&p.nonce.to_le_bytes());
                        o[at + 0x20..at + 0x24].copy_from_slice(&1u32.to_le_bytes());
                    }
                }
            }
        }
""")
    open(p, 'w', encoding='utf-8', newline='').write(s)

    p = root + '/crates/d1-oracle/src/main.rs'
    s = open(p, encoding='utf-8').read()
    s = rep(s, """        #[arg(long, default_value = "capture-rig/builds/later-56578/unlock-flags.tsv")]
        flag_table: String,
        /// Print the payload and the decode; write nothing.
        #[arg(long)]
        dry_run: bool,
    },
}
""", """        #[arg(long, default_value = "capture-rig/builds/later-56578/unlock-flags.tsv")]
        flag_table: String,
        /// ⭐ A pending world-loot record `CONTENT_INDEX:SOURCE_KIND:KEY:NONCE` (repeatable), e.g.
        /// the starter Khvostov `3598:3:1:0x...` that the Breach loot drop (incident 1504) names.
        #[arg(long)]
        pending_loot: Vec<String>,
        /// Print the payload and the decode; write nothing.
        #[arg(long)]
        dry_run: bool,
    },
}
""")
    s = rep(s, "            complete_flags, flag_table, dry_run,", "            complete_flags, flag_table, pending_loot, dry_run,")
    s = rep(s, """                    subs.sort_unstable();
                    subs.dedup();
                    subs
                },
            };
""", """                    subs.sort_unstable();
                    subs.dedup();
                    subs
                },
                pending_loot: pending_loot.iter().map(|v| {
                    let f: Vec<&str> = v.split(':').collect();
                    let num = |t: &str| t.strip_prefix("0x").map_or_else(|| t.parse::<u64>().ok(), |h| u64::from_str_radix(h, 16).ok());
                    match f.as_slice() {
                        [i, k, key, n] => Some(profile::PendingLoot {
                            content_index: num(i)? as u16, source_kind: num(k)? as u8, key: num(key)? as u32, nonce: num(n)?,
                        }),
                        _ => None,
                    }.ok_or_else(|| anyhow::anyhow!("--pending-loot {v:?}: want CONTENT_INDEX:SOURCE_KIND:KEY:NONCE"))
                }).collect::<Result<Vec<_>, _>>()?,
            };
""")
    open(p, 'w', encoding='utf-8', newline='').write(s)
    print('ok', root)
