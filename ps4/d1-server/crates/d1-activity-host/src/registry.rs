// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later

//! AH id allocation and the endpoint registry - the two levers the server actually has.
//!
//! ```text
//! [server]  BAP 6  -> BAP 7   { u8 2, u64 host_token, char[128] host_text }   ← lever 1
//!    ...    the client talks to itself for four steps (see the retraction below)
//! [server]  BAP 16 -> BAP 17  { u64 echo, u32be ipv4, u32be port }            ← lever 2
//! ```
//!
//! ## ⛔ The server cannot set a session parameter, so these two are the whole surface
//!
//! `s_activity_host_connection_information` replicates **peer-to-peer** over the game's own session
//! channel (`0x25` host→peer, `0x26` peer→host) and the write predicate `0xB8D630` is *"is this
//! session's state one of the four `host-*` states"* - i.e. **"am I the host peer."** The server is
//! not a peer: no `c_network_session`, no peer index, no bit in any peer mask. Any design that
//! reads *"the server delivers the activity-host connection information"* is impossible.
//!
//! ## ⭐ What `host_text` should contain - the plan's `[U]`, closed
//!
//! `historicaldocs/design/SERVER-ROLES-PLAN.md` §3.3 leaves it open, guessing `"host:port"` or an AH name. It is
//! a **session id**, and it is not an address:
//!
//! * `0x594C60` copies `{u64 at +1, 128 bytes at +9}` into one contiguous `0x88` blob and delivers
//!   it as `user_defined_type 10`.
//! * `user_defined_type 10` is AH message **10 `start_activity_host_response`**, whose layout is
//!   `{u64 ah_id, char session_id[]}` and whose size is **`0x88`** - the same `0x88`.
//! * The address arrives separately, in message 17. A second copy here would be redundant, and the
//!   field is copied as a *string* by the fixup at `0x76C1D0`.
//!
//! ⇒ `host_token` **is** `ah_id`; `host_text` **is** `session_id`. `[I]`, but with two independent
//! size agreements and no competing reading. ⚠ **`d1-responder`'s `D1_AH_HOST_TEXT` currently
//! defaults to `<ip>:<port>`** - see [`HostEntry::session_id`]; that default is probably wrong, and
//! it is free to change because the falsifier (does message 16 appear?) is driven by the *id*, not
//! by this string.
//!
//! ## The falsifier this module exists to arm
//!
//! The `[I]` step between the two levers - who writes the AH id into the session parameter - was
//! searched for by two agents and not found. It has a free falsifier: **after message 7 is
//! answered, if message 16 never appears on the wire, the inference is wrong.** [`HostRegistry`]
//! records `manager_responses` and `proxy_requests` separately so the run log can say which of the
//! two happened.

use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddrV4};

use d1_bap::activity_host::{
    ActivityHostManagerRequest, ActivityHostManagerResponse, GetActivityHostProxyRequest,
    GetActivityHostProxyResponse,
};

use crate::messages::StartActivityHostResponse;
use crate::AhError;

/// One allocated Activity Host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostEntry {
    /// ⭐ Non-zero, always. `0` is the "no activity host" sentinel in the session parameter, and
    /// `FUN_00594320` returns false and issues nothing while it reads zero.
    pub ah_id: u64,
    /// What goes in BAP message 7's `host_text` and in `join_result.session_id`. See the module
    /// doc: this is an identifier, **not** an address.
    pub session_id: String,
    /// Where our AH listener is reachable - what BAP message 17 hands out.
    pub endpoint: SocketAddrV4,
    /// The launch command line the client sent us, verbatim.
    /// `/ah_type:group|fireteam [/ah_package:X] [/ah_slice_set_names:Y]`.
    pub launch_args: String,
}

impl HostEntry {
    /// The `0x88` struct the client will see as `user_defined_type 10`.
    pub fn start_activity_host_response(&self) -> StartActivityHostResponse {
        StartActivityHostResponse {
            ah_id: self.ah_id,
            session_id: self.session_id.as_bytes().to_vec(),
        }
    }
}

/// The launch command line, split into the three keys the client's builder writes (`0x5949B0`).
///
/// Parsing it is not required to answer message 7 - but it is the only place the *server* learns
/// what activity the client thinks it is starting, so it belongs in the run log.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LaunchArgs {
    /// `group` or `fireteam`, per the client's own alternation.
    pub ah_type: Option<String>,
    pub ah_package: Option<String>,
    pub ah_slice_set_names: Option<String>,
    /// Anything we did not recognise - kept rather than dropped.
    pub unrecognised: Vec<String>,
}

impl LaunchArgs {
    /// Split on whitespace and pick apart `/key:value`. Deliberately forgiving: this is a log aid,
    /// and a parse failure must never be able to fail a launch.
    pub fn parse(args: &str) -> Self {
        let mut out = Self::default();
        for tok in args.split_whitespace() {
            let Some(rest) = tok.strip_prefix('/') else {
                out.unrecognised.push(tok.to_string());
                continue;
            };
            match rest.split_once(':') {
                Some(("ah_type", v)) => out.ah_type = Some(v.to_string()),
                Some(("ah_package", v)) => out.ah_package = Some(v.to_string()),
                Some(("ah_slice_set_names", v)) => out.ah_slice_set_names = Some(v.to_string()),
                _ => out.unrecognised.push(tok.to_string()),
            }
        }
        out
    }
}

/// Allocates activity-host ids and remembers where each one lives.
///
/// One per server process. It is deliberately not `async` and holds no sockets - the listener owns
/// those, and keeping allocation synchronous makes the id sequence in a run log reproducible.
#[derive(Debug)]
pub struct HostRegistry {
    endpoint: SocketAddrV4,
    next: u64,
    entries: HashMap<u64, HostEntry>,
    /// How many message-6 requests we have answered.
    pub manager_responses: u64,
    /// ⭐ How many message-16 requests have arrived. **The falsifier**: if this stays 0 while
    /// `manager_responses` climbs, the `[I]` link in the plan's §4.2 is wrong.
    pub proxy_requests: u64,
}

/// The first id handed out. Recognisable on sight in a hex dump, and non-zero.
///
/// The high nibble mirrors the `0xD1…` convention the rest of this project uses for
/// server-allocated SOIDs, so an id that turns up somewhere unexpected is obviously ours.
pub const FIRST_ACTIVITY_HOST_ID: u64 = 0xD1A0_0000_0000_0001;

impl HostRegistry {
    /// `endpoint` is where our AH listener is reachable - the address message 17 will hand out.
    pub fn new(endpoint: SocketAddrV4) -> Self {
        Self {
            endpoint,
            next: FIRST_ACTIVITY_HOST_ID,
            entries: HashMap::new(),
            manager_responses: 0,
            proxy_requests: 0,
        }
    }

    /// A registry whose first allocation is `first` (non-zero; 0 falls back to
    /// [`FIRST_ACTIVITY_HOST_ID`]). An activity-host connection answers message 6 with the id its
    /// instance presents, so that connection's message 7 and its pushes name the same host.
    pub fn with_first_id(endpoint: SocketAddrV4, first: u64) -> Self {
        Self { next: if first == 0 { FIRST_ACTIVITY_HOST_ID } else { first }, ..Self::new(endpoint) }
    }

    /// Allocate an Activity Host for a launch command line.
    pub fn allocate(&mut self, launch_args: &str) -> &HostEntry {
        let ah_id = self.next;
        self.next = self.next.wrapping_add(1);
        // Derived from the id so a run log line and a packet can be matched by eye.
        let session_id = format!("d1ah-{ah_id:016x}");
        let entry = HostEntry {
            ah_id,
            session_id,
            endpoint: self.endpoint,
            launch_args: launch_args.to_string(),
        };
        self.entries.entry(ah_id).or_insert(entry)
    }

    /// Look one up.
    pub fn get(&self, ah_id: u64) -> Option<&HostEntry> {
        self.entries.get(&ah_id)
    }

    /// Everything allocated, for the run log.
    pub fn entries(&self) -> impl Iterator<Item = &HostEntry> {
        self.entries.values()
    }

    // ── lever 1: BAP message 6 -> 7 ───────────────────────────────────────────────────────────

    /// Answer an activity-host-manager request.
    ///
    /// The request body is `d1-bap`'s to decode; this takes the decoded form and returns the
    /// decoded response, so no byte of framing crosses this boundary in either direction.
    pub fn answer_manager_request(
        &mut self,
        request: &ActivityHostManagerRequest,
    ) -> Result<(ActivityHostManagerResponse, HostEntry), AhError> {
        let args = String::from_utf8_lossy(&request.args).trim_end_matches('\0').to_string();
        let entry = self.allocate(&args).clone();
        self.manager_responses += 1;
        let response = ActivityHostManagerResponse::new(entry.ah_id, &entry.session_id).ok_or(
            AhError::FieldOverflow {
                field: "activity_host_manager_response.host_text",
                len: entry.session_id.len(),
                max: d1_bap::activity_host::AH_HOST_TEXT_LEN - 1,
            },
        )?;
        Ok((response, entry))
    }

    // ── lever 2: BAP message 16 -> 17 ─────────────────────────────────────────────────────────

    /// Resolve an AH id to an address.
    ///
    /// ⚠ **The echo is mandatory and getting it wrong produces no error at all.** `0x594E70` drops
    /// the response silently unless `activity_host_id` equals the id it asked for - which looks
    /// exactly like "the server never answered". `d1-bap`'s constructor takes the request so the
    /// echo cannot be forgotten; this function keeps that discipline.
    ///
    /// `0` is legal and means "the proxy itself" (`0x594E70` prints the literal `"proxy"`). We
    /// answer it with our own listener too - being our own proxy is the whole topology here.
    pub fn answer_proxy_request(
        &mut self,
        request: GetActivityHostProxyRequest,
    ) -> GetActivityHostProxyResponse {
        self.proxy_requests += 1;
        let endpoint = self
            .get(request.activity_host_id)
            .map(|e| e.endpoint)
            .unwrap_or(self.endpoint);
        GetActivityHostProxyResponse::new(request, *endpoint.ip(), endpoint.port())
    }

    /// Where the listener is.
    pub fn endpoint(&self) -> SocketAddrV4 {
        self.endpoint
    }

    /// Has the falsifier fired? `true` means the client asked us to resolve an AH id, which it
    /// cannot do until something wrote that id into its session parameter.
    pub fn falsifier_passed(&self) -> bool {
        self.proxy_requests > 0
    }
}

impl Default for HostRegistry {
    fn default() -> Self {
        Self::new(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> HostRegistry {
        HostRegistry::new(SocketAddrV4::new(Ipv4Addr::new(192, 168, 1, 84), 4110))
    }

    /// ⭐ The invariant a zero would silently break: `FUN_00594320` issues nothing while the AH id
    /// reads zero, so an allocator that could return 0 would produce a run that looks exactly like
    /// "the client never asked".
    #[test]
    fn an_allocated_activity_host_id_is_never_zero() {
        let mut r = registry();
        for _ in 0..64 {
            assert_ne!(r.allocate("/ah_type:fireteam").ah_id, 0);
        }
        assert_eq!(FIRST_ACTIVITY_HOST_ID & 0xFFFF_FFFF_FFFF_FFFF, FIRST_ACTIVITY_HOST_ID);
        assert_ne!(FIRST_ACTIVITY_HOST_ID, 0);
    }

    #[test]
    fn ids_are_unique_and_each_gets_its_own_session_id() {
        let mut r = registry();
        let a = r.allocate("x").clone();
        let b = r.allocate("y").clone();
        assert_ne!(a.ah_id, b.ah_id);
        assert_ne!(a.session_id, b.session_id);
        assert_eq!(r.get(a.ah_id).unwrap().launch_args, "x");
    }

    /// ⭐ The named regression for the silent drop at `0x594E70`.
    #[test]
    fn the_proxy_response_echoes_the_requested_id_verbatim() {
        let mut r = registry();
        let req = GetActivityHostProxyRequest { activity_host_id: 0xDEAD_BEEF_0000_0001 };
        let rsp = r.answer_proxy_request(req);
        assert_eq!(rsp.activity_host_id, 0xDEAD_BEEF_0000_0001);
        assert_eq!(rsp.ipv4(), Ipv4Addr::new(192, 168, 1, 84));
        assert_eq!(rsp.port(), 4110);
        // Even an id we never allocated is echoed - the echo is about routing, not about
        // authorisation, and a mismatch is invisible rather than an error.
        assert!(r.get(0xDEAD_BEEF_0000_0001).is_none());
    }

    #[test]
    fn proxy_id_zero_means_the_proxy_itself_and_is_answered() {
        let mut r = registry();
        let rsp = r.answer_proxy_request(GetActivityHostProxyRequest { activity_host_id: 0 });
        assert_eq!(rsp.activity_host_id, 0);
        assert_eq!(rsp.port(), 4110);
    }

    /// A connection's registry answers message 6 with the id its instance presents (a second
    /// connection under D1_AH_DISTINCT_HOST_IDS presents the next id), and 0 is never handed out.
    #[test]
    fn a_registry_can_start_at_the_instances_id() {
        let req = ActivityHostManagerRequest {
            inner_type: 3,
            args: Vec::new(),
            nested_msg: Vec::new(),
            bitstream: Vec::new(),
            trailing_id: 0,
        };
        let endpoint = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0);
        let mut r = HostRegistry::with_first_id(endpoint, FIRST_ACTIVITY_HOST_ID + 1);
        let (rsp, entry) = r.answer_manager_request(&req).unwrap();
        assert_eq!(rsp.host_token, FIRST_ACTIVITY_HOST_ID + 1);
        assert_eq!(entry.session_id, format!("d1ah-{:016x}", FIRST_ACTIVITY_HOST_ID + 1));
        let mut zero = HostRegistry::with_first_id(endpoint, 0);
        assert_eq!(zero.answer_manager_request(&req).unwrap().0.host_token, FIRST_ACTIVITY_HOST_ID);
    }

    /// The `[U]` this module closes: message 7's `host_text` is the session id, not an address.
    #[test]
    fn the_manager_response_carries_the_session_id_not_an_endpoint() {
        let mut r = registry();
        let req = ActivityHostManagerRequest {
            inner_type: d1_bap::activity_host::AH_MANAGER_INNER_TYPE_WITH_ARGS,
            args: b"/ah_type:fireteam /ah_package:pkg\0".to_vec(),
            nested_msg: Vec::new(),
            bitstream: Vec::new(),
            trailing_id: 0,
        };
        let (rsp, entry) = r.answer_manager_request(&req).unwrap();
        assert_eq!(rsp.host_token, entry.ah_id);
        assert_ne!(rsp.host_token, 0);
        let text = rsp.host_text_str().to_string();
        assert_eq!(text, entry.session_id);
        assert!(!text.contains(':'), "a session id, not host:port - see the module doc");
        // And it is exactly what AH udt 10 would carry.
        let udt10 = entry.start_activity_host_response().struct_bytes().unwrap();
        assert_eq!(&udt10[0..8], &entry.ah_id.to_le_bytes());
    }

    #[test]
    fn the_launch_command_line_is_parsed_for_the_run_log() {
        let a = LaunchArgs::parse("/ah_type:fireteam /ah_package:pvp /ah_slice_set_names:a,b /wat");
        assert_eq!(a.ah_type.as_deref(), Some("fireteam"));
        assert_eq!(a.ah_package.as_deref(), Some("pvp"));
        assert_eq!(a.ah_slice_set_names.as_deref(), Some("a,b"));
        assert_eq!(a.unrecognised, vec!["/wat"]);
        // A launch must never fail on a parse: garbage in, empty out.
        assert_eq!(LaunchArgs::parse("").ah_type, None);
    }

    /// ⭐ The falsifier, as a value rather than a note in a document.
    #[test]
    fn the_falsifier_starts_unfired_and_fires_only_on_a_proxy_request() {
        let mut r = registry();
        let req = ActivityHostManagerRequest {
            inner_type: 3,
            args: Vec::new(),
            nested_msg: Vec::new(),
            bitstream: Vec::new(),
            trailing_id: 0,
        };
        r.answer_manager_request(&req).unwrap();
        assert_eq!(r.manager_responses, 1);
        assert!(!r.falsifier_passed(), "answering 7 proves nothing on its own");
        r.answer_proxy_request(GetActivityHostProxyRequest { activity_host_id: 1 });
        assert!(r.falsifier_passed());
    }
}
