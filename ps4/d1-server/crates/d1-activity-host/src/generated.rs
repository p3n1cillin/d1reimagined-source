// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! ⭐ **The generated world tables: a world for any activity, from the client's own content.**
//!
//! `tools/ps4_setup/d1-world-table.py` writes five identifier tables to
//! `ps4/fixtures/56578/worlds/` (format: `ps4/fixtures/56578/README.md` and
//! `notes/ps4-56578-worlds-2026-11-05.md`). This module reads four of them and plans, per activity
//! index, what the hand-built `D1_AH_WORLD_TABLE` row would say: the world's rung-33 values
//! (PRIVATE checksum, bubble count, choosable bubbles), the start slice set, the sensor group and
//! the sensor-client list of the start slice set.
//!
//! The list is the union of the start slice set's bundle rows whose lookup group names its bubble
//! or every bubble (`-1`) and that carry an A or B value type. That rule reproduces the client's
//! own pending lists (Tower set 0, cine_reveal 25, cine_speaker 1, The Dark Within 8, Highway 113,
//! First Light 0; `tests/test_world_table.py`). Values are left at their defaults: a list that
//! needs served values (squad states, spawn bundles) still needs its hand-built file.
//!
//! ⭐ Knob: `D1_AH_WORLDS=<folder>` (main.rs). Unset = no change. Rows from `D1_AH_WORLD_TABLE`
//! win over generated ones for the same index.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};

/// The lifetime record's row type: the group holding it is the frame's name group.
pub const LIFETIME_TYPE: u8 = 0x10;

/// One world's scenario facts (`worlds.tsv`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldFacts {
    pub name: String,
    pub scenario_tag: u32,
    /// `scenario_record+0x0C`, the PRIVATE checksum rung 33 compares.
    pub checksum: u32,
    pub bubble_count: u32,
    /// Bubbles with a valid state, which may be marked chosen.
    pub chosen: Vec<usize>,
}

/// One slice set's bundle tags (`slice-sets.tsv`), global, authoritative, private.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SliceSet {
    pub public: bool,
    pub bundles: Vec<u32>,
    /// The bubble's region name hash (FNV-1 of the slice the scripts name, `plaza`), when the
    /// table has the column.
    pub region_hash: Option<u32>,
    pub valid: bool,
    /// The scenario state's name hash: the FNV-1 of the mission phase the state belongs to
    /// (`ps4/fixtures/56578/scripts/slice-states.tsv`, [`GeneratedWorlds::load_slice_states`]).
    pub state_hash: Option<u32>,
}

/// One sensor client of a bundle: row type, row index, A and B value types present.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Client {
    pub kind: u8,
    pub index: i16,
    pub has_a: bool,
    pub has_b: bool,
}

/// One activity's binding (`activities.tsv`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityWorld {
    pub index: u16,
    pub hash: u32,
    pub name: String,
    pub world: Option<String>,
    pub start_region: Option<u32>,
    pub start_public: bool,
    pub private_start: Option<u32>,
    pub status: String,
}

/// What the host serves for one activity: the inputs of a `ServedWorld`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub index: u16,
    pub world: WorldFacts,
    pub start_region: u32,
    pub group: u32,
    /// A sensor-client list (`hash i8 sub has_a has_b` per line), for `sensor_list::params_for_list`.
    pub list: String,
    pub status: String,
}

#[derive(Debug, Default)]
pub struct GeneratedWorlds {
    pub worlds: HashMap<String, WorldFacts>,
    /// `worlds.tsv`'s `public_checksum` (`scenario_record+0x10`), by world: what a public
    /// instance of the world publishes (stream O; `public_host::PublicHost::public_checksum`).
    pub public_checksums: HashMap<String, u32>,
    pub slices: HashMap<(String, u32), SliceSet>,
    /// Per bundle tag: (group, lookup bubble, clients), in file order.
    pub bundles: HashMap<u32, Vec<(u32, i32, Vec<Client>)>>,
    pub activities: BTreeMap<u16, ActivityWorld>,
}

fn num(s: &str) -> Result<u32> {
    let r = match s.strip_prefix("0x") {
        Some(h) => u32::from_str_radix(h, 16),
        None => s.parse(),
    };
    r.map_err(|_| anyhow!("bad number {s:?}"))
}

fn opt(s: &str) -> Result<Option<u32>> {
    if s == "-" {
        Ok(None)
    } else {
        num(s).map(Some)
    }
}

fn list(s: &str) -> Result<Vec<u32>> {
    if s == "-" {
        return Ok(Vec::new());
    }
    s.split(',').map(num).collect()
}

/// The rows of one generated TSV: `# comment` lines, a header, then tab-separated rows.
fn rows<'a>(text: &'a str, want: &[&str]) -> Result<Vec<HashMap<&'a str, &'a str>>> {
    let mut lines = text
        .lines()
        .map(|l| l.trim_end_matches('\r'))
        .filter(|l| !l.is_empty() && !l.starts_with('#'));
    let header: Vec<&str> = lines
        .next()
        .ok_or_else(|| anyhow!("no header"))?
        .split('\t')
        .collect();
    for w in want {
        if !header.contains(w) {
            bail!("no column {w:?} (have {header:?})");
        }
    }
    lines
        .map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            if f.len() != header.len() {
                bail!("{} fields, want {}: {l:?}", f.len(), header.len());
            }
            Ok(header.iter().copied().zip(f).collect())
        })
        .collect()
}

/// Only `worlds.tsv`'s PUBLIC checksums, by world (what `GeneratedWorlds::public_checksums` holds),
/// for a host that serves public instances without loading the other tables.
pub fn read_public_checksums(dir: &Path) -> Result<HashMap<String, u32>> {
    let text = std::fs::read_to_string(dir.join("worlds.tsv")).with_context(|| format!("{}", dir.join("worlds.tsv").display()))?;
    let mut out = HashMap::new();
    for r in rows(&text, &["world", "public_checksum"]).context("worlds.tsv")? {
        if let Some(c) = opt(r["public_checksum"])?.filter(|c| *c != 0) {
            out.insert(r["world"].to_string(), c);
        }
    }
    Ok(out)
}

/// `D1_AH_WORLDS_ACTIVITIES`: the activity indexes to serve. `all` (or `*`) serves every activity
/// with a world, as an unset knob does; otherwise a comma list (`166,167,0`), unparsable items
/// dropped.
pub fn parse_activities(value: &str) -> Option<Vec<u16>> {
    let v = value.trim();
    if v.eq_ignore_ascii_case("all") || v == "*" {
        return None;
    }
    Some(v.split(',').filter_map(|s| s.trim().parse().ok()).collect())
}

impl GeneratedWorlds {
    /// Read `worlds.tsv`, `slice-sets.tsv`, `bundles.tsv` and `activities.tsv` from `dir`.
    pub fn load(dir: &Path) -> Result<Self> {
        let read = |n: &str| {
            std::fs::read_to_string(dir.join(n))
                .with_context(|| format!("{}", dir.join(n).display()))
        };
        Self::parse(
            &read("worlds.tsv")?,
            &read("slice-sets.tsv")?,
            &read("bundles.tsv")?,
            &read("activities.tsv")?,
        )
    }

    pub fn parse(worlds: &str, slices: &str, bundles: &str, activities: &str) -> Result<Self> {
        let mut g = GeneratedWorlds::default();
        for r in rows(
            worlds,
            &[
                "world",
                "scenario_tag",
                "private_checksum",
                "bubble_count",
                "chosen",
            ],
        )
        .context("worlds.tsv")?
        {
            let w = WorldFacts {
                name: r["world"].to_string(),
                scenario_tag: num(r["scenario_tag"])?,
                checksum: num(r["private_checksum"])?,
                bubble_count: num(r["bubble_count"])?,
                chosen: list(r["chosen"])?.into_iter().map(|b| b as usize).collect(),
            };
            if let Some(c) = r.get("public_checksum").copied().map(opt).transpose()?.flatten().filter(|c| *c != 0) {
                g.public_checksums.insert(w.name.clone(), c);
            }
            g.worlds.insert(w.name.clone(), w);
        }
        for r in rows(
            slices,
            &[
                "world",
                "slice_set",
                "public",
                "global",
                "authoritative",
                "private",
            ],
        )
        .context("slice-sets.tsv")?
        {
            let mut tags = list(r["global"])?;
            tags.extend(list(r["authoritative"])?);
            tags.extend(list(r["private"])?);
            g.slices.insert(
                (r["world"].to_string(), num(r["slice_set"])?),
                SliceSet {
                    public: r["public"] == "1",
                    bundles: tags,
                    region_hash: r.get("region_hash").map(|v| num(v)).transpose()?,
                    valid: r.get("valid").is_none_or(|v| *v == "1"),
                    state_hash: None,
                },
            );
        }
        for r in rows(
            bundles,
            &["bundle_tag", "group", "lookup_bubble", "clients"],
        )
        .context("bundles.tsv")?
        {
            let mut clients = Vec::new();
            for tok in r["clients"].split_whitespace() {
                let f: Vec<&str> = tok.split(':').collect();
                let ab = f.get(2).map(|s| s.as_bytes()).unwrap_or_default();
                if f.len() != 3 || ab.len() != 2 {
                    bail!("bundles.tsv: bad client {tok:?} (want TYPE:INDEX:AB)");
                }
                clients.push(Client {
                    kind: f[0].parse()?,
                    index: f[1].parse()?,
                    has_a: ab[0] == b'1',
                    has_b: ab[1] == b'1',
                });
            }
            g.bundles.entry(num(r["bundle_tag"])?).or_default().push((
                num(r["group"])?,
                r["lookup_bubble"].parse()?,
                clients,
            ));
        }
        for r in rows(
            activities,
            &[
                "index",
                "hash",
                "name",
                "world",
                "start_region",
                "start_public",
                "private_start",
                "status",
            ],
        )
        .context("activities.tsv")?
        {
            let index: u16 = r["index"].parse()?;
            g.activities.insert(
                index,
                ActivityWorld {
                    index,
                    hash: num(r["hash"])?,
                    name: r["name"].to_string(),
                    world: (r["world"] != "-").then(|| r["world"].to_string()),
                    start_region: opt(r["start_region"])?,
                    start_public: r["start_public"] == "1",
                    private_start: opt(r["private_start"])?,
                    status: r["status"].to_string(),
                },
            );
        }
        Ok(g)
    }

    /// Read `slice-states.tsv` (`world slice_set bubble valid state_hash region_hash`, written by
    /// `tools/script-host/tools/export-bundles.mjs --tables`): each slice set's state name hash.
    /// A slice set the world tables do not have is skipped.
    pub fn load_slice_states(&mut self, text: &str) -> Result<usize> {
        let mut n = 0;
        for r in rows(text, &["world", "slice_set", "state_hash"]).context("slice-states.tsv")? {
            if let Some(s) = self.slices.get_mut(&(r["world"].to_string(), num(r["slice_set"])?)) {
                s.state_hash = Some(num(r["state_hash"])?);
                n += 1;
            }
        }
        Ok(n)
    }

    /// The bundle groups `slice_set` loads, in the client's registration order (global,
    /// authoritative, private), each once: what a slice change makes active.
    pub fn slice_groups(&self, world: &str, slice_set: u32) -> Vec<u32> {
        let mut out: Vec<u32> = Vec::new();
        for tag in self.slices.get(&(world.to_string(), slice_set)).map(|s| s.bundles.as_slice()).unwrap_or_default() {
            for (group, _, _) in self.bundles.get(tag).map(Vec::as_slice).unwrap_or_default() {
                if !out.contains(group) {
                    out.push(*group);
                }
            }
        }
        out
    }

    /// Every slice set of `world`, in order.
    pub fn slice_sets(&self, world: &str) -> Vec<u32> {
        let mut v: Vec<u32> = self.slices.keys().filter(|(w, _)| w == world).map(|(_, s)| *s).collect();
        v.sort_unstable();
        v
    }

    /// The slice sets of `world` that `slice-sets.tsv` marks public, in order: the regions whose
    /// PAH record the host marks public (`+0x04`) and whose ambassador it releases to start a
    /// public session (`+0x19`, [`crate::messages::ReplicateMembership::PAH_REGION_THRESHOLD`]).
    pub fn public_slice_sets(&self, world: &str) -> Vec<u32> {
        let mut v: Vec<u32> = self
            .slices
            .iter()
            .filter(|((w, _), s)| w == world && s.public)
            .map(|((_, id), _)| *id)
            .collect();
        v.sort_unstable();
        v
    }

    /// The slice set a mission phase runs in: the valid state whose name hash is the phase's
    /// (`cine_107_twr_phase` → cine_reveal's 26); else, by the slice the phase names, the bubble's
    /// first valid private state (`plaza` → 24). `None` when neither is known.
    pub fn phase_slice_set(&self, world: &str, phase_hash: u32, slice_hash: Option<u32>) -> Option<u32> {
        let sets = self.slice_sets(world);
        let get = |s: &u32| self.slices.get(&(world.to_string(), *s));
        sets.iter()
            .find(|s| get(s).is_some_and(|x| x.valid && x.state_hash == Some(phase_hash)))
            .or_else(|| {
                let h = slice_hash?;
                sets.iter().find(|s| get(s).is_some_and(|x| x.valid && !x.public && x.region_hash == Some(h)))
            })
            .copied()
    }

    /// The sensor clients `slice_set` registers: (group, type, index, has_a, has_b), deduplicated.
    pub fn sensor_clients(&self, world: &str, slice_set: u32) -> Vec<(u32, Client)> {
        let Some(s) = self.slices.get(&(world.to_string(), slice_set)) else {
            return Vec::new();
        };
        let bubble = (slice_set / 8) as i32;
        let mut out: Vec<(u32, Client)> = Vec::new();
        for tag in &s.bundles {
            for (group, lb, clients) in self.bundles.get(tag).map(Vec::as_slice).unwrap_or_default()
            {
                if *lb != -1 && *lb != bubble {
                    continue;
                }
                for c in clients {
                    if !out
                        .iter()
                        .any(|(g, o)| g == group && o.kind == c.kind && o.index == c.index)
                    {
                        out.push((*group, *c));
                    }
                }
            }
        }
        out
    }

    /// The list file text for `slice_set`, as `sensor_list::params_for_list` reads it.
    pub fn sensor_list(&self, world: &str, slice_set: u32) -> String {
        let mut text = format!("# {world} slice set {slice_set}: generated (D1_AH_WORLDS)\n");
        for (g, c) in self.sensor_clients(world, slice_set) {
            text += &format!(
                "{g:#010x} {:#x} {} {} {}\n",
                c.kind,
                c.index,
                u8::from(c.has_a),
                u8::from(c.has_b)
            );
        }
        text
    }

    /// What to serve for activity `index`, or why not. With `private_start`, a public start slice set
    /// (which needs a public session the host cannot serve yet) is replaced by the world's first
    /// private one. A world with no private slice set at all (the Lighthouse's social space) keeps
    /// its public start: there is nothing private to stand in.
    pub fn plan(&self, index: u16, private_start: bool) -> Result<Plan> {
        let a = self
            .activities
            .get(&index)
            .ok_or_else(|| anyhow!("activity {index}: no row"))?;
        let name = a
            .world
            .as_deref()
            .ok_or_else(|| anyhow!("activity {index} ({}): no world ({})", a.name, a.status))?;
        let world = self
            .worlds
            .get(name)
            .ok_or_else(|| anyhow!("activity {index}: world {name:?} not in worlds.tsv"))?;
        let mut start = a
            .start_region
            .ok_or_else(|| anyhow!("activity {index}: no start slice set"))?;
        if private_start && a.start_public {
            if let Some(private) = a.private_start {
                start = private;
            }
        }
        let clients = self.sensor_clients(name, start);
        let group = clients
            .iter()
            .find(|(_, c)| c.kind == LIFETIME_TYPE)
            .map(|(g, _)| *g)
            .ok_or_else(|| {
                anyhow!("activity {index}: {name} slice set {start} has no lifetime record")
            })?;
        Ok(Plan {
            index,
            world: world.clone(),
            start_region: start,
            group,
            list: self.sensor_list(name, start),
            status: a.status.clone(),
        })
    }

    /// Plans for `only` (or every activity with a world), and the indexes left out with the reason.
    pub fn plans(
        &self,
        only: Option<&[u16]>,
        private_start: bool,
    ) -> (Vec<Plan>, Vec<(u16, String)>) {
        let (mut ok, mut skipped) = (Vec::new(), Vec::new());
        let all: Vec<u16> = self.activities.keys().copied().collect();
        for &i in only.unwrap_or(&all) {
            match self.plan(i, private_start) {
                Ok(p) => ok.push(p),
                Err(e) => skipped.push((i, e.to_string())),
            }
        }
        (ok, skipped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORLDS: &str = "# GENERATED\nworld\tscenario_tag\tfamily\tprivate_checksum\tpublic_checksum\tbubble_count\tchosen\tpublic_slice_sets\tdestination\tstatic_world\n\
        test_world\t0x80cb800d\t0x1\t0x8fe07299\t0x2\t3\t0,2\t0\tcosmo\t0x19971ce8\n";
    const SLICES: &str = "world\tslice_set\tbubble\tvalid\tpublic\tregion_hash\tdescriptor\tstatic_world\tstatic_bubble\tglobal\tauthoritative\tprivate\n\
        test_world\t0\t0\t1\t1\t0x1\t0x2\t0x3\t0\t0x80000001\t-\t-\n\
        test_world\t16\t2\t1\t0\t0x1\t0x2\t0x3\t2\t0x80000001\t0x80000002\t-\n";
    const BUNDLES: &str = "bundle_tag\tgroup\tlookup_bubble\tclients\n\
        0x80000001\t0x4786c0e0\t-1\t15:0:10 16:3:10 12:5:11\n\
        0x80000002\t0x99999999\t-1\t6:0:10\n\
        0x80000002\t0x99999999\t2\t4:2:11\n\
        0x80000002\t0x99999999\t1\t4:9:11\n";
    const ACTIVITIES: &str = "index\thash\tname\ttype\tworld\tstart_region\tstart_source\tstart_public\tprivate_start\tsensor_group\tstatus\tevidence\n\
        7\t0x6e0dae89\tA Test\tSTORY\ttest_world\t0\tcapture\t1\t16\t0x4786c0e0\tcross-checked\tlive capture\n\
        8\t0xfb5d13a3\tSkirmish\tPVP\t-\t-\t-\t-\t-\t-\tmissing evidence\tno binding\n";

    fn synthetic() -> GeneratedWorlds {
        GeneratedWorlds::parse(WORLDS, SLICES, BUNDLES, ACTIVITIES).unwrap()
    }

    #[test]
    fn public_slice_sets_are_the_rows_marked_public() {
        let g = synthetic();
        assert_eq!(g.public_slice_sets("test_world"), vec![0], "slice set 0 is public, 16 is not");
        assert!(g.public_slice_sets("no_such_world").is_empty());
    }

    #[test]
    fn a_plan_carries_the_worlds_values_and_the_start_sets_clients() {
        let g = synthetic();
        let p = g.plan(7, false).unwrap();
        assert_eq!(
            (
                p.world.checksum,
                p.world.bubble_count,
                p.world.chosen.clone()
            ),
            (0x8FE0_7299, 3, vec![0, 2])
        );
        assert_eq!((p.start_region, p.group), (0, 0x4786_C0E0));
        assert_eq!(
            p.list.lines().skip(1).collect::<Vec<_>>(),
            [
                "0x4786c0e0 0xf 0 1 0",
                "0x4786c0e0 0x10 3 1 0",
                "0x4786c0e0 0xc 5 1 1"
            ]
        );
        // A public start, served privately: the first private slice set, and its bubble's rows only.
        let p = g.plan(7, true).unwrap();
        assert_eq!(p.start_region, 16);
        assert!(
            p.list.contains("0x99999999 0x4 2 1 1") && !p.list.contains(" 9 1 1"),
            "{}",
            p.list
        );
        // Every list line parses as the host's own reader does.
        assert!(p
            .list
            .lines()
            .skip(1)
            .all(|l| d1_oracle::sensor::SensorClientRef::parse_line(l).is_some()));
    }

    #[test]
    fn activities_without_a_world_are_skipped_with_the_reason() {
        let g = synthetic();
        let (ok, skipped) = g.plans(None, false);
        assert_eq!(ok.iter().map(|p| p.index).collect::<Vec<_>>(), [7]);
        assert_eq!(skipped.len(), 1);
        assert!(
            skipped[0].1.contains("no world (missing evidence)"),
            "{}",
            skipped[0].1
        );
        assert!(g.plans(Some(&[9]), false).1[0].1.contains("no row"));
    }

    #[test]
    fn a_public_only_world_keeps_its_public_start() {
        let acts = format!("{ACTIVITIES}10\t0x1\tPublic Only\tSOCIAL\ttest_world\t0\tstand-in\t1\t-\t0x4786c0e0\tgenerated\tx\n");
        let g = GeneratedWorlds::parse(WORLDS, SLICES, BUNDLES, &acts).unwrap();
        assert_eq!(g.plan(10, true).unwrap().start_region, 0, "nothing private to stand in");
        assert_eq!(g.plan(7, true).unwrap().start_region, 16, "the private stand-in");
    }

    #[test]
    fn malformed_rows_are_refused() {
        assert!(GeneratedWorlds::parse(
            WORLDS,
            SLICES,
            "bundle_tag\tgroup\tlookup_bubble\tclients\n0x1\t0x2\t-1\t15:0\n",
            ACTIVITIES
        )
        .is_err());
        assert!(GeneratedWorlds::parse("world\n", SLICES, BUNDLES, ACTIVITIES).is_err());
        assert!(
            GeneratedWorlds::parse(WORLDS, SLICES, BUNDLES, "index\thash\n1\t0x1\t3\n").is_err()
        );
    }

    fn committed() -> GeneratedWorlds {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/56578/worlds");
        GeneratedWorlds::load(&dir).expect("committed tables")
    }

    /// Stream O: every world's PUBLIC checksum is read; a destination's worlds share one.
    #[test]
    fn committed_worlds_carry_their_public_checksums() {
        let g = committed();
        assert_eq!(g.public_checksums.get("act_cosmo_m1_dome_a"), Some(&0xe354_f1ea));
        assert_eq!(g.public_checksums.get("act_cosmo_m0_highway"), Some(&0xe354_f1ea));
        assert_eq!(g.public_checksums.get("act_mars_m01_valley"), Some(&0x325c_4bed));
        assert!(g.public_checksums.len() >= g.worlds.len() / 2, "{} of {}", g.public_checksums.len(), g.worlds.len());
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/56578/worlds");
        assert_eq!(read_public_checksums(&dir).unwrap(), g.public_checksums);
    }

    /// The committed tables agree with every world the host measured or read before them.
    #[test]
    fn committed_worlds_match_the_known_worlds() {
        let g = committed();
        for k in crate::worlds::KNOWN_WORLDS {
            let w = g
                .worlds
                .get(k.name)
                .unwrap_or_else(|| panic!("{} not generated", k.name));
            assert_eq!(
                (w.checksum, w.bubble_count),
                (k.checksum, k.bubble_count),
                "{}",
                k.name
            );
            if k.scenario_tag != 0 {
                assert_eq!(w.scenario_tag, k.scenario_tag, "{}", k.name);
            }
            assert!(
                k.chosen.iter().all(|b| w.chosen.contains(b)),
                "{}: {:?} vs {:?}",
                k.name,
                k.chosen,
                w.chosen
            );
        }
    }

    /// The hand-built rows' worlds and starts, as the generated tables give them.
    #[test]
    fn committed_plans_cover_the_hand_built_activities() {
        let g = committed();
        let p = g.plan(164, false).unwrap();
        assert_eq!(
            (p.world.name.as_str(), p.start_region, p.group),
            ("act_cosmo_m0_highway", 113, 0x4786_C0E0)
        );
        let p = g.plan(165, false).unwrap();
        assert_eq!((p.world.name.as_str(), p.start_region), ("cine_reveal", 25));
        assert_eq!(
            p.list.lines().count() - 1,
            22,
            "the client's own cine_reveal list has 22 records"
        );
        assert_eq!(g.plan(0, false).unwrap().world.name, "city_tower_default1");
        let (ok, _) = g.plans(None, false);
        assert!(ok.len() > 250, "{} plans", ok.len());
        for p in &ok {
            let mut params = d1_oracle::sensor::SensorPhaseUpdate {
                gen: 0,
                name_hash: 0,
                name_i8: LIFETIME_TYPE,
                name_sub: 3,
                phase: 3,
                tail: None,
                clients: Vec::new(),
                authority: None,
            };
            params = crate::sensor_list::params_for_list(&params, p.group, -1, &p.list).unwrap();
            assert!(!params.clients.is_empty(), "activity {}", p.index);
        }
    }
}
