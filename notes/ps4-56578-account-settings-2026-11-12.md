# Account settings survive sign-in — 12 November 2026

On build 56578, the client saves account user data through investment service 701,
`profile_set_account_profile`. A change in Settings sends this request, and periodic saves follow
about every 30 seconds. The responder previously acknowledged the request and discarded its data,
so a new session returned to the served defaults.

`D1_ACCOUNT_USER_DATA=1` retains the decoded account user data and serves it at the next sign-in.
The full-game profile enables it. The baseline profile leaves it off. With the player store enabled,
the data survives a server restart. With the store disabled, its lifetime is the server process.

The schema metadata derives the layout for 56578. Request tag `0x808016dd` contains the 1088-byte
`0x80801716` user-data structure. Account tag `0x808018c7` carries the same structure in optional
field 5 at offset 128. The responder checks that layout against the loaded executable's schemas.
It preserves the client-supplied image as opaque data, including the native settings choices.

When serving an account, the implementation replaces only field 5 in the encoded stream and
preserves the surrounding bits. It decodes the result to verify the replacement and the other
account fields before sending it. Account pushes use the same operation on a wire copy, keeping
the stored account payload separate from this optional feature.

If a save fails, the newest data remains pending in memory and takes precedence over the older
stored value. A successful later save clears the pending copy. Persistence and pending-data reads
share one ordering lock.

## Native evidence

Both runs used build 56578 and emulator commit `1ac34e4`, whose tree matches fork `d1` commit
`73ef311`. They used the same player store, with fresh server and emulator processes between runs.

| Run | Server revision | Observed in game |
| --- | --- | --- |
| set2 | `596fe6ba` | Accessibility initially selected Language-Based (Default). Selecting Enabled moved the native subtitle selection marker. The responder saved the account user data. |
| set3 | `c0d101a7` | After restarting both servers and emulator, Accessibility already selected Enabled. No new confirmation press was sent before inspecting it. The weapon-search subtitle also appeared on entering the Wall and is present in the recording. |

The native persistence check covers Subtitles. Screenshots, recordings and request captures remain
private.

## Offline checks

With the owned 56578 executable, the responder suite passed 225 tests with one existing ignored
test. The new regression forces a real store-write failure, checks that pending data is served,
then verifies a successful replacement and disk reload. Additional checks cover update ordering,
account-bit preservation and malformed requests. Supplied missing or malformed executable inputs
fail the schema checks. Tests skip only when optional input is unavailable.

The profile suite passed 12 tests and 394 subtests with the final full-game activation.
