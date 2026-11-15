// @ts-check
import * as claudeCode from './claude-code.js';
import * as agy from './agy.js';
import * as codex from './codex.js';
import * as opencode from './opencode.js';
import { parsePayload } from './payload.js';
import { relayError, validateEvent } from '../protocol.js';
/** @type {Record<string, import("../types.js").Adapter>} */
const adapters = { 'claude-code': claudeCode, codex, opencode, agy };
/** @param {string} harness */
export function adapterFor(harness) {
  if (!Object.hasOwn(adapters, harness)) throw relayError('UNSUPPORTED_HARNESS', 'unsupported harness');
  return adapters[harness];
}
// The subscriber-facing hook mapping for one native event; UNSUPPORTED_EVENT when the adapter has no decoder for it.
/** @param {import("../types.js").Adapter} adapter @param {string} nativeEvent */
export function hookEventOf(adapter, nativeEvent) {
  if (!Object.hasOwn(adapter.hookEvents, nativeEvent)) throw relayError('UNSUPPORTED_EVENT', 'unsupported harness event');
  return adapter.hookEvents[nativeEvent];
}
// The nearest process any adapter recognises, skipping index zero (the caller itself).
// See docs/adapters.md ("Codex model resolution", step 3).
/** @param {import("../types.js").ProcessRow[]} chain @param {{executableOf: (pid: number) => string}} ops @returns {{harness: string, index: number}|null} */
export function nearestHarness(chain, ops) {
  for (let index = 1; index < chain.length; index++) {
    for (const [harness, adapter] of Object.entries(adapters)) if (adapter.isHarness(chain[index], ops)) return { harness, index };
  }
  return null;
}
/** @param {string} harness @param {string} nativeEvent @param {unknown} raw */
export function normalizeEvent(harness, nativeEvent, raw) {
  const adapter = adapterFor(harness);
  adapter.stateForEvent(nativeEvent);
  const kind = adapter.events[nativeEvent].kind;
  const data = parsePayload(raw, nativeEvent, adapter.nativeFields);
  const worker = adapter.workerIdOf?.(data, nativeEvent) ?? null, sessionId = worker ?? data.session_id;
  const background = adapter.backgroundOf?.(data, nativeEvent);
  return validateEvent({ schema: 1, kind, harness, nativeEvent, agentId: `${harness}:${sessionId}`, sessionId,
    ...(worker === null ? {} : { root: `${harness}:${data.session_id}` }), cwd: data.cwd,
    ...(Object.hasOwn(data, 'stop_hook_active') ? { stopHookActive: data.stop_hook_active } : {}),
    ...(background === undefined ? {} : { background }),
    tool: kind === 'before-tool' ? { name: data.tool_name, input: data.tool_input,
      command: data.tool_name === 'Bash' && typeof data.tool_input.command === 'string' ? data.tool_input.command : null } : null });
}
