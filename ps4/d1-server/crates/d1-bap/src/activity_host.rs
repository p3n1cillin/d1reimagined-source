//! The activity-host message family: ids **6/7**, **8**, **16/17**, **100** and **102**.
//!
//! These are **hand-rolled fixed-size codecs**, not descriptor-table-driven - the `/SignOn`
//! *aux-pointers-are-relocations* trap does not apply and there is no descriptor table to go
//! looking for. Every layout below was read out of the client's own codec, and the decompiled
//! source of each codec is named in its doc comment so it can be re-read rather than re-derived:
//!
//! | id | codec | what settles the layout |
//! |---:|---|---|
//! | 6 | `0x55AC00` `…manager_request::vfunc[3]` | declared size `0x100F`; `memcpy` of `0x100E` (type 1) or `0xE0C` (type 3); byteswaps at `+1`, `+0x203`, `+0x405` |
//! | 7 | `0x55AF10` `…manager_response::vfunc[3]` | declared size `0x89`; inner type must be `2`; `memcpy` `0x88`; one `bswap64` at `+1` |
//! | 8 | `0x55EC70` (shared with id 160) | declared size `< 0x1DF0E`; type 2 swaps `u32` at `+1` and `+5` |
//! | 16 | `0x562B90` | declared size exactly `8`; one `bswap64` |
//! | 17 | `0x562D60` | declared size exactly `0x10`; `bswap64` at `+0`, `bswap32` at `+8` and `+0xC` |
//! | 100 | `0x55E690` (encode) / `0x55E130` (decode) | wire `= payload + 0x11`; `bswap64` at `+1`, `bswap32` at `+9` and `+0xD` |
//!
//! Reach: **`static-client-inference`**. None of these has been seen on a wire, ours or Bungie's.
//! The moment our server answers message 7 or 17, everything downstream is `synthetic_ancestor`.
//!
//! ⚠ **Header classes.** Ids 6 and 16 are class 1 (6-byte header) and 7 and 17 are class 2 (8-byte
//! header with status) - the two shapes this crate already emits. Ids 8, 100 and 102 are classes 3,
//! 6 and 5, whose layouts are `[U]`; the codecs here produce **bodies only**, and
//! [`crate::conversation::BapConversation`] refuses to frame them.

use std::borrow::Cow;

// ─── sizes ────────────────────────────────────────────────────────────────────────────────────

/// `[C]` Message 6's body is **always** this long, for every inner type. `0x55AC00` compares the
/// declared size to `0x100F` and the buffer size to `0x100E` before looking at anything else.
pub const AH_MANAGER_REQUEST_BODY_LEN: usize = 0x100F;
/// `[C]` Message 7's body: `0x55AF10` compares the declared size to `0x89`.
pub const AH_MANAGER_RESPONSE_BODY_LEN: usize = 0x89;
/// `[C]` The `args` field of message 6, type 1 - the activity-host launch command line.
pub const AH_ARGS_FIELD_LEN: usize = 0x200;
/// `[C]` The nested-message field of message 6.
pub const AH_NESTED_FIELD_LEN: usize = 0x200;
/// `[C]` The bit-packed blob field of message 6.
pub const AH_BITSTREAM_FIELD_LEN: usize = 0xC00;
/// `[C]` Message 16's body: `0x562B90` compares the declared size to `8`.
pub const AH_PROXY_REQUEST_BODY_LEN: usize = 8;
/// `[C]` Message 17's body: `0x562D60` compares the declared size to `0x10`.
pub const AH_PROXY_RESPONSE_BODY_LEN: usize = 0x10;
/// `[C]` The payload ceiling both notification codecs enforce (`payload < 0` or `> 0x1DF00` is a
/// decode failure naming `received bad payload size %d for type %s`).
pub const AH_NOTIFICATION_MAX_PAYLOAD: usize = 0x1DF00;
/// `[C]` Message 7's ASCII field, copied with a bounded strlcpy-style copy.
pub const AH_HOST_TEXT_LEN: usize = 0x80;
/// `[R]` Message 102's body - a 256-byte Pascal string. **Not re-derived here**; see
/// [`ActivityHostToClientLogNotification`].
pub const AH_LOG_NOTIFICATION_BODY_LEN: usize = 0x100;

/// Why an activity-host body would not decode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AhDecodeError {
    /// The client's own first test: the declared body length is not what the codec accepts. Its
    /// diagnostic is `failed to decode [bad size %d/%d]`.
    BadSize { got: usize, want: usize },
    /// Byte 0 is not an inner type this codec handles. The client logs
    /// `received unknown type %s`, where `%s` is the **decimal** of the byte (`0x16008CC` is the
    /// literal `"%d"`) or the literal `enum_out_of_range` - there is no enum-to-string table.
    BadInnerType(u8),
    /// A `u16be` length field points past the fixed field that contains it.
    LengthOverflow { field: &'static str, len: usize, max: usize },
    /// A notification's payload size is outside `0..=0x1DF00`.
    PayloadOutOfRange(i32),
    /// The buffer ended before a fixed field did.
    Truncated { need: usize, got: usize },
}

impl std::fmt::Display for AhDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadSize { got, want } => write!(f, "bad size {got}/{want}"),
            Self::BadInnerType(t) => write!(f, "received unknown type {t}"),
            Self::LengthOverflow { field, len, max } => {
                write!(f, "{field} length {len} exceeds its {max}-byte field")
            }
            Self::PayloadOutOfRange(n) => write!(f, "bad payload size {n}"),
            Self::Truncated { need, got } => write!(f, "truncated: need {need}, got {got}"),
        }
    }
}

impl std::error::Error for AhDecodeError {}

fn be_u16(b: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([b[at], b[at + 1]])
}

fn be_u32(b: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn be_u64(b: &[u8], at: usize) -> u64 {
    let mut v = [0u8; 8];
    v.copy_from_slice(&b[at..at + 8]);
    u64::from_be_bytes(v)
}

// ─── messages 16 / 17 - get_activity_host_proxy - Phase 3 ─────────────────────────────────────

/// `[C]` BAP id **16** (`0x0010`), header class 1. Body **exactly 8 bytes**.
///
/// Decoder `0x562B90`: `if (declared == 8 && buffer >= 8) { *out = *in; bswap64(out); }`. Anything
/// else logs `get_activity_host_proxy_request: failed to decode [bad size %d/%d]`.
///
/// Sent by the client on **channel 0** (the control channel) under **policy index 22**.
/// ⚠ The send site at `0x594D40` passes the literal `22`, which is a **policy index, not a message
/// id** - reading it as an id gives `client_to_bap_update_purchased_offers_response`, a flat
/// contradiction. Resolve it as `0x1923CB0[22] → vtable 0x19243A0 → vfunc[2] → 16`; that vtable is
/// `c_investment_bap_message_policy_client_to_bap_get_activity_host_proxy_request` and its slot 2
/// thunk is `mov eax, 0x10 ; ret`, which is exactly the row
/// [`crate::tables::POLICIES`] carries for request 16.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GetActivityHostProxyRequest {
    /// `[C]` The activity-host id to resolve.
    ///
    /// **`0` is legal** and means "the proxy itself" - `0x594E70` prints the literal `"proxy"`
    /// rather than an id in that case.
    pub activity_host_id: u64,
}

impl GetActivityHostProxyRequest {
    pub fn decode(body: &[u8]) -> Result<Self, AhDecodeError> {
        if body.len() != AH_PROXY_REQUEST_BODY_LEN {
            return Err(AhDecodeError::BadSize {
                got: body.len(),
                want: AH_PROXY_REQUEST_BODY_LEN,
            });
        }
        Ok(Self { activity_host_id: be_u64(body, 0) })
    }

    /// The client builds these; we encode only to test the decoder against itself.
    pub fn encode(&self) -> Vec<u8> {
        self.activity_host_id.to_be_bytes().to_vec()
    }
}

/// `[C]` BAP id **17** (`0x0011`), header class 2 (8-byte header, status 200). Body **exactly 16
/// bytes**. ⭐ This is the reply that tells the client *where* its activity host is.
///
/// Decoder `0x562D60`, read from the decompile: two `u64` slots are copied, then
/// `bswap64` at `+0`, `bswap32` at `+8`, `bswap32` at `+0xC`. So the field boundaries are not a
/// guess - they are where the byteswaps land.
///
/// ⚠ **The recipient (`0x594E70`) DROPS this SILENTLY unless `activity_host_id` equals the id it
/// asked for.** The echo is mandatory, not cosmetic, and getting it wrong produces no error at all
/// - it looks exactly like "the server never answered".
///
/// **The observable, and the cheapest success signal in the whole activity plan:**
/// `networking:activity_client:messaging_interface:bap: AH IP resolution successful: '%s -> %s'.`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GetActivityHostProxyResponse {
    /// `[C]` Must echo the request's id verbatim.
    pub activity_host_id: u64,
    /// `[C]` IPv4, network order: wire bytes `0x08..0x0C` are `a.b.c.d` in that order.
    ///
    /// Byte order derived three ways - the decoder's `bswap32`, the resolver's `bswap32`, and the
    /// connect block at `0x594320` swapping it back out. Three sites, one consistent reading.
    pub proxy_ipv4: u32,
    /// `[I]` **on the width**, `[C]` on the low half. The decoder swaps 32 bits here; `0x594E70`
    /// consumes only the low 16 (`port_hi = wire[0x0E]`, `port_lo = wire[0x0F]`). A `u32be` port
    /// with `wire[0x0C..0x0E] == 0` satisfies every observed instruction, and so does
    /// `{u16 ignored, u16be port}`.
    ///
    /// ⭐ **This `[U]` is closed by construction, not by evidence:** the client cannot distinguish
    /// the two readings, and neither can a capture (BAP is AES-128-GCM). Emitting a big-endian
    /// `u32` satisfies both. [`Self::new`] takes a `u16` and widens it, so a caller cannot
    /// accidentally put something in the high half.
    pub proxy_port: u32,
}

impl GetActivityHostProxyResponse {
    /// Build the reply for a request. The id is taken **from the request** so it cannot drift -
    /// the one mistake here has no error path at all.
    pub fn new(request: GetActivityHostProxyRequest, ipv4: std::net::Ipv4Addr, port: u16) -> Self {
        Self {
            activity_host_id: request.activity_host_id,
            proxy_ipv4: u32::from_be_bytes(ipv4.octets()),
            proxy_port: u32::from(port),
        }
    }

    /// The historical big-endian encoding. **Measured wrong on the wire (2026-08-08)** - kept
    /// because [`Self::decode`] is its symmetric inverse and the round-trip is a unit test.
    /// ⛔ Do not send this to a client; use [`Self::encode_client_order`].
    pub fn encode(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(AH_PROXY_RESPONSE_BODY_LEN);
        b.extend_from_slice(&self.activity_host_id.to_be_bytes());
        b.extend_from_slice(&self.proxy_ipv4.to_be_bytes());
        b.extend_from_slice(&self.proxy_port.to_be_bytes());
        b
    }

    /// ⛔⛔ **RETRACTED THE SAME EVENING - THIS ORDER IS WRONG. DO NOT SEND IT.**
    ///
    /// Kept only as the reproducible negative control (`D1_AH_ENDIAN=le`). The line
    /// `AH IP resolution successful: 'proxy -> 2.0.77.10:780'` is a **byte-reversed PRINT of a
    /// correct value**, not evidence of a byte-reversed value. Proof, two runs:
    ///
    /// | run | encoder | resolver printed | `transport_endpoint_async_connect()` got |
    /// |---|---|---|---|
    /// | `20260808-195418` | [`Self::encode`] (BE) | `2.0.77.10:780` | ✅ reached `_connected[10.77.0.2:3075]` **x11** |
    /// | `20260808-202647` | this one (LE) | `10.77.0.2:3075` | ⛔ **`[ip address 2.0.77.10:780]`**, call failed |
    ///
    /// ⇒ There are **two consumers and exactly one swap between them**. Making the *printer* look
    /// right makes the *connect* wrong. [`Self::proxy_ipv4`]'s original note - "three sites, one
    /// consistent reading", including `0x594320` swapping it back out - **was correct**, and the
    /// retraction of it is itself retracted.
    ///
    /// ⚠ The lesson is not about endianness. **A log line is a rendering, not a value.** The
    /// disassembly said there were three swaps; the print showed two of them. Confirming a
    /// disassembly-derived layout requires watching the *consumer that matters* - here,
    /// `transport_endpoint_async_connect()`, which prints its argument too and was in the same log.
    ///
    /// ---
    /// Original (wrong) rationale follows.
    ///
    /// **The order the CLIENT actually reads - measured, not inferred.**
    ///
    /// Run `20260808-195418-F2-STATE3`: we sent `D1_AH_IP=10.77.0.2`, `D1_AH_PORT=3075` with
    /// [`Self::encode`], and the guest printed its own verdict:
    ///
    /// ```text
    /// networking:activity_client:messaging_interface:bap:
    ///     AH IP resolution successful: 'proxy -> 2.0.77.10:780'.
    /// ```
    ///
    /// **Both fields came back byte-reversed**, and the client then dialled that dead address and
    /// died with `_connection_failure_suicide`, timing out rung 30 into
    /// `error.starting_private_activity_instance_timed_out_waiting_for_join_response`.
    ///
    /// | field | wire we sent | client rendered | ⇒ client reads |
    /// |---|---|---|---|
    /// | ipv4 `0x08..0x0C` | `0A 4D 00 02` | `2.0.77.10` | **LE u32** (full reversal) |
    /// | port `0x0C..0x10` | `00 00 0C 03` | `780` = `0x030C` | **LE u16 at `0x0E`** |
    ///
    /// ⛔ **This RETRACTS the byte-order note on [`Self::proxy_ipv4`]** ("derived three ways - the
    /// decoder's `bswap32`, the resolver's `bswap32`, and the connect block at `0x594320` swapping
    /// it back out. Three sites, one consistent reading"). Three agreeing static readings lost to
    /// one live log line, which is the correct outcome: **an odd/even `bswap` count is exactly the
    /// thing static reading gets wrong, and the client prints the answer for free.**
    ///
    /// ⛔ It also retracts the `[U]`-closed-by-construction claim on [`Self::proxy_port`]. The two
    /// candidate readings were `port_hi = wire[0x0E]; port_lo = wire[0x0F]`. It is the **other way
    /// round**, and the client could distinguish them all along - by failing.
    pub fn encode_client_order(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(AH_PROXY_RESPONSE_BODY_LEN);
        // The id echo is a u64 and is NOT known to be reversed - it round-tripped as big-endian
        // through message 16/17 already (`0x594E70` drops a mismatch silently, and it did not).
        b.extend_from_slice(&self.activity_host_id.to_be_bytes());
        b.extend_from_slice(&self.proxy_ipv4.to_le_bytes());
        b.extend_from_slice(&[0, 0]);
        b.extend_from_slice(&(self.proxy_port as u16).to_le_bytes());
        debug_assert_eq!(b.len(), AH_PROXY_RESPONSE_BODY_LEN);
        b
    }

    pub fn decode(body: &[u8]) -> Result<Self, AhDecodeError> {
        if body.len() != AH_PROXY_RESPONSE_BODY_LEN {
            return Err(AhDecodeError::BadSize {
                got: body.len(),
                want: AH_PROXY_RESPONSE_BODY_LEN,
            });
        }
        Ok(Self {
            activity_host_id: be_u64(body, 0),
            proxy_ipv4: be_u32(body, 8),
            proxy_port: be_u32(body, 12),
        })
    }

    /// The address as the client will read it.
    pub fn ipv4(&self) -> std::net::Ipv4Addr {
        std::net::Ipv4Addr::from(self.proxy_ipv4.to_be_bytes())
    }

    /// The 16 bits `0x594E70` actually consumes.
    pub fn port(&self) -> u16 {
        self.proxy_port as u16
    }
}

// ─── messages 6 / 7 - activity_host_manager - Phase 2 ─────────────────────────────────────────

/// `[C]` BAP id **6** (`0x0006`), header class 1. **We only ever decode this.**
///
/// ⭐ Cross-validated: the decoder byteswaps `u16` length fields at exactly `+0x001`/`+0x203`/
/// `+0x405` (type 1) and `+0x001`/`+0x203` (type 3), and the *independent* builder at `0x5946B0`
/// writes `u16` length fields at exactly those offsets. Two functions that do not call each other,
/// one answer.
///
/// Byte 0 is an inner type from a 6-value enum shared across the activity-host manager family
/// (1 and 3 here; 2 is the response; 4 is `bap_to_activity_host_manager_request`; **0 and 5 are
/// `[U]`** and no codec in the binary handles them).
///
/// ## Type 1 - the field table is normative, not the Rust below
///
/// ```text
/// +0x000  u8            inner_type = 1
/// +0x001  u16be         args_len          = strlen(args) + 1
/// +0x003  char[0x200]   args              ⭐ the activity-host LAUNCH COMMAND LINE (0x5949B0):
///                                            "/ah_type:group|fireteam[ /ah_package:X]
///                                             [ /ah_slice_set_names:Y]"
/// +0x203  u16be         nested_len        (<= 0x200)
/// +0x205  u8[0x200]     nested_msg        ⭐ an ENCODED BAP MESSAGE ID 10
///                                            (client_to_world_server_request)
/// +0x405  u16be         bitstream_len     in BYTES, = (bits + 7) / 8
/// +0x407  u8[0xC00]     bitstream         `[U]`. Strong `[I]`: the network-session parameter set
/// +0x1007 u64be         trailing_id       pre-swapped by the BUILDER. DO NOT SWAP TWICE.
/// ```
///
/// ## Type 3 - the same, minus `args`, every later field shifted back by `0x202`
///
/// `[C]` and it is not an assumption. `0x55AC00`'s type-3 branch `memcpy`s **`0xE0C`** bytes (vs
/// `0x100E` for type 1) and byteswaps at `+0x001` and `+0x203` only. `0xE0C = 0x100E - 0x202`, and
/// `1 + 0xE0C = 0xE0D = 0xE05 + 8` places `trailing_id` exactly at `+0xE05`. Every number agrees.
///
/// ⚠ **The wire frame is still `0x100F` bytes** - the size check is on the declared length and does
/// not vary by type - so a type-3 body carries `0x202` **trailing bytes the client never reads**.
/// Decoding drops them; [`Self::encode`] writes zeros there. A type-3 frame with non-zero padding
/// therefore does not survive `decode → encode` byte-identically, and that is deliberate: inventing
/// a field to store bytes the client discards would be inventing a field.
///
/// ⛔ **Retracted premise:** "`received unknown type %s` implies a name table". `0x16008CC` is the
/// literal string `"%d"` and `0x10495C0` is a bounded snprintf - the `%s` prints the **decimal** of
/// the type byte, or the literal `enum_out_of_range`. There is no enum-to-string table to find.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityHostManagerRequest {
    /// `1` (with args) or `3` (without). Kept as the raw byte: the enum has six values and only
    /// three have codecs, so narrowing it to a Rust enum would assert more than we know.
    pub inner_type: u8,
    /// The `args_len` bytes as they appeared, **NUL included**. Not trimmed, not interpreted -
    /// round-tripping the exact bytes is worth more than a tidy `String`. Empty for type 3.
    pub args: Vec<u8>,
    /// The `nested_len` bytes: an encoded BAP message id 10.
    pub nested_msg: Vec<u8>,
    /// The `bitstream_len` bytes. `[U]` grammar.
    pub bitstream: Vec<u8>,
    /// `[U]` semantics. Big-endian in the struct because the builder pre-swaps it.
    pub trailing_id: u64,
}

/// Inner type for [`ActivityHostManagerRequest`] carrying an `args` command line.
pub const AH_MANAGER_INNER_TYPE_WITH_ARGS: u8 = 1;
/// Inner type for [`ActivityHostManagerRequest`] with no `args` field.
pub const AH_MANAGER_INNER_TYPE_NO_ARGS: u8 = 3;
/// Inner type of [`ActivityHostManagerResponse`]. `0x55AF10` decodes nothing else.
pub const AH_MANAGER_INNER_TYPE_RESPONSE: u8 = 2;

impl ActivityHostManagerRequest {
    pub fn decode(body: &[u8]) -> Result<Self, AhDecodeError> {
        if body.len() != AH_MANAGER_REQUEST_BODY_LEN {
            return Err(AhDecodeError::BadSize {
                got: body.len(),
                want: AH_MANAGER_REQUEST_BODY_LEN,
            });
        }
        let inner_type = body[0];
        // Offsets of {args_len, nested_len, bitstream_len, trailing_id}. Type 3 is type 1 shifted
        // back by 0x202 with the args pair removed - see the type-3 note above.
        let (args, mut cursor) = match inner_type {
            AH_MANAGER_INNER_TYPE_WITH_ARGS => {
                let len = usize::from(be_u16(body, 0x001));
                if len > AH_ARGS_FIELD_LEN {
                    return Err(AhDecodeError::LengthOverflow {
                        field: "args",
                        len,
                        max: AH_ARGS_FIELD_LEN,
                    });
                }
                (body[0x003..0x003 + len].to_vec(), 0x203usize)
            }
            AH_MANAGER_INNER_TYPE_NO_ARGS => (Vec::new(), 0x001usize),
            other => return Err(AhDecodeError::BadInnerType(other)),
        };
        let nested_len = usize::from(be_u16(body, cursor));
        if nested_len > AH_NESTED_FIELD_LEN {
            return Err(AhDecodeError::LengthOverflow {
                field: "nested_msg",
                len: nested_len,
                max: AH_NESTED_FIELD_LEN,
            });
        }
        let nested_msg = body[cursor + 2..cursor + 2 + nested_len].to_vec();
        cursor += 2 + AH_NESTED_FIELD_LEN;

        let bitstream_len = usize::from(be_u16(body, cursor));
        if bitstream_len > AH_BITSTREAM_FIELD_LEN {
            return Err(AhDecodeError::LengthOverflow {
                field: "bitstream",
                len: bitstream_len,
                max: AH_BITSTREAM_FIELD_LEN,
            });
        }
        let bitstream = body[cursor + 2..cursor + 2 + bitstream_len].to_vec();
        cursor += 2 + AH_BITSTREAM_FIELD_LEN;

        Ok(Self { inner_type, args, nested_msg, bitstream, trailing_id: be_u64(body, cursor) })
    }

    /// Rebuild the 4111-byte body. **The client sends these; we never do** - this exists so the
    /// decoder can be tested against a byte string rather than against itself.
    pub fn encode(&self) -> Vec<u8> {
        let mut b = vec![0u8; AH_MANAGER_REQUEST_BODY_LEN];
        b[0] = self.inner_type;
        let mut cursor = 0x001usize;
        if self.inner_type == AH_MANAGER_INNER_TYPE_WITH_ARGS {
            b[cursor..cursor + 2].copy_from_slice(&(self.args.len() as u16).to_be_bytes());
            b[cursor + 2..cursor + 2 + self.args.len()].copy_from_slice(&self.args);
            cursor += 2 + AH_ARGS_FIELD_LEN;
        }
        b[cursor..cursor + 2].copy_from_slice(&(self.nested_msg.len() as u16).to_be_bytes());
        b[cursor + 2..cursor + 2 + self.nested_msg.len()].copy_from_slice(&self.nested_msg);
        cursor += 2 + AH_NESTED_FIELD_LEN;

        b[cursor..cursor + 2].copy_from_slice(&(self.bitstream.len() as u16).to_be_bytes());
        b[cursor + 2..cursor + 2 + self.bitstream.len()].copy_from_slice(&self.bitstream);
        cursor += 2 + AH_BITSTREAM_FIELD_LEN;

        b[cursor..cursor + 8].copy_from_slice(&self.trailing_id.to_be_bytes());
        b
    }

    /// The launch command line as text, NUL trimmed. Lossy on purpose: this is for the log, and a
    /// non-UTF-8 byte in it is itself worth seeing rather than an error to propagate.
    pub fn args_str(&self) -> Cow<'_, str> {
        let end = self.args.iter().position(|b| *b == 0).unwrap_or(self.args.len());
        String::from_utf8_lossy(&self.args[..end])
    }
}

/// `[C]` BAP id **7** (`0x0007`), header class 2. Body **exactly `0x89` = 137 bytes**.
/// ⭐⭐ **This is the reply that hands the client its activity host.**
///
/// ```text
/// +0x00  u8          inner_type, must be 2 - 0x55AF10 decodes nothing else
/// +0x01  u64be       host_token   (the codec's only byteswap: FUN_002A2BF0 on param_6 + 1)
/// +0x09  char[0x80]  host_text    NUL-terminated ASCII, never byteswapped
/// ```
///
/// Consumer: `c_bap_host_manager_response_handler::vfunc[0]` at `0x594C60`. It copies
/// `{u64 at +1, 128 bytes at +9}` into one contiguous `0x88` blob, optionally rewrites the `u64`
/// through the address fixup `FUN_0076C1D0` (keyed by `*(u32*)(iface + 0x78)`, slot 0 or 1), and
/// delivers it to the activity client as **`user_defined_type` 10**.
///
/// # The stub, and its falsifier
///
/// `[U]` **what `host_text` should contain.** The fixup copies it as a string, so it is plausibly
/// `"host:port"` or an activity-host name; nothing in the binary pins it. The plan's minimal safe
/// stub is: `inner_type = 2`, a **non-zero** `host_token` (the AH id we intend to hand out), and
/// `host_text` = the endpoint our AH listener is actually reachable at.
///
/// ⭐ **Its falsifier is free.** After this reply lands, either message **16** appears on the wire -
/// confirming the `[I]` link where the client-as-session-host writes the AH id into
/// `s_activity_host_connection_information` - or it does not, in which case that inference is wrong
/// and the AH id reaches the parameter some other way. Do not paper over the difference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityHostManagerResponse {
    /// `[I]` A host/session identity or a packed address; it passes through `FUN_0076C1D0` before
    /// use, and that fixup is what makes "identity" and "address" indistinguishable from here.
    /// **Must be non-zero**: `FUN_00594320` returns false and issues nothing while the id is 0.
    pub host_token: u64,
    /// `[I]` NUL-terminated ASCII, zero-padded to 128 bytes.
    pub host_text: [u8; AH_HOST_TEXT_LEN],
}

impl ActivityHostManagerResponse {
    /// The minimal safe stub. Returns `None` if `text` does not fit with its NUL terminator.
    pub fn new(host_token: u64, text: &str) -> Option<Self> {
        if text.len() >= AH_HOST_TEXT_LEN {
            return None;
        }
        let mut host_text = [0u8; AH_HOST_TEXT_LEN];
        host_text[..text.len()].copy_from_slice(text.as_bytes());
        Some(Self { host_token, host_text })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(AH_MANAGER_RESPONSE_BODY_LEN);
        b.push(AH_MANAGER_INNER_TYPE_RESPONSE);
        b.extend_from_slice(&self.host_token.to_be_bytes());
        b.extend_from_slice(&self.host_text);
        b
    }

    pub fn decode(body: &[u8]) -> Result<Self, AhDecodeError> {
        if body.len() != AH_MANAGER_RESPONSE_BODY_LEN {
            return Err(AhDecodeError::BadSize {
                got: body.len(),
                want: AH_MANAGER_RESPONSE_BODY_LEN,
            });
        }
        if body[0] != AH_MANAGER_INNER_TYPE_RESPONSE {
            return Err(AhDecodeError::BadInnerType(body[0]));
        }
        let mut host_text = [0u8; AH_HOST_TEXT_LEN];
        host_text.copy_from_slice(&body[9..9 + AH_HOST_TEXT_LEN]);
        Ok(Self { host_token: be_u64(body, 1), host_text })
    }

    /// `host_text` up to its NUL.
    pub fn host_text_str(&self) -> Cow<'_, str> {
        let end = self.host_text.iter().position(|b| *b == 0).unwrap_or(AH_HOST_TEXT_LEN);
        String::from_utf8_lossy(&self.host_text[..end])
    }
}

// ─── message 8 - client_to_activity_host_notification - Phase 4, decode only ──────────────────

/// `[C]` body / `[U]` header. BAP id **8** (`0x0008`), **header class 3 = 14 bytes** - a header
/// shape we have never written or read. Codec `0x55EC70`, **shared with message 160**.
///
/// ```text
/// +0x00  u8     inner_type (1 or 2)
/// +0x01  u32be  user_defined_type   ⭐ the activity-host APPLICATION protocol key
/// +0x05  i32be  payload_size        rejected outside 0..=0x1DF00
/// +0x09  u32be  aux                 TYPE 1 ONLY. `[U]`
/// +0x0D  u8[payload_size]           opaque to BAP
/// ```
///
/// Wire body is `payload_size + 0x0D` (type 1) or `+ 0x09` (type 2). The client's own builder
/// (`send_message_via_bap`, `0x594550`) writes type 1 unconditionally.
///
/// ⚠ `user_defined_type == 8` **never reaches the wire**: it is intercepted client-side and rebuilt
/// as message 6 instead.
///
/// **We decode, log the `user_defined_type`, and do not answer.** The policy table says
/// `expects_response == None`, so [`crate::conversation::BapConversation::enqueue`] classifies it
/// [`crate::conversation::EnqueueOutcome::FireAndForget`] and issues **no ticket** - R6 is enforced
/// by the type system here rather than by anyone's discipline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientToActivityHostNotification {
    /// 1 or 2. Type 2 has no `aux`.
    pub inner_type: u8,
    /// ⭐ The activity-host application protocol key. Its legal values are **not** a BAP question:
    /// the switch lives behind `i_activity_client_messaging_interface` vtable slot 2. Known points:
    /// `8` is intercepted and becomes message 6; `10` is what the message-7 handler injects.
    pub user_defined_type: u32,
    /// `[U]` - present only on inner type 1. Nothing in the binary reads it that has been found.
    pub aux: Option<u32>,
    /// Opaque to BAP.
    pub payload: Vec<u8>,
}

impl ClientToActivityHostNotification {
    pub fn decode(body: &[u8]) -> Result<Self, AhDecodeError> {
        if body.len() < 9 {
            return Err(AhDecodeError::Truncated { need: 9, got: body.len() });
        }
        let inner_type = body[0];
        let header_len = match inner_type {
            1 => 0x0D,
            2 => 0x09,
            other => return Err(AhDecodeError::BadInnerType(other)),
        };
        let payload_size = be_u32(body, 5) as i32;
        if payload_size < 0 || payload_size as usize > AH_NOTIFICATION_MAX_PAYLOAD {
            return Err(AhDecodeError::PayloadOutOfRange(payload_size));
        }
        let want = header_len + payload_size as usize;
        if body.len() != want {
            return Err(AhDecodeError::BadSize { got: body.len(), want });
        }
        Ok(Self {
            inner_type,
            user_defined_type: be_u32(body, 1),
            aux: if inner_type == 1 { Some(be_u32(body, 9)) } else { None },
            payload: body[header_len..].to_vec(),
        })
    }

    /// The client builds these; encoding is for tests.
    pub fn encode(&self) -> Vec<u8> {
        let mut b = Vec::new();
        b.push(self.inner_type);
        b.extend_from_slice(&self.user_defined_type.to_be_bytes());
        b.extend_from_slice(&(self.payload.len() as u32).to_be_bytes());
        if let Some(aux) = self.aux {
            b.extend_from_slice(&aux.to_be_bytes());
        }
        b.extend_from_slice(&self.payload);
        b
    }
}

// ─── message 100 - activity_host_to_client_notification - Phase 4, push ───────────────────────

/// `[C]` body / `[U]` **HEADER**. BAP id **100** (`0x0064`), **header class 6 = 32 bytes**.
///
/// ⚠⚠ **Nobody has read header class 6.** [`crate::conversation::BapConversation::push`] therefore
/// refuses this id - see [`crate::conversation::PushError::UnwritableHeader`]. Pushing it with a
/// 6-byte header would park the client in its resumable header read (`0xB2ED40`) with **no timeout,
/// no log and no error branch**, which is indistinguishable from "the game never got there".
/// **Read `0x19234A0` class 6 first.** The body codec below is ready for that day.
///
/// The layout is `[C]`, from the client's own encoder `0x55E690` and decoder `0x55E130`:
///
/// ```text
/// +0x00  u8     inner_type, must be 1 - only one exists
/// +0x01  u64be  recipient_id     `[I]` addressing key
/// +0x09  u32be  user_defined_type   `[C]` get_user_defined_type (0x55E480) reads exactly here
/// +0x0D  i32be  payload_size     rejected outside 0..=0x1DF00
/// +0x11  u8[payload_size]        opaque to BAP
/// ```
///
/// Wire body is `payload_size + 0x11`, and the decoder's bound (`< 0x1DF12`) is exactly
/// `0x1DF00 + 0x11 + 1` - the two numbers corroborate each other.
///
/// > ### ⛔ RETRACTED IN PLACE, 2026-08-09 - the `recipient_id` gate was named wrong TWICE
/// >
/// > This doc used to say *"the push policy (`0x55F6B0`) gates delivery on `FUN_0029B6D0(low
/// > u32)`"*. `push.rs` then retracted that to *"whatever validates `recipient_id`, it is
/// > downstream of the queue"*. Both are superseded; the real chain is now read end to end.
/// >
/// > `recipient_id` is matched in **`FUN_0075AB60(activity_manager, id, udt == 4)`**, reached from
/// > message **9**'s push policy (`0x5642E0` → `0x75CB40` → `0x75CB70`). It walks the **three**
/// > activity-manager slots at `mgr + 0x5B8 + i*0x1F0` and compares our 8 bytes against:
/// > * `c_activity_client::vfunc[2]` = `0x2992A0` = **`[ac + 0xC98]`, the ESTABLISHED activity-host
/// >   id** - which is `0` until a `join_result` has been accepted (it is what the client renders
/// >   as `AH->%llx`); and
/// > * for `udt == 4` only, a second branch: when the client is *not* established but `ac+0x130 &
/// >   8` is set (join request sent), it compares `FUN_0029B230(ac) + 8` - the AH id of the
/// >   **pending join request**.
/// >
/// > ⇒ Two consequences, both load-bearing:
/// > 1. `recipient_id` must be the **activity-host id** (`d1-activity-host`'s `state.rs` is right;
/// >    `d1-responder`'s `D1_AH_RECIPIENT` defaulting to the account SOID is wrong).
/// > 2. ⭐ **Order is mandatory at the ROUTER even though it is not at the gate.** `udt 0`, `1` and
/// >    `11` can only match through `[ac+0xC98]`, so they are unroutable until `udt 4` has been
/// >    accepted. `FUN_00298EF0` is arrival-completeness; `FUN_0075AB60` is not.
/// >
/// > And it is **silent**: `FUN_0075AB60` returning `-1` makes `FUN_0075CB70` return `true` with no
/// > diagnostic at all. The only log on that path - *"Failed to handle activity host notification
/// > [msg type %d][AH %s]"* - fires when a slot **was** found and `FUN_00594960` refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityHostToClientNotification {
    /// `[I]` addressing key. See the silent-drop warning above.
    pub recipient_id: u64,
    /// The activity-host application protocol key. `[U]` legal values - not a BAP question.
    pub user_defined_type: u32,
    pub payload: Vec<u8>,
}

/// The only inner type message 100 has.
pub const AH_CLIENT_NOTIFICATION_INNER_TYPE: u8 = 1;

impl ActivityHostToClientNotification {
    pub fn encode(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(0x11 + self.payload.len());
        b.push(AH_CLIENT_NOTIFICATION_INNER_TYPE);
        b.extend_from_slice(&self.recipient_id.to_be_bytes());
        b.extend_from_slice(&self.user_defined_type.to_be_bytes());
        b.extend_from_slice(&(self.payload.len() as u32).to_be_bytes());
        b.extend_from_slice(&self.payload);
        b
    }

    pub fn decode(body: &[u8]) -> Result<Self, AhDecodeError> {
        if body.len() < 0x11 {
            return Err(AhDecodeError::Truncated { need: 0x11, got: body.len() });
        }
        if body[0] != AH_CLIENT_NOTIFICATION_INNER_TYPE {
            return Err(AhDecodeError::BadInnerType(body[0]));
        }
        let payload_size = be_u32(body, 0x0D) as i32;
        if payload_size < 0 || payload_size as usize > AH_NOTIFICATION_MAX_PAYLOAD {
            return Err(AhDecodeError::PayloadOutOfRange(payload_size));
        }
        let want = 0x11 + payload_size as usize;
        if body.len() != want {
            return Err(AhDecodeError::BadSize { got: body.len(), want });
        }
        Ok(Self {
            recipient_id: be_u64(body, 1),
            user_defined_type: be_u32(body, 9),
            payload: body[0x11..].to_vec(),
        })
    }
}

// ─── message 9 - bap_to_client_notification - ⭐ THE ONE THE CLIENT ACTUALLY CONSUMES ─────────

/// BAP id **9** (`0x0009`), header class **0 = 6 bytes**.
///
/// ⭐⭐ **Message 9 carries the SAME body as message 100 - literally the same encoder and decoder
/// in the client** - and it is the *only* one of the pair whose deliver step reaches the activity
/// client. `[C]`, from the pinned 01.29 eboot:
///
/// * `c_investment_bap_message_bap_to_client_notification` (typeinfo `0x1923850`, vtable
///   `0x1923810`) and `…_activity_host_to_client_notification` (`0x19237E0` / `0x19237A0`) **share
///   vtable slots 2, 3, 4 and 6** - `0x55E690` (encode), `0x55E710`→`0x55E130` (decode),
///   `0x559D30`, `0x55E750`. Only slot 0 differs: `mov eax, 9` vs `mov eax, 0x64`. So
///   [`ActivityHostToClientNotification`] is the body codec for both, and this alias says so
///   rather than duplicating it (`design/format-description.md`: no format twice).
/// * Their **push policies** are opposites. `push_policy_bap_to_client_notification`
///   (vtable `0x1925B10`): slots 1/2 = `0x1F70` (`mov al,1`), slot 3 = `0x5642B0`, slot 5 =
///   `0x5642E0`. `push_policy_activity_host_to_client_notification` (`0x1925DB0`): slots 1/2/3 =
///   `0x14D0` (`xor al,al`), slot 5 = `0x564C20`.
/// * `handle_message_internal` (`0x563170`) decodes a push and calls `policy->vfunc[5]`:
///   * **9 → `0x5642E0`** → `FUN_0075CB40` → `FUN_0075CB70(activity_manager, body)` → recipient
///     lookup `FUN_0075AB60` → `FUN_00594960` → `c_activity_client::vfunc[9]`
///     (`0x299430`, `receive_message_from_host`). **Delivery.**
///   * **100 → `0x564C20`** → read the class-6 header's `+0x06` u64 as a *connection index*, and
///     if `FUN_00558AA0(idx) == 3` re-emit the identical body as **message 9 on that connection**.
///     **A relay, not a delivery** - and it returns `1` whether or not it forwarded anything.
///
/// ⇒ The AH↔client path in Bungie's design is two hops through the BAP server:
/// `client -8-> BAP -160-> AH` and `AH -100-> BAP -9-> client`. A server that speaks directly to
/// the client must send **9**. Measured: run `20260808-234535-CODEC-LIVE2` pushed four message-100
/// frames on the AH connection and the client relayed all four back to the *primary* connection as
/// message 9, byte for byte, 1:1 - because we leave header `+0x06` at zero and connection 0 was
/// connected. `historicaldocs/design/establish-client-gate.md`.
pub type BapToClientNotification = ActivityHostToClientNotification;

/// BAP id 9. Header class 0, so [`crate::conversation::BapConversation::push`] writes the plain
/// 6-byte header - no extras, nothing left `[U]` in the framing.
pub const BAP_TYPE_BAP_TO_CLIENT_NOTIFICATION: u16 = 9;
/// BAP id 100. Kept so the falsifier can name it.
pub const BAP_TYPE_AH_TO_CLIENT_NOTIFICATION: u16 = 100;

/// The 17 `user_defined_type`s an Activity Host may send **to** a client, with the client's own
/// names, so a message-9 log line reads as a protocol event rather than a number.
///
/// ⚠ This is a *subset copy* of `d1_activity_host::protocol::AH_MESSAGE_NAMES`, which stays the
/// authority. `d1-responder` cannot depend on `d1-activity-host` (that would drag `unicorn-engine`
/// into the responder and re-open the whole-workspace-vs-`-p` feature trap), so the table is here.
/// **The drift risk is closed by a control, not by a comment**: `d1-activity-host`'s
/// `ah_to_client_udt_names_agree_with_the_protocol_table` asserts all 17 entries against the
/// authority, and `d1-activity-host controls` exits 2 if they diverge.
pub const AH_TO_CLIENT_UDT_NAMES: [(u32, &str); 17] = [
    (0, "entity_slots_allocated"),
    (1, "global_activity_state"),
    (2, "world_globals_state"),
    (4, "join_result"),
    (5, "sensor_auth_update"),
    (7, "sensor_message"),
    (10, "start_activity_host_response"),
    (11, "replicate_membership"),
    (18, "incident"),
    (23, "claim_authority_over_abandoned_entity_slots"),
    (24, "purge_abandoned_entity_slots"),
    (27, "reset_entity_slot_authority_mask"),
    (29, "query_entity_slot_authority_mask"),
    (38, "script_state"),
    (39, "script_event"),
    (42, "advance_replication_epoch"),
    (43, "reservations_failed"),
];

/// Name an AH→client `user_defined_type`, or `None` if it is not one the client can receive.
///
/// `None` is a **finding**, not a formatting problem: it means the id is either client→AH only or
/// outside the 49-entry space, and the client's `receive_message_from_host` (`0x299430`) would log
/// `unknown message type '%d'` for it.
pub fn ah_to_client_udt_name(udt: u32) -> Option<&'static str> {
    AH_TO_CLIENT_UDT_NAMES.iter().find(|(id, _)| *id == udt).map(|(_, n)| *n)
}

// ─── message 102 - ⛔ DO NOT PUSH ─────────────────────────────────────────────────────────────

/// ⛔ **BAP id 102 (`0x0066`) IS NOT PUSHABLE. Do not send this on the primary connection.**
///
/// It is **absent from the client's 13-entry push accept-set** at `0x1925A90`, which this crate
/// re-derived: `{6, 8, 9, 20, 27, 28, 47, 100, 123, 124, 160, 161, 301}`. The brief's "100 / 102 -
/// we push" is half wrong, and the wrong half kills the connection with the Weasel.
/// [`crate::conversation::BapConversation::push`] refuses it; that refusal is a test.
///
/// The schema is here because it is cheap to keep and expensive to re-derive, not because we may
/// use it. `[R]` **transcribed from `historicaldocs/design/SERVER-ROLES-PLAN.md` §3.6 and not independently
/// re-derived in this session**, unlike everything else in this module: a fixed 256-byte body,
/// `u8 len` then `char[0xFF]`, codec shared verbatim with message 27, header class 5 = 24 bytes.
///
/// `[U]` where 102 *is* legally delivered - probably an `OUT FAH` / `OUT GAH*` question, answerable
/// by reading the push handlers installed on the connections `0x559650(index)` builds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityHostToClientLogNotification {
    /// Up to 255 bytes of text. The wire field is always 256 bytes; the rest is padding.
    pub text: Vec<u8>,
}

impl ActivityHostToClientLogNotification {
    pub fn new(text: &str) -> Option<Self> {
        (text.len() <= 0xFF).then(|| Self { text: text.as_bytes().to_vec() })
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut b = vec![0u8; AH_LOG_NOTIFICATION_BODY_LEN];
        b[0] = self.text.len() as u8;
        b[1..1 + self.text.len()].copy_from_slice(&self.text);
        b
    }

    pub fn decode(body: &[u8]) -> Result<Self, AhDecodeError> {
        if body.len() != AH_LOG_NOTIFICATION_BODY_LEN {
            return Err(AhDecodeError::BadSize {
                got: body.len(),
                want: AH_LOG_NOTIFICATION_BODY_LEN,
            });
        }
        let len = usize::from(body[0]);
        Ok(Self { text: body[1..1 + len].to_vec() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    /// ⭐ `observed-live`. A **complete message-9 frame the client itself put on the wire**, from
    /// `runs/20260808-234535-CODEC-LIVE2/responder.log` (`SECURED (177B)`, seq 464, `msg#465`).
    ///
    /// It is the client relaying our `global_activity_state` push back at us: header `00 09` +
    /// `u32be 464`, then message 100's body verbatim. 177 = 6 + 0x11 + 154, and the 154 is the
    /// schema-coded udt-1 payload `d1-activity-host` encoded through the client's own
    /// `FUN_002A4140`.
    ///
    /// ⚠ Provenance, stated so it cannot be over-claimed: the **payload** is `synthetic_ancestor`
    /// (we made it). The **framing** - that message 9 is header class 0, that its body is message
    /// 100's, and that the client emits it at all - is `observed-live`.
    const CAPTURED_MESSAGE_9_FRAME: &str = concat!(
        "0009000001d001d1a0000000000001000000010000009a800000000000000010",
        "0000001010101010101010101010101010101010101010101010101010101010",
        "1010101010101010101010101010101010101010101010101010101010101010",
        "1010100008000000000000000000000000000000000000000000060002020000",
        "08000800080000000e1e3f4dfe3eff3edefdfedb0dfe8e9e7e8f7e1f98080808",
        "0808080808080808080808080808080800",
    );

    fn unhex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
    }

    /// ⭐⭐ The control for the whole message-9 reading, on bytes nobody here invented the shape of.
    ///
    /// The axis it tests is **the header length**: if message 9 were class 2 (8 bytes) or class 6
    /// (32), the body would decode from the wrong offset and the length self-check inside
    /// [`ActivityHostToClientNotification::decode`] would reject it. It passing at exactly `+6`,
    /// with the declared payload size tiling the frame to its last byte, is what pins class 0.
    #[test]
    fn the_captured_message_9_frame_decodes_at_offset_six_as_a_message_100_body() {
        let frame = unhex(CAPTURED_MESSAGE_9_FRAME);
        assert_eq!(frame.len(), 177);
        assert_eq!(&frame[0..2], &[0x00, 0x09], "u16be type = 9");
        assert_eq!(u32::from_be_bytes(frame[2..6].try_into().unwrap()), 464);

        let hdr = crate::header_len(BAP_TYPE_BAP_TO_CLIENT_NOTIFICATION).unwrap();
        assert_eq!(hdr, 6, "message 9 is header class 0");

        let n = BapToClientNotification::decode(&frame[hdr..]).expect("the shared codec must decode");
        assert_eq!(n.recipient_id, 0xD1A0_0000_0000_0001, "our FIRST_ACTIVITY_HOST_ID, echoed");
        assert_eq!(n.user_defined_type, 1);
        assert_eq!(ah_to_client_udt_name(1), Some("global_activity_state"));
        assert_eq!(n.payload.len(), 154);
        assert_eq!(hdr + 0x11 + n.payload.len(), frame.len(), "the body tiles the frame exactly");

        // NEGATIVE, and it is the axis: decoding from any other header length must FAIL. A decoder
        // that accepted the frame at +8 or +32 would make the class-0 claim unfalsifiable.
        for wrong in [8usize, 14, 24, 32] {
            assert!(
                BapToClientNotification::decode(&frame[wrong..]).is_err(),
                "message 9's body must not also decode at +{wrong}"
            );
        }
    }

    // ── 16 / 17 ───────────────────────────────────────────────────────────────────────────────

    #[test]
    fn proxy_request_round_trips_and_rejects_every_other_length() {
        let req = GetActivityHostProxyRequest { activity_host_id: 0x0102_0304_0506_0708 };
        let bytes = req.encode();
        assert_eq!(bytes, vec![1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(GetActivityHostProxyRequest::decode(&bytes), Ok(req));
        // 0x562B90's own first test.
        assert_eq!(
            GetActivityHostProxyRequest::decode(&[0u8; 7]),
            Err(AhDecodeError::BadSize { got: 7, want: 8 })
        );
        assert_eq!(
            GetActivityHostProxyRequest::decode(&[0u8; 9]),
            Err(AhDecodeError::BadSize { got: 9, want: 8 })
        );
    }

    /// `0` is a legal activity-host id and means "the proxy itself" - `0x594E70` prints the literal
    /// `"proxy"`. A decoder that treated it as absent would be wrong.
    #[test]
    fn activity_host_id_zero_is_legal() {
        assert_eq!(
            GetActivityHostProxyRequest::decode(&[0u8; 8]),
            Ok(GetActivityHostProxyRequest { activity_host_id: 0 })
        );
    }

    /// ⭐ The echo is mandatory: `0x594E70` drops a mismatched reply **silently**, which looks
    /// exactly like "the server never answered". `new` takes the request so it cannot drift.
    #[test]
    fn proxy_response_echoes_the_requested_id_and_lays_out_the_address() {
        let req = GetActivityHostProxyRequest { activity_host_id: 0xD100_0000_0000_00AA };
        let rsp = GetActivityHostProxyResponse::new(req, Ipv4Addr::new(192, 168, 1, 84), 7003);
        assert_eq!(rsp.activity_host_id, req.activity_host_id);
        let bytes = rsp.encode();
        assert_eq!(bytes.len(), AH_PROXY_RESPONSE_BODY_LEN);
        // Wire bytes 0x08..0x0C are a.b.c.d in that order.
        assert_eq!(&bytes[8..12], &[192, 168, 1, 84]);
        // The port is a BE u32 whose high half is zero: both readings of the `[U]` width hold.
        assert_eq!(&bytes[12..16], &[0x00, 0x00, 0x1B, 0x5B]);
        assert_eq!(GetActivityHostProxyResponse::decode(&bytes), Ok(rsp));
        assert_eq!(rsp.ipv4(), Ipv4Addr::new(192, 168, 1, 84));
        assert_eq!(rsp.port(), 7003);
    }

    #[test]
    fn proxy_response_rejects_every_other_length() {
        assert_eq!(
            GetActivityHostProxyResponse::decode(&[0u8; 15]),
            Err(AhDecodeError::BadSize { got: 15, want: 16 })
        );
    }

    // ── 6 / 7 ─────────────────────────────────────────────────────────────────────────────────

    fn sample_type1() -> ActivityHostManagerRequest {
        ActivityHostManagerRequest {
            inner_type: AH_MANAGER_INNER_TYPE_WITH_ARGS,
            args: b"/ah_type:fireteam /ah_package:x\0".to_vec(),
            nested_msg: vec![0x00, 0x0A, 0xDE, 0xAD],
            bitstream: vec![0xAA; 64],
            trailing_id: 0xD100_0000_0000_0001,
        }
    }

    #[test]
    fn manager_request_type1_round_trips() {
        let req = sample_type1();
        let bytes = req.encode();
        assert_eq!(bytes.len(), AH_MANAGER_REQUEST_BODY_LEN);
        assert_eq!(ActivityHostManagerRequest::decode(&bytes), Ok(req.clone()));
        assert_eq!(ActivityHostManagerRequest::decode(&bytes).unwrap().encode(), bytes);
        assert_eq!(req.args_str(), "/ah_type:fireteam /ah_package:x");
    }

    /// ⭐ The offsets are the whole finding. If any of these three moves, the decoder is reading
    /// somebody else's field and will do it silently.
    #[test]
    fn manager_request_type1_length_fields_sit_where_the_decoder_byteswaps() {
        let bytes = sample_type1().encode();
        assert_eq!(be_u16(&bytes, 0x001), 32, "args_len at +0x001");
        assert_eq!(be_u16(&bytes, 0x203), 4, "nested_len at +0x203");
        assert_eq!(be_u16(&bytes, 0x405), 64, "bitstream_len at +0x405");
        assert_eq!(be_u64(&bytes, 0x1007), 0xD100_0000_0000_0001, "trailing_id at +0x1007");
    }

    /// Type 3 shifts every later field back by `0x202`, which is exactly the difference between the
    /// two `memcpy` lengths in `0x55AC00` (`0x100E` vs `0xE0C`).
    #[test]
    fn manager_request_type3_shifts_every_field_back_by_0x202() {
        let req = ActivityHostManagerRequest {
            inner_type: AH_MANAGER_INNER_TYPE_NO_ARGS,
            args: Vec::new(),
            nested_msg: vec![0x11, 0x22],
            bitstream: vec![0x33; 8],
            trailing_id: 0xFEED_FACE_CAFE_BEEF,
        };
        let bytes = req.encode();
        assert_eq!(bytes.len(), AH_MANAGER_REQUEST_BODY_LEN, "the frame is still 0x100F");
        assert_eq!(be_u16(&bytes, 0x001), 2, "nested_len at +0x001 (0x203 - 0x202)");
        assert_eq!(be_u16(&bytes, 0x203), 8, "bitstream_len at +0x203 (0x405 - 0x202)");
        assert_eq!(be_u64(&bytes, 0xE05), 0xFEED_FACE_CAFE_BEEF, "trailing_id at +0xE05");
        // …and the last 0x202 bytes are padding the client's memcpy never touches.
        assert!(bytes[0xE0D..].iter().all(|b| *b == 0));
        assert_eq!(ActivityHostManagerRequest::decode(&bytes), Ok(req));
    }

    #[test]
    fn manager_request_rejects_unhandled_inner_types_and_wrong_sizes() {
        let mut bytes = sample_type1().encode();
        bytes[0] = 5; // in the six-value enum, but no codec handles it
        assert_eq!(ActivityHostManagerRequest::decode(&bytes), Err(AhDecodeError::BadInnerType(5)));
        assert_eq!(
            ActivityHostManagerRequest::decode(&[0u8; 16]),
            Err(AhDecodeError::BadSize { got: 16, want: AH_MANAGER_REQUEST_BODY_LEN })
        );
    }

    /// ⭐ The reply that hands the client its activity host. 137 bytes, inner type 2, and the token
    /// must be non-zero or `FUN_00594320` never issues message 16.
    #[test]
    fn manager_response_is_137_bytes_with_inner_type_2() {
        let rsp = ActivityHostManagerResponse::new(0xD100_0000_0000_00AA, "203.0.113.84:7003")
            .expect("fits");
        let bytes = rsp.encode();
        assert_eq!(bytes.len(), AH_MANAGER_RESPONSE_BODY_LEN);
        assert_eq!(bytes[0], AH_MANAGER_INNER_TYPE_RESPONSE);
        assert_eq!(&bytes[1..9], &0xD100_0000_0000_00AAu64.to_be_bytes());
        assert_eq!(&bytes[9..26], b"203.0.113.84:7003");
        assert_eq!(bytes[26], 0, "host_text is NUL-terminated and zero-padded");
        assert_eq!(ActivityHostManagerResponse::decode(&bytes), Ok(rsp.clone()));
        assert_eq!(rsp.host_text_str(), "203.0.113.84:7003");
    }

    #[test]
    fn manager_response_rejects_any_inner_type_but_2() {
        let mut bytes = ActivityHostManagerResponse::new(1, "x").unwrap().encode();
        bytes[0] = 1;
        assert_eq!(
            ActivityHostManagerResponse::decode(&bytes),
            Err(AhDecodeError::BadInnerType(1))
        );
        assert!(ActivityHostManagerResponse::new(1, &"x".repeat(128)).is_none());
    }

    // ── 8 ─────────────────────────────────────────────────────────────────────────────────────

    #[test]
    fn client_to_activity_host_notification_round_trips_both_inner_types() {
        let one = ClientToActivityHostNotification {
            inner_type: 1,
            user_defined_type: 10,
            aux: Some(0x1234_5678),
            payload: vec![1, 2, 3],
        };
        let bytes = one.encode();
        assert_eq!(bytes.len(), 0x0D + 3);
        assert_eq!(ClientToActivityHostNotification::decode(&bytes), Ok(one));

        let two = ClientToActivityHostNotification {
            inner_type: 2,
            user_defined_type: 7,
            aux: None,
            payload: vec![9],
        };
        let bytes = two.encode();
        assert_eq!(bytes.len(), 0x09 + 1);
        assert_eq!(ClientToActivityHostNotification::decode(&bytes), Ok(two));
    }

    #[test]
    fn notification_payload_ceiling_is_enforced() {
        let mut bytes = vec![1u8, 0, 0, 0, 10];
        bytes.extend_from_slice(&(AH_NOTIFICATION_MAX_PAYLOAD as u32 + 1).to_be_bytes());
        bytes.extend_from_slice(&[0; 4]);
        assert_eq!(
            ClientToActivityHostNotification::decode(&bytes),
            Err(AhDecodeError::PayloadOutOfRange(AH_NOTIFICATION_MAX_PAYLOAD as i32 + 1))
        );
    }

    #[test]
    fn client_notifications_round_trip_at_payload_boundaries() {
        let sizes = [
            0usize,
            1,
            127,
            128,
            255,
            256,
            1024,
            AH_NOTIFICATION_MAX_PAYLOAD,
        ];
        for inner_type in [1u8, 2] {
            for (case, size) in sizes.into_iter().enumerate() {
                let payload: Vec<u8> = (0..size)
                    .map(|i| ((i.wrapping_mul(37) + case) & 0xFF) as u8)
                    .collect();
                let user_defined_type = if case % 2 == 0 { 0 } else { u32::MAX };
                let aux = (inner_type == 1).then_some(if case % 2 == 0 { 0 } else { u32::MAX });
                let notification = ClientToActivityHostNotification {
                    inner_type,
                    user_defined_type,
                    aux,
                    payload: payload.clone(),
                };
                let wire = notification.encode();
                let header = if inner_type == 1 { 13 } else { 9 };
                assert_eq!(wire.len(), header + size);
                assert_eq!(&wire[1..5], &user_defined_type.to_be_bytes());
                assert_eq!(&wire[5..9], &(size as u32).to_be_bytes());
                if let Some(value) = aux {
                    assert_eq!(&wire[9..13], &value.to_be_bytes());
                }
                assert_eq!(&wire[header..], payload.as_slice());
                assert_eq!(
                    ClientToActivityHostNotification::decode(&wire),
                    Ok(notification)
                );

                let mut with_extra = wire.clone();
                with_extra.push(0xA5);
                assert_eq!(
                    ClientToActivityHostNotification::decode(&with_extra),
                    Err(AhDecodeError::BadSize {
                        got: wire.len() + 1,
                        want: wire.len()
                    })
                );
                if size != 0 {
                    assert_eq!(
                        ClientToActivityHostNotification::decode(&wire[..wire.len() - 1]),
                        Err(AhDecodeError::BadSize {
                            got: wire.len() - 1,
                            want: wire.len()
                        })
                    );
                }
            }
        }
    }

    #[test]
    fn client_notification_rejects_bad_lengths_and_aux_shape() {
        let one = ClientToActivityHostNotification {
            inner_type: 1,
            user_defined_type: 10,
            aux: None,
            payload: Vec::new(),
        };
        assert_eq!(
            ClientToActivityHostNotification::decode(&one.encode()),
            Err(AhDecodeError::BadSize { got: 9, want: 13 })
        );
        let two = ClientToActivityHostNotification {
            inner_type: 2,
            aux: Some(1),
            ..one
        };
        assert_eq!(
            ClientToActivityHostNotification::decode(&two.encode()),
            Err(AhDecodeError::BadSize { got: 13, want: 9 })
        );

        let mut wire = ClientToActivityHostNotification {
            inner_type: 2,
            user_defined_type: u32::MAX,
            aux: None,
            payload: Vec::new(),
        }
        .encode();
        for invalid in [0, 3, 255] {
            wire[0] = invalid;
            assert_eq!(
                ClientToActivityHostNotification::decode(&wire),
                Err(AhDecodeError::BadInnerType(invalid))
            );
        }
        wire[0] = 2;
        for length in [AH_NOTIFICATION_MAX_PAYLOAD as u32 + 1, u32::MAX] {
            wire[5..9].copy_from_slice(&length.to_be_bytes());
            assert_eq!(
                ClientToActivityHostNotification::decode(&wire),
                Err(AhDecodeError::PayloadOutOfRange(length as i32))
            );
        }
    }

    // ── 100 / 102 ─────────────────────────────────────────────────────────────────────────────

    #[test]
    fn activity_host_to_client_notification_round_trips() {
        let n = ActivityHostToClientNotification {
            recipient_id: 0xD100_0000_0000_0101,
            user_defined_type: 10,
            payload: vec![0xAB; 5],
        };
        let bytes = n.encode();
        assert_eq!(bytes.len(), 0x11 + 5, "wire body is payload + 0x11");
        assert_eq!(bytes[0], 1);
        assert_eq!(&bytes[1..9], &0xD100_0000_0000_0101u64.to_be_bytes());
        assert_eq!(&bytes[9..13], &10u32.to_be_bytes(), "udt at +0x09, where 0x55E480 reads it");
        assert_eq!(ActivityHostToClientNotification::decode(&bytes), Ok(n));
    }

    #[test]
    fn host_notifications_round_trip_at_payload_and_header_limits() {
        let sizes = [0usize, 1, 127, 128, 1024, AH_NOTIFICATION_MAX_PAYLOAD];
        for (index, size) in sizes.into_iter().enumerate() {
            let payload: Vec<u8> = (0..size)
                .map(|i| ((i.wrapping_mul(53) + index) & 0xFF) as u8)
                .collect();
            for (recipient_id, user_defined_type) in
                [(0, 0), (u64::MAX, u32::MAX), (0xD100_0000_0000_0101, 10)]
            {
                let notification = ActivityHostToClientNotification {
                    recipient_id,
                    user_defined_type,
                    payload: payload.clone(),
                };
                let wire = notification.encode();
                assert_eq!(wire.len(), 0x11 + size);
                assert_eq!(wire[0], AH_CLIENT_NOTIFICATION_INNER_TYPE);
                assert_eq!(&wire[1..9], &recipient_id.to_be_bytes());
                assert_eq!(&wire[9..13], &user_defined_type.to_be_bytes());
                assert_eq!(&wire[13..17], &(size as u32).to_be_bytes());
                assert_eq!(&wire[17..], payload.as_slice());
                assert_eq!(
                    ActivityHostToClientNotification::decode(&wire),
                    Ok(notification)
                );

                let mut with_extra = wire.clone();
                with_extra.push(0x5A);
                assert_eq!(
                    ActivityHostToClientNotification::decode(&with_extra),
                    Err(AhDecodeError::BadSize {
                        got: wire.len() + 1,
                        want: wire.len()
                    })
                );
                if size != 0 {
                    assert_eq!(
                        ActivityHostToClientNotification::decode(&wire[..wire.len() - 1]),
                        Err(AhDecodeError::BadSize {
                            got: wire.len() - 1,
                            want: wire.len()
                        })
                    );
                }
            }
        }

        let mut wire = ActivityHostToClientNotification {
            recipient_id: 0,
            user_defined_type: 0,
            payload: Vec::new(),
        }
        .encode();
        for inner in [0, 2, 255] {
            wire[0] = inner;
            assert_eq!(
                ActivityHostToClientNotification::decode(&wire),
                Err(AhDecodeError::BadInnerType(inner))
            );
        }
        wire[0] = AH_CLIENT_NOTIFICATION_INNER_TYPE;
        for length in [AH_NOTIFICATION_MAX_PAYLOAD as u32 + 1, u32::MAX] {
            wire[13..17].copy_from_slice(&length.to_be_bytes());
            assert_eq!(
                ActivityHostToClientNotification::decode(&wire),
                Err(AhDecodeError::PayloadOutOfRange(length as i32))
            );
        }
    }

    /// The body encoder existing must never be read as permission to send it. The refusal lives in
    /// `conversation`; this asserts the two facts that make the refusal correct.
    #[test]
    fn message_100_is_class_6_and_message_102_is_not_pushable() {
        assert_eq!(crate::tables::header_class(100), Some(6));
        assert_eq!(crate::tables::HEADER_LEN[6], 32);
        assert!(!crate::tables::is_pushable(102));
    }

    #[test]
    fn log_notification_is_a_fixed_256_byte_pascal_string() {
        let n = ActivityHostToClientLogNotification::new("hello").unwrap();
        let bytes = n.encode();
        assert_eq!(bytes.len(), AH_LOG_NOTIFICATION_BODY_LEN);
        assert_eq!(bytes[0], 5);
        assert_eq!(&bytes[1..6], b"hello");
        assert_eq!(ActivityHostToClientLogNotification::decode(&bytes), Ok(n));
        assert!(ActivityHostToClientLogNotification::new(&"x".repeat(256)).is_none());
    }
}

#[cfg(test)]
mod proxy_order_tests {
    use super::*;

    /// ⭐ The exact bytes the live client resolved correctly. This is a REGRESSION LOCK on a
    /// measurement, not on a belief: run `20260808-195418-F2-STATE3` proved the big-endian encoder
    /// produced `2.0.77.10:780` from `10.77.0.2:3075`.
    #[test]
    fn client_order_is_little_endian_for_ip_and_port() {
        let req = GetActivityHostProxyRequest { activity_host_id: 0xD100_0000_0000_0A01 };
        let rsp = GetActivityHostProxyResponse::new(
            req, std::net::Ipv4Addr::new(10, 77, 0, 2), 3075);
        let b = rsp.encode_client_order();
        assert_eq!(b.len(), AH_PROXY_RESPONSE_BODY_LEN);
        assert_eq!(&b[0..8], &0xD100_0000_0000_0A01u64.to_be_bytes(), "id echo stays big-endian");
        assert_eq!(&b[8..12], &[0x02, 0x00, 0x4D, 0x0A], "ipv4 LE - the FALSIFIED order, locked so it cannot be re-adopted by accident");
        assert_eq!(&b[12..16], &[0x00, 0x00, 0x03, 0x0C], "port is an LE u16 at wire 0x0E");

        // NEGATIVE CONTROL: the old encoder must differ, or this test proves nothing.
        assert_ne!(rsp.encode(), b, "encode() and encode_client_order() must not agree");
        assert_eq!(&rsp.encode()[8..12], &[0x0A, 0x4D, 0x00, 0x02], "the exact bytes that failed");
    }
}
