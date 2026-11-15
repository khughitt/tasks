// @ts-check
// Schema-2 hook events for subscribers. A HookEvent is the policy view of one native hook; the
// registry Event (schema 1, src/protocol.js) stays the publication view. Fields are explicit Facts
// with bounds, so policy can see a missing or invalid value instead of a coerced one.
// See docs/adapters.md ("Hook events").
import { isAbsolute } from 'node:path';
import { adapterFor, hookEventOf } from './index.js';
import { parseNativeObject } from './payload.js';
import { projectToolCall } from './agy.js';
import { relayError, validateEvent } from '../protocol.js';

export const HOOK_KINDS = Object.freeze(['session-start', 'prompt', 'before-tool', 'after-tool', 'tool-error', 'activity', 'idle',
  'needs-input', 'needs-approval', 'turn-end', 'turn-error', 'session-end', 'child-start', 'child-stop', 'model-change',
  'compact-start', 'compact-end']);
export const TOOL_KINDS = Object.freeze(['before-tool', 'after-tool', 'tool-error']);

const KiB = 1024, MiB = 1024 * KiB;
const LIMITS = Object.freeze({ small: 4096, id: 1024, path: 8192, name: 256, command: 256 * KiB, prompt: MiB, error: 16 * KiB,
  toolInputBytes: MiB, toolInputDepth: 64, toolInputNodes: 65536, toolResultText: MiB,
  backgroundTasks: 1024, backgroundText: 256, crons: 128, cronPrompt: 8192, cronsBytes: MiB });
const TOOL_EVENTS = ['PreToolUse', 'PostToolUse', 'PostToolUseFailure'];
const POST_TOOL_EVENTS = ['PostToolUse', 'PostToolUseFailure'];
const CLAUDE_CODEX = ['claude-code', 'codex'];
/** Facts per member: the harnesses and native events where each applies. */
const FACT_SCOPE = Object.freeze({
  stopHookActive: { harnesses: CLAUDE_CODEX, events: ['Stop', 'SubagentStop'] },
  terminationReason: { harnesses: ['agy'], events: ['Stop'] },
  executionNum: { harnesses: ['agy'], events: ['Stop'] },
  fullyIdle: { harnesses: ['agy'], events: ['Stop'] },
  childId: { harnesses: ['claude-code'], events: ['SubagentStart', 'SubagentStop'] },
  agentType: { harnesses: CLAUDE_CODEX, events: ['SubagentStart', 'SubagentStop'] },
  model: { harnesses: CLAUDE_CODEX, events: ['SessionStart'] },
  fromModel: { harnesses: ['claude-code'], events: ['PostModelSwitch'] },
  toModel: { harnesses: ['claude-code'], events: ['PostModelSwitch'] },
  compactionTrigger: { harnesses: ['claude-code'], events: ['PreCompact', 'PostCompact'] },
});
/** Private fields: delivered to guards and notices, and to an observer only when it selects them. */
export const PRIVATE_SCOPE = Object.freeze({
  toolInput: { harnesses: [...CLAUDE_CODEX, 'agy'], events: TOOL_EVENTS },
  command: { harnesses: [...CLAUDE_CODEX, 'agy'], events: TOOL_EVENTS },
  durationMs: { harnesses: CLAUDE_CODEX, events: POST_TOOL_EVENTS },
  toolResult: { harnesses: CLAUDE_CODEX, events: ['PostToolUse'] },
  failure: { harnesses: ['claude-code'], events: ['PostToolUseFailure'] },
  prompt: { harnesses: CLAUDE_CODEX, events: ['UserPromptSubmit'] },
  backgroundTasks: { harnesses: ['claude-code'], events: ['Stop'] },
  sessionCrons: { harnesses: ['claude-code'], events: ['Stop'] },
  transcriptPath: { harnesses: [...CLAUDE_CODEX, 'agy'], events: ['SessionStart'] },
  scratchpadDir: { harnesses: ['claude-code'], events: ['SessionStart', 'PostModelSwitch'] },
});
/** @param {Record<string, {harnesses: string[], events: string[]}>} scope @param {string} harness @param {string} nativeEvent */
const applicable = (scope, harness, nativeEvent) => Object.keys(scope).filter((key) => scope[key].harnesses.includes(harness) && scope[key].events.includes(nativeEvent));
/** @param {string} harness @param {string} nativeEvent @returns {string[]} */
export const privateFieldsFor = (harness, nativeEvent) => applicable(PRIVATE_SCOPE, harness, nativeEvent);

/** @template T @param {T} value @returns {import("../types.js").Fact<T>} */
const valid = (value) => ({ status: 'valid', value });
/** @returns {{status: 'missing'}} */
const missing = () => ({ status: 'missing' });
/** @returns {{status: 'invalid'}} */
const invalid = () => ({ status: 'invalid' });
/** @param {unknown} v @returns {v is Record<string, any>} */
const object = (v) => v !== null && typeof v === 'object' && !Array.isArray(v);
/** @param {string} s */
const bytes = (s) => Buffer.byteLength(s, 'utf8');
/** @param {number} limit @param {{nonempty?: boolean}} [options] @returns {(v: unknown) => v is string} */
const textOf = (limit, { nonempty = false } = {}) => /** @type {(v: unknown) => v is string} */ ((v) => typeof v === 'string' && bytes(v) <= limit && (!nonempty || v.trim().length > 0));
const smallText = textOf(LIMITS.small), idText = textOf(LIMITS.id, { nonempty: true }), nameText = textOf(LIMITS.name, { nonempty: true });
/** @param {unknown} v @returns {v is string} */
const absolutePath = (v) => typeof v === 'string' && !v.includes('\0') && isAbsolute(v) && bytes(v) <= LIMITS.path;
/** @param {unknown} v @returns {v is boolean} */
const boolean = (v) => typeof v === 'boolean';
/** @param {unknown} v @returns {v is number} */
const count = (v) => typeof v === 'number' && Number.isSafeInteger(v) && v >= 0;
/** @param {unknown} v @returns {v is number} */
const nonnegative = (v) => typeof v === 'number' && Number.isFinite(v) && v >= 0 && v <= Number.MAX_SAFE_INTEGER;

/**
 * A Fact for one own property: missing when absent, invalid when check rejects it.
 * @template T @param {Record<string, any>} data @param {string} key @param {(v: unknown) => boolean} check @param {(v: any) => T} [project]
 * @returns {import("../types.js").Fact<T>}
 */
function fact(data, key, check, project = (v) => v) {
  if (!Object.hasOwn(data, key)) return missing();
  return check(data[key]) ? valid(project(data[key])) : invalid();
}

// Structural bounds measured on the native value before any projection: nesting depth, total
// JSON values and serialized size. Returns false as soon as a bound is crossed.
/** @param {unknown} root */
function withinToolInputBounds(root) {
  let nodes = 0;
  /** @param {unknown} v @param {number} depth @returns {boolean} */
  const walk = (v, depth) => {
    if (++nodes > LIMITS.toolInputNodes) return false;
    if (v === null || typeof v !== 'object') return true;
    if (depth > LIMITS.toolInputDepth) return false;
    for (const child of Array.isArray(v) ? v : Object.values(v)) if (!walk(child, depth + 1)) return false;
    return true;
  };
  return walk(root, 1) && bytes(JSON.stringify(root)) <= LIMITS.toolInputBytes;
}
/** @param {unknown} v */
const toolInputObject = (v) => object(v) && withinToolInputBounds(v);

/** @param {unknown} v @returns {Record<string, any>|null} The decoded tool response object, or null when it is not one. */
function toolResponseObject(v) {
  if (object(v)) return v;
  if (typeof v !== 'string' || bytes(v) > LIMITS.toolResultText) return null;
  try { const decoded = JSON.parse(v); return object(decoded) ? decoded : null; } catch { return null; }
}
/** @param {Record<string, any>} data */
function toolResultFact(data) {
  if (!Object.hasOwn(data, 'tool_response')) return missing();
  const response = toolResponseObject(data.tool_response);
  if (response === null) return invalid();
  return valid({ taskName: fact(response, 'task_name', idText), scheduledForMs: fact(response, 'scheduledFor', nonnegative), id: fact(response, 'id', idText) });
}
/** @param {unknown} v */
const backgroundList = (v) => Array.isArray(v) && v.length <= LIMITS.backgroundTasks
  && v.every((e) => object(e) && textOf(LIMITS.backgroundText)(e.type) && textOf(LIMITS.backgroundText)(e.status));
/** @param {Record<string, any>[]} v */
const projectBackground = (v) => v.map(({ type, status }) => ({ type, status }));
/** @param {unknown} v */
function cronList(v) {
  if (!Array.isArray(v) || v.length > LIMITS.crons || !v.every((e) => object(e) && idText(e.id) && textOf(LIMITS.cronPrompt)(e.prompt))) return false;
  return bytes(JSON.stringify(projectCrons(v))) <= LIMITS.cronsBytes;
}
/** @param {Record<string, any>[]} v */
const projectCrons = (v) => v.map(({ id, prompt }) => ({ id, prompt }));

/** @param {string} harness @param {string} nativeEvent @param {Record<string, any>} data */
function factsOf(harness, nativeEvent, data) {
  /** @type {Record<string, (d: Record<string, any>) => import("../types.js").Fact<any>>} */
  const readers = {
    stopHookActive: (d) => fact(d, 'stop_hook_active', boolean),
    terminationReason: (d) => fact(d, 'terminationReason', smallText),
    executionNum: (d) => fact(d, 'executionNum', count),
    fullyIdle: (d) => fact(d, 'fullyIdle', boolean),
    childId: (d) => fact(d, 'agent_id', idText),
    agentType: (d) => fact(d, 'agent_type', smallText),
    model: (d) => fact(d, 'model', smallText),
    fromModel: (d) => fact(d, 'from_model', smallText),
    toModel: (d) => fact(d, 'to_model', smallText),
    compactionTrigger: (d) => fact(d, 'trigger', smallText),
  };
  return Object.fromEntries(applicable(FACT_SCOPE, harness, nativeEvent).map((key) => [key, readers[key](data)]));
}

/** @param {string} harness @param {string} nativeEvent @param {Record<string, any>} data @param {Record<string, any>|null} toolCall */
function privateOf(harness, nativeEvent, data, toolCall) {
  const agy = harness === 'agy';
  /** @type {Record<string, () => import("../types.js").Fact<any>>} */
  const readers = {
    toolInput: () => {
      if (!agy) return fact(data, 'tool_input', toolInputObject);
      const { args } = /** @type {Record<string, any>} */ (toolCall);
      return withinToolInputBounds(object(args) ? args : {}) ? valid(agyTool(toolCall).input) : invalid();
    },
    command: () => {
      if (agy) {
        const { name, args } = /** @type {Record<string, any>} */ (toolCall);
        return name === 'run_command' && object(args) ? fact(args, 'CommandLine', textOf(LIMITS.command)) : missing();
      }
      if (!Object.hasOwn(data, 'tool_input')) return missing();
      return object(data.tool_input) ? fact(data.tool_input, 'command', textOf(LIMITS.command)) : invalid();
    },
    durationMs: () => fact(data, 'duration_ms', nonnegative),
    toolResult: () => toolResultFact(data),
    failure: () => valid({ error: fact(data, 'error', textOf(LIMITS.error)), isInterrupt: fact(data, 'is_interrupt', boolean) }),
    prompt: () => fact(data, 'prompt', textOf(LIMITS.prompt)),
    backgroundTasks: () => fact(data, 'background_tasks', backgroundList, projectBackground),
    sessionCrons: () => fact(data, 'session_crons', cronList, projectCrons),
    transcriptPath: () => fact(data, agy ? 'transcriptPath' : 'transcript_path', absolutePath),
    scratchpadDir: () => fact(data, 'scratchpad_dir', absolutePath),
  };
  return Object.fromEntries(privateFieldsFor(harness, nativeEvent).map((key) => [key, readers[key]()]));
}

// agy's projected tool; normalizeHookEvent has already required a toolCall with a string name.
/** @param {Record<string, any>|null} toolCall */
const agyTool = (toolCall) => /** @type {NonNullable<ReturnType<typeof projectToolCall>>} */ (projectToolCall(toolCall));
/** @type {(message: string) => never} */
const reject = (message) => { throw relayError('INVALID_INPUT', message); };

/** @param {string} harness @param {import("../types.js").Adapter} adapter @param {string} nativeEvent @param {Record<string, any>} data */
function standardIdentity(harness, adapter, nativeEvent, data) {
  if (!idText(data.session_id)) reject('native input requires a session id');
  const worker = adapter.workerIdOf?.(data, nativeEvent) ?? null;
  if (worker !== null && !idText(worker)) reject('invalid worker agent_id');
  const sessionId = worker ?? data.session_id;
  return valid({ agentId: `${harness}:${sessionId}`, sessionId, ...(worker === null ? {} : { root: `${harness}:${data.session_id}` }) });
}

// Events normalized here whose cwd came from the invoking directory rather than the native
// payload. Registry publication needs the native cwd, so registryEventFor refuses them.
/** @type {WeakMap<object, {nativeCwd: boolean, toolInput?: Record<string, any>}>} */
const provenance = new WeakMap();

/**
 * Normalizes one native hook payload into a schema-2 HookEvent.
 * @param {string} harness @param {string} nativeEvent @param {unknown} raw @param {{invokingCwd: string}} options
 * @returns {import("../types.js").HookEvent}
 */
export function normalizeHookEvent(harness, nativeEvent, raw, { invokingCwd } = /** @type {{invokingCwd: string}} */ ({})) {
  if (!absolutePath(invokingCwd)) throw relayError('USAGE', 'normalizeHookEvent requires an absolute invokingCwd');
  const adapter = adapterFor(harness);
  const { kind } = hookEventOf(adapter, nativeEvent);
  const data = parseNativeObject(raw);
  const agyStop = harness === 'agy' && nativeEvent === 'Stop';
  let identity, nativeCwd, toolCall = null;
  if (harness === 'agy') {
    identity = fact(data, 'conversationId', idText, (sessionId) => ({ agentId: `agy:${sessionId}`, sessionId }));
    const first = Array.isArray(data.workspacePaths) ? data.workspacePaths[0] : undefined;
    nativeCwd = absolutePath(first) ? first : null;
    if (!agyStop && identity.status !== 'valid') reject('native input requires a conversation id');
    if (nativeEvent === 'PreToolUse') {
      if (!object(data.toolCall) || !nameText(data.toolCall.name)) reject('invalid native tool fields');
      toolCall = data.toolCall;
    }
  } else {
    if (Object.hasOwn(data, 'hook_event_name') && data.hook_event_name !== nativeEvent.split(':')[0]) reject('native event does not match argv');
    identity = standardIdentity(harness, adapter, nativeEvent, data);
    nativeCwd = absolutePath(data.cwd) ? data.cwd : null;
  }
  if (nativeCwd === null && !agyStop) reject('native input requires an absolute cwd');
  /** @type {import("../types.js").HookEvent['tool']} */
  let tool = null;
  if (TOOL_EVENTS.includes(nativeEvent)) {
    tool = harness === 'agy'
      ? { name: valid(agyTool(toolCall).name), id: missing() }
      : { name: fact(data, 'tool_name', nameText), id: fact(data, 'tool_use_id', idText) };
  }
  const event = { schema: /** @type {2} */ (2), kind, harness: /** @type {import("../types.js").Harness} */ (harness), nativeEvent,
    cwd: nativeCwd ?? invokingCwd, identity, facts: factsOf(harness, nativeEvent, data), tool, private: privateOf(harness, nativeEvent, data, toolCall) };
  // A tool input that is an object but crosses a structural bound is an invalid private fact, not a
  // malformed event: only a guard that would receive it fails closed (dispatch). The registry keeps
  // publishing the native object, as the released normalizeEvent did.
  const rawInput = kind !== 'before-tool' ? undefined : harness === 'agy' ? agyTool(toolCall).input : data.tool_input;
  const oversize = kind === 'before-tool' && event.private.toolInput?.status === 'invalid' && object(rawInput);
  if (kind === 'before-tool' && (event.tool?.name.status !== 'valid' || (event.private.toolInput?.status !== 'valid' && !oversize))) reject('invalid native tool fields');
  provenance.set(event, { nativeCwd: nativeCwd !== null, ...(oversize ? { toolInput: rawInput } : {}) });
  return event;
}

/**
 * The registry Event a hook event publishes, or null for metadata events with no registry effect.
 * Accepts only an event returned by normalizeHookEvent, whose cwd provenance is known.
 * @param {import("../types.js").HookEvent} event @returns {import("../types.js").Event|null}
 */
export function registryEventFor(event) {
  const origin = provenance.get(event);
  if (origin === undefined) throw relayError('USAGE', 'registryEventFor requires an event returned by normalizeHookEvent');
  const adapter = adapterFor(event.harness);
  if (!hookEventOf(adapter, event.nativeEvent).registry) return null;
  if (event.identity.status !== 'valid') reject('registry publication requires a usable identity');
  if (!origin.nativeCwd) reject('registry publication requires the native cwd');
  const { agentId, sessionId, root } = event.identity.value;
  const stopFact = event.harness === 'agy' ? event.facts.executionNum : event.facts.stopHookActive;
  if (stopFact?.status === 'invalid') reject('invalid stop flag');
  const stopHookActive = stopFact?.status === 'valid' ? (event.harness === 'agy' ? stopFact.value > 0 : stopFact.value) : undefined;
  const tasks = event.private.backgroundTasks;
  const background = tasks === undefined || tasks.status === 'missing' ? undefined
    : tasks.status === 'invalid' ? null : adapter.backgroundOf?.({ background_tasks: tasks.value }, event.nativeEvent);
  let tool = null;
  if (event.kind === 'before-tool') {
    const name = /** @type {string} */ (/** @type {{status: 'valid', value: string}} */ (event.tool?.name).value);
    const fact = event.private.toolInput;
    const input = fact?.status === 'valid' ? fact.value : origin.toolInput;
    tool = { name, input, command: name === 'Bash' && typeof input.command === 'string' ? input.command : null };
  }
  return validateEvent({ schema: 1, kind: adapter.events[event.nativeEvent].kind, harness: event.harness, nativeEvent: event.nativeEvent,
    agentId, sessionId, ...(root === undefined ? {} : { root }), cwd: event.cwd,
    ...(stopHookActive === undefined ? {} : { stopHookActive }), ...(background === undefined ? {} : { background }), tool });
}

/**
 * The stdin envelope for one subscriber. Guards and notices receive every applicable private field;
 * an observer receives only the private fields it selected for this native event.
 * @param {import("../types.js").HookEvent} event @param {import("../types.js").Subscriber} subscriber @param {import("../types.js").RegistryAccess} registry
 * @returns {import("../types.js").SubscriberEnvelope}
 */
export function projectEnvelope(event, subscriber, registry) {
  let selected = event.private;
  if (subscriber.role === 'observer') {
    const fields = subscriber.privateFields?.find((entry) => entry.nativeEvent === event.nativeEvent)?.fields ?? [];
    selected = Object.fromEntries(fields.filter((key) => Object.hasOwn(event.private, key)).map((key) => [key, event.private[key]]));
  }
  return { schema: 2, type: 'event', subscriber: subscriber.name, event: { ...event, private: selected }, registry };
}
