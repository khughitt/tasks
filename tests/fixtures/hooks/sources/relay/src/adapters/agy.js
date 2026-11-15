// @ts-check
import { basename, dirname, isAbsolute } from 'node:path';
import { createProcessOps } from '../bus/proc.js';
import { relayError } from '../protocol.js';
import { nearestSessionPid, nearestTerminalPid } from './ancestry.js';

/** @type {import("../types.js").Adapter["events"]} */
export const events = Object.freeze({
  'SessionStart': Object.freeze({ kind: 'session-start', level: 'idle' }),
  'PreToolUse': Object.freeze({ kind: 'before-tool', level: 'working' }),
  'Stop': Object.freeze({ kind: 'turn-end', level: 'done' }),
});
// Hook kinds for subscribers. PreInvocation stays absent until a native field fixture exists.
/** @type {import("../types.js").Adapter["hookEvents"]} */
export const hookEvents = Object.freeze({
  'SessionStart': Object.freeze({ kind: 'session-start', registry: true }),
  'PreToolUse': Object.freeze({ kind: 'before-tool', registry: true }),
  'Stop': Object.freeze({ kind: 'turn-end', registry: true }),
});
/** @param {string} event */
export function stateForEvent(event) {
  if (!Object.hasOwn(events, event)) throw relayError('UNSUPPORTED_EVENT', 'unsupported harness event');
  return events[event].level;
}
/** @param {import("../types.js").State|null} level */
export const reduceState = (level) => level;
// agy replies (agy 1.4.12 evidence; installed-version qualification is separate): PreToolUse refuses
// with {"decision":"deny"}; Stop holds the turn with {"decision":"continue","reason"} on exit 0;
// SessionStart and PreInvocation inject context as one systemMessage step. PreInvocation has no
// decoder yet, so only its encoding exists.
/** @type {Readonly<Record<string, import("../types.js").Capability>>} */
export const replies = Object.freeze({
  'PreToolUse': Object.freeze({ refuse: true, context: false }),
  'Stop': Object.freeze({ refuse: true, context: false }),
  'SessionStart': Object.freeze({ refuse: false, context: true }),
  'PreInvocation': Object.freeze({ refuse: false, context: true }),
});
// agy's documented handler default; a handler timeout raises it, so installs raise both together.
/** @type {import("../types.js").Adapter["outerTimeouts"]} */
export const outerTimeouts = Object.freeze({ default: 30000, max: Object.freeze({}) });
// Allow is empty stdout (agy's no-op, so its own permission flow stays in force); never {"decision":"allow"}.
// Exit stays 0: agy reads the decision from JSON and ignored exit 2 in the probe.
// agy fails a tool call when PreToolUse hooks exit 1 (measured on agy 1.4.12), so relay failures exit 0 and report on stderr.
/** @type {0} */
export const failureExitCode = 0;
/** @param {string} nativeEvent @param {import("../types.js").Reply} reply */
export function encodeReply(nativeEvent, reply) {
  const capability = Object.hasOwn(replies, nativeEvent) ? replies[nativeEvent] : { refuse: false, context: false };
  if (reply.decision === 'deny') {
    if (!capability.refuse) throw relayError('UNSUPPORTED_REPLY', `${nativeEvent} has no refusal channel`);
    const decision = nativeEvent === 'Stop' ? 'continue' : 'deny';
    return { stdout: `${JSON.stringify({ decision, reason: reply.reason })}\n`, stderr: `${reply.reason}\n`, exitCode: 0 };
  }
  if (reply.context === undefined) return { stdout: '', stderr: '', exitCode: 0 };
  if (!capability.context) throw relayError('UNSUPPORTED_REPLY', `${nativeEvent} has no context channel`);
  return { stdout: `${JSON.stringify({ injectSteps: [{ systemMessage: { systemMessage: reply.context } }] })}\n`, stderr: '', exitCode: 0 };
}
/** @param {unknown} v @returns {v is Record<string, any>} */
const object = (v) => v !== null && typeof v === 'object' && !Array.isArray(v);
/**
 * Extracts `<app-data>` from `<app-data>/brain/<sessionId>` in an agy hook payload.
 * @param {Buffer|string} raw
 * @param {string} sessionId
 * @returns {string|undefined}
 */
export function appDataDirFromPayload(raw, sessionId) {
  if (typeof sessionId !== 'string' || sessionId.length === 0) return undefined;
  let data;
  try {
    data = JSON.parse(typeof raw === 'string' ? raw : raw.toString('utf8'));
  } catch {
    return undefined;
  }
  if (!object(data) || typeof data.artifactDirectoryPath !== 'string') return undefined;
  const artifactDir = data.artifactDirectoryPath;
  if (artifactDir.length === 0 || artifactDir.includes('\0') || !isAbsolute(artifactDir)) return undefined;
  if (basename(artifactDir) !== sessionId) return undefined;
  const brainDir = dirname(artifactDir);
  if (basename(brainDir) !== 'brain') return undefined;
  const appData = dirname(brainDir);
  return appData.length > 0 && isAbsolute(appData) ? appData : undefined;
}
/** Translates agy's camelCase payload into the fields parsePayload checks; returns a new object.
 * @type {NonNullable<import("../types.js").Adapter["nativeFields"]>} */
export function nativeFields(data, nativeEvent) {
  /** @type {Record<string, any>} */
  const out = {};
  if (Object.hasOwn(data, 'conversationId')) out.session_id = data.conversationId;
  const first = Array.isArray(data.workspacePaths) ? data.workspacePaths[0] : undefined;
  if (typeof first === 'string' && !first.includes('\0') && isAbsolute(first)) out.cwd = first;
  if (nativeEvent === 'Stop' && Object.hasOwn(data, 'executionNum')) {
    const n = data.executionNum;
    if (!Number.isSafeInteger(n) || n < 0) throw relayError('INVALID_INPUT', 'invalid executionNum');
    out.stop_hook_active = n > 0;
  }
  const tool = nativeEvent === 'PreToolUse' ? projectToolCall(data.toolCall) : null;
  if (tool !== null) { out.tool_name = tool.name; out.tool_input = tool.input; }
  return out;
}
// agy's toolCall as relay's tool: run_command becomes Bash with { command: CommandLine };
// every other tool keeps its own name and argument object ({} when absent). null when
// toolCall carries no string name.
/** @param {unknown} toolCall @returns {{name: string, input: Record<string, any>}|null} */
export function projectToolCall(toolCall) {
  if (!object(toolCall) || typeof toolCall.name !== 'string') return null;
  const { name, args } = toolCall;
  return { name: name === 'run_command' ? 'Bash' : name,
    input: name === 'run_command' && object(args) && typeof args.CommandLine === 'string' ? { command: args.CommandLine } : object(args) ? args : {} };
}
const ops = () => createProcessOps();
/** @type {import("../types.js").Adapter["isHarness"]} */
export const isHarness = (row) => row.comm === 'agy';
/** @param {import("../types.js").AncestryOptions} [options] */
export function resolveSessionPid({ startPid = process.pid, ancestors = (pid) => ops().ancestors(pid) } = {}) {
  return nearestSessionPid(ancestors(startPid), { isHarness: (row) => isHarness(row, { executableOf: () => '' }) });
}
/** @param {import("../types.js").AncestryOptions} [options] */
export function resolveTerminalPid({ startPid = process.pid, ancestors = (pid) => ops().ancestors(pid) } = {}) {
  return nearestTerminalPid(ancestors(startPid), (row) => row.comm === 'agy');
}
