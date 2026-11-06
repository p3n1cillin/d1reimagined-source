// SPDX-FileCopyrightText: Copyright 2026 Destiny 1 Preservation Project
// SPDX-License-Identifier: GPL-2.0-or-later
//
// One activity instance: a BullShcript ScriptRuntime driven by host
// messages, its outputs turned into protocol messages (PROTOCOL.md). Time
// moves only on `tick`, one script tick per message, so the same messages
// replay to the same outputs.

import { pathToFileURL } from "node:url";
import { join } from "node:path";
import { existsSync, readFileSync } from "node:fs";
import { installNatives56578, missingNativeOf } from "./natives56578.mjs";
import { aliasContent, annotateContent, parseBundles, standInBundles } from "./scripts.mjs";
import {
  lifetimeWirePhase,
  RUNTIME_LIFETIME_ENDED,
  types56578,
} from "./types56578.mjs";

export const PROTOCOL_VERSION = 1;

// BullShcript's own rule for incident listener fields (instance_building.ts
// INCIDENT_LISTENER_FIELD): host bookkeeping, never a row.
const INCIDENT_LISTENER = /(?:^|\.)(?:k?e?il_[^.]*|_incident_listener)$/;
export function isHostSideBinding(path) {
  return INCIDENT_LISTENER.test(path);
}
export const DEFAULT_TICK_HZ = 10;

const SharedRuntimeCache = new Map();

async function sharedRuntimeClass(bs) {
  if (!SharedRuntimeCache.has(bs.dir)) {
    const mod = await import(
      pathToFileURL(join(bs.dir, "dist/host/activity/runtime.js")).href
    );
    SharedRuntimeCache.set(bs.dir, mod.ScriptRuntime);
  }
  return SharedRuntimeCache.get(bs.dir);
}

const item = (m) => ({
  bundle: Number(m.bundle) >>> 0,
  typeIndex: Number(m.type_index ?? m.typeIndex),
});

export class ActivitySession {
  // options: { bs, scripts, incidents, activity, bundles?, extraBundles?,
  //   tickHz?, seed?, phaseIndex?, emit(msg), strict? }
  static async create(options) {
    const Runtime = await sharedRuntimeClass(options.bs);
    return new ActivitySession(options, Runtime);
  }

  constructor(options, Runtime) {
    const { bs, scripts, incidents } = options;
    this.bs = bs;
    this.api = bs.api;
    this.scripts = scripts;
    this.incidents = incidents;
    this.emit = options.emit;
    this.types = types56578(this.api.ActivityBundleItemType);
    this.stubs = installNatives56578(bs, (line) => this.log(line));
    this.activity = scripts.activity(options.activity);
    if (!this.activity) {
      throw new Error(`no script starts activity ${options.activity}`);
    }
    this.tickHz = options.tickHz ?? DEFAULT_TICK_HZ;
    this.now = 0;
    this.ticks = 0;
    this.phaseIndex = options.phaseIndex ?? 0;
    this.players = undefined;
    this.soloKey = undefined;
    this.skips = new Map();
    this.changes = [];
    this.out = [];
    this.missing = new Map();
    this.stubHits = new Map();
    this.rowGaps = new Map();
    this.lifetime = undefined;
    this.endedSent = false;
    this.hardWipeArmed = false;
    this.allDead = false;
    this.activeBundles = undefined;

    if (options.bundles) {
      this.bundles = parseBundles(options.bundles);
      this.standIn = false;
      this.bundleNotes = [];
      // The scripts' own declarations name each Encounter row's archetype.
      const twins = standInBundles(scripts, this.activity, this.types, {
        extra: this.bundles.map((t) => t.bundle),
      });
      const n = annotateContent(this.bundles, twins.bundles, this.types);
      if (n > 0) this.bundleNotes.push(`content: ${n} Encounter rows take their archetype from the scripts`);
      const aliases = aliasContent(this.bundles, twins.bundles, this.api.fnv1);
      if (aliases > 0) this.bundleNotes.push(`content: ${aliases} script members bound to the one content row their name prefixes`);
    } else {
      const made = standInBundles(scripts, this.activity, this.types, {
        extra: options.extraBundles ?? [],
      });
      this.bundles = made.bundles;
      this.standIn = true;
      this.bundleNotes = made.notes;
    }
    const scriptByTag = new Map();
    for (const tag of this.bundles) {
      const file = scripts.scriptForBundle(tag.bundle, this.activity.destination);
      if (file && scripts.classDecl(file.path)?.chunk) {
        scriptByTag.set(tag.tag >>> 0, scripts.scriptTagOf(file.path));
      }
    }
    const rootSchema = scripts.schemaOf(this.activity.root);
    this.phaseRowOf = new Map(
      (rootSchema?.members ?? [])
        .filter((m) => m.kind === "class_instance")
        .map((m) => [m.type, m.name])
    );
    this.closure = scripts.closure([
      this.activity.root,
      ...this.bundles
        .map((t) => scripts.scriptForBundle(t.bundle, this.activity.destination)?.path)
        .filter(Boolean),
    ]);
    this.runtime = new Runtime({
      bundles: this.bundles,
      clock: { activityTicks: () => this.now },
      log: (line) => this.log(line),
      onActivityComplete: () => this.activityComplete(),
      onActivityFlag: (hash, active) =>
        this.out.push({ active, hash: hash >>> 0, type: "activity_flag" }),
      onItemChange: (change) => this.changes.push(change),
      onItemsCleared: (items) =>
        this.out.push({ items: items.map((i) => ({ ...i })), type: "rows_cleared" }),
      onPhaseStarted: (decl) => this.phaseStarted(decl),
      phaseProgress: {
        read: () => this.phaseIndex,
        write: (index) => {
          this.phaseIndex = index;
        },
      },
      players: () => this.playerList(),
      scriptFor: (tag) => scriptByTag.get(tag >>> 0),
      scripts: scripts.scriptSource(),
      seed: options.seed,
      squadPlacedByDefault: () => false,
      tickSeconds: 1 / this.tickHz,
      types: this.types,
    });
  }

  log(line) {
    const stub = /^stub56578 (.*?):(\w+) /.exec(line);
    if (stub) {
      this.stubHits.set(stub[2], (this.stubHits.get(stub[2]) ?? 0) + 1);
    }
    let missing = missingNativeOf(line);
    if (missing && this.bs.nativeRegistry.lookup(missing.method)) {
      // A verb BullShcript has, answering "unknown" for a row it cannot
      // place: a binding gap (stand-in bundles, or content missing a row).
      this.rowGaps.set(missing.method, (this.rowGaps.get(missing.method) ?? 0) + 1);
      missing = undefined;
    }
    if (missing) {
      const entry = this.missing.get(missing.method) ?? { calls: 0, on: new Set() };
      entry.calls += 1;
      entry.on.add(missing.on);
      this.missing.set(missing.method, entry);
    }
    const msg = { text: line, type: "log" };
    if (stub) msg.stub = stub[2];
    if (missing) msg.missing = missing.method;
    this.out.push(msg);
  }

  playerList() {
    if (this.players) {
      return this.players;
    }
    return this.soloKey === undefined
      ? []
      : [{ characterKey: this.soloKey, powerLevel: undefined, ready: true }];
  }

  // Reply to hello: what will run.
  loaded() {
    const a = this.activity;
    return {
      activity: a.name,
      bundles: this.bundles.map((t) => ({
        bundle: t.bundle,
        path: t.path,
        rows: t.rows.length,
        tag: t.tag,
      })),
      compile_errors: this.scripts.compileErrors(this.closure),
      drives_lifetime: this.scripts.drivesLifetime(this.closure),
      notes: this.bundleNotes,
      phases: a.phases.map((p, index) => ({
        index,
        name: p.name,
        name_hash: p.hash >>> 0,
        slice: p.slice,
        slice_hash: this.api.sliceRegionHash(p.slice) >>> 0,
      })),
      proto: PROTOCOL_VERSION,
      script_tags: this.runtime.scriptTags,
      stand_in: this.standIn,
      start: {
        region_spawn_transitions: a.start.regionSpawnTransitions ?? {},
        spawn_transition: a.start.spawnTransition,
        start_phase: a.start.startPhase?.name,
        start_slice: a.start.startSlice,
        time_of_day: a.start.timeOfDay,
      },
      stub: a.stub,
      tick_hz: this.tickHz,
      type: "loaded",
    };
  }

  start() {
    this.runtime.start();
    this.collect();
    this.out.unshift({
      drives_lifetime: this.scripts.drivesLifetime(this.closure),
      script_tags: this.runtime.scriptTags,
      stubs: [...this.stubs],
      type: "ready",
      // BullShcript keeps incident listeners host-side by design (they count
      // incidents for an import identifier and have no row in any bundle; the
      // content export confirms it), so they are listed apart, not as gaps.
      host_side: this.runtime.unresolvedBindings.filter((p) => this.hostSide(p)),
      unresolved: this.runtime.unresolvedBindings.filter((p) => !this.hostSide(p)),
    });
  }

  // Bindings no bundle row backs by design: incident listeners (BullShcript's
  // rule) and members declared `activity_flag`, which are flags on the players
  // bound by identifier (BullShcript names only `_activity_flag` fields so; the
  // content export has no row for either).
  hostSide(path) {
    if (isHostSideBinding(path)) return true;
    if (!this.activityFlagMembers) {
      this.activityFlagMembers = new Set();
      for (const p of this.closure) {
        for (const m of this.scripts.schemaOf(p)?.members ?? []) {
          if (m.type === "activity_flag") this.activityFlagMembers.add(m.name);
        }
      }
    }
    return this.activityFlagMembers.has(path.slice(path.lastIndexOf(".") + 1));
  }

  phaseStarted(decl) {
    const name = this.phaseRowOf.get(decl) ?? decl;
    const authored = this.activity.phases.find((p) => p.name === name);
    this.out.push({
      decl,
      index: this.runtime.phaseIndex,
      name,
      name_hash: this.api.fnv1(name) >>> 0,
      slice: authored?.slice,
      slice_hash:
        authored?.slice === undefined
          ? undefined
          : this.api.sliceRegionHash(authored.slice) >>> 0,
      type: "phase",
    });
  }

  activityComplete() {
    const known = this.incidents.byName.get("activity_complete");
    this.out.push({
      id: known?.id,
      name: "activity_complete",
      name_hash: known?.hash ?? this.api.fnv1("activity_complete") >>> 0,
      type: "activity_complete",
    });
  }

  // Item changes become one `rows` message; then the derived signals.
  collect() {
    if (this.changes.length > 0) {
      const changes = this.changes.map((change) => {
        if (change.kind !== "lifetime") {
          return change;
        }
        this.lifetime = change.state.state;
        return {
          ...change,
          wire: { phase: lifetimeWirePhase(change.state.state) },
        };
      });
      this.changes = [];
      // rows go before whatever the same step said about phases and ends
      this.out.unshift({ changes, type: "rows" });
    }
    const armed = this.runtime.hardWipeArmed === true;
    if (armed !== this.hardWipeArmed) {
      this.hardWipeArmed = armed;
      this.out.push({ armed, type: "hard_wipe_armed" });
    }
    const dead = this.runtime.allPlayersDead === true;
    if (dead !== this.allDead) {
      this.allDead = dead;
      this.out.push({ dead, type: "all_dead" });
    }
    if (
      !this.endedSent &&
      (this.lifetime === RUNTIME_LIFETIME_ENDED || this.runtime.activityEnded)
    ) {
      this.endedSent = true;
      this.out.push({ type: "ended" });
    }
  }

  // Runs one host message; returns the messages it produced, in order.
  handle(msg) {
    this.out = [];
    const r = this.runtime;
    switch (msg.type) {
      case "start":
        this.start();
        return this.flush();
      case "tick": {
        if (msg.now !== undefined) {
          this.now = Number(msg.now);
        }
        r.tick();
        this.ticks += 1;
        this.collect();
        const done = { now: this.now, ticks: this.ticks, time: r.now, type: "tick_done" };
        if (msg.threads) {
          done.threads = r.debugSnapshot().threads;
        }
        this.out.push(done);
        return this.flush(false);
      }
      case "active_bundles":
        this.activeBundles = new Set((msg.bundles ?? []).map((b) => Number(b) >>> 0));
        r.setActiveBundles(this.activeBundles);
        break;
      case "event_start":
      case "event_stop":
        this.publicEvent(msg);
        break;
      case "players":
        this.players = (msg.players ?? []).map((p) => ({
          characterKey: String(p.key),
          powerLevel: p.power,
          ready: p.ready ?? false,
          teamIndex: p.team,
        }));
        break;
      case "player_joined":
        r.notePlayerJoined(String(msg.key));
        break;
      case "player_left":
        r.notePlayerLeft(String(msg.key));
        break;
      case "player_spawned":
        this.spawned(msg.key === undefined ? undefined : String(msg.key));
        break;
      case "player_died":
        r.reportPlayerDied(String(msg.key));
        break;
      case "player_respawned":
        r.reportPlayerRespawned(String(msg.key));
        break;
      case "trigger":
        r.reportTriggerFired(item(msg));
        break;
      case "cinematic_finished":
        r.reportCinematicFinished(item(msg));
        break;
      case "cinematic_skipped":
        this.skipped(msg);
        break;
      case "incident":
        this.incident(msg);
        break;
      case "squad_alive":
        r.reportSquadAlive(item(msg), Number(msg.count));
        break;
      case "squad_combat":
        r.reportSquadCombatCount(item(msg), Number(msg.count));
        break;
      case "squad_tally":
        r.reportSquadTally(item(msg), (msg.tally ?? []).map(Number));
        break;
      case "squad_kill":
        r.reportSquadKill(Number(msg.ident) >>> 0, msg.at);
        break;
      case "kill":
        r.reportKill(msg.details ?? {});
        break;
      case "object_alive":
        r.reportObjectAlive(item(msg), Boolean(msg.alive));
        break;
      case "player_monitor":
        r.reportPlayerMonitor(item(msg), Number(msg.count));
        break;
      case "health":
        r.reportHealth(item(msg), Number(msg.current), Number(msg.max));
        break;
      case "objective_count":
        r.reportObjectiveCount(item(msg), Number(msg.slot ?? 0), Number(msg.count));
        break;
      case "actor_bound":
        r.reportActorBound(item(msg), Boolean(msg.flag7), Boolean(msg.flag8));
        break;
      case "actor_health":
        r.reportActorHealth(item(msg), Number(msg.fraction));
        break;
      case "actor_drop_ack":
        r.reportActorDropAck(item(msg), msg.drop_id, Boolean(msg.in_progress));
        break;
      case "actor_standing":
        r.reportActorStanding(item(msg), msg.epoch, Boolean(msg.standing));
        break;
      case "actor_command_script":
        r.reportActorCommandScript(item(msg), {
          cookieRunning: msg.cookie_running,
          heartbeat: msg.heartbeat,
          pcReached: msg.pc_reached,
        });
        break;
      case "device":
        r.reportDeviceSense(item(msg), {
          position: Number(msg.position),
          positionStamp: Number(msg.position_stamp ?? 0),
        });
        break;
      case "interactable":
        r.reportInteractableSense(item(msg), {
          used: Boolean(msg.used),
          usedStamp: Number(msg.used_stamp ?? 0),
        });
        break;
      case "capturable":
        r.reportCapturableSense(item(msg), msg.sense ?? {});
        break;
      case "faction":
        r.reportFactionSense(item(msg), Number(msg.faction));
        break;
      case "safe_zone":
        r.reportSafeZone(item(msg), Number(msg.inside));
        break;
      case "interaction": {
        const names = new Set((msg.names ?? []).map((n) => Number(n) >>> 0));
        r.reportInteraction((n) => names.has(n >>> 0));
        break;
      }
      case "loot": {
        const materials = new Set((msg.materials ?? []).map((n) => Number(n) >>> 0));
        r.reportLootGathered((n) => materials.has(n >>> 0));
        break;
      }
      case "hard_wipe":
        r.resetCurrentPhase();
        break;
      case "snapshot":
        this.out.push({ snapshot: r.debugSnapshot(), type: "snapshot" });
        break;
      default:
        this.out.push({ text: `unknown message type ${msg.type}`, type: "error" });
        return this.flush(false);
    }
    this.collect();
    return this.flush();
  }

  // One public event (PROTOCOL.md "Public events"): its bundle's threads
  // start or stop running, the rest of the active set unchanged. An event
  // runs from its own bundle's scripts, so this is the bundle's switch; the
  // rows the event left are kept (no 56578 evidence says they reset).
  publicEvent(msg) {
    const bundle = Number(msg.bundle) >>> 0;
    const start = msg.type === "event_start";
    const reply = { bundle, event_id: msg.event_id, type: "event" };
    if (!this.bundles.some((t) => t.bundle === bundle)) {
      this.out.push({ ...reply, reason: "the activity loads no such bundle", state: "refused" });
      return;
    }
    const active = new Set(this.activeBundles ?? this.bundles.map((t) => t.bundle));
    if (start) active.add(bundle);
    else active.delete(bundle);
    this.activeBundles = active;
    this.runtime.setActiveBundles(active);
    this.out.push({ ...reply, state: start ? "started" : "stopped" });
  }

  spawned(key) {
    if (this.players) {
      for (const p of this.players) {
        if (key === undefined || p.characterKey === key) {
          p.ready = true;
        }
      }
    } else if (this.soloKey === undefined) {
      this.soloKey = key ?? "0";
    }
    this.runtime.reportPlayerSpawned();
  }

  // A skip request ends the cinematic once every player present asked (one
  // player: at once). The host may decide otherwise and send
  // cinematic_finished itself.
  skipped(msg) {
    const at = item(msg);
    const k = `${at.bundle}:${at.typeIndex}`;
    const keys = this.skips.get(k) ?? new Set();
    keys.add(String(msg.key ?? "0"));
    this.skips.set(k, keys);
    const players = this.playerList();
    if (players.length === 0 || players.every((p) => keys.has(p.characterKey))) {
      this.skips.delete(k);
      this.runtime.reportCinematicFinished(at);
    }
  }

  incident(msg) {
    const found = this.incidents.resolve(msg, this.api.fnv1);
    if (!found) {
      this.out.push({ text: `incident ${JSON.stringify(msg)} not in the catalogue`, type: "error" });
      return;
    }
    switch (found.name) {
      case "player_spawned":
        this.spawned(msg.key === undefined ? undefined : String(msg.key));
        return;
      case "cinematic_finished":
        if (msg.bundle !== undefined) {
          this.runtime.reportCinematicFinished(item(msg));
          return;
        }
        break;
      case "cinematic_skip":
        if (msg.bundle !== undefined) {
          this.skipped(msg);
          return;
        }
        break;
      default:
        break;
    }
    this.runtime.reportClientIncident(found.hash >>> 0);
  }

  flush(withMarker = false) {
    const out = this.out;
    this.out = [];
    if (withMarker) {
      out.push({ type: "done" });
    }
    return out;
  }

  // Native names the scripts called that nothing answered, with counts;
  // and the 56578 stubs they called.
  missingNatives() {
    return [...this.missing].map(([method, e]) => ({
      calls: e.calls,
      method,
      on: [...e.on].slice(0, 5),
    }));
  }

  rowGapCalls() {
    return [...this.rowGaps].map(([method, calls]) => ({ calls, method }));
  }

  stubCalls() {
    return [...this.stubHits].map(([method, calls]) => ({ calls, method }));
  }
}

// Reads a content export file (the private ActivityBundleTag[] JSON, or
// tools/export-bundles.mjs's per-world document).
export function readBundlesFile(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

// The export for an activity under D1_BUNDLES_DIR (`<dir>/<world>.json`, the
// world being the activity's own name), when one is there.
export function bundlesPathFor(activity, env = process.env) {
  const dir = env.D1_BUNDLES_DIR;
  if (!dir || !activity) return undefined;
  const path = join(dir, `${activity}.json`);
  return existsSync(path) ? path : undefined;
}
