// @ts-check
import { basename } from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { relayError } from '../protocol.js';
const defaultExec = promisify(execFile);
/** @param {unknown} url */
export function normalizeRemote(url) {
  if (typeof url !== 'string' || !url.trim()) return null;
  let rest;
  if (/^[a-zA-Z][a-zA-Z0-9+.-]*:\/\//.test(url.trim())) {
    // URL parses the complete authority, including credentials containing @.
    let parsed;
    try { parsed = new URL(url.trim()); } catch { return null; }
    rest = `${parsed.hostname}${parsed.pathname}`;
  } else {
    rest = url.trim().split(/[?#]/, 1)[0].replace(/^[^/]*@/, '').replace(/:/, '/');
  }
  const parts = rest.replace(/\/+$/, '').replace(/\.git$/i, '').split('/').filter(Boolean);
  if (parts.length < 2 || !parts[0].includes('.')) return null;
  return parts.join('/').toLowerCase();
}
/** @param {string} cwd */
export async function gitContext(cwd, { exec = defaultExec, clock = performance.now.bind(performance), deadline = clock() + 12000, timeoutMs = 2000 } = {}) {
  /** @param {string[]} args */
  const run = async (args) => {
    const remaining = Math.min(2000, timeoutMs, deadline - clock());
    if (remaining <= 0) throw relayError('GIT_ERROR', 'repository deadline exceeded');
    return exec('git', ['-C', cwd, ...args], { timeout: Math.max(1, Math.floor(remaining)),
      killSignal: 'SIGKILL', maxBuffer: 4 * 1024 * 1024, encoding: 'utf8', env: { ...process.env, LC_ALL: 'C' } });
  };
  let repoRoot;
  try { repoRoot = (await run(['rev-parse', '--show-toplevel'])).stdout.trim(); }
  catch (caught) { const cause = /** @type {import('../types.js').Failure} */ (caught);
    if (cause.code === 128 && !cause.killed && !cause.signal && /^fatal: not a git repository(?: \(|:)/.test(cause.stderr ?? '')) return { repoRoot: null, remote: null };
    throw relayError('GIT_ERROR', 'cannot resolve repository', cause);
  }
  if (!repoRoot.startsWith('/')) throw relayError('GIT_ERROR', 'invalid repository root');
  let remote;
  try { remote = normalizeRemote((await run(['config', '--get', 'remote.origin.url'])).stdout); }
  catch (caught) { const cause = /** @type {import('../types.js').Failure} */ (caught);
    if (cause.code === 1 && !cause.killed && !cause.signal && !(cause.stderr ?? '').trim()) remote = null;
    else throw relayError('GIT_ERROR', 'cannot resolve repository remote', cause);
  }
  return { repoRoot, remote };
}
/** @param {{remote: string|null, repoRoot: string|null, cwd: string}} context */
export const projectKeyFor = ({ remote, repoRoot, cwd }) => remote ?? repoRoot ?? cwd;
/** @param {{repoRoot: string|null, cwd: string, remote?: string|null}} context */
export const displayProject = ({ repoRoot, cwd }) => basename(repoRoot ?? cwd) || '/';
