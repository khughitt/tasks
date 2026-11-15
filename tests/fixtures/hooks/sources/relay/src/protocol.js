// @ts-check
import { isAbsolute } from 'node:path';
export const STATES = Object.freeze(['idle', 'working', 'needs-input', 'needs-approval', 'done', 'error']);
export const KINDS = Object.freeze(['session-start', 'prompt', 'before-tool', 'activity', 'idle', 'needs-input', 'needs-approval', 'turn-end', 'turn-error', 'session-end']);
/** @param {string|number|undefined} code @param {string} message @param {unknown} [cause] */
export const relayError = (code, message, cause) => Object.assign(new Error(message, { cause }), { code });
/** @type {(message: string) => never} */
const fail = (message) => { throw relayError('INVALID_SCHEMA', message); };
/** @param {unknown} v @returns {v is Record<string, any>} */
const object = (v) => v !== null && typeof v === 'object' && !Array.isArray(v);
/** @param {unknown} v @returns {v is string} */
const text = (v) => typeof v === 'string' && v.trim().length > 0;
/** @param {unknown} v @returns {v is string} */
const absolute = (v) => text(v) && isAbsolute(v);
/** @param {number} v */
const integer = (v) => Number.isSafeInteger(v) && v >= 0;
/** @param {unknown} v @returns {import("./types.js").Background} */
function counts(v) {
  if (!object(v) || !integer(v.agents) || !integer(v.other)) fail('invalid background');
  return { agents: v.agents, other: v.other };
}
/** @param {unknown} v */
const uuid = (v) => typeof v === 'string' && /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(v);
const harnesses = ['claude-code', 'codex', 'opencode', 'agy'];
/** @param {unknown} text @returns {string} */
export function parseStart(text) {
  if (typeof text !== 'string' || !/^(0|[1-9][0-9]*)$/.test(text)
      || text.length > 20 || BigInt(text) > 18446744073709551615n) {
    throw relayError('INVALID_SCHEMA', 'start must be canonical decimal u64 text');
  }
  return text;
}
/** @param {unknown} v @returns {import("./types.js").Handle} */
export function validateHandle(v) {
  if (!object(v) || !['linux', 'darwin'].includes(v.platform) || !text(v.host)
      || !Number.isInteger(v.pid) || v.pid <= 0 || v.pid > 4294967295
      || !(v.bootId === null || (v.platform === 'linux' && uuid(v.bootId)))) fail('invalid process handle');
  return { platform: v.platform, host: v.host, bootId: v.bootId, pid: v.pid, start: parseStart(v.start) };
}
/** @param {Record<string, any>} v @param {string} id */
function identity(v, id) {
  if (!harnesses.includes(v.harness) || !text(v.sessionId) || id !== `${v.harness}:${v.sessionId}`
      || !absolute(v.cwd)) fail('invalid agent identity');
}
// A Codex worker names its root session's agent id; nothing else carries a root.
// See docs/adapters.md ("Event mappings").
/** @param {unknown} root @param {string} id */
const rootOf = (root, id) => text(root) && root.startsWith('codex:') && text(root.slice('codex:'.length)) && root !== id;
/** @param {unknown} v @param {string} key @returns {import("./types.js").Agent} */
function validateAgent(v, key) {
  if (!object(v)) fail('invalid agent');
  identity(v, v.id);
  if (Object.hasOwn(v, 'background') && v.harness !== 'claude-code') fail('invalid background harness');
  const worker = v.scope === 'worker';
  if (v.id !== key || !(worker ? v.harness === 'codex' && rootOf(v.root, v.id)
      : v.scope === (v.harness === 'opencode' ? 'process' : 'session') && !Object.hasOwn(v, 'root'))
      || !(v.repoRoot === null || absolute(v.repoRoot)) || !(v.remote === null || text(v.remote))
      || !text(v.projectKey) || !text(v.project) || !STATES.includes(v.state) || !integer(v.updatedAt)) fail('invalid agent');
  return { id: v.id, harness: v.harness, sessionId: v.sessionId, scope: v.scope, ...(worker ? { root: v.root } : {}),
    process: v.process === null ? null : validateHandle(v.process), cwd: v.cwd,
    repoRoot: v.repoRoot, remote: v.remote, projectKey: v.projectKey, project: v.project,
    state: v.state, updatedAt: v.updatedAt,
    ...(Object.hasOwn(v, 'background') ? { background: counts(v.background) } : {}) };
}
/** @param {unknown} v @returns {import("./types.js").Snapshot} */
export function validateSnapshot(v) {
  if (!object(v) || v.schema !== 2 || !uuid(v.generation) || !integer(v.revision) || !object(v.agents)) fail('invalid registry snapshot');
  return { schema: 2, generation: v.generation, revision: v.revision,
    agents: Object.fromEntries(Object.entries(v.agents).map(([key, agent]) => [key, validateAgent(agent, key)])) };
}
/** @param {unknown} v @returns {import("./types.js").Event} */
export function validateEvent(v) {
  if (!object(v) || v.schema !== 1 || !KINDS.includes(v.kind) || !text(v.nativeEvent)) fail('invalid event');
  identity(v, v.agentId);
  if (Object.hasOwn(v, 'root') && !(v.harness === 'codex' && rootOf(v.root, v.agentId))) fail('invalid event root');
  if (Object.hasOwn(v, 'stopHookActive') && typeof v.stopHookActive !== 'boolean') fail('invalid stop flag');
  if (Object.hasOwn(v, 'background') && !(v.harness === 'claude-code' && v.nativeEvent === 'Stop')) fail('invalid background event');
  let tool = null;
  if (v.kind === 'before-tool') {
    if (!object(v.tool) || !text(v.tool.name) || !object(v.tool.input)
        || !(v.tool.command === null || typeof v.tool.command === 'string')) fail('invalid tool');
    tool = { name: v.tool.name, input: v.tool.input, command: v.tool.command };
  } else if (v.tool !== null) fail('unexpected tool');
  return { schema: 1, kind: v.kind, harness: v.harness, nativeEvent: v.nativeEvent,
    agentId: v.agentId, sessionId: v.sessionId,
    ...(Object.hasOwn(v, 'root') ? { root: v.root } : {}), cwd: v.cwd,
    ...(Object.hasOwn(v, 'stopHookActive') ? { stopHookActive: v.stopHookActive } : {}),
    ...(Object.hasOwn(v, 'background') ? { background: v.background === null ? null : counts(v.background) } : {}), tool };
}
