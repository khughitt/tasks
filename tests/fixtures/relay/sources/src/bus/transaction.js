// @ts-check
import { randomUUID } from 'node:crypto';
import { isAbsolute, join } from 'node:path';
import { relayError, validateEvent, validateSnapshot } from '../protocol.js';
import { adapterFor } from '../adapters/index.js';
import { classifyConversation, conversationDatabase } from '../adapters/agy-conversation.js';
import { paths as defaultPaths } from './paths.js';
import { createProcessOps } from './proc.js';
import { gitContext as defaultGitContext, projectKeyFor, displayProject } from './identity.js';
import { readJson, writeJsonAtomic } from './store.js';
import { withLock } from './lock.js';
import { pruneDead } from './prune.js';
const FINISHED = new Set(['done', 'error']);
const MAX_PEER_CLASSIFICATIONS = 4;
/** @param {unknown} value */
const isValidDir = (value) => typeof value === 'string' && value.length > 0 && isAbsolute(value) && !value.includes('\0');
/** @param {ReturnType<typeof options>} opts */
function resolveAgyAppDataDir(opts) {
  if (opts.agyAppDataDir !== undefined) return isValidDir(opts.agyAppDataDir) ? opts.agyAppDataDir : null;
  if (opts.env.ANTIGRAVITY_APP_DATA_DIR !== undefined) return isValidDir(opts.env.ANTIGRAVITY_APP_DATA_DIR) ? opts.env.ANTIGRAVITY_APP_DATA_DIR : null;
  if (isValidDir(opts.env.HOME)) return join(/** @type {string} */ (opts.env.HOME), '.gemini', 'antigravity-cli');
  return null;
}
/** @param {import("../types.js").RegistryOptions} given */
function options(given) {
  const clock = given.clock ?? performance.now.bind(performance);
  return { ...given, clock, deadline:given.deadline ?? clock()+12000, paths:given.paths ?? defaultPaths(),
    processOps:given.processOps ?? createProcessOps(), now:given.now ?? Date.now, uuid:given.uuid ?? randomUUID,
    env:given.env ?? process.env };
}
// Schema 1 published the terminal owner's pid; schema 2 publishes the session's own process.
// A schema-1 row can hold a handle borrowed from another session, and which of two rows sharing
// a handle is rightful cannot be told, so its rows are dropped rather than migrated. Spec:
// docs/registry.md, "Schema-1 supersession".
const SUPERSEDED_MESSAGE = 'registry predates session handles; run relay reap or wait for any hook event';
/** @param {any} value */
const isSchemaOne = (value) => value !== null && typeof value === 'object' && !Array.isArray(value) && value.schema === 1;
/** @param {ReturnType<typeof options>} opts */
async function read(opts) {
  const value = await readJson(opts.paths.agentsPath,opts);
  if (value === undefined) return null;
  if (isSchemaOne(value)) throw relayError('REGISTRY_SUPERSEDED',SUPERSEDED_MESSAGE);
  return validateSnapshot(value);
}
/** @param {ReturnType<typeof options>} opts @param {import("../types.js").Diagnostic[]} diagnostics */
async function readForMutation(opts, diagnostics) {
  const value = await readJson(opts.paths.agentsPath,opts);
  if (value === undefined) return {before:null,superseded:false};
  if (!isSchemaOne(value)) return {before:validateSnapshot(value),superseded:false};
  const agents = value.agents;
  const ids = agents !== null && typeof agents === 'object' && !Array.isArray(agents) ? Object.keys(agents) : [];
  diagnostics.push(...(ids.length === 0
    ? [{code:'REGISTRY_SUPERSEDED',message:'replaced a schema-1 registry with no agents'}]
    : ids.map(agentId=>({code:'REGISTRY_SUPERSEDED',message:`dropped ${agentId} from a superseded schema-1 registry`,agentId}))));
  return {before:null,superseded:true};
}
/** @param {import("../types.js").RegistryOptions} [given] */
export async function readSnapshot(given = {}) { return read(options(given)); }

// The current validated snapshot read under the writer lock, so no mutation is in flight. Nothing
// but this read runs under the lock: callers apply what they learn after it is released.
/** @param {ReturnType<typeof options>} opts */
async function authoritative(opts) {
  if (opts.clock() >= opts.deadline) throw relayError('DEADLINE_EXCEEDED','registry read deadline exceeded');
  const locked = await withLock(opts.paths.lockPath,async () => read(opts),opts);
  return locked.value;
}
/** @param {import("../types.js").RegistryOptions} [given] @returns {Promise<import("../types.js").Snapshot|null>} */
export async function readAuthoritative(given = {}) { return authoritative(options(given)); }

/**
 * The consumption primitive for snapshot-derived observers: the current snapshot when its generation
 * equals the given one, otherwise null (a different or absent registry). Generation ids are never
 * ordered; a consumer adopts a changed generation only from this read. Errors stay explicit.
 * @param {{generation: string}} position @param {import("../types.js").RegistryOptions} [given]
 * @returns {Promise<import("../types.js").Snapshot|null>}
 */
export async function readSnapshotFor({ generation }, given = {}) {
  const snapshot = await authoritative(options(given));
  return snapshot !== null && snapshot.generation === generation ? snapshot : null;
}
/** @param {import("../types.js").RegistryOptions} [given] */
export async function listAgents(given = {}) {
  const opts = options(given), snapshot = await read(opts);
  const liveness = Object.fromEntries(Object.entries(snapshot?.agents ?? {}).map(([id,a]) => [id,opts.processOps.probe(a.process,opts)]));
  return {snapshot,liveness};
}
/** @param {Record<string, import("../types.js").Liveness>} liveness */
const diagnosticsFor = (liveness) => Object.entries(liveness).filter(([,v])=>v.status === 'unknown')
  .map(([agentId])=>({code:'PROCESS_UNAVAILABLE',message:'agent liveness unknown',agentId}));
/** @param {import('../types.js').Handle|null} a @param {import('../types.js').Handle|null} b */
const sameProcess = (a,b) => a !== null && b !== null && (a.platform !== 'linux' || a.bootId !== null)
  && a.platform === b.platform && a.host === b.host && a.bootId === b.bootId && a.pid === b.pid && a.start === b.start;
/** @param {ReturnType<typeof options>} opts @param {import("../types.js").Event|null} event @param {{remote: string|null, repoRoot: string|null}|null} context @param {import("../types.js").Handle|null} handle @param {import("../types.js").Diagnostic[]} diagnostics @returns {Promise<import("../types.js").RegistryResult>} */
async function mutate(opts, event, context, handle, diagnostics) {
  // No mutation is admitted once its deadline has passed, and none commits after it.
  if (opts.clock() >= opts.deadline) throw relayError('DEADLINE_EXCEEDED','registry mutation deadline exceeded before admission');
  const locked = await withLock(opts.paths.lockPath,async () => {
    const {before,superseded} = await readForMutation(opts,diagnostics);
    const {agents,reaped,liveness} = pruneDead(before?.agents ?? {},{probe:h=>opts.processOps.probe(h,opts)});
    diagnostics.push(...diagnosticsFor(liveness));
    let changed = superseded || reaped.length > 0;
    if (event) {
      const adapter = adapterFor(event.harness), level = adapter.stateForEvent(event.nativeEvent);
      if (level === null) {
        // SessionEnd also removes workers retained after this root was replaced.
        const ended = Object.keys(agents).filter(id => id === event.agentId || agents[id].root === event.agentId);
        changed ||= ended.length > 0;
        for (const id of ended) delete agents[id];
      }
      else if (event.harness === 'codex' && event.nativeEvent === 'SubagentStop' && event.root !== undefined
        && !agents[event.root] && Object.values(agents).some(a => a.harness === 'codex' && a.scope === 'session' && sameProcess(a.process,handle))) {
        changed ||= Object.hasOwn(agents,event.agentId);
        delete agents[event.agentId];
      }
      else {
        // pi's hook runs unawaited, so it can outlive pi: a row without a process could never be reaped.
        let upsert = event.harness !== 'pi' || handle !== null;
        if (event.harness === 'codex' && event.nativeEvent === 'SessionStart' && event.root === undefined) {
          for (const [id,a] of Object.entries(agents)) {
            if (a.harness === 'codex' && sameProcess(a.process,handle)
              && ((a.scope === 'session' && id !== event.agentId) || (a.scope === 'worker' && a.root !== event.agentId && a.state === 'done'))) delete agents[id];
          }
        }
        else if (event.harness === 'agy') {
          const peers = handle === null ? [] : Object.entries(agents).filter(([id,a]) => id !== event.agentId && a.harness === 'agy' && sameProcess(a.process,handle));
          if (peers.length > 0 && (event.nativeEvent !== 'PreToolUse' || peers.some(([,a]) => FINISHED.has(a.state)))) {
            const appData = resolveAgyAppDataDir(opts);
            const self = await classifyConversation(appData === null ? null : conversationDatabase(appData,event.sessionId),
              {conversation:event.sessionId,clock:opts.clock,deadline:opts.deadline,importSqlite:opts.importSqlite});
            if (self.kind === 'unknown') diagnostics.push(self.diagnostic);
            else if (self.kind === 'subagent') {
              if (event.nativeEvent === 'Stop') {
                changed ||= Object.hasOwn(agents,event.agentId);
                delete agents[event.agentId];
                upsert = false;
              }
            }
            else if (event.nativeEvent === 'SessionStart') {
              let classifiedPeers = 0;
              for (const [peerId,peerRow] of peers) {
                if (FINISHED.has(peerRow.state)) delete agents[peerId];
                else if (classifiedPeers >= MAX_PEER_CLASSIFICATIONS) {
                  diagnostics.push({code:'AGY_CLASSIFICATION_UNAVAILABLE',message:'peer classification cap (4) reached',agentId:peerId});
                }
                else {
                  classifiedPeers++;
                  const peerClass = await classifyConversation(appData === null ? null : conversationDatabase(appData,peerRow.sessionId),
                    {conversation:peerRow.sessionId,clock:opts.clock,deadline:opts.deadline,importSqlite:opts.importSqlite});
                  if (peerClass.kind === 'root') delete agents[peerId];
                  else if (peerClass.kind === 'unknown') diagnostics.push(peerClass.diagnostic);
                }
              }
            }
            else {
              for (const [peerId,peerRow] of peers) if (FINISHED.has(peerRow.state)) delete agents[peerId];
            }
          }
        }
        if (upsert) {
          const previous = agents[event.agentId];
          const background = event.background === undefined ? previous?.background : event.background === null ? undefined : event.background;
          const state = adapter.reduceState(level,previous?.state ?? null,{background});
          const {remote,repoRoot} = /** @type {{remote: string|null, repoRoot: string|null}} */ (context);
          agents[event.agentId] = {id:event.agentId,harness:event.harness,sessionId:event.sessionId,
            scope:event.harness === 'opencode' ? 'process' : event.root === undefined ? 'session' : 'worker',
            ...(event.root === undefined ? {} : {root:event.root}),
            ...(background === undefined ? {} : {background}),process:handle,cwd:event.cwd,repoRoot,remote,
            projectKey:projectKeyFor({remote,repoRoot,cwd:event.cwd}),project:displayProject({remote,repoRoot,cwd:event.cwd}),state: /** @type {import("../types.js").State} */ (state),updatedAt:opts.now()};
          changed = true;
        }
      }
    }
    if (!changed) return {snapshot:before,changed:false,reaped,diagnostics};
    const snapshot = validateSnapshot({schema:2,generation:before?.generation ?? opts.uuid(),revision:(before?.revision ?? 0)+1,agents});
    if (opts.clock() >= opts.deadline) throw relayError('DEADLINE_EXCEEDED','registry mutation deadline exceeded before commit');
    // A caller that has already reported this publication failed refuses its commit.
    if (opts.admitCommit && !opts.admitCommit()) throw relayError('DEADLINE_EXCEEDED','registry mutation abandoned by its caller before commit');
    await writeJsonAtomic(opts.paths.agentsPath,snapshot,opts);
    return {snapshot,changed:true,reaped,diagnostics};
  },opts);
  if (locked.cleanupError) locked.value.diagnostics.push({code: /** @type {string} */ (locked.cleanupError.code),
    message:locked.value.changed ? 'publication completed; lock cleanup failed' : 'registry unchanged; lock cleanup failed'});
  return locked.value;
}
/** @param {unknown} value @param {import("../types.js").RegistryOptions} [given] */
export async function applyEvent(value, given = {}) {
  const event = validateEvent(value), adapter = adapterFor(event.harness);
  adapter.stateForEvent(event.nativeEvent);
  if (adapter.events[event.nativeEvent].kind !== event.kind) throw relayError('INVALID_SCHEMA','event kind does not match native event');
  const opts = options(given), diagnostics = [];
  let context = null, handle = null;
  if (event.kind !== 'session-end') {
    context = await (given.gitContext ?? defaultGitContext)(event.cwd,opts);
    try {
      const pid = given.resolveSessionPid ? given.resolveSessionPid(adapter) : adapter.resolveSessionPid({
        ancestors:p=>opts.processOps.ancestors(p,opts),executableOf:p=>opts.processOps.executableOf(p,opts),argvOf:p=>opts.processOps.argvOf(p,opts)});
      handle = pid === null ? null : opts.processOps.handleOf(pid,opts);
    } catch (caught) { const cause = /** @type {import('../types.js').Failure} */ (caught);
      if (!(/** @type {unknown[]} */ (['PROCESS_UNREADABLE','PROCESS_UNAVAILABLE'])).includes(cause.code)) throw cause;
    }
    if (!handle) diagnostics.push({code:'PROCESS_UNAVAILABLE',message:'agent process identity unavailable',agentId:event.agentId});
  }
  return mutate(opts,event,context,handle,diagnostics);
}
/** @param {import("../types.js").RegistryOptions} [given] */
export async function reapRegistry(given = {}) { return mutate(options(given),null,null,null,[]); }
