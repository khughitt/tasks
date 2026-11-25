// @ts-check
import { createProcessOps } from '../bus/proc.js';
import { relayError } from '../protocol.js';
import { encodeStandardReply } from './reply.js';
import { nearestSessionPid, nearestTerminalPid } from './ancestry.js';

/** @type {import("../types.js").Adapter["events"]} */
export const events = Object.freeze({
  'SessionStart': Object.freeze({ kind: 'session-start', level: 'idle' }),
  'UserPromptSubmit': Object.freeze({ kind: 'prompt', level: 'working' }),
  'PreToolUse': Object.freeze({ kind: 'before-tool', level: 'working' }),
  'Stop': Object.freeze({ kind: 'turn-end', level: 'done' }),
  'SessionEnd': Object.freeze({ kind: 'session-end', level: null }),
  'PermissionRequest': Object.freeze({ kind: 'needs-approval', level: 'needs-approval' }),
  'SubagentStart': Object.freeze({ kind: 'session-start', level: 'working' }),
  'SubagentStop': Object.freeze({ kind: 'turn-end', level: 'done' }),
});
// Hook kinds for subscribers. Codex child events keep their registry worker projection above.
/** @type {import("../types.js").Adapter["hookEvents"]} */
export const hookEvents = Object.freeze({
  'SessionStart': Object.freeze({ kind: 'session-start', registry: true }),
  'UserPromptSubmit': Object.freeze({ kind: 'prompt', registry: true }),
  'PreToolUse': Object.freeze({ kind: 'before-tool', registry: true }),
  'Stop': Object.freeze({ kind: 'turn-end', registry: true }),
  'SessionEnd': Object.freeze({ kind: 'session-end', registry: true }),
  'PermissionRequest': Object.freeze({ kind: 'needs-approval', registry: true }),
  'SubagentStart': Object.freeze({ kind: 'child-start', registry: true }),
  'SubagentStop': Object.freeze({ kind: 'child-stop', registry: true }),
  'PostToolUse': Object.freeze({ kind: 'after-tool', registry: false }),
});
/** @param {string} event */
export function stateForEvent(event) {
  if (!Object.hasOwn(events, event)) throw relayError('UNSUPPORTED_EVENT', 'unsupported harness event');
  return events[event].level;
}
/** @param {import("../types.js").State|null} level */
export const reduceState = (level) => level;
// Reply channels verified against the Codex hooks reference on 2026-09-17. PermissionRequest
// uses a different decision object and stays unexposed in v1.
/** @type {Readonly<Record<string, import("../types.js").Capability>>} */
export const replies = Object.freeze({
  // SessionStart context: hookSpecificOutput.additionalContext; live delivery is qualified separately.
  'SessionStart': Object.freeze({ refuse: false, context: true }),
  'UserPromptSubmit': Object.freeze({ refuse: true, context: true }),
  'PreToolUse': Object.freeze({ refuse: true, context: true }),
  'Stop': Object.freeze({ refuse: true, context: false }),
  // SubagentStop's reply schema is Stop's at Codex 0.162.1.
  'SubagentStop': Object.freeze({ refuse: true, context: false }),
});
// SessionEnd and Interrupt default to one second and cap at three.
/** @type {import("../types.js").Adapter["outerTimeouts"]} */
export const outerTimeouts = Object.freeze({ default: 600000, SessionEnd: 1000, max: Object.freeze({ SessionEnd: 3000 }) });
/** @param {string} nativeEvent @param {import("../types.js").Reply} reply */
export const encodeReply = (nativeEvent, reply) => encodeStandardReply(nativeEvent, reply, replies[nativeEvent]);
// A worker's hooks carry the root's session_id and the worker's own agent_id (its thread id);
// no root hook carries agent_id. See docs/adapters.md ("Event mappings").
const SUBAGENT_EVENTS = Object.freeze(['SubagentStart', 'SubagentStop']);
/** @param {Record<string, any>} data @param {string} nativeEvent @returns {string|null} */
export function workerIdOf(data, nativeEvent) {
  if (!Object.hasOwn(data, 'agent_id')) {
    if (SUBAGENT_EVENTS.includes(nativeEvent)) throw relayError('INVALID_INPUT', 'a subagent event requires agent_id');
    return null;
  }
  const id = data.agent_id;
  if (typeof id !== 'string' || !id.trim() || id === data.session_id) throw relayError('INVALID_INPUT', 'invalid worker agent_id');
  return id;
}
const ops = () => createProcessOps();
/** @type {import("../types.js").Adapter["isHarness"]} */
export const isHarness = (row) => row.comm === 'codex';
/** @param {import("../types.js").AncestryOptions} [options] */
export function resolveSessionPid({ startPid = process.pid, ancestors = (pid) => ops().ancestors(pid) } = {}) {
  return nearestSessionPid(ancestors(startPid), { isHarness: (row) => isHarness(row, { executableOf: () => '' }) });
}
/** @param {import("../types.js").AncestryOptions} [options] */
export function resolveTerminalPid({ startPid = process.pid, ancestors = (pid) => ops().ancestors(pid) } = {}) {
  return nearestTerminalPid(ancestors(startPid), (row) => row.comm === 'codex');
}
