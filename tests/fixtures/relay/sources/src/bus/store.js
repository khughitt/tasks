// @ts-check
import * as fs from 'node:fs/promises';
import { dirname } from 'node:path';
import { randomUUID } from 'node:crypto';
import { relayError } from '../protocol.js';
/** @param {string} path @param {boolean} directory @param {import("../types.js").IOOptions} [options] */
export async function checkPrivate(path, directory, { io = fs } = {}) {
  let info;
  try { info = await io.lstat(path); }
  catch (caught) { const cause = /** @type {import('../types.js').Failure} */ (caught); if (cause.code === 'ENOENT') return false; throw relayError('IO_ERROR', 'cannot inspect state path', cause); }
  if (info.isSymbolicLink() || !(directory ? info.isDirectory() : info.isFile())
      || info.uid !== (/** @type {() => number} */ (process.getuid))() || (info.mode & 0o7777) !== (directory ? 0o700 : 0o600)) {
    throw relayError('IO_ERROR', 'state path must be privately owned with safe type and mode');
  }
  return true;
}
/** @param {string} path @param {import("../types.js").IOOptions} [options] */
export async function ensurePrivateDir(path, { io = fs } = {}) {
  if (await checkPrivate(path,true,{io})) return;
  try { await io.mkdir(path,{recursive:true,mode:0o700}); }
  catch (cause) { throw relayError('IO_ERROR','cannot create state directory',cause); }
  await checkPrivate(path,true,{io});
}
/** @param {string} path @param {import("../types.js").IOOptions} [options] */
export async function readJson(path, { io = fs } = {}) {
  if (!await checkPrivate(dirname(path),true,{io})) return undefined;
  if (!await checkPrivate(path,false,{io})) return undefined;
  let text;
  try { text = await io.readFile(path,'utf8'); }
  catch (caught) { const cause = /** @type {import('../types.js').Failure} */ (caught); if (cause.code === 'ENOENT') return undefined; throw relayError('IO_ERROR','cannot read state file',cause); }
  try { return JSON.parse(text); }
  catch (cause) { throw relayError('CORRUPT_STATE','cannot read registry JSON',cause); }
}
/** @param {string} path @param {unknown} value @param {import("../types.js").IOOptions} [options] */
export async function writeJsonAtomic(path, value, { io = fs } = {}) {
  await ensurePrivateDir(dirname(path),{io});
  await checkPrivate(path,false,{io});
  const temp = `${path}.tmp.${process.pid}.${randomUUID()}`;
  let error;
  try {
    await io.writeFile(temp,`${JSON.stringify(value,null,2)}\n`,{flag:'wx',mode:0o600});
    await io.rename(temp,path);
    return;
  } catch (cause) { error = relayError('IO_ERROR','cannot publish state file',cause); }
  try { await io.unlink(temp); }
  catch (caught) { const cause = /** @type {import('../types.js').Failure} */ (caught);
    if (cause.code !== 'ENOENT') throw new AggregateError([error,relayError('IO_ERROR','cannot clean temporary state',cause)],'state write and cleanup failed');
  }
  throw error;
}
