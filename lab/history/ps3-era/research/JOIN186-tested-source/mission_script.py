"""Opt-in phase scopes for pending activity-host mission reactions."""

from dataclasses import dataclass


@dataclass(frozen=True)
class TeleportReaction:
    zone: tuple


def reaction_scope(step):
    """Return (lane, phase), or None for the original independent-rule behavior.

    All reactions belonging to one handover use the same lane/phase. A matched reaction in
    another phase of that lane cancels pending changes from the previous handover. Other
    lanes, unscoped reactions and changes already applied are unaffected.
    """
    if "lane" not in step and "phase" not in step:
        return None
    if "lane" not in step or "phase" not in step:
        raise ValueError("mission reaction scope needs both lane and phase")
    if "on" not in step or "at" in step or not step.get("set"):
        raise ValueError("mission reaction scope needs an on-triggered state change")
    lane, phase = step["lane"], step["phase"]
    if not all(isinstance(value, str) and value.strip() for value in (lane, phase)):
        raise ValueError("mission reaction lane and phase must be nonempty strings")
    return lane, phase


class ReactionRule(tuple):
    """Keep the four-item CLI/preflight rule shape, with optional mission-only metadata."""

    def __new__(cls, on_type, matcher, delay, spec, scope=None):
        rule = super().__new__(cls, (on_type, matcher, delay, spec))
        rule.scope = scope
        return rule


class ReactionQueue:
    def __init__(self):
        self._pending = []
        self._phases = {}

    def schedule(self, when, spec, reason, scope=None):
        """Queue one change, returning the number of obsolete pending changes cancelled."""
        cancelled = 0
        if scope is not None:
            lane, phase = scope
            if self._phases.get(lane) != phase:
                previous = len(self._pending)
                self._pending = [entry for entry in self._pending
                                 if entry[3] is None or entry[3][0] != lane]
                cancelled = previous - len(self._pending)
                self._phases[lane] = phase
        self._pending.append((when, spec, reason, scope))
        return cancelled

    def has_due(self, now):
        return any(entry[0] <= now for entry in self._pending)

    def pop_due(self, now):
        """Take due changes in insertion order, preserving siblings from the same handover."""
        due = [entry[:3] for entry in self._pending if entry[0] <= now]
        self._pending = [entry for entry in self._pending if entry[0] > now]
        return due
