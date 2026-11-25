// @ts-check
import * as fs from 'node:fs/promises';
import { dirname } from 'node:path';
import { randomUUID } from 'node:crypto';
import { isDeepStrictEqual } from 'node:util';
import { relayError, validateHandle } from '../protocol.js';
import { DEADLINE_CUT, createProcessOps } from './proc.js';
import { checkPrivate, ensurePrivateDir } from './store.js';

/** @param {string} path @param {typeof fs} io */
async function readToken(path, io) {
  if (!await checkPrivate(path,false,{io})) return null;
  let text;
  try { text = await io.readFile(path,'utf8'); }
  catch (caught) { const cause = /** @type {import('../types.js').Failure} */ (caught); if (cause.code === 'ENOENT') return null; throw relayError('IO_ERROR','cannot read lock owner',cause); }
  try {
    const token = JSON.parse(text);
    if (!token || token.schema !== 1 || typeof token.nonce !== 'string'
        || !/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(token.nonce)) throw new Error('invalid token');
    validateHandle(token.holder);
    return token;
  } catch (cause) { throw relayError('LOCK_REPAIR_REQUIRED','invalid lock owner token; operator repair required',cause); }
}
/** @param {string} path @param {unknown} token @param {typeof fs} io */
async function release(path, token, io) {
  try {
    const current = await readToken(path,io);
    if (!isDeepStrictEqual(current,token)) return relayError('LOCK_OWNERSHIP_LOST','lock ownership changed');
    await io.unlink(path);
    return null;
  } catch (cause) { return relayError('LOCK_CLEANUP_FAILED','cannot release lock',cause); }
}
/** @param {string} path @param {unknown} token @param {typeof fs} io */
async function acquire(path, token, io) {
  const temp = `${path}.tmp.${process.pid}.${randomUUID()}`;
  let acquired = false, failure;
  try {
    await io.writeFile(temp,JSON.stringify(token),{flag:'wx',mode:0o600});
    try { await io.link(temp,path); acquired = true; }
    catch (caught) { const cause = /** @type {import('../types.js').Failure} */ (caught); if (cause.code !== 'EEXIST') throw cause; }
  } catch (cause) { failure = relayError('IO_ERROR','cannot publish lock token',cause); }
  try { await io.unlink(temp); }
  catch (caught) { const cause = /** @type {import('../types.js').Failure} */ (caught);
    if (cause.code !== 'ENOENT') {
      const cleanup = relayError('LOCK_CLEANUP_FAILED','cannot clean private lock token',cause);
      const errors = [failure,cleanup].filter(Boolean);
      if (acquired) { const error = await release(path,token,io); if (error) errors.push(error); }
      if (errors.length > 1) throw new AggregateError(errors,'lock token cleanup failed');
      throw cleanup;
    }
  }
  if (failure) throw failure;
  return acquired;
}
/** @template T @param {string} lockPath @param {() => Promise<T>} fn @param {import("../types.js").Deadline & import("../types.js").IOOptions & {processOps?: import("../types.js").ProcessOps, sleep?: (ms: number) => Promise<void>}} [options] @returns {Promise<{value: T, cleanupError: Error & {code: string|number|undefined}|null}>} */
export async function withLock(lockPath, fn, { clock = performance.now.bind(performance), deadline = clock() + 12000,
  processOps = createProcessOps(), sleep = ms => new Promise(resolve => setTimeout(resolve,ms)), io = fs } = {}) {
  let holder;
  try { holder = validateHandle(processOps.handleOf(process.pid,{clock,deadline})); }
  catch (cause) { throw relayError('LOCK_IDENTITY_UNKNOWN','writer identity unavailable',cause); }
  if ((holder.platform === 'linux' && !holder.bootId) || processOps.probe(holder,{clock,deadline}).status !== 'alive') {
    throw relayError('LOCK_IDENTITY_UNKNOWN','writer identity unavailable');
  }
  await ensurePrivateDir(dirname(lockPath),{io});
  let unknown = false, token;
  // A probe the deadline cut short learned nothing, so the last completed verdict stands: a
  // live owner at the deadline is LOCK_TIMEOUT, not LOCK_IDENTITY_UNKNOWN.
  /** @param {unknown} handle */
  const observe = (handle) => {
    const { status, reason } = processOps.probe(handle,{clock,deadline});
    if (reason !== DEADLINE_CUT) unknown = status === 'unknown';
    return status;
  };
  const expired = () => {
    if (clock() >= deadline) throw relayError(unknown ? 'LOCK_IDENTITY_UNKNOWN' : 'LOCK_TIMEOUT',unknown ? 'lock owner identity unavailable' : 'lock acquisition deadline exceeded');
  };
  const pause = async () => { expired(); await sleep(Math.min(20,deadline-clock())); };
  for (;;) {
    expired();
    token = {schema:1,nonce:randomUUID(),holder};
    if (await acquire(lockPath,token,io)) break;
    const owner = await readToken(lockPath,io);
    if (!owner) continue;
    const status = observe(owner.holder);
    if (status !== 'dead') { await pause(); continue; }
    const guardPath = `${lockPath}.reclaim`, guard = {schema:1,nonce:randomUUID(),holder};
    if (!await acquire(guardPath,guard,io)) {
      const current = await readToken(guardPath,io);
      if (!current) continue;
      if (observe(current.holder) === 'dead') {
        // Only a holder that died holding the guard leaves its token behind; a changed or missing
        // token was released during the probe.
        if (isDeepStrictEqual(await readToken(guardPath,io),current)) throw relayError('LOCK_REPAIR_REQUIRED','abandoned reclaim guard; operator repair required');
        continue;
      }
      await pause(); continue;
    }
    let error;
    try {
      // Probe, then confirm the token is unchanged: its holder may have released it, and a new
      // writer taken the lock, while the probe ran. Once the holder is dead, only a guard holder
      // can change a file that still holds its token (docs/registry.md, "Lock protocol and stale-lock reclamation").
      const current = await readToken(lockPath,io);
      if (current && observe(current.holder) === 'dead'
          && isDeepStrictEqual(await readToken(lockPath,io),current)) await io.unlink(lockPath);
    } catch (caught) { const cause = /** @type {import('../types.js').Failure} */ (caught); error = (/** @type {string|undefined} */ (cause.code))?.startsWith('LOCK_') ? cause : relayError('IO_ERROR','cannot reclaim lock',cause); }
    const cleanup = await release(guardPath,guard,io);
    if (error && cleanup) throw new AggregateError([error,cleanup],'reclamation and cleanup failed');
    if (cleanup) throw relayError('LOCK_CLEANUP_FAILED','cannot release reclaim guard',cleanup);
    if (error) throw error;
  }
  let value, error, failed = false;
  try { expired(); value = await fn(); } catch (caught) { const cause = /** @type {import('../types.js').Failure} */ (caught); failed = true; error = cause; }
  const cleanupError = await release(lockPath,token,io);
  if (failed && cleanupError) throw new AggregateError([error,cleanupError],'callback and lock cleanup failed');
  if (failed) throw error;
  return {value: /** @type {T} */ (value),cleanupError};
}
