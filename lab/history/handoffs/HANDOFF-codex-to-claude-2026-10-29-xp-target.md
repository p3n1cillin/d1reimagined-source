# Codex to Claude: collector target2 gate found, 29 Oct afternoon

Early on-disk hand-back while I continue the spinner/CONTINUE trace. No input
or publication from me. sense54 read: phase7, scoreboard component/UI1/1,
collector state4/target4/activity164/XP0,0,0; profile117=900; selected165.

## The missing start is an investment-profile activity transition

Native56e580 profile diff at56f185 compares OLD and NEW profile **+2c20**.
When the activity id changes:
- NEW id !=ffff:56f44a calls **574470(collector, new activity record)**.
  Its first store at574496 is **word[collector]=0200**: state0,target2.
  It also sets collector activity from the new record and its activity context.
- NEW id ==ffff:56f1c1 calls5745e0, which raises target to4 only if it was>=2.

The same profile-diff function then checks NEW completed byte+2c38=1 and OLD
byte=0 at56f452. Its completion path eventually calls5745e0 at56f8d3.
Thus introducing id164 AND completed1 in the first completion delta creates
target2 and immediately raises it to4 in the same diff. The next573570 pass
seeds and finalizes together. That explains both0/0/0 and900/900/900 results.
Neither the AH activity index nor lifetime phase alone starts this collector.

## Concrete next-run sequence (owner input only)

1. After the initial profile baseline is accepted, while its activity id is
   stillffff, send a family4 profile DELTA with **activity164, completed0,
   progression117=0**. Require native collector state2/target2/activity164
   and XP samples0/0/0 before testing XP. An explicit delta avoids relying on
   whether initial baseline import happens to announce a usable old/new diff.
2. During that interval, or in a later completion delta with the SAME id164,
   raise117 to900. 5747b0 only accepts changes in collector states2 or3.
3. Raise completed0->1 after the collector has been seeded. With id unchanged
   there is no new574470 reset. XP/progression diffs are processed earlier in
   56e580 than the completion check, so a same-id164 completion+XP900 delta
   can record the sample before target rises to4. Separate early-XP and later
   completion deltas give a clearer diagnostic if desired.

UNSERVED draft: `F:\d1\d1preservation-main\cfw-staging\profile-active164-xp0.draft.bin`.
It clones your completed164/prog baseline with ONLY+2c38 cleared. Owned object
codec round trip IDENTICAL. Like the original baseline it has empty profile
inventory/equipment; merge these activity fields into your newer authoritative
profile if other profile fields have since changed.

This draft is for a fresh run with OLD activityffff. It does NOT reset sense54
merely by clearing completed while the activity remains164: unchanged+2c20
does not call574470, and target4 does not automatically drop back to2.
I have not changed the parked run or authored a reset/rewind experiment for it.

## Stage2 equipped-item mask

573950 seeds the XP triples, clears result queues, then sets mask+50=ffffffff.
573570 examines only the FIRST20 bits (equipped profile SOIDs at+2b80).
An empty equipped SOID clears its bit immediately. Other entries pass through
573bb0, which may wait for definition/item progression data or queue upgrades.
8f800(mask,20) returns1 when those20 bits are CLEAR; upper12 bits are ignored.
In sense54 the retained mask isfff00000: all20 relevant bits are clear.
The mask is therefore NOT blocking this parked run. With an empty equipped
list, stage2 is a stable collection interval while target2; once target4 is
requested it proceeds to stage3/build and finishes4 without a timed delay.

Native evidence and profile codec check are in the private postgame-codex
folder. Spinner/CONTINUE analysis is still in progress; no missing-XP claim
is being used as proof of the spinner's cause.
