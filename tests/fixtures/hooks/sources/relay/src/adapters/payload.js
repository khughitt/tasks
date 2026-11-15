// @ts-check
import { isAbsolute } from 'node:path';
import { relayError } from '../protocol.js';
const MAX_BYTES = 4 * 1024 * 1024;
// The bounded outer read shared by registry and hook normalization: at most 4 MiB of strict
// UTF-8 JSON whose top level is an object. Field validation belongs to each caller.
/** @param {unknown} raw @returns {Record<string, any>} */
export function parseNativeObject(raw) {
  if (!(typeof raw === 'string' || Buffer.isBuffer(raw))) throw relayError('INVALID_INPUT', 'native input must be bytes or text');
  if (Buffer.byteLength(raw) > MAX_BYTES) throw relayError('INPUT_TOO_LARGE', 'native input exceeds byte limit');
  let data;
  try { data = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(Buffer.isBuffer(raw) ? raw : Buffer.from(raw))); }
  catch (cause) { throw relayError('INVALID_INPUT', 'native input is not valid UTF-8 JSON', cause); }
  if (!data || typeof data !== 'object' || Array.isArray(data)) throw relayError('INVALID_INPUT', 'native input must be a JSON object');
  return data;
}
/** @param {unknown} raw @param {string} nativeEvent @param {import("../types.js").Adapter['nativeFields']} [nativeFields] @returns {Record<string, any>} */
export function parsePayload(raw, nativeEvent, nativeFields) {
  let data = parseNativeObject(raw);
  if (nativeFields) data = nativeFields(data, nativeEvent);
  if (typeof data.session_id !== 'string' || !data.session_id.trim()
      || typeof data.cwd !== 'string' || !isAbsolute(data.cwd)) throw relayError('INVALID_INPUT', 'native input requires session and absolute cwd');
  if (Object.hasOwn(data, 'stop_hook_active') && typeof data.stop_hook_active !== 'boolean') throw relayError('INVALID_INPUT', 'invalid stop flag');
  if (Object.hasOwn(data, 'hook_event_name') && data.hook_event_name !== nativeEvent.split(':')[0]) throw relayError('INVALID_INPUT', 'native event does not match argv');
  if (nativeEvent === 'PreToolUse' && (typeof data.tool_name !== 'string' || !data.tool_name.trim()
      || !data.tool_input || typeof data.tool_input !== 'object' || Array.isArray(data.tool_input))) throw relayError('INVALID_INPUT', 'invalid native tool fields');
  return data;
}
/** @param {import("node:stream").Readable} stream @param {import("../types.js").Deadline} [options] @returns {Promise<Buffer>} */
export function readNativeInput(stream, { clock = performance.now.bind(performance), deadline } = {}) {
  const started = clock();
  const effectiveDeadline = Math.min(deadline ?? Infinity, started + 5000);
  const remaining = effectiveDeadline - started;
  if (remaining <= 0) return Promise.reject(relayError('INPUT_TIMEOUT', 'native input deadline exceeded'));
  return new Promise((resolve, reject) => {
    /** @type {Buffer[]} */
    const chunks = []; let size = 0, finished = false;
    /** @param {Error} [error] */
    const done = (error) => {
      if (finished) return; finished = true;
      if (!error && clock() >= effectiveDeadline) error = relayError('INPUT_TIMEOUT', 'native input deadline exceeded');
      clearTimeout(timer);
      stream.removeListener('data', data); stream.removeListener('end', end);
      stream.removeListener('error', failed); stream.removeListener('close', closed);
      stream.pause();
      if (error) reject(error); else resolve(Buffer.concat(chunks, size));
    };
    /** @param {Buffer|string} chunk */
    const data = (chunk) => {
      if (clock() >= effectiveDeadline) return done(relayError('INPUT_TIMEOUT', 'native input deadline exceeded'));
      const bytes = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
      size += bytes.length;
      if (size > MAX_BYTES) return done(relayError('INPUT_TOO_LARGE', 'native input exceeds byte limit'));
      chunks.push(bytes);
    };
    const end = () => done();
    /** @param {unknown} cause */
    const failed = (cause) => done(relayError('INVALID_INPUT', 'cannot read native input', cause));
    const closed = () => done(relayError('INVALID_INPUT', 'native input closed before EOF'));
    const timer = setTimeout(() => done(relayError('INPUT_TIMEOUT', 'native input deadline exceeded')), remaining);
    stream.on('data', data); stream.once('end', end); stream.once('error', failed); stream.once('close', closed);
    if (stream.readableEnded) done();
    else if (stream.destroyed) closed();
  });
}
