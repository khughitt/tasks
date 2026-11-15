// @ts-check
import { createProcessOps } from '../bus/proc.js';
import { relayError } from '../protocol.js';
import { encodeStandardReply } from './reply.js';
import { nearestSessionPid, nearestTerminalPid } from './ancestry.js';

/** @type {import("../types.js").Adapter["events"]} */
export const events = Object.freeze({
  'init': Object.freeze({ kind: 'session-start', level: 'idle' }),
  'session.busy': Object.freeze({ kind: 'activity', level: 'working' }),
  'permission.pending': Object.freeze({ kind: 'needs-approval', level: 'needs-approval' }),
  'session.error': Object.freeze({ kind: 'turn-error', level: 'error' }),
  'session.idle': Object.freeze({ kind: 'idle', level: 'idle' }),
  'dispose': Object.freeze({ kind: 'session-end', level: null }),
});
// OpenCode keeps its existing observation mapping: every event publishes.
/** @type {import("../types.js").Adapter["hookEvents"]} */
export const hookEvents = Object.freeze(Object.fromEntries(Object.entries(events).map(([name, { kind }]) => [name, Object.freeze({ kind, registry: true })])));
/** @param {string} event */
export function stateForEvent(event) {
  if (!Object.hasOwn(events, event)) throw relayError('UNSUPPORTED_EVENT', 'unsupported harness event');
  return events[event].level;
}
const ACTIVE = new Set(['working', 'needs-input', 'needs-approval']);
/** @param {import("../types.js").State|null} level @param {import("../types.js").State|null} [previous] */
export function reduceState(level, previous) {
  if (level !== 'idle') return level;
  if (previous === 'error') return 'error';
  return (/** @type {Set<unknown>} */ (ACTIVE)).has(previous) ? 'done' : 'idle';
}
const ops = () => createProcessOps();
/** @type {import("../types.js").Adapter["isHarness"]} */
export const isHarness = (row) => row.comm === 'opencode';
/** @param {import("../types.js").AncestryOptions} [options] */
export function resolveSessionPid({ startPid = process.pid, ancestors = (pid) => ops().ancestors(pid) } = {}) {
  return nearestSessionPid(ancestors(startPid), { isHarness: (row) => isHarness(row, { executableOf: () => '' }) });
}
/** @param {import("../types.js").AncestryOptions} [options] */
export function resolveTerminalPid({ startPid = process.pid, ancestors = (pid) => ops().ancestors(pid) } = {}) {
  return nearestTerminalPid(ancestors(startPid), (row) => row.comm === 'opencode');
}
// The server plugin observes only; its spawnHook lifetime is five seconds.
/** @type {Readonly<Record<string, import("../types.js").Capability>>} */
export const replies = Object.freeze({});
/** @type {import("../types.js").Adapter["outerTimeouts"]} */
export const outerTimeouts = Object.freeze({ default: 5000, max: Object.freeze({}) });
/** @param {string} nativeEvent @param {import("../types.js").Reply} reply */
export const encodeReply = (nativeEvent, reply) => encodeStandardReply(nativeEvent, reply);
