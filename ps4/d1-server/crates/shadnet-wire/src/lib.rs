// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-FileCopyrightText: Copyright 2019-2026 rpcsn Project
// SPDX-FileCopyrightText: Copyright 2026 shadNet Project / shadPS4 Emulator Project
// SPDX-License-Identifier: GPL-2.0-or-later
//
// Derived from shadPS4 (GPL-2.0): protocol constants, packet framing and enum
// discriminants follow src/shadnet/client.{h,cpp}. See ATTRIBUTION.md.
//! ShadNet packet framing.
//!
//! Mirrors shadPS4's `src/shadnet/client.{h,cpp}`. **These constants and enums must stay in lockstep
//! with that file** - the client and this server have no negotiation beyond a protocol version, so a
//! drifted discriminant is a silent wire break.
//!
//! ## Header - 15 bytes, little-endian throughout
//!
//! ```text
//! offset  size  field
//!   0      1    PacketType   (0 Request, 1 Reply, 2 Notification, 3 ServerInfo)
//!   1      2    CommandType
//!   3      4    total length, INCLUDING these 15 header bytes
//!   7      8    packet id - echoed back on the matching reply
//!  15      ..   payload
//! ```
//!
//! ## Payload
//!
//! * **Request** - `u32` LE blob length, then protobuf bytes.
//! * **Reply** - one [`ErrorType`] byte, then `u32` LE blob length, then protobuf bytes.
//!
//! One sharp edge worth stating: shadPS4 rejects a `Login` reply whose proto blob is **empty** as

//! malformed, so a successful `LoginReply` must carry at least one populated field.

use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub const HEADER_SIZE: usize = 15;
pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_PACKET_SIZE: u32 = 0x0080_0000; // 8 MiB
pub const DEFAULT_PORT: u16 = 31313;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PacketType {
    Request = 0,
    Reply = 1,
    Notification = 2,
    ServerInfo = 3,
}

impl PacketType {
    pub fn from_u8(v: u8) -> Option<Self> {
        Some(match v {
            0 => Self::Request,
            1 => Self::Reply,
            2 => Self::Notification,
            3 => Self::ServerInfo,
            _ => return None,
        })
    }
}

macro_rules! commands {
    ($($name:ident = $val:expr),* $(,)?) => {
        /// Every request the client can send. Discriminants come from `CommandType` in `client.h`.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[repr(u16)]
        pub enum Command { $($name = $val),* }

        impl Command {
            pub fn from_u16(v: u16) -> Option<Self> {
                Some(match v { $($val => Self::$name,)* _ => return None })
            }
            pub fn as_str(self) -> &'static str {
                match self { $(Self::$name => stringify!($name)),* }
            }
            /// Every command the client defines, in discriminant order. The controls check this
            /// against shadPS4's own `enum class CommandType` rather than against itself.
            pub const ALL: &'static [(&'static str, u16)] = &[$((stringify!($name), $val)),*];
        }
    };
}

commands! {
    Login = 0,
    Terminate = 1,
    Create = 2,
    Delete = 3,
    SendToken = 4,
    SendResetToken = 5,
    ResetPassword = 6,
    ResetState = 7,
    AddFriend = 8,
    RemoveFriend = 9,
    AddBlock = 10,
    RemoveBlock = 11,
    GetServerFeatures = 12,
    SetClientVersion = 13,
    GetBoardInfos = 30,
    RecordScore = 31,
    RecordScoreData = 32,
    GetScoreData = 33,
    GetScoreRange = 34,
    GetScoreFriends = 35,
    GetScoreNpid = 36,
    GetScoreAccountId = 37,
    GetScoreGameDataByAccId = 38,
    GetToken = 39,
    SetAppearOffline = 40,
    LookupOnlineId = 41,
    ContextStart = 100,
    CreateRoom = 101,
    JoinRoom = 102,
    LeaveRoom = 103,
    SearchRoom = 104,
    RequestSignalingInfos = 105,
    ContextStop = 106,
    SetUserInfo = 107,
    SetRoomDataInternal = 108,
    SetRoomDataExternal = 109,
    KickoutRoomMember = 110,
    GetWorldInfoList = 111,
    GetRoomDataExternalList = 112,
    GetUserInfoList = 113,
    GetRoomMemberDataExternalList = 114,
    SendRoomMessage = 115,
    TusSetData = 201,
    TusGetData = 202,
    TusSetMultiSlotVariable = 203,
    TusGetMultiSlotVariable = 204,
    TusAddAndGetVariable = 205,
    TusGetMultiSlotDataStatus = 206,
    TusGetMultiUserDataStatus = 207,
    TusGetFriendsDataStatus = 208,
    TusDeleteMultiSlotData = 209,
    TusGetMultiUserVariable = 210,
    TusTryAndSetVariable = 211,
    TusGetFriendsVariable = 212,
    TusDeleteMultiSlotVariable = 213,
    TssGetData = 214,
    UnlockTrophy = 301,
    SyncTrophies = 302,
}

impl std::fmt::Display for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Server → client notifications (unsolicited; `packet_id` is unused).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum Notification {
    FriendQuery = 5,
    FriendNew = 6,
    FriendLost = 7,
    FriendStatus = 8,
    RoomEvent = 10,
    RoomMessage = 11,
    WebApiPushEvent = 17,
}

impl Notification {
    pub const ALL: &'static [(&'static str, u16)] = &[
        ("FriendQuery", 5),
        ("FriendNew", 6),
        ("FriendLost", 7),
        ("FriendStatus", 8),
        ("RoomEvent", 10),
        ("RoomMessage", 11),
        ("WebApiPushEvent", 17),
    ];
}

impl PacketType {
    pub const ALL: &'static [(&'static str, u8)] = &[
        ("Request", 0),
        ("Reply", 1),
        ("Notification", 2),
        ("ServerInfo", 3),
    ];
}

/// Orbis room-event ids carried in `NotifyRoomEvent.event`.
///
/// These are **Sony's** ids, quoted from `np_types2.h`, not ours. ⚠ shadPS4's `HandleRoomEvent`
/// implements only four of the eight: `ROOM_DESTROYED`, `ROOM_OWNER_CHANGED`,
/// `UPDATED_ROOM_MEMBER_DATA_INTERNAL` and `UPDATED_SIGNALING_OPT_PARAM` fall to
/// `default: LOG_WARNING("unhandled room event"); return;` - **dropped, no callback fires.**
/// [`RoomEvent::handled_by_shadps4`] records which, so the server can refuse to emit an event the
/// client will silently discard instead of pretending it was delivered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum RoomEvent {
    MemberJoined = 0x1101,
    MemberLeft = 0x1102,
    KickedOut = 0x1103,
    RoomDestroyed = 0x1104,
    RoomOwnerChanged = 0x1105,
    UpdatedRoomDataInternal = 0x1106,
    UpdatedRoomMemberDataInternal = 0x1107,
    UpdatedSignalingOptParam = 0x1108,
}

impl RoomEvent {
    pub const ALL: &'static [(&'static str, u16)] = &[
        ("MEMBER_JOINED", 0x1101),
        ("MEMBER_LEFT", 0x1102),
        ("KICKEDOUT", 0x1103),
        ("ROOM_DESTROYED", 0x1104),
        ("ROOM_OWNER_CHANGED", 0x1105),
        ("UPDATED_ROOM_DATA_INTERNAL", 0x1106),
        ("UPDATED_ROOM_MEMBER_DATA_INTERNAL", 0x1107),
        ("UPDATED_SIGNALING_OPT_PARAM", 0x1108),
    ];

    /// True when shadPS4's `HandleRoomEvent` has a `case` for this event.
    pub fn handled_by_shadps4(self) -> bool {
        matches!(
            self,
            Self::MemberJoined | Self::MemberLeft | Self::KickedOut | Self::UpdatedRoomDataInternal
        )
    }
}

/// `OrbisNpMatching2EventCause`, quoted from `np_types2.h`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EventCause {
    LeaveAction = 1,
    KickoutAction = 2,
    GrantOwnerAction = 3,
    ServerOperation = 4,
    MemberDisappeared = 5,
    ServerInternal = 6,
    ConnectionError = 7,
    NpSignedOut = 8,
    SystemError = 9,
    ContextError = 10,
    ContextAction = 11,
}

impl EventCause {
    pub const ALL: &'static [(&'static str, u8)] = &[
        ("LEAVE_ACTION", 1),
        ("KICKOUT_ACTION", 2),
        ("GRANT_OWNER_ACTION", 3),
        ("SERVER_OPERATION", 4),
        ("MEMBER_DISAPPEARED", 5),
        ("SERVER_INTERNAL", 6),
        ("CONNECTION_ERROR", 7),
        ("NP_SIGNED_OUT", 8),
        ("SYSTEM_ERROR", 9),
        ("CONTEXT_ERROR", 10),
        ("CONTEXT_ACTION", 11),
    ];
}

macro_rules! errors {
    ($($name:ident = $val:expr),* $(,)?) => {
        /// First byte of every reply payload.
        ///
        /// ⚠ The client hands this byte to the title **as its Orbis error code**:
        /// `error_code = (error == NoError) ? 0 : (s32)error` (`DispatchRequestComplete`,
        /// `np_matching2_mm.cpp`). So Destiny sees `33`, not an `ORBIS_NP_MATCHING2_ERROR_*` value.
        /// That is a client-side defect we inherit; it does not make our choice of error free -
        /// the emulator's own log line names it, which is how a gate gets named.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
        #[repr(u8)]
        pub enum ErrorType { $($name = $val),* }

        impl ErrorType {
            pub fn from_u8(v: u8) -> Option<Self> {
                Some(match v { $($val => Self::$name,)* _ => return None })
            }
            pub fn as_str(self) -> &'static str {
                match self { $(Self::$name => stringify!($name)),* }
            }
            /// Every value the client defines, in discriminant order. Used by the controls to
            /// cross-check against shadPS4's own `enum class ErrorType`.
            pub const ALL: &'static [(&'static str, u8)] = &[$((stringify!($name), $val)),*];
        }

        impl std::fmt::Display for ErrorType {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}

errors! {
    NoError = 0,
    Malformed = 1,
    Invalid = 2,
    InvalidInput = 3,
    TooSoon = 4,
    LoginError = 5,
    LoginAlreadyLoggedIn = 6,
    LoginInvalidUsername = 7,
    LoginInvalidPassword = 8,
    LoginInvalidToken = 9,
    CreationError = 10,
    CreationExistingUsername = 11,
    CreationBannedEmailProvider = 12,
    CreationExistingEmail = 13,
    RoomMissing = 14,
    RoomAlreadyJoined = 15,
    RoomFull = 16,
    RoomPasswordMismatch = 17,
    RoomPasswordMissing = 18,
    RoomGroupNoJoinLabel = 19,
    RoomGroupFull = 20,
    RoomGroupJoinLabelNotFound = 21,
    RoomGroupMaxSlotMismatch = 22,
    Unauthorized = 23,
    DbFail = 24,
    EmailFail = 25,
    NotFound = 26,
    Blocked = 27,
    AlreadyFriend = 28,
    ScoreNotBest = 29,
    ScoreInvalid = 30,
    ScoreHasData = 31,
    CondFail = 32,
    Unsupported = 33,
    TooLarge = 34,
}

#[derive(Debug, Clone)]
pub struct Packet {
    pub packet_type: PacketType,
    pub command: u16,
    pub packet_id: u64,
    /// Payload with the 15-byte header already stripped.
    pub payload: Vec<u8>,
}

impl Packet {
    /// The protobuf blob inside a **request** payload (`u32` length prefix, then bytes).
    pub fn request_blob(&self) -> &[u8] {
        extract_blob(&self.payload, 0).unwrap_or(&[])
    }

    pub fn command_name(&self) -> String {
        Command::from_u16(self.command)
            .map(|c| c.as_str().to_string())
            .unwrap_or_else(|| format!("UNKNOWN({})", self.command))
    }
}

/// Read a `u32`-LE length-prefixed blob starting at `pos`. `None` if the frame is short.
pub fn extract_blob(buf: &[u8], pos: usize) -> Option<&[u8]> {
    let len_end = pos.checked_add(4)?;
    if len_end > buf.len() {
        return None;
    }
    let len = u32::from_le_bytes(buf[pos..len_end].try_into().ok()?) as usize;
    let end = len_end.checked_add(len)?;
    if end > buf.len() {
        return None;
    }
    Some(&buf[len_end..end])
}

fn framed(packet_type: PacketType, command: u16, packet_id: u64, payload: &[u8]) -> Vec<u8> {
    let total = (HEADER_SIZE + payload.len()) as u32;
    let mut out = Vec::with_capacity(total as usize);
    out.push(packet_type as u8);
    out.extend_from_slice(&command.to_le_bytes());
    out.extend_from_slice(&total.to_le_bytes());
    out.extend_from_slice(&packet_id.to_le_bytes());
    out.extend_from_slice(payload);
    out
}

fn length_prefixed(proto: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + proto.len());
    out.extend_from_slice(&(proto.len() as u32).to_le_bytes());
    out.extend_from_slice(proto);
    out
}

/// Build a reply: `[ErrorType][u32 len][proto]`.
pub fn encode_reply(command: u16, packet_id: u64, error: ErrorType, proto: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(1 + 4 + proto.len());
    payload.push(error as u8);
    payload.extend_from_slice(&length_prefixed(proto));
    framed(PacketType::Reply, command, packet_id, &payload)
}

/// Build a notification. These carry no error byte - the proto blob starts immediately.
pub fn encode_notification(kind: Notification, proto: &[u8]) -> Vec<u8> {
    framed(PacketType::Notification, kind as u16, 0, &length_prefixed(proto))
}

/// Build the `ServerInfo` handshake - **the server must send this first, before the client says
/// anything.** shadPS4 blocks on it right after `connect()` and drops the socket if it doesn't
/// arrive within `SHAD_CONNECT_TIMEOUT_MS`, so a server that waits to be spoken to never gets a
/// `Login` at all.
///
/// Unlike every other packet the payload is **not** length-prefixed and **not** protobuf: it is a
/// bare `u32` LE protocol version, which the client compares against its own and disconnects on
/// mismatch.
pub fn encode_server_info(protocol_version: u32) -> Vec<u8> {
    framed(
        PacketType::ServerInfo,
        0,
        0,
        &protocol_version.to_le_bytes(),
    )
}

#[derive(Debug)]
pub enum ReadError {
    /// Peer closed cleanly between packets.
    Eof,
    /// `total` outside `[HEADER_SIZE, MAX_PACKET_SIZE]` - the client treats this as fatal too.
    Corrupt(u32),
    Io(std::io::Error),
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Eof => write!(f, "peer closed"),
            Self::Corrupt(n) => write!(f, "corrupt packet (total_size={n})"),
            Self::Io(e) => write!(f, "io: {e}"),
        }
    }
}

impl std::error::Error for ReadError {}

/// Read exactly one packet. Returns [`ReadError::Eof`] on a clean close at a frame boundary.
pub async fn read_packet<R>(reader: &mut R) -> Result<Packet, ReadError>
where
    R: AsyncReadExt + Unpin,
{
    let mut header = [0u8; HEADER_SIZE];
    match reader.read_exact(&mut header).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Err(ReadError::Eof),
        Err(e) => return Err(ReadError::Io(e)),
    }

    let packet_type = PacketType::from_u8(header[0]);
    let command = u16::from_le_bytes([header[1], header[2]]);
    let total = u32::from_le_bytes([header[3], header[4], header[5], header[6]]);
    let packet_id = u64::from_le_bytes(header[7..15].try_into().unwrap());

    if (total as usize) < HEADER_SIZE || total > MAX_PACKET_SIZE {
        return Err(ReadError::Corrupt(total));
    }

    let payload_len = total as usize - HEADER_SIZE;
    let mut payload = vec![0u8; payload_len];
    if payload_len > 0 {
        reader
            .read_exact(&mut payload)
            .await
            .map_err(ReadError::Io)?;
    }

    Ok(Packet {
        // An unknown discriminant is still worth surfacing, so fall back to Request rather than
        // dropping the frame - the handler logs whatever it can't name.
        packet_type: packet_type.unwrap_or(PacketType::Request),
        command,
        packet_id,
        payload,
    })
}

pub async fn write_all<W>(writer: &mut W, bytes: &[u8]) -> std::io::Result<()>
where
    W: AsyncWriteExt + Unpin,
{
    writer.write_all(bytes).await?;
    writer.flush().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_is_fifteen_bytes() {
        // If this ever changes, shadPS4's SHAD_HEADER_SIZE changed and every frame is wrong.
        assert_eq!(HEADER_SIZE, 15);
    }

    #[test]
    fn reply_layout_matches_client_expectations() {
        let proto = [0xAAu8, 0xBB];
        let pkt = encode_reply(Command::Login as u16, 0x0102_0304_0506_0708, ErrorType::NoError, &proto);
        assert_eq!(pkt[0], PacketType::Reply as u8);
        assert_eq!(u16::from_le_bytes([pkt[1], pkt[2]]), 0);
        // total length counts the header
        assert_eq!(
            u32::from_le_bytes(pkt[3..7].try_into().unwrap()) as usize,
            pkt.len()
        );
        assert_eq!(u64::from_le_bytes(pkt[7..15].try_into().unwrap()), 0x0102_0304_0506_0708);
        assert_eq!(pkt[HEADER_SIZE], ErrorType::NoError as u8);
        // then a u32 length prefix, then the proto
        assert_eq!(extract_blob(&pkt[HEADER_SIZE..], 1).unwrap(), &proto);
    }

    #[test]
    fn server_info_is_a_bare_u32_version() {
        let pkt = encode_server_info(PROTOCOL_VERSION);
        assert_eq!(pkt.len(), HEADER_SIZE + 4);
        assert_eq!(pkt[0], PacketType::ServerInfo as u8);
        assert_eq!(
            u32::from_le_bytes(pkt[3..7].try_into().unwrap()) as usize,
            pkt.len()
        );
        // payload is the version itself, with no length prefix in front of it
        assert_eq!(
            u32::from_le_bytes(pkt[HEADER_SIZE..HEADER_SIZE + 4].try_into().unwrap()),
            PROTOCOL_VERSION
        );
    }

    #[test]
    fn extract_blob_rejects_short_frames() {
        assert!(extract_blob(&[1, 0, 0], 0).is_none());
        assert!(extract_blob(&[4, 0, 0, 0, 1, 2], 0).is_none());
        assert_eq!(extract_blob(&[2, 0, 0, 0, 7, 8], 0).unwrap(), &[7, 8]);
    }

    #[test]
    fn command_names_round_trip() {
        for (v, name) in [(0u16, "Login"), (39, "GetToken"), (12, "GetServerFeatures"), (101, "CreateRoom")] {
            assert_eq!(Command::from_u16(v).unwrap().as_str(), name);
        }
        assert!(Command::from_u16(9999).is_none());
    }
}
