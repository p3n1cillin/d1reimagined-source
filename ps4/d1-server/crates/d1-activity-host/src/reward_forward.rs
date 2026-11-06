// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! # Rewards across the process boundary: the activity host's grant forwarder
//!
//! Stream M's `rewards::grant_activity_rewards` runs in `d1-responder`, beside the player store.
//! The activity host knows when an activity ends. This module carries the one across to the other
//! through the store's command file (`D1_STORE_CONTROL`, polled each second by the responder), one
//! line per character:
//!
//! ```text
//! grant CHARACTER ACTIVITY_HASH OUTCOME [page=PAGE] [engrams=1] [seed=SEED] [source=WORD]
//! ```
//!
//! The grammar is `d1-responder`'s `rewards::remote` (the reader); both sides test the same lines.
//! `CHARACTER` is the client's 40-byte identity (`id:` + 80 hex digits, `join_request+0x10`,
//! [`d1_mp::identity::MemberIdentity`]: its character SOID is at `+0x18`), or an 8-byte key.
//!
//! **⭐ The function every reward path calls is [`GrantForwarder::forward`]** (stream K's unified
//! `ActivityRewards` trait should call it from its implementation):
//!
//! ```text
//! forwarder.forward(GrantTarget::Identity(identity_bytes), activity_hash, GrantOutcome::Completed, GrantDetails::default())
//! ```
//!
//! It never waits: the line is queued to a writer thread (a disk stall must not hold the
//! connection loop past the client's host-silence limit), so it returns at once. The two reward
//! traits on `main` today are implemented here over it:
//!
//! * [`crate::rewards::ActivityRewards`] (the script bridge's; `CharacterRef` = the identity);
//! * [`crate::open_world::ActivityRewards`] (stream G's open world; an 8-byte character key).
//!
//! **When it is on**: [`GrantForwarder::from_env`] gives one when `D1_REWARDS=1`, `D1_STORE=1` and
//! `D1_STORE_CONTROL` are all set, the same knobs that make the responder apply a grant. Otherwise
//! the hosts keep their stubs (`NoRewards`, `LoggedRewards`) and nothing changes.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};

/// How the activity ended for the character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrantOutcome {
    Completed,
    Incomplete,
    Abandoned,
}

impl GrantOutcome {
    pub fn word(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Incomplete => "incomplete",
            Self::Abandoned => "abandoned",
        }
    }
}

impl From<crate::rewards::ActivityOutcome> for GrantOutcome {
    fn from(o: crate::rewards::ActivityOutcome) -> Self {
        match o {
            crate::rewards::ActivityOutcome::Completed => Self::Completed,
            crate::rewards::ActivityOutcome::EndedIncomplete => Self::Incomplete,
            crate::rewards::ActivityOutcome::Abandoned => Self::Abandoned,
        }
    }
}

/// Who the grant is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrantTarget {
    /// The client's 40-byte identity (preferred: it names the character SOID).
    Identity(Vec<u8>),
    /// An 8-byte key: a character SOID, or the open world's key (the identity's first 8 bytes).
    Key(u64),
}

/// The optional parts of a grant.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GrantDetails {
    /// The Cryptarch guide page (`rewards::ActivityOutcome::community_page` in the responder).
    pub page: Option<String>,
    /// Roll the placeholder end-of-activity engrams.
    pub engrams: bool,
    pub seed: Option<u64>,
    /// One word for the responder's log: `script`, `event:12:gold`, `chest`, ...
    pub source: Option<String>,
}

/// The control line for one grant.
pub fn grant_line(target: &GrantTarget, activity_hash: u32, outcome: GrantOutcome, d: &GrantDetails) -> Result<String, String> {
    let who = match target {
        GrantTarget::Identity(b) if b.len() == d1_mp::identity::MemberIdentity::SIZE => {
            format!("id:{}", b.iter().map(|x| format!("{x:02x}")).collect::<String>())
        }
        GrantTarget::Identity(b) => return Err(format!("an identity is 40 bytes, got {}", b.len())),
        GrantTarget::Key(k) => format!("{k:#018x}"),
    };
    let word = |w: &str, what: &str| {
        if w.is_empty() || w.chars().any(char::is_whitespace) {
            Err(format!("{what} {w:?} must be one word"))
        } else {
            Ok(())
        }
    };
    let mut line = format!("grant {who} {activity_hash:#010x} {}", outcome.word());
    if let Some(p) = &d.page {
        word(p, "page")?;
        line += &format!(" page={p}");
    }
    if d.engrams {
        line += " engrams=1";
    }
    if let Some(s) = d.seed {
        line += &format!(" seed={s:#x}");
    }
    if let Some(s) = &d.source {
        word(s, "source")?;
        line += &format!(" source={s}");
    }
    Ok(line)
}

/// Append `line` (plus a newline) to the control file in one write.
pub fn append_line(path: &Path, line: &str) -> std::io::Result<()> {
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    f.write_all(format!("{line}\n").as_bytes())
}

/// ⭐ Forwards grants to the responder's store, off the caller's thread.
#[derive(Clone)]
pub struct GrantForwarder {
    path: PathBuf,
    tx: Arc<Mutex<Sender<String>>>,
}

impl std::fmt::Debug for GrantForwarder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GrantForwarder").field("path", &self.path).finish()
    }
}

impl GrantForwarder {
    /// A forwarder writing to `path` (the responder's `D1_STORE_CONTROL`).
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let (tx, rx) = channel::<String>();
        let at = path.clone();
        std::thread::Builder::new()
            .name("grant-forward".into())
            .spawn(move || {
                for line in rx {
                    match append_line(&at, &line) {
                        Ok(()) => tracing::info!("⭐ rewards: forwarded to the store: {line}"),
                        Err(e) => tracing::warn!("⛔ rewards: {}: {e} - not forwarded: {line}", at.display()),
                    }
                }
            })
            .expect("spawn the grant writer");
        Self { path, tx: Arc::new(Mutex::new(tx)) }
    }

    /// `D1_REWARDS=1` with the store on (`D1_STORE=1`) and its command file (`D1_STORE_CONTROL`):
    /// the knobs under which the responder applies a grant. `None` otherwise.
    pub fn from_vars(var: impl Fn(&str) -> Option<String>) -> Option<Self> {
        let on = |k: &str| var(k).is_some_and(|v| v.trim() == "1");
        let control = var("D1_STORE_CONTROL").filter(|p| !p.trim().is_empty())?;
        (on("D1_REWARDS") && on("D1_STORE")).then(|| Self::new(control))
    }

    pub fn from_env() -> Option<Self> {
        Self::from_vars(|k| std::env::var(k).ok())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// ⭐ **The call.** Queue one character's grant for the responder and return at once.
    pub fn forward(&self, target: GrantTarget, activity_hash: u32, outcome: GrantOutcome, details: GrantDetails) -> Result<(), String> {
        let line = grant_line(&target, activity_hash, outcome, &details)?;
        self.tx.lock().map_err(|_| "the grant writer is poisoned".to_string())?.send(line).map_err(|_| "the grant writer stopped".to_string())
    }
}

/// The script bridge's trait (`CharacterRef` is the client's identity).
impl crate::rewards::ActivityRewards for GrantForwarder {
    fn grant_activity_rewards(
        &self,
        character: &crate::rewards::CharacterRef,
        activity_hash: u32,
        outcome: crate::rewards::ActivityOutcome,
    ) -> Result<(), String> {
        let details = GrantDetails { source: Some("script".into()), ..GrantDetails::default() };
        self.forward(GrantTarget::Identity(character.identity.clone()), activity_hash, outcome.into(), details)
    }
}

/// Stream G's open-world trait (the character is the registry's 8-byte key).
impl crate::open_world::ActivityRewards for GrantForwarder {
    fn grant_activity_rewards(
        &mut self,
        character: u64,
        activity_hash: u32,
        outcome: &crate::open_world::ActivityOutcome,
    ) -> Result<(), String> {
        use crate::open_world::ActivityOutcome as O;
        let source = match outcome {
            O::PublicEvent { event_id, tier, .. } => match tier {
                Some(t) => format!("event:{event_id}:{}", format!("{t:?}").to_lowercase()),
                None => format!("event:{event_id}"),
            },
            O::PatrolMission { beacon } => format!("patrol:{beacon:#x}"),
            O::Chest { bundle, index } => format!("chest:{bundle:#x}:{index}"),
            O::ResourceNode { bundle, index } => format!("node:{bundle:#x}:{index}"),
        };
        let o = if outcome.completed() { GrantOutcome::Completed } else { GrantOutcome::Incomplete };
        self.forward(GrantTarget::Key(character), activity_hash, o, GrantDetails { source: Some(source), ..GrantDetails::default() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::open_world::{ActivityOutcome as OpenOutcome, ActivityRewards as OpenRewards, EventTier};
    use crate::rewards::{ActivityOutcome, ActivityRewards, CharacterRef};

    fn identity() -> Vec<u8> {
        let mut b = vec![0u8; 40];
        b[..8].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
        b[0x10..0x18].copy_from_slice(&0xD100_0000_0000_0001u64.to_le_bytes());
        b[0x18..0x20].copy_from_slice(&0xD100_0000_0000_0101u64.to_le_bytes());
        b
    }

    /// The lines `d1-responder`'s `rewards::remote` tests read (keep the two in step).
    #[test]
    fn lines_follow_the_responders_grammar() {
        let d = GrantDetails { page: Some("siva-crisis-heroic-strike-playlist".into()), engrams: true, seed: Some(0xD1F0), source: Some("script".into()) };
        assert_eq!(
            grant_line(&GrantTarget::Key(0xD100_0000_0000_0101), 0x0BBD_0E26, GrantOutcome::Completed, &d).unwrap(),
            "grant 0xd100000000000101 0x0bbd0e26 completed page=siva-crisis-heroic-strike-playlist engrams=1 seed=0xd1f0 source=script"
        );
        let id = grant_line(&GrantTarget::Identity(identity()), 12345, GrantOutcome::Abandoned, &GrantDetails::default()).unwrap();
        assert!(id.starts_with("grant id:0102030405060708") && id.ends_with(" 0x00003039 abandoned"), "{id}");
        assert_eq!(d1_mp::identity::MemberIdentity::parse(&identity()).unwrap().character.0, 0xD100_0000_0000_0101);
        assert!(grant_line(&GrantTarget::Identity(vec![0; 8]), 1, GrantOutcome::Completed, &GrantDetails::default()).is_err());
        let spaced = GrantDetails { source: Some("two words".into()), ..GrantDetails::default() };
        assert!(grant_line(&GrantTarget::Key(1), 1, GrantOutcome::Completed, &spaced).is_err());
    }

    fn wait_for(path: &Path, lines: usize) -> String {
        for _ in 0..200 {
            if let Ok(t) = std::fs::read_to_string(path) {
                if t.lines().count() >= lines {
                    return t;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        std::fs::read_to_string(path).unwrap_or_default()
    }

    #[test]
    fn both_reward_traits_write_the_control_file() {
        let dir = std::env::temp_dir().join(format!("d1-grant-fwd-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("store-control.txt");
        let _ = std::fs::remove_file(&path);
        let mut f = GrantForwarder::new(&path);
        let c = CharacterRef { identity: identity() };
        ActivityRewards::grant_activity_rewards(&f, &c, 0x0BBD_0E26, ActivityOutcome::Completed).unwrap();
        let event = OpenOutcome::PublicEvent { event_id: 12, completed: true, tier: Some(EventTier::Gold) };
        OpenRewards::grant_activity_rewards(&mut f, 0x0102_0304_0506_0708, 0x1111_2222, &event).unwrap();
        let failed = OpenOutcome::PublicEvent { event_id: 13, completed: false, tier: None };
        OpenRewards::grant_activity_rewards(&mut f, 7, 0x1111_2222, &failed).unwrap();
        let text = wait_for(&path, 3);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 3, "{text}");
        assert!(lines[0].starts_with("grant id:") && lines[0].ends_with(" 0x0bbd0e26 completed source=script"));
        assert_eq!(lines[1], "grant 0x0102030405060708 0x11112222 completed source=event:12:gold");
        assert_eq!(lines[2], "grant 0x0000000000000007 0x11112222 incomplete source=event:13");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn it_is_on_only_with_the_responders_knobs() {
        let vars = |set: &'static [(&'static str, &'static str)]| move |k: &str| set.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string());
        assert!(GrantForwarder::from_vars(vars(&[])).is_none());
        assert!(GrantForwarder::from_vars(vars(&[("D1_REWARDS", "1"), ("D1_STORE", "1")])).is_none(), "no control file");
        assert!(GrantForwarder::from_vars(vars(&[("D1_REWARDS", "1"), ("D1_STORE_CONTROL", "c.txt")])).is_none(), "store off");
        let f = GrantForwarder::from_vars(vars(&[("D1_REWARDS", "1"), ("D1_STORE", "1"), ("D1_STORE_CONTROL", "c.txt")])).unwrap();
        assert_eq!(f.path(), Path::new("c.txt"));
    }
}
