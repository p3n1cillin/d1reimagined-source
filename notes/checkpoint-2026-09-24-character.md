# PS3 character creation checkpoint (2026-09-24)

This is the current handoff for the local RPCS3 Destiny test. The game signs in through
the loopback services, passes first-run screen bounds/brightness/subtitles, and reaches
class and appearance selection. The Xbox One controller works as RPCS3 Player 1
`XInput Pad #1` (Xbox A = PlayStation Cross). The user selected Titan, Exo male. Pressing
Finish sends investment web service 501, but in comparisons 050-054 the game remained
on the appearance screen with a spinning cursor. No playable character or gameplay has
been reached. Rendering is severely rainbow-distorted; this predated the shader-mode
change and is a separate issue.

## Local instance and service stack

- RPCS3 copy: `<PS3_ROOT>\RPCS3_TEST048_SETUP\rpcs3.exe`, game executable
  `<PS3_ROOT>\01_WORKING\ps3_destiny\PS3_GAME\USRDIR\EBOOT.BIN`.
- `<PS3_ROOT>\RPCS3_TEST048_SETUP\config\config.yml` uses
  `Shader Mode: Async Recompiler (multi-threaded)`. Shader Interpreter had stalled at
  198/6650 variants. The previous config is backed up as
  `config.shader-interpreter.2026-09-24.yml.bak`.
- Private runtime material (CA, schemas, logs, game captures) is under
  `<PS3_ROOT>\SIGNON_LOCAL`, outside Git. Do not commit it.
- Services needed for a fresh sign-in: `<PS3_ROOT>\RPCN_LOCAL\rpcn.exe`,
  `server/dns_sinkhole.py --destiny-hosts --log <PS3_ROOT>\SIGNON_LOCAL\logs\...`,
  `server/stun_stub.py`, `server/signon_probe.py --mode auth3 --destiny-hosts
  --manifest-url http://content.local.test/manifest --manifest-packages
  <PS3_ROOT>\01_WORKING\ps3_destiny\PS3_GAME\USRDIR\packages`,
  `server/lobby_probe.py --timeout 60`, and `server/bap_server.py --ws-signin
  --schema <PS3_ROOT>\SIGNON_LOCAL\investment_schema_character.json`.
  All are loopback-only. Check listeners/processes before retrying: an earlier set of
  DNS, STUN and SignOn helpers had silently exited, causing the generic “Destiny
  servers are not available” screen *before* setup. They were restarted on this turn.
- Current test 054 BAP log:
  `<PS3_ROOT>\SIGNON_LOCAL\logs\TEST054_SELF_CHARACTER-bap.log`.
  DNS/STUN restart logs have the `TEST053_RESTART-` prefix. The SignOn helper currently
  runs in a direct terminal session, so restart it if that session closes.
- The user authorized pushing work to the private
  `https://github.com/p3n1cillin/d1reimagined` repository. `gh auth status` showed the
  `p3n1cillin` account active with `repo` scope. Current changes still need committing
  and pushing once the checkpoint is documented and verified.

## Comparisons 049-054

| Test | Change | Observed result |
| --- | --- | --- |
| 049 | `server/lobby_probe.py` echoes a four-zero-byte post-auth keepalive | Lobby remained up across repeated ~40 s keepalives, beyond the earlier caterpillar disconnect. |
| 050 | Service 501 returns result 0 and a nonzero character ID; subsequent service 206 select-character family includes the ID | Service 501 then 206 arrived; Finish still spun. |
| 051 | Also push an updated self (family 4) snapshot after 501 | Same screen. |
| 052 | Push the self update as a version-2 delta (flags 0) instead | Same screen. Guest memory contains the new character ID, so the client read it. |
| 053 | Include a type-1 character object with ID, class and appearance in the family-3 response | Once the stopped DNS/STUN/SignOn helpers were restored, Finish still spun. Guest memory contained one additional copy of the character ID at `0x700d9e40`, inside the new object. |
| 054 | Also include the self-family type-1 character object in the version-2 update | Finish still spins. Service 501 received/replied at 21:41:35; the server pushed self family 4 and answered the follow-up family 3 fetch. Only keepalives and service 701 followed, with no disconnect through at least 21:46. The new ID also appeared in guest memory at `0x700e0d48`, inside the self-family character object. |

The relevant raw BAP captures and logs are in
`<PS3_ROOT>\SIGNON_LOCAL\captures\bap` and `...\logs`. For comparison 052,
the 501 request was frame 14 and the follow-up 206 fetch was frame 15 in session
`20260924-211857-796656`. Service 701 continues to be called periodically and gets
schema-encoded result `error_none`; that alone does not prove it is implemented fully.

## Protocol findings and code

- `server/investment.py` has a bit reader mirroring the existing serializer. It decodes
  the 52-byte service-501 create request; a round-trip of the captured request matched.
- The 501 request has a three-byte class struct at offset 0 and the 36-byte appearance
  struct at offset 4. The local test's captured Titan/Exo request decoded to
  `{0: {0: 2, 1: 0, 2: 0}, 4: {...}}`; do not infer all enum meanings from one sample.
- Service 501 response type `0x8080185d` is `{result, u64 at offset 8}`. The stand-in
  sends result 0 and a stable per-connection character ID derived from the local
  account ID.
- The select-character family (type 3) has a root object with hash `0xa2ac2c74`,
  handle `0x80801951`, and a second object slot with hash `0xf65abd52`, handle
  `0x80801954`. The latter is a large character object; optional fields at offsets
  0, 8 and 0xc are ID, class and appearance, matching the create request types.
  `server/bap_server.py` now sends that object alongside the root in test 053.
- The self family (type 4) root object has a character-ID list at nested offset 8.
  Its type-1 character object uses hash `0x242b2f61`, handle `0x80801743`. That object
  has the same ID/class/appearance fields, nested one level below offset 0.
  `server/bap_server.py` now pushes the root and this object together as a version-2
  delta after creation. Its follow-up family-3 fetch uses a full snapshot, version 1.
- `scripts/rpcs3_scan_id.py` scans live guest memory read-only. In comparison 052,
  the new ID was present at guest addresses `0x700c2c68`, `0x700d9db0` and
  `0x700ded60`. Test 053 added `0x700d9e40` (family-3 character), and 054 added
  `0x700e0d48` (family-4 character). The earlier scan ending at `0x20000000` found nothing because the
  RPCS3 guest heap is around `0x70000000`; this was corrected. These positions are
  instance-specific.
- The private schema `investment_schema_character.json` merges
  `investment_schema.json` (responses and object types) with
  `investment_schema_501.json` (the 501 request type). It has both handle
  `0x80801860` for decoding 501 and handle `0x80801954` for encoding the character.
  The server's `--schema` argument must point to this merged file for test 053.

## Next step

The test-054 network path is healthy and both new character objects reached guest
memory. Trace the game-side completion condition for service 501 and its family-3
follow-up, or inspect the family-3/4 object fields the UI reads after creation.
The manager pointer is read from guest `0x19068c4`; family/object manager routines
are `0x41b75c` and `0x41b9ec`. A generic success response for periodic service 701
may also omit data the game expects, but no evidence yet isolates it as the blocker.
Avoid restarting
services just to repeat a hypothesis: each BAP restart disconnects the user and forces
all first-run setup screens again. There is no persistent local character record yet.

After a verified result, update this note, run
`C:\Python314\python.exe -m unittest discover -s server -p 'test_*.py' -q` and
`git diff --check`, then commit and push only code/docs (no private captures or ROMs).
