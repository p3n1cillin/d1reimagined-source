// SPDX-FileCopyrightText: Copyright 2019-2026 rpcs3 Project
// SPDX-FileCopyrightText: Copyright 2024-2026 shadPS4 Emulator Project
// SPDX-License-Identifier: GPL-2.0-or-later
//
// VENDORED VERBATIM EXCERPT - do not edit by hand.
//
// These enum blocks are copied byte-for-byte out of the shadPS4 emulator so that this repository
// can check its own Rust enums against the CLIENT'S OWN CONSTANTS rather than against numbers we
// typed in ourselves. See `crates/shadnet-server/src/controls.rs`.
//
//   src/shadnet/client.h
//     sha256 987e69ee45183a1403caaa964fcde72fc3f169ffe58d854d31bbebd019e87639
//   src/core/libraries/np/np_types2.h
//     sha256 31e801dff01898ceaf3fbc079c9229231f204ab459ad327cc602364e3d30580d
//
// Upstream: https://github.com/shadps4-emu/shadPS4
// Taken from d1reimagined-shadps4 (branch d1) at commit
//   34c579fee98c5fec3ce0910d3d939213e702dec1
// (first from the local working tree $HOME/dev/shadPS4-d1trace at 1a51a41696a17a9a3b67ad6658dda547ddca4a98)
// (`src/shadnet/shadnet.proto` in that same tree hashes to the sha256 recorded in
//  ATTRIBUTION.md for our vendored `proto/shadnet.proto`, i.e. the schema has not moved.)
//
// The controls re-derive both sha256s WHEN the shadPS4 tree is present (SHADPS4_SRC, default
// $HOME/dev/shadPS4-d1trace) and additionally assert that every block below occurs
// VERBATIM in the upstream file. When the tree is absent those checks report SKIPPED - they are
// never silently counted as passes.

// ── from src/shadnet/client.h ────────────────────────────────────────────────────────────────

enum class PacketType : u8 {
    Request = 0,
    Reply = 1,
    Notification = 2,
    ServerInfo = 3,
};

enum class CommandType : u16 {
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
    // Matchmaking
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
    // Title User Storage (TUS)
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
    // Trophies
    UnlockTrophy = 301,
    SyncTrophies = 302,
};

enum class NotificationType : u16 {
    FriendQuery = 5,
    FriendNew = 6,
    FriendLost = 7,
    FriendStatus = 8,
    RoomEvent = 10,
    RoomMessage = 11,
    WebApiPushEvent = 17, // Generic NP WebApi push event
};

enum class ErrorType : uint8_t {
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
    TooLarge = 34, // TSS file on the server exceeds the slot's documented maximum
};

// ── from src/core/libraries/np/np_types2.h ───────────────────────────────────────────────────

enum OrbisNpMatching2Event : u16 {
    ORBIS_NP_MATCHING2_REQUEST_EVENT_GET_WORLD_INFO_LIST = 0x0002,
    ORBIS_NP_MATCHING2_REQUEST_EVENT_GET_ROOM_MEMBER_DATA_EXTERNAL_LIST = 0x0003,
    ORBIS_NP_MATCHING2_REQUEST_EVENT_SET_ROOM_DATA_EXTERNAL = 0x0004,
    ORBIS_NP_MATCHING2_REQUEST_EVENT_GET_ROOM_DATA_EXTERNAL_LIST = 0x0005,
    ORBIS_NP_MATCHING2_REQUEST_EVENT_SET_USER_INFO = 0x0007,
    ORBIS_NP_MATCHING2_REQUEST_EVENT_GET_USER_INFO_LIST = 0x0008,
    ORBIS_NP_MATCHING2_REQUEST_EVENT_CREATE_JOIN_ROOM = 0x0101,
    ORBIS_NP_MATCHING2_REQUEST_EVENT_JOIN_ROOM = 0x0102,
    ORBIS_NP_MATCHING2_REQUEST_EVENT_LEAVE_ROOM = 0x0103,
    ORBIS_NP_MATCHING2_REQUEST_EVENT_KICKOUT_ROOM_MEMBER = 0x0104,
    ORBIS_NP_MATCHING2_REQUEST_EVENT_GRANT_ROOM_OWNER = 0x0105,
    ORBIS_NP_MATCHING2_REQUEST_EVENT_SEARCH_ROOM = 0x0106,
    ORBIS_NP_MATCHING2_REQUEST_EVENT_SEND_ROOM_MESSAGE = 0x0108,
    ORBIS_NP_MATCHING2_REQUEST_EVENT_SET_ROOM_DATA_INTERNAL = 0x0109,
    ORBIS_NP_MATCHING2_REQUEST_EVENT_GET_ROOM_DATA_INTERNAL = 0x010A,
    ORBIS_NP_MATCHING2_REQUEST_EVENT_JOIN_LOBBY = 0x0201,
    ORBIS_NP_MATCHING2_REQUEST_EVENT_SIGNALING_GET_PING_INFO = 0x0E01,
    ORBIS_NP_MATCHING2_ROOM_EVENT_MEMBER_JOINED = 0x1101,
    ORBIS_NP_MATCHING2_ROOM_EVENT_MEMBER_LEFT = 0x1102,
    ORBIS_NP_MATCHING2_ROOM_EVENT_KICKEDOUT = 0x1103,
    ORBIS_NP_MATCHING2_ROOM_EVENT_ROOM_DESTROYED = 0x1104,
    ORBIS_NP_MATCHING2_ROOM_EVENT_ROOM_OWNER_CHANGED = 0x1105,
    ORBIS_NP_MATCHING2_ROOM_EVENT_UPDATED_ROOM_DATA_INTERNAL = 0x1106,
    ORBIS_NP_MATCHING2_ROOM_EVENT_UPDATED_ROOM_MEMBER_DATA_INTERNAL = 0x1107,
    ORBIS_NP_MATCHING2_ROOM_EVENT_UPDATED_SIGNALING_OPT_PARAM = 0x1108,
    ORBIS_NP_MATCHING2_SIGNALING_EVENT_DEAD = 0x5101,
    ORBIS_NP_MATCHING2_SIGNALING_EVENT_ESTABLISHED = 0x5102,
    ORBIS_NP_MATCHING2_SIGNALING_EVENT_NETINFO_ERROR = 0x5103,
    ORBIS_NP_MATCHING2_CONTEXT_EVENT_START_OVER = 0x6F01,
    ORBIS_NP_MATCHING2_CONTEXT_EVENT_STARTED = 0x6F02,
    ORBIS_NP_MATCHING2_CONTEXT_EVENT_STOPPED = 0x6F03,
    ORBIS_NP_MATCHING2_REQUEST_EVENT_CREATE_JOIN_ROOM_A = 0x7101,
    ORBIS_NP_MATCHING2_REQUEST_EVENT_JOIN_ROOM_A = 0x7102,
    ORBIS_NP_MATCHING2_REQUEST_EVENT_GET_ROOM_MEMBER_DATA_EXTERNAL_LIST_A = 0x7003,
    ORBIS_NP_MATCHING2_REQUEST_EVENT_GET_ROOM_DATA_EXTERNAL_LIST_A = 0x7005,
    ORBIS_NP_MATCHING2_REQUEST_EVENT_SEARCH_ROOM_A = 0x7106,
    ORBIS_NP_MATCHING2_ROOM_EVENT_MEMBER_JOINED_A = 0x8101,
    ORBIS_NP_MATCHING2_ROOM_EVENT_MEMBER_LEFT_A = 0x8102,
    ORBIS_NP_MATCHING2_ROOM_EVENT_UPDATED_ROOM_MEMBER_DATA_INTERNAL_A = 0x8107,
    ORBIS_NP_MATCHING2_ROOM_MSG_EVENT_MESSAGE = 0x2102,
    ORBIS_NP_MATCHING2_ROOM_MSG_EVENT_MESSAGE_A = 0x9102,
};

enum OrbisNpMatching2EventCause : u8 {
    ORBIS_NP_MATCHING2_EVENT_CAUSE_LEAVE_ACTION = 1,
    ORBIS_NP_MATCHING2_EVENT_CAUSE_KICKOUT_ACTION = 2,
    ORBIS_NP_MATCHING2_EVENT_CAUSE_GRANT_OWNER_ACTION = 3,
    ORBIS_NP_MATCHING2_EVENT_CAUSE_SERVER_OPERATION = 4,
    ORBIS_NP_MATCHING2_EVENT_CAUSE_MEMBER_DISAPPEARED = 5,
    ORBIS_NP_MATCHING2_EVENT_CAUSE_SERVER_INTERNAL = 6,
    ORBIS_NP_MATCHING2_EVENT_CAUSE_CONNECTION_ERROR = 7,
    ORBIS_NP_MATCHING2_EVENT_CAUSE_NP_SIGNED_OUT = 8,
    ORBIS_NP_MATCHING2_EVENT_CAUSE_SYSTEM_ERROR = 9,
    ORBIS_NP_MATCHING2_EVENT_CAUSE_CONTEXT_ERROR = 10,
    ORBIS_NP_MATCHING2_EVENT_CAUSE_CONTEXT_ACTION = 11,
};
