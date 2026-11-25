// Resolve the nearest harness boundary without borrowing an outer session's identity.
import { adapterFor } from '../adapters/index.js';
import { ROLES } from '../adapters/claude-code.js';
import { relayError } from '../protocol.js';

/** @type {readonly import('../types.js').Harness[]} */
const HARNESSES = ['claude-code', 'codex', 'opencode', 'agy', 'pi'];
/** @typedef {{kind: 'outside'} | {kind: 'boundary', harness: import('../types.js').Harness, pid: number, ppid: number, start: string, role: 'session'|'daemon'|'bg-pty-host'}} WalkResult */

/**
 * @param {{startPid?: number, processOps: import('../types.js').ProcessOps}} options
 * @returns {WalkResult}
 */
export function harnessWalk({ startPid = process.pid, processOps }) {
  const seen = new Set();
  /** @param {number} pid */
  const read = (pid) => {
    if (seen.has(pid)) throw relayError('PROCESS_UNREADABLE', 'process ancestor loop');
    seen.add(pid);
    const row = processOps.recordOf(pid);
    if (!row) throw relayError('PROCESS_UNREADABLE', 'process ancestor vanished');
    return row;
  };
  try {
    // Index zero is the caller, even when its comm names a harness.
    const caller = read(startPid);
    if (caller.pid === 1 || caller.ppid === 0) return { kind: 'outside' };
    let pid = caller.ppid;
    for (let depth = 0; depth < 64; depth++) {
      const row = read(pid);
      const harness = HARNESSES.find(name => adapterFor(name).isHarness(row, {
        executableOf: id => processOps.executableOf(id),
      }));
      if (harness) {
        const role = harness === 'claude-code' ? processOps.argvOf(pid)[1] : 'session';
        return { kind: 'boundary', harness, pid: row.pid, ppid: row.ppid, start: row.starttime,
          role: ROLES.includes(role) ? /** @type {'daemon'|'bg-pty-host'} */ (role) : 'session' };
      }
      if (row.pid === 1 || row.ppid === 0) return { kind: 'outside' };
      pid = row.ppid;
    }
    throw relayError('PROCESS_UNREADABLE', 'process ancestry exceeds 64 levels');
  } catch (cause) {
    throw relayError('PROCESS_UNREADABLE', 'cannot read process ancestry', cause);
  }
}
