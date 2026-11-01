// SPDX-License-Identifier: GPL-2.0-or-later
// Cross-check: decode d1-bap vectors with Blam Network's @blamnetwork/rsat (MIT), re-encode
// with Blam, and compare bytes. Usage: node crosscheck.cjs [vectors.txt]
// D1_TIGER is a built checkout of https://github.com/Blam-Network/tiger.
"use strict";
const fs = require("fs");
const path = require("path");
const tiger = process.env.D1_TIGER || "F:/d1/third-party/Blam-Network_tiger";
const { rsat, BitReader, BitWriter } = require(
  path.join(tiger, "packages/rsat/dist-cjs/index.js")
);

const vec = Object.fromEntries(
  fs.readFileSync(process.argv[2] || path.join(__dirname, "vectors.txt"), "utf8")
    .trim().split(/\r?\n/).map((l) => l.split(" "))
);
const B = (h) => Buffer.from(h, "hex");
const H = (b) => Buffer.from(b).toString("hex");
let pass = 0, fail = 0;
function check(name, got, want) {
  const ok = got === want;
  ok ? pass++ : fail++;
  console.log(`${ok ? "OK  " : "DIFF"} ${name}${ok ? "" : `\n      blam ${got}\n      ours ${want}`}`);
}
const replacer = (_k, v) => (typeof v === "bigint" ? "0x" + v.toString(16) : v);
const show = (o) => JSON.stringify(o, replacer);

// placeholder for a field whose type we do not model (only ever absent in these vectors)
const ABSENT = () => rsat.optional(rsat.u8());
const absentFields = (from, to) =>
  Object.fromEntries(Array.from({ length: to - from }, (_, i) => [`f${from + i}`, ABSENT()]));

// ---- character (instance 0x3431F1BA) -------------------------------------------------------
const Identity = rsat.schema(0xc0000001, {
  race: rsat.optional(rsat.i8()), // default signed bias 0x80
  gender: rsat.optional(rsat.i8()),
  class: rsat.optional(rsat.i8()),
});
const Character = rsat.schema(0xc0000002, {
  soid: rsat.optional(rsat.u64()),
  identity: rsat.optional(rsat.nested(Identity)),
  ...absentFields(2, 6), // the "0000" tail: 4 more absent top-level fields
});

// ---- {u4 count; u64 storage[10]} (0x80804621), shared by roster and account ----------------
const CharList = rsat.array(rsat.u64(), { lengthBits: 4, max: 10 });
const Roster = rsat.schema(0xa2ac2c74, {
  key: rsat.optional(rsat.u64()),
  characters: rsat.optional(CharList),
  ...absentFields(2, 4),
});
const Blob3 = rsat.schema(0xc0000003, { bits: rsat.u8({ size: 3 }) });
const Blob2 = rsat.schema(0xc0000004, { bits: rsat.u8({ size: 2 }) });
const Account = rsat.schema(0x808018c7, {
  key: rsat.optional(rsat.u64()),
  characters: rsat.optional(CharList),
  serial: rsat.optional(rsat.u32()),
  profile: rsat.optional(rsat.u64()),
  ...absentFields(4, 6),
  f6: rsat.optional(rsat.nested(Blob3)),
  ...absentFields(7, 20),
  f20: rsat.optional(rsat.nested(Blob2)),
  ...absentFields(21, 25),
});

// ---- profile (0x80801714, 18 fields); field 2 = 0x8080181A, 12 sub-fields ------------------
const Settings = rsat.schema(0x8080181a, {
  ...absentFields(0, 10),
  last_orbited_destination: rsat.optional(rsat.u32()),
  f11: ABSENT(),
});
const Profile = rsat.schema(0x80801714, {
  soid: rsat.optional(rsat.u64()),
  f1: ABSENT(),
  settings: rsat.optional(rsat.nested(Settings)),
  ...absentFields(3, 18),
});

// ---- small message bodies ------------------------------------------------------------------
const Inv503 = rsat.schema(0xc0000005, { soid: rsat.optional(rsat.u64()) });
const Result = rsat.schema(0x80801796, {
  status: rsat.i32({ size: 4, bias: 1 }),
  value: rsat.i32(), // default bias 0x80000000
});
const CreateResp = rsat.schema(0x8080185d, { result: rsat.nested(Result), soid: rsat.u64() });
const Resp206 = rsat.schema(0x80801869, { result: rsat.i32({ size: 4, bias: 1 }) });
const Req206 = rsat.schema(0x80801774, { family: rsat.i32({ size: 4, bias: 1 }), key: rsat.u64() });
const UnlockFlag = rsat.schema(0xc0000006, {
  index: rsat.optional(rsat.i16()), // default bias 0x8000
  value: rsat.optional(rsat.i32({ size: 2, bias: 1 })),
});
const UnlockArray = rsat.array(rsat.nested(UnlockFlag), { lengthBits: 6, max: 50 });

const acct = 0xd100000000000001n, chr = 0xd100000000000101n, prof = 0xd100000000000201n;

// round-trip helper: Blam decodes OUR bytes, then Blam re-encodes the decoded value
function roundTrip(name, schema, hex) {
  const v = rsat.decode(schema, B(hex));
  console.log(`     ${name} decoded: ${show(v)}`);
  check(`${name}: blam re-encode == ours`, H(rsat.encode(schema, v)), hex);
  return v;
}

console.log("== object payloads ==");
roundTrip("character", Character, vec.character);
roundTrip("character_warlock_exo_male", Character, vec.character_warlock_exo_male);
check("character from scratch", H(rsat.encode(Character, {
  soid: chr, identity: { race: 0, gender: 1, class: 0 }, f2: undefined, f3: undefined, f4: undefined, f5: undefined,
})), vec.character);
for (const n of ["roster0", "roster1", "roster3"]) roundTrip(n, Roster, vec[n]);
for (const n of ["account", "account_serial7", "account_noprof", "account_fields"]) roundTrip(n, Account, vec[n]);
roundTrip("profile", Profile, vec.profile);
const pd = roundTrip("profile_dest", Profile, vec.profile_dest);
const dest = pd.settings.last_orbited_destination;
check("u32 decode is unsigned (2468353467)", String(dest), "2468353467");
roundTrip("inv503_soid", Inv503, vec.inv503_soid);
roundTrip("create_resp", CreateResp, vec.create_resp);
roundTrip("result_ok", Result, vec.result_ok);

console.log("== server_message framing (u16 id | args | 1-bit payload [u16 len | bytes]) ==");
// ours: 8-byte class-2 header then the body; Blam's helper builds the body only
const body206 = vec.resp206.slice(16);
check("206 reply body, no span", H(rsat.encodeServerMessage(206, Resp206, { result: 0 }, null)), body206);
check("206 reply body, family-3 span",
  H(rsat.encodeServerMessage(206, Resp206, { result: 0 }, B(vec.f3_empty_roster))),
  "00ce" + vec.body206_span);
const dm = rsat.decodeServerMessage(Buffer.concat([B("00ce"), B(vec.body206_span)]), Resp206);
check("206 decode span == f3 body", H(dm.optional), vec.f3_empty_roster);
const req = rsat.decode(Req206, B("4d1000000000000010"));
console.log(`     206 request decoded: ${show(req)}`);
check("206 request family (biased +1)", String(req.family), "3");

console.log("== queuez family framing ==");
check("f3 empty roster", H(rsat.encodeQueuezFamily({ familyType: 3, rootSoid: acct, version: 0, flags: 1, objects: [
  { headerChecksum: 0xa2ac2c74, soid: acct, payload: B(vec.roster0) },
] })), vec.f3_empty_roster);
check("f3 character", H(rsat.encodeQueuezFamily({ familyType: 3, rootSoid: acct, version: 0, flags: 1, objects: [
  { headerChecksum: 0x3431f1ba, soid: chr, payload: B(vec.character) },
] })), vec.f3_character);
check("f4 baseline (account + profile)", H(rsat.encodeQueuezFamily({ familyType: 4, rootSoid: acct, version: 0, flags: 1, objects: [
  { headerChecksum: 0x64ae75bf, soid: acct, payload: B(vec.account) },
  { headerChecksum: 0x8127f15a, soid: prof, payload: B(vec.profile) },
] })), vec.f4_baseline);
check("f4 delta ping", H(rsat.encodeQueuezFamily({ familyType: 4, rootSoid: acct, version: 1, flags: 0, objects: [] })),
  vec.f4_delta_ping);
check("Blam encodeQueuezFamily DEFAULTS (version/flags omitted) vs our baseline",
  H(rsat.encodeQueuezFamily({ familyType: 3, rootSoid: acct, objects: [
    { headerChecksum: 0xa2ac2c74, soid: acct, payload: B(vec.roster0) }] })), vec.f3_empty_roster);

console.log("== unlock flag array inside 205 body (bit 141) ==");
function decodeAt(hex, bitOff, field) {
  const br = new BitReader(B(hex));
  for (let left = bitOff; left > 0; left -= Math.min(32, left)) br.readNumber(Math.min(32, left));
  const v = field.decode(br);
  return { v, end: br.bitPos };
}
const { v: flags, end } = decodeAt(vec.unlock205, 141, UnlockArray);
console.log(`     flags: ${show(flags)} (array ends at bit ${end})`);
check("unlock indices", flags.map((f) => f.index).join(","), "98,834,5015,6018");
check("unlock values", flags.map((f) => f.value).join(","), "2,2,2,2");
// re-encode: prefix bits from our body, Blam array, our tail
{
  const src = B(vec.unlock205);
  const bit = (n) => (src[n >> 3] >> (7 - (n & 7))) & 1;
  const bw = new BitWriter();
  for (let i = 0; i < 141; i++) bw.writeBit(bit(i));
  UnlockArray.encode(bw, flags);
  for (let i = end; i < src.length * 8; i++) bw.writeBit(bit(i));
  check("unlock205 re-encode", H(bw.finish()), vec.unlock205);
}

console.log("== Blam default-bias probe for narrow signed leaves ==");
const Narrow = rsat.schema(0xc0000007, { a: rsat.i32({ size: 4 }), b: rsat.i8({ size: 2 }) });
console.log(`     i32({size:4}) + i8({size:2}) of {0,0} -> ${H(rsat.encode(Narrow, { a: 0, b: 0 }))} (D1 narrow leaves want bias 1: 0001 01.. = 14)`);

console.log(`\n${pass} OK, ${fail} DIFF`);
process.exitCode = fail ? 1 : 0;
