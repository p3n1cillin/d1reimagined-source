import sys


def rep(s, old, new, n=1):
    assert s.count(old) == n, (old[:70], s.count(old))
    return s.replace(old, new)


S = [
("pub const TAG_NAME_REF: u32 = 0x8080_04B1;\n",
 """pub const TAG_NAME_REF: u32 = 0x8080_04B1;
/// The sensor AUTHORITY table, decoded by `0x2A1C40` into `world+0xA88` when `has_sensor_block`
/// is set (56578: the schema pointer at `0x191A310`, filled at load).
pub const TAG_AUTHORITY: u32 = 0x8080_0CF7;
/// Entries in the authority table: 65 blocks of 12 bytes after a `u32`.
pub const AUTHORITY_BLOCKS: usize = 65;
"""),
("""    pub clients: Vec<SensorClientRef>,
}
""",
 """    pub clients: Vec<SensorClientRef>,
    /// ⭐ The authority table (`has_sensor_block = 1`). `None` keeps the audited frame.
    pub authority: Option<SensorAuthority>,
}

/// ⭐ The sensor authority table: which machine simulates each of the 65 authority blocks.
///
/// 56578 `0x2A1C40` reads `has_sensor_block`; when set, a 65-bit changed mask (block 0 first),
/// then this struct (tag `0x80800CF7`, 784 bytes: `u32`, then 65 × `{peer {u8 @+1}, u16 seq @+8,
/// bool applied @+0xA}`) through the client's own codec, copied to `world+0xA88`. `0x2A1AB0`
/// applies each changed entry: a new `seq` opens a transfer (`0x240A60`), `applied` completes it
/// (`0x240AC0`, the game interface's authority hook). With the table all zero no machine simulates
/// mission objects: the objective handler `0x40A8D0` then never computes "this objective is mine".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SensorAuthority {
    /// Bit `i` set = entry `i` changed (65 bits used).
    pub changed: u128,
    /// The 784-byte struct image.
    pub image: Vec<u8>,
}

impl SensorAuthority {
    /// Every block to `machine` (the 6-bit machine id the client calls local, e.g. 14), with
    /// sequence `seq`, applied.
    pub fn all_to(machine: u8, seq: u16) -> Self {
        let mut image = vec![0u8; 4 + 12 * AUTHORITY_BLOCKS];
        for i in 0..AUTHORITY_BLOCKS {
            let e = 4 + 12 * i;
            image[e + 1] = machine;
            image[e + 8..e + 10].copy_from_slice(&seq.to_le_bytes());
            image[e + 0xA] = 1;
        }
        Self { changed: (1u128 << AUTHORITY_BLOCKS) - 1, image }
    }
}
"""),
("""        w.put(u64::from(self.gen), 8);
        w.put(0, 1); // has_sensor_block = 0
""",
 """        w.put(u64::from(self.gen), 8);
        match &self.authority {
            None => w.put(0, 1), // has_sensor_block = 0
            Some(a) => {
                w.put(1, 1); // has_sensor_block
                for i in 0..AUTHORITY_BLOCKS {
                    w.put(((a.changed >> i) & 1) as u64, 1);
                }
                let e = ObjectOracle::new(TAG_AUTHORITY)?.encode(&a.image, 0x1_0000)?;
                w.raw(&e.payload, e.bits as usize);
            }
        }
"""),
("""        tail,
        clients: Vec::new(),
    }
}
""",
 """        tail,
        clients: Vec::new(),
        authority: None,
    }
}
"""),
]
OM = [("""                tail,
                clients: Vec::new(),
            };
            let payload = m.encode()?;""",
       """                tail,
                clients: Vec::new(),
                authority: None,
            };
            let payload = m.encode()?;""")]
AM = [("""                        tail,
                        clients,
                    };""",
       """                        tail,
                        clients,
                        // ⭐ D1_AH_SENSOR_AUTHORITY=<machine>[,<seq>]: every authority block to
                        // that machine (see `SensorAuthority`). Unset = no authority table.
                        authority: std::env::var("D1_AH_SENSOR_AUTHORITY").ok().and_then(|v| {
                            let mut it = v.split(',').map(|s| s.trim().parse::<u32>().ok());
                            let machine = it.next()??;
                            let seq = it.next().flatten().unwrap_or(1);
                            tracing::info!("⭐ D1_AH_SENSOR_AUTHORITY: all 65 authority blocks to machine {machine}, seq {seq}");
                            Some(d1_oracle::sensor::SensorAuthority::all_to(machine as u8, seq as u16))
                        }),
                    };""")]
T_O = "fn audit(phase: i8, tail: Option<(u32, u32)>) -> SensorPhaseUpdate {\n"
T_N = """#[cfg(test)]
mod authority_tests {
    #[test]
    fn all_to_fills_every_block_and_marks_them_changed() {
        let a = super::SensorAuthority::all_to(14, 1);
        assert_eq!(a.image.len(), 784);
        assert_eq!(a.changed.count_ones(), 65);
        for i in 0..65 {
            let e = 4 + 12 * i;
            assert_eq!((a.image[e + 1], a.image[e + 8], a.image[e + 0xA]), (14, 1, 1));
        }
    }
}

""" + T_O

for root in sys.argv[1:]:
    c = root + '/crates/'
    for fn, pairs in [('d1-oracle/src/sensor.rs', S + [(T_O, T_N)]), ('d1-oracle/src/main.rs', OM), ('d1-activity-host/src/main.rs', AM)]:
        s = open(c + fn, encoding='utf-8').read()
        for o, n in pairs:
            s = rep(s, o, n)
        open(c + fn, 'w', encoding='utf-8', newline='').write(s)
    print('ok', root)
