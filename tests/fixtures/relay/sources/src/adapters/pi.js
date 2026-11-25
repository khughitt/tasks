// @ts-check
// pi's extension API (pi 1.1.0). integrations/pi forwards these events; the suffixed settles are
// its own names for pi's one agent_settled. See docs/adapters.md ("Event mappings").
import { createProcessOps } from '../bus/proc.js';
import { relayError } from '../protocol.js';
import { encodeStandardReply } from './reply.js';
import { nearestSessionPid, nearestTerminalPid } from './ancestry.js';

/** @type {import("../types.js").Adapter["events"]} */
export const events = Object.freeze({
  'session_start': Object.freeze({ kind: 'session-start', level: 'idle' }),
  'agent_start': Object.freeze({ kind: 'activity', level: 'working' }),
  'agent_settled': Object.freeze({ kind: 'turn-end', level: 'done' }),
  'agent_settled:aborted': Object.freeze({ kind: 'idle', level: 'idle' }),
  'agent_settled:error': Object.freeze({ kind: 'turn-error', level: 'error' }),
  'session_shutdown': Object.freeze({ kind: 'session-end', level: null }),
});
/** @type {import("../types.js").Adapter["hookEvents"]} */
export const hookEvents = Object.freeze(Object.fromEntries(Object.entries(events).map(([name, { kind }]) => [name, Object.freeze({ kind, registry: true })])));
/** @param {string} event */
export function stateForEvent(event) {
  if (!Object.hasOwn(events, event)) throw relayError('UNSUPPORTED_EVENT', 'unsupported harness event');
  return events[event].level;
}
/** @type {import("../types.js").Adapter["reduceState"]} */
export const reduceState = (level) => level;
const ops = () => createProcessOps();
// pi names its own process: `pi` from its command line in every mode, `pi-rpc` from its RPC entry point.
/** @type {import("../types.js").Adapter["isHarness"]} */
export const isHarness = (row) => row.comm === 'pi' || row.comm === 'pi-rpc';
/** @param {import("../types.js").AncestryOptions} [options] */
export function resolveSessionPid({ startPid = process.pid, ancestors = (pid) => ops().ancestors(pid) } = {}) {
  return nearestSessionPid(ancestors(startPid), { isHarness: (row) => isHarness(row, { executableOf: () => '' }) });
}
/** @param {import("../types.js").AncestryOptions} [options] */
export function resolveTerminalPid({ startPid = process.pid, ancestors = (pid) => ops().ancestors(pid) } = {}) {
  return nearestTerminalPid(ancestors(startPid), (row) => isHarness(row, { executableOf: () => '' }));
}
// The extension observes only; its hook process lives at most five seconds.
/** @type {Readonly<Record<string, import("../types.js").Capability>>} */
export const replies = Object.freeze({});
/** @type {import("../types.js").Adapter["outerTimeouts"]} */
export const outerTimeouts = Object.freeze({ default: 5000, max: Object.freeze({}) });
/** @param {string} nativeEvent @param {import("../types.js").Reply} reply */
export const encodeReply = (nativeEvent, reply) => encodeStandardReply(nativeEvent, reply);
