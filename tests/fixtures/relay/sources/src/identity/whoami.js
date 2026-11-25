// Resolve a caller's nearest harness process to its registered session.
import { isDeepStrictEqual } from 'node:util';
import { createProcessOps } from '../bus/proc.js';
import { readSnapshot } from '../bus/transaction.js';
import { relayError } from '../protocol.js';
import { readNative, hintFor } from './native.js';
import { harnessWalk } from './walk.js';

/** @param {string} text */
export const printable = text => /\p{Cc}/u.test(text) ? '[control characters]' : text;

/**
 * @param {import('../types.js').Agent[]} agents
 * @param {import('../types.js').Handle} process
 * @param {import('../types.js').Harness} harness
 * @param {string|null} hint
 */
function noMatch(agents, process, harness, hint) {
  for (const agent of agents) {
    const handle = agent.process;
    if (!handle || handle.host !== process.host || handle.pid !== process.pid || handle.start !== process.start) continue;
    if (agent.harness !== harness) return `the agent on pid ${process.pid} is ${printable(agent.harness)}, but the nearest harness is ${harness}`;
    if (handle.platform !== process.platform) return `agent ${printable(agent.id)} has a ${printable(handle.platform)} handle, which is not an identity candidate`;
    if (handle.bootId !== process.bootId) return `agent ${printable(agent.id)} was recorded on an earlier boot`;
  }
  const headless = agents.find(agent => agent.process === null && agent.harness === harness && agent.sessionId === hint);
  if (headless) return `agent ${printable(headless.id)} is this ${harness} session but relay recorded no process handle for it: relay's hook found no ${harness} session process among its ancestors, or found only a daemon or pty host`;
  return `no relay agent matches the nearest ${harness} ancestor, pid ${process.pid}`;
}

/**
 * @param {import('../types.js').Harness} harness
 * @param {string|null} root
 * @param {import('../types.js').Agent[]} candidates
 */
function hintMismatch(harness, root, candidates) {
  if (harness === 'agy') return 'relay has no row for this agy conversation (relay has not yet seen its first hook, or retired its row), so it cannot claim as another conversation on the same agy process';
  if (harness === 'codex' && root !== null && candidates.some(agent => agent.sessionId === root)) {
    return candidates.some(agent => agent.scope === 'worker')
      ? "relay has no row for this Codex worker thread (relay has not yet seen this worker's first hook, or pruned its row), so the worker cannot claim as its controller"
      : 'relay has no row for this Codex worker thread (the relay writing the registry predates worker rows, or its Codex hooks are not installed), so the worker cannot claim as its controller';
  }
  return 'the native actor hint does not match any qualifying relay row';
}

/**
 * @param {import('../types.js').Deadline & {startPid?: number, processOps?: import('../types.js').ProcessOps, paths?: import('../types.js').Paths, readRegistry?: () => Promise<import('../types.js').Snapshot|null>, platform?: string, readRaw?: () => Buffer, env?: Record<string,string|undefined>}} [options]
 * @returns {Promise<import('../types.js').WhoamiResult>}
 */
export async function whoami({ startPid, processOps, paths, readRegistry, platform = process.platform, readRaw, env, clock, deadline } = {}) {
  const { native, nativeSource, nativeSession, rawError } = readNative({ platform, readRaw, env });
  const evidence = { native, nativeSource, nativeSession };
  try {
    if (!['linux', 'darwin'].includes(platform)) throw relayError('UNSUPPORTED_PLATFORM', 'unsupported identity platform');
    if (platform === 'linux' && rawError !== null) throw relayError('ENVIRONMENT_UNREADABLE', 'cannot read raw process environment');
    const ops = processOps ?? createProcessOps({ platform });
    const walk = harnessWalk({ startPid, processOps: ops });
    if (walk.kind === 'outside') return { schema: 1, status: 'outside', ...evidence };
    let process;
    try { process = ops.handleOf(walk.pid); }
    catch (cause) { throw relayError('PROCESS_UNREADABLE', 'cannot read harness process handle', cause); }
    if (process === null || process.start !== walk.start) throw relayError('PROCESS_UNREADABLE', 'harness process vanished or changed after the ancestry walk');
    if (platform === 'linux' && process.bootId === null) throw relayError('BOOT_ID_UNREADABLE', 'the host boot id is unreadable');
    const { harness } = walk;
    const boundary = { harness, role: walk.role, process };
    /** @type {string|null} */
    let hint = null;
    /** @param {string} code @param {string} message @returns {import('../types.js').WhoamiResult} */
    const unresolved = (code, message) => ({ schema: 1, status: 'unresolved', harness, boundary, hint, ...evidence, code, message });
    if (walk.role !== 'session') return unresolved('HOSTING_PROCESS', `the nearest Claude Code process, pid ${walk.pid}, is its ${walk.role} process, not a session`);
    if (walk.pid === 1 && walk.ppid === 0) return unresolved('HIDDEN_ANCESTRY', 'the nearest harness ancestor is pid 1 with no parent, so this PID namespace hides the harness session process');
    const effective = hintFor(harness, native);
    hint = effective.hint;
    if (effective.invalid !== null) return unresolved('INVALID_NATIVE_VALUE', `${effective.invalid} is invalid or unverified`);
    if (effective.missing) return unresolved('MISSING_HINT', harness === 'pi'
      ? 'PI_SESSION_ID is missing, and a session this pi process has replaced cannot be told from the current one without it'
      : 'ANTIGRAVITY_CONVERSATION_ID is missing, and conversations sharing the agy process cannot be told apart without it');
    let snapshot;
    try { snapshot = await (readRegistry ?? (() => readSnapshot({ paths, processOps: ops, clock, deadline })))(); }
    catch (cause) {
      const code = cause !== null && typeof cause === 'object' && 'code' in cause && typeof cause.code === 'string' ? cause.code : 'UNKNOWN';
      return unresolved('REGISTRY_UNREADABLE', `cannot read registry: ${printable(code)}`);
    }
    const agents = Object.values(snapshot?.agents ?? {});
    const candidates = agents.filter(agent => agent.harness === harness && isDeepStrictEqual(agent.process, process));
    const root = harness === 'codex' && native.CODEX_SESSION_ID.state === 'present' ? native.CODEX_SESSION_ID.value : null;
    if (candidates.length === 0) return unresolved('NOT_REGISTERED', noMatch(agents, process, harness, hint));
    let agent;
    if (candidates.length === 1) agent = candidates[0];
    else if ((harness === 'codex' || harness === 'agy' || harness === 'pi') && hint !== null) {
      agent = candidates.find(candidate => candidate.sessionId === hint);
      if (!agent) return unresolved('HINT_MISMATCH', hintMismatch(harness, root, candidates));
    } else return unresolved('AMBIGUOUS', `several relay agents match pid ${walk.pid}: ${candidates.map(candidate => printable(candidate.id)).join(', ')}`);
    if (hint !== null && hint !== agent.sessionId) return unresolved('HINT_MISMATCH', hintMismatch(harness, root, [agent]));
    if (agent.scope === 'worker' && (native.CODEX_THREAD_ID.state !== 'present' || native.CODEX_THREAD_ID.value !== agent.sessionId)) {
      return unresolved('WORKER_NEEDS_THREAD', "the qualifying relay row is a Codex worker row, and only that worker's own CODEX_THREAD_ID selects it; this caller exports none");
    }
    return { schema: 1, status: 'resolved', harness, boundary, hint, ...evidence, agent, rootSession: root === null ? null : `codex:${root}` };
  } catch (cause) {
    const error = /** @type {import('../types.js').Failure} */ (cause);
    throw Object.assign(relayError(error.code === 'BOOT_ID_UNREADABLE' || error.code === 'ENVIRONMENT_UNREADABLE' || error.code === 'UNSUPPORTED_PLATFORM'
      ? error.code : 'PROCESS_UNREADABLE', printable(error.message), cause), evidence);
  }
}
