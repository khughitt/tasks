// @ts-check
import { relayError } from '../protocol.js';
// Claude Code caps hook strings at 10,000 characters and Codex spills large output to a
// file (both references checked 2026-09-17). Relay treats more as an error, never truncation.
export const CONTEXT_LIMIT = 10000;
const NONE = Object.freeze({ refuse: false, context: false });
const FIELDS = ['decision', 'reason', 'context'];
/** @param {unknown} v @returns {v is Record<string, any>} */
const object = (v) => v !== null && typeof v === 'object' && !Array.isArray(v);
/** @param {unknown} v @returns {v is string} */
const text = (v) => typeof v === 'string' && v.trim().length > 0;
/** @type {(message: string, cause?: unknown) => never} */
const fail = (message, cause) => { throw relayError('INVALID_REPLY', message, cause); };
/** @param {string} output @param {{limit?: number}} [options] @returns {import("../types.js").Reply} */
export function parseReply(output, { limit = CONTEXT_LIMIT } = {}) {
  let value;
  try { value = JSON.parse(output); } catch (cause) { fail('subscriber reply is not JSON', cause); }
  if (!object(value)) fail('subscriber reply must be a JSON object');
  const extra = Object.keys(value).find((k) => !FIELDS.includes(k));
  // The field name is the subscriber's output, which may echo private input: never repeat it.
  if (extra !== undefined) fail('unexpected reply field');
  if (!['allow', 'deny'].includes(value.decision)) fail('reply decision must be allow or deny');
  for (const field of ['reason', 'context']) {
    if (Object.hasOwn(value, field) && (!text(value[field]) || value[field].length > limit)) fail(`reply ${field} must be nonempty text of at most ${limit} characters`);
  }
  if (value.decision === 'deny' && !Object.hasOwn(value, 'reason')) fail('a denial requires a reason');
  return { decision: value.decision, ...(Object.hasOwn(value, 'reason') ? { reason: value.reason } : {}),
    ...(Object.hasOwn(value, 'context') ? { context: value.context } : {}) };
}
// The standard decision model both Claude Code and Codex document: PreToolUse answers inside
// hookSpecificOutput; UserPromptSubmit and Stop use top-level decision/reason. A refusal is
// written as JSON and as exit 2 with the reason on stderr, which both harnesses honor together.
/** @param {string} nativeEvent @param {import("../types.js").Reply} reply @param {import("../types.js").Capability} [capability] */
export function encodeStandardReply(nativeEvent, reply, capability = NONE) {
  /** @param {unknown} v */
  const line = (v) => `${JSON.stringify(v)}\n`;
  if (reply.decision === 'deny') {
    if (!capability.refuse) throw relayError('UNSUPPORTED_REPLY', `${nativeEvent} has no refusal channel`);
    const stdout = nativeEvent === 'PreToolUse'
      ? line({ hookSpecificOutput: { hookEventName: 'PreToolUse', permissionDecision: 'deny', permissionDecisionReason: reply.reason } })
      : line({ decision: 'block', reason: reply.reason });
    return { stdout, stderr: `${reply.reason}\n`, exitCode: 2 };
  }
  if (reply.context === undefined) return { stdout: '', stderr: '', exitCode: 0 };
  if (!capability.context) throw relayError('UNSUPPORTED_REPLY', `${nativeEvent} has no context channel`);
  return { stdout: line({ hookSpecificOutput: { hookEventName: nativeEvent, additionalContext: reply.context } }), stderr: '', exitCode: 0 };
}
