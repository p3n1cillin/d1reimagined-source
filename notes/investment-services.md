# Investment (BAP) web services, read from the disc (2026-09-28)

Source: the service table tag `0x80a5806b` in `ps3_investment_globals_012c` (66 records of element class
`0x80801820`: u32 name hash, u16 wire id, request type, response type) and the type definitions compiled
into the EBOOT (`scripts/d1_types.py services --packages <dir> --oodle <dll>`). Wrapper = the per-service
request function in the 0x411000-0x413000 block that loads the wire id (`li rN, wire`); its callers are the
game code using that service. Meanings are CONFIRMED only where a run showed them.

The name hash is the 32-bit FNV-1 of the service name (offset basis `0x811C9DC5`, prime `0x01000193`,
multiply then xor). Names are given where a search of one to three words joined by `_` found the hash
(31 October 2026; about one false match in twenty is expected at that size, so a longer name could still
collide). For example, 903 is `vendor_sell_item`; its request `0x80801734` is `{bool @0, u64 @8, i16 @0x10}`,
`i32 @0x18`, `u8 @0x1C` (32 bytes, 121 bits), by a round trip through the client codec.

| Wire | Name hash | Wrapper | Request | Response | Meaning |
| ---: | --- | --- | --- | --- | --- |
| 104 | `0x5a146eab` |  | `0x80801811`  | `0x808018f8` {u8:4 i32} {u64?x2 {i32:5 [0x808046a3]}? {i32:5 [0x80804743]}? {i32:3 [0x80804655]}? u32?} | universe object (CONFIRMED, stand-in) |
| 105 | `0x2dcc40d2` | `0x411750` | `0x808018af` i16 | `0x80801702` {u8:4 i32} | world transition; request = i16 world id (CONFIRMED) |
| 201 | `0xafc44c8f` |  | `0x8080170b` u64 | `0x80801813` u8:4 {u64? {i32:4 [0x80804787]}? u64? {{0x8080172c}? {0x808017d6}?}? u8:2? {i32? {0x808045 | fetch account object by id (LIKELY: response = the 0x2000-byte account) |
| 202 | `0x869bab67` |  | `0x8080192b` u64 | `0x808018eb` u8:4 {u64? {u8?x3}? {i16 u8x3 i16x3 {0x80804705}x2 i16 u32 u8:2}? {i32? {0x80804764}?}? {u | fetch character by id (LIKELY: response = full character incl. 256-slot inventory, 20 equipped ids) |
| 203 | `0xd5f72313` |  | `0x8080177c` u64x2 | `0x808016d9` u8:4 {u64? {u64 u8:2}? {i32?x2 i16?}? i16? {{0x8080176b}? {0x80804629}? i16? {0x808043c6}? | item details (LIKELY: {character?, item instance} -> one item) |
| 204 | `0xd913db61` |  | `0x80801748`  | `0x808017c9` u8:4 u64 |  |
| 205 | `0xa52ae87f` |  | `0x80801874`  | `0x80801827` u8:4 {u64?x2 {i32:5 [0x808046a3]}? {i32:5 [0x80804743]}? {i32:3 [0x80804655]}? u32?} |  |
| 206 | `0x516f3c62` | `0x4118b0` | `0x80801774` u8:4 u64 | `0x80801869` u8:4 | family fetch {u4 family, u64 root} (CONFIRMED) |
| 207 | `0x40a3a7ca` |  | `0x8080182d` u64 | `0x808016ca` u8:4 {u32x8} |  |
| 303 | `0xabbef2d3` |  | `0x808016fd` u64 i16 | `0x808016ec` {u8:4 i32} |  |
| 304 | `0x3eb709c0` |  | `0x80801802` u64x2 {{i32:4 [0x8080547d]} {i32:1 [0x80804792]} {i32:9 [0x80804407]}} | `0x8080188c` u8:4 |  |
| 305 | `0x58502e05` | `0x4119f4` | `0x8080184e` u64x2 {{i32:4 [0x8080547d]} {i32:1 [0x80804792]} {i32:9 [0x80804407]}} | `0x808016ea` u8:4 |  |
| 402 | `0x3931cb8a` | `0x411ca0` | `0x80801767` {bool u64 i16} i32 | `0x80801953` {u8:4 i32} | item move with quantity (LIKELY transfer/vault: {bool, u64 instance, i16 item index} + i32 count) |
| 403 | `0xc579f488` `inventory_equip_item` |  | `0x80801942` u64 | `0x80801834` {u8:4 i32} | item action by instance id (UNKNOWN: equip?) |
| 404 | `0x9befe885` `inventory_unequip_item` | `0x411dd8` | `0x8080189a` u64 | `0x808018ab` {u8:4 i32} | item action by instance id (UNKNOWN) |
| 405 | `0x82842f7b` `inventory_transfer_item` | `0x411f08` | `0x808018ce` u8x2 u64 i16 i32 | `0x80801719` {u8:4 i32} | item move between owners/buckets (LIKELY: u8 x2 + instance + index + count) |
| 501 | `0xe906221f` `login_create_character` | `0x411360` | `0x80801860` {u8?x3} {i16 u8x3 i16x3 {i16x3}x2 i16 u32 u8:2} {{u8x17}} | `0x8080185d` {u8:4 i32} u64 | create character: class, appearance, name bytes (CONFIRMED) |
| 502 | `0xf721133e` `login_delete_character` | `0x4114b0` | `0x80801939` u64 | `0x808017f9` {u8:4 i32} | delete character (LIKELY: u64 character id) |
| 503 | `0x0c662090` `login_account` | `0x412044` | `0x80801726` {{u8x36} u64} | `0x80801825` {u8:4 i32} u64 u32x2 {u64?x2 {i32:5 [0x808046a3]}? {i32:5 [0x80804743]}? {i32:3 [0x8080465 | sign in (CONFIRMED) |
| 504 | `0x7d6146cc` `login_character` | `0x4121a0` | `0x808017ce` u64 | `0x80801845` {u8:4 i32} | select character (CONFIRMED) |
| 505 | `0x1ce11bb6` `signout_character` | `0x412a9c` | `0x808017e5`  | `0x80801886` {u8:4 i32} | leave activity / back to orbit (CONFIRMED in TEST158b) |
| 506 | `0x27b12a16` |  | `0x80801814` {i16 u8x3 i16x3 {i16x3}x2 i16 u32 u8:2} | `0x80801736` {u8:4 i32} | edit appearance (LIKELY: the appearance struct only) |
| 601 | `0x3bf631f4` `loot_pickup_item` |  | `0x80801757` u8:3 {{i32 u64} i32} | `0x808017ab` {u8:4 i32} {{i32 u64} i32} |  |
| 701 | `0x54f74ceb` `profile_set_account_profile` | `0x41258c` | `0x808016dd` {{{0x808047c9}? {0x808041c3}?}? {{0x80801705}? {0x808017f0}?}?} | `0x8080189b` {u8:4 i32} |  |
| 702 | `0xfc75eb1b` `profile_set_character_profile` | `0x4126e8` | `0x808016cf` {{{0x808016d2}? {0x808041c3}? f32?}? {{0x808018db}? {0x80801927}? {0x808018fe}? u32? {0x80 | `0x8080194d` {u8:4 i32} | activity/character history update (LIKELY: same record types as the self character +0x18e8) |
| 801 | `0xf9d0480f` | `0x412e54` | `0x8080171f` u64 u8 | `0x80801720` {u8:4 i32} | item flag (LIKELY lock/track: instance + u8) |
| 802 | `0x9997cc20` | `0x412f88` | `0x8080174d` u64 u8 | `0x8080183d` {u8:4 i32} | item flag (LIKELY lock/track: instance + u8) |
| 803 | `0x0484d2fb` |  | `0x8080184a` u64 {{u8x2} u8} u8:3 | `0x80801792` {u8:4 i32} |  |
| 901 | `0x4d0ea80c` `vendor_buy_item` | `0x412be0` | `0x80801777` u8 i16 u64? | `0x80801888` {u8:4 i32} | vendor purchase (LIKELY: u8 vendor, i16 item index, u64?) |
| 902 | `0x890c0597` |  | `0x8080180d` {bool u64 i16} i16 | `0x808016ef` {u8:4 i32} | item action with {bool,u64,i16} + i16 (UNKNOWN: refund/dismantle?) |
| 903 | `0x5610f2e4` `vendor_sell_item` | `0x412d18` | `0x80801734` {bool u64 i16} i32 u8 | `0x8080183a` {u8:4 i32} | item action with {bool,u64,i16} + i32 + u8 (UNKNOWN) |
| 1101 | `0x74bce44f` |  | `0x80801718` f32 | `0x8080188d` u8:4 |  |
| 1102 | `0xd4c4c1d7` |  | `0x808018a4` i32 | `0x8080187e` u8:4 |  |
| 1103 | `0xf15cb19c` |  | `0x80801799` u8:3 | `0x8080175c` u8:4 i32 |  |
| 1201 | `0x8c657e37` | `0x41121c` | `0x80801742` u64? | `0x80801817` {u8:4 i32} |  |
| 1202 | `0xadd20cb8` |  | `0x8080172f` i16 bool i16x3 i32 | `0x8080186f` {u8:4 i32} |  |
| 1203 | `0xb29bf9f2` |  | `0x808016e9` i16 i32 | `0x808016c1` {u8:4 i32} |  |
| 1205 | `0x3b43ce9d` |  | `0x808017b8` {i32?x2 i16? {bool? {0x80804768}?}?x2 {i32:3 [0x80804575]}? bool?x2 i32? u32?} | `0x80801859` {u8:4 i32} |  |
| 1206 | `0x64ef3beb` |  | `0x808017f4` i16 i32 | `0x8080174b` {u8:4 i32} |  |
| 1207 | `0x7849c31b` |  | `0x8080175d` i16 u8:2 | `0x80801895` {u8:4 i32} |  |
| 1208 | `0x7e52ab1f` |  | `0x808017eb`  | `0x8080175b` {u8:4 i32} |  |
| 1209 | `0x7eb2b211` |  | `0x80801815` u64? | `0x8080190e` {u8:4 i32} |  |
| 1210 | `0x9f6b0a8f` |  | `0x808016e6`  | `0x80801723` {u8:4 i32} |  |
| 1211 | `0xb095c2f9` |  | `0x808017d2`  | `0x80801958` {u8:4 i32} |  |
| 1212 | `0xa4fdb1f6` |  | `0x80801880` i16 i32 | `0x80801775` {u8:4 i32} |  |
| 1213 | `0x62cf7cfd` |  | `0x8080170a` i16 bool i16x3 | `0x808016cd` {u8:4 i32} |  |
| 1214 | `0x0624757c` |  | `0x80801709` i16 | `0x8080180f` u8:4 |  |
| 1215 | `0xb0974caa` |  | `0x808017ad` {{u8x129}} | `0x808017bc` {u8:4 i32} |  |
| 1216 | `0xf53b73e3` |  | `0x808018a9` i16 i32 | `0x808017d7` {u8:4 i32} |  |
| 1217 | `0xf2db9698` |  | `0x80801878` {i16x27} {i16x5} | `0x808018df` {u8:4 i32} | UI/tracking list (LIKELY: 27 + 5 i16, as in the account object) |
| 1218 | `0x5d150aa3` |  | `0x8080181b` u64x2 {{i32:4 [0x8080547d]} {i32:1 [0x80804792]} {i32:9 [0x80804407]}} | `0x80801926` u8:4 |  |
| 1219 | `0xc1a6ca2c` |  | `0x80801960` u8 | `0x808017cf` {u8:4 i32} |  |
| 1220 | `0x55c71485` |  | `0x8080193e`  | `0x808018f7` {u8:4 i32} |  |
| 1221 | `0xd10410dd` |  | `0x8080188e` u8:4 | `0x8080182c` {u8:4 i32} |  |
| 1222 | `0x4bf715e3` |  | `0x808018f0` i16 i32 | `0x808018bc` {u8:4 i32} |  |
| 1223 | `0x9e8b6a6c` |  | `0x808017bf` u8 u64 i16 i32 u8 | `0x80801940` {u8:4 i32} |  |
| 1224 | `0xca10be1a` |  | `0x80801901` u8x3 | `0x80801884` {u8:4 i32} |  |
| 1225 | `0xc9c608ec` |  | `0x8080187b` bool i32 | `0x80801835` u8:4 |  |
| 1226 | `0x2f122feb` |  | `0x808018e0` u8 i32 | `0x8080172e` u8:4 |  |
| 1227 | `0xfe6ce91d` |  | `0x80801858` u8x3 | `0x808016d6` {u8:4 i32} |  |
| 1301 | `0xdddb7b36` |  | `0x80801746` i16x2 | `0x808016e7` u8:4 |  |
| 1302 | `0x4d7ef259` |  | `0x8080173f`  | `0x8080185b` u8:4 |  |
| 1303 | `0x5ed31b0e` |  | `0x8080186e` i16 u8:2 boolx2 | `0x808017aa` u8:4 {i32:9 [u8x500]} u64? i16 u32 boolx2 | message/mail blobs (LIKELY: response carries up to 9 x 500-byte blobs) |
| 1304 | `0x1a31e889` |  | `0x8080195e` u64x2 i16x2 | `0x8080193d` u8:4 boolx3 |  |
| 1401 | `0x33784d64` |  | `0x80801703`  | `0x8080194a` {u8:4 i32} |  |
| 1501 | `0xfe4e505f` | `0x412428` | `0x80801818` u64 {u32x10} | `0x808018a8` u8:4 |  |
