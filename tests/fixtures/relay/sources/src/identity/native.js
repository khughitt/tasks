// Native session evidence and harness-specific effective hints.
import { readFileSync } from 'node:fs';

/** @type {readonly string[]} */
export const NATIVE_VARIABLES = Object.freeze([
  'CLAUDE_CODE_SESSION_ID', 'CODEX_THREAD_ID', 'CODEX_SESSION_ID',
  'ANTIGRAVITY_CONVERSATION_ID', 'PI_SESSION_ID',
]);
/** @type {readonly string[]} */
export const ELIGIBLE = Object.freeze([
  'CLAUDE_CODE_SESSION_ID', 'CODEX_THREAD_ID', 'ANTIGRAVITY_CONVERSATION_ID', 'PI_SESSION_ID',
]);

/**
 * @param {string|undefined} value
 * @param {boolean} decoded
 * @returns {import('../types.js').NativeEntry}
 */
function classify(value, decoded) {
  if (value === undefined) return { state: 'absent' };
  if (value === '') return { state: 'empty' };
  if (decoded && value.includes('\uFFFD')) return { state: 'unverified', value };
  if (/\p{Cc}/u.test(value)) return { state: 'invalid', reason: 'control-characters', value };
  return { state: 'present', value };
}

/**
 * @param {{platform?: string, readRaw?: () => Buffer, env?: Record<string, string|undefined>}} [options]
 * @returns {{native: Record<string, import('../types.js').NativeEntry>, nativeSource: 'raw'|'decoded', nativeSession: boolean, rawError: string|null}}
 */
export function readNative({ platform = process.platform, readRaw = () => readFileSync('/proc/self/environ'), env = process.env } = {}) {
  /** @type {'raw'|'decoded'} */
  let nativeSource = 'decoded';
  /** @type {string|null} */
  let rawError = null;
  /** @type {Buffer|undefined} */
  let bytes;
  if (platform === 'linux') {
    try {
      bytes = readRaw();
      nativeSource = 'raw';
    } catch (error) {
      rawError = error !== null && typeof error === 'object' && 'code' in error && typeof error.code === 'string'
        ? error.code : 'UNKNOWN';
    }
  }

  /** @type {Record<string, import('../types.js').NativeEntry>} */
  const native = {};
  if (bytes !== undefined) {
    const values = new Map();
    for (let start = 0; start < bytes.length;) {
      const nul = bytes.indexOf(0, start);
      const end = nul === -1 ? bytes.length : nul;
      const entry = bytes.subarray(start, end);
      const equal = entry.indexOf(61);
      if (equal !== -1) {
        const key = entry.subarray(0, equal).toString('utf8');
        if (NATIVE_VARIABLES.includes(key) && !values.has(key)) {
          values.set(key, entry.subarray(equal + 1));
        }
      }
      start = end + 1;
    }
    const decoder = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });
    for (const key of NATIVE_VARIABLES) {
      if (!values.has(key)) {
        native[key] = { state: 'absent' };
        continue;
      }
      try {
        native[key] = classify(decoder.decode(values.get(key)), false);
      } catch {
        native[key] = { state: 'invalid', reason: 'not-unicode' };
      }
    }
  } else {
    for (const key of NATIVE_VARIABLES) native[key] = classify(env[key], true);
  }
  const nativeSession = ELIGIBLE.some((key) => !['absent', 'empty'].includes(native[key].state));
  return { native, nativeSource, nativeSession, rawError };
}

/**
 * @param {import('../types.js').Harness} harness
 * @param {Record<string, import('../types.js').NativeEntry>} native
 * @returns {{hint: string|null, invalid: string|null, missing: boolean}}
 */
export function hintFor(harness, native) {
  if (harness === 'claude-code') {
    const entry = native.CLAUDE_CODE_SESSION_ID;
    const hint = entry.state === 'present' || (entry.state === 'invalid' && entry.reason === 'control-characters')
      ? entry.value : null;
    return { hint, invalid: null, missing: false };
  }
  const keys = harness === 'codex' ? ['CODEX_THREAD_ID', 'CODEX_SESSION_ID']
    : harness === 'agy' ? ['ANTIGRAVITY_CONVERSATION_ID'] : harness === 'pi' ? ['PI_SESSION_ID'] : [];
  const invalid = keys.find((key) => ['invalid', 'unverified'].includes(native[key].state));
  if (invalid) return { hint: null, invalid, missing: false };
  for (const key of keys) {
    const entry = native[key];
    if (entry.state === 'present') return { hint: entry.value, invalid: null, missing: false };
  }
  return { hint: null, invalid: null, missing: harness === 'agy' || harness === 'pi' };
}
