// @ts-check
import { readFileSync, readlinkSync } from 'node:fs';
import { basename } from 'node:path';
import { hostname as osHostname } from 'node:os';
import { spawnSync } from 'node:child_process';
import { parseStart, validateHandle, relayError } from '../protocol.js';

/** @param {unknown} text @returns {import("../types.js").ProcessRow} */
export function parseStat(text) {
  if (typeof text !== 'string') throw relayError('PROCESS_UNREADABLE', 'invalid proc stat');
  const open = text.indexOf('('), close = text.lastIndexOf(')');
  const prefix = text.slice(0, open).trim();
  const after = text.slice(close + 1).trim().split(/\s+/);
  if (open < 0 || close < open || !/^[1-9][0-9]*$/.test(prefix)
      || !/^[0-9]+$/.test(after[1]) || !/^-?[0-9]+$/.test(after[4])
      || !/^[A-Za-z]$/.test(after[0])) throw relayError('PROCESS_UNREADABLE', 'invalid proc stat');
  const pid = Number(prefix), ppid = Number(after[1]), ttyNr = Number(after[4]);
  if (pid > 4294967295 || ppid > 4294967295 || !Number.isSafeInteger(ttyNr)) throw relayError('PROCESS_UNREADABLE', 'invalid proc identity');
  let starttime;
  try { starttime = parseStart(after[19]); }
  catch (cause) { throw relayError('PROCESS_UNREADABLE', 'invalid proc start', cause); }
  return { pid, ppid, comm: text.slice(open + 1, close), tty: ttyNr === 0 ? null : true, starttime, state: after[0] };
}

const DARWIN_ROW = /^\s*(\d+)\s+(\d+)\s+(\S+)\s+(Sun|Mon|Tue|Wed|Thu|Fri|Sat)\s+(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\s+(\d{1,2})\s+(\d{2}):(\d{2}):(\d{2})\s+(\d{4})\s+(.+)$/;
const DARWIN_WEEKDAYS = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
const DARWIN_MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun',
  'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

/** @param {string} raw */
export function normalizeDarwinTty(raw) {
  if (raw === '??') return null;
  const match = /^(?:tty)?s([0-9a-f]+)$/i.exec(raw);
  if (!match) throw relayError('PROCESS_UNREADABLE', 'unsafe Darwin tty');
  return `ttys${match[1]}`;
}

/** @param {string} line @returns {import("../types.js").ProcessRow} */
export function parseDarwinRow(line, { terminal = true } = {}) {
  const match = DARWIN_ROW.exec(line);
  if (!match) throw relayError('PROCESS_UNREADABLE', 'Darwin ps: malformed row');
  const pid = Number(match[1]);
  const ppid = Number(match[2]);
  const day = Number(match[6]);
  const hour = Number(match[7]);
  const minute = Number(match[8]);
  const second = Number(match[9]);
  const year = Number(match[10]);
  const month = DARWIN_MONTHS.indexOf(match[5]);
  const started = new Date(0);
  started.setFullYear(year, month, day);
  started.setHours(hour, minute, second, 0);
  const starttime = started.getTime() / 1000;
  if (!Number.isInteger(pid) || pid <= 0 || pid > 4294967295 || !Number.isInteger(ppid) || ppid < 0 || ppid > 4294967295
      || started.getFullYear() !== year || started.getMonth() !== month
      || started.getDate() !== day || started.getHours() !== hour
      || started.getMinutes() !== minute || started.getSeconds() !== second
      || DARWIN_WEEKDAYS[started.getDay()] !== match[4]
      || !Number.isInteger(starttime) || match[11].trim() === '') {
    throw relayError('PROCESS_UNREADABLE', 'Darwin ps: malformed row');
  }
  return {
    pid,
    ppid,
    comm: basename(match[11]),
    tty: terminal ? normalizeDarwinTty(match[3]) : null,
    starttime: parseStart(String(starttime)),
    state: null,
  };
}

/** @param {string} tool @param {string[]} args */
function runDarwinTool(tool, args, { clock = performance.now.bind(performance), deadline = clock() + 2000, spawn = spawnSync } = {}) {
  const remaining = Math.min(2000, deadline - clock());
  if (remaining <= 0) throw relayError('DEADLINE_EXCEEDED', 'process deadline exceeded');
  const result = spawn(tool, args, { encoding: 'utf8', env: { ...process.env, LC_ALL: 'C' },
    timeout: Math.max(1, Math.floor(remaining)), maxBuffer: 4 * 1024 * 1024, killSignal: 'SIGKILL' });
  // A lookup killed at the caller's deadline, rather than at the 2 s cap, says nothing about
  // the process: ps -p takes 3-6 ms on a Mac, and the last lookup before a deadline gets less.
  if ((/** @type {NodeJS.ErrnoException|undefined} */ (result.error))?.code === 'ETIMEDOUT' && remaining < 2000) throw relayError('DEADLINE_EXCEEDED', 'process deadline exceeded', result.error);
  if (result.error || result.signal || result.status !== 0) throw relayError('PROCESS_UNAVAILABLE', 'cannot run process lookup', result.error);
  return result.stdout;
}
// The reason of an unknown verdict whose process read the caller's deadline cut short.
export const DEADLINE_CUT = 'deadline exceeded';
/** @param {string[]} args @param {import("../types.js").Deadline} [options] */
export const runDarwinPs = (args, options) => runDarwinTool('/bin/ps', args, options);
/** @param {string[]} args @param {import("../types.js").Deadline} [options] */
export const runDarwinLsof = (args, options) => runDarwinTool('/usr/sbin/lsof', args, options);
// lsof -F prints the process as `p<pid>`, then each text file as `n<path>`, some versions
// preceding it with `ftxt`. The executable is the first; the Darwin probe's D4 checks that
// order on a Mac.
/** @param {unknown} output @param {number} pid */
export function parseDarwinExecutable(output, pid) {
  const lines = typeof output === 'string' ? output.split('\n') : [];
  const name = lines[1] === 'ftxt' ? lines[2] : lines[1];
  if (lines[0] !== `p${pid}` || !name?.startsWith('n/')) throw relayError('PROCESS_UNREADABLE', 'invalid process executable');
  return name.slice(1);
}
/** @param {number} pid @param {(pid: number) => import("../types.js").ProcessRow|null} recordOf */
function walk(pid, recordOf) {
  const seen = new Set(), chain = [];
  while (pid > 1) {
    if (seen.has(pid)) throw relayError('PROCESS_UNREADABLE', 'process ancestor loop');
    seen.add(pid);
    const row = recordOf(pid);
    if (!row) break;
    chain.push(row); pid = row.ppid;
  }
  return chain;
}
/** @param {{platform?: string, readText?: (path: string) => string, readLink?: (path: string) => string, runPs?: typeof runDarwinPs, runLsof?: typeof runDarwinLsof, kill?: (pid: number, signal: number) => unknown, hostname?: () => string, now?: () => number}} [options] */
export function createProcessOps({ platform = process.platform, readText = (p) => readFileSync(p, 'utf8'),
  readLink = (p) => readlinkSync(p), runPs = runDarwinPs, runLsof = runDarwinLsof, kill = (pid, signal) => process.kill(pid, signal), hostname = osHostname, now = Date.now } = {}) {
  if (!['linux', 'darwin'].includes(platform)) throw relayError('PROCESS_UNAVAILABLE', 'unsupported process platform');
  /** @param {import("../types.js").Failure} error */
  const absent = (error) => (/** @type {unknown[]} */ (['ENOENT', 'ESRCH'])).includes(error.code);
  const local = () => {
    let host, bootId = null;
    try { host = (platform === 'linux' ? readText('/proc/sys/kernel/hostname') : hostname()).trim(); }
    catch { return null; }
    if (!host) return null;
    if (platform === 'linux') {
      try { bootId = readText('/proc/sys/kernel/random/boot_id').trim();
        validateHandle({ platform, host, bootId, pid: 1, start: '0' }); }
      catch { bootId = null; }
    }
    return { platform, host, bootId };
  };
  /** @param {number} pid */
  const linuxRow = (pid) => {
    let text;
    try { text = readText(`/proc/${pid}/stat`); }
    catch (caught) { const cause = /** @type {import("../types.js").Failure} */ (caught); if (absent(cause)) return null; throw relayError('PROCESS_UNREADABLE', 'cannot read process', cause); }
    const row = parseStat(text);
    if (row.pid !== pid) throw relayError('PROCESS_UNREADABLE', 'process identity mismatch');
    return row;
  };
  /** @param {number} pid @param {import("../types.js").Deadline} [options] @param {boolean} [terminal] */
  const darwinRow = (pid, options, terminal) => {
    let output;
    try { output = runPs(['-p', String(pid), '-o', 'pid=,ppid=,tty=,lstart=,comm='], options); }
    catch (caught) { const cause = /** @type {import('../types.js').Failure} */ (caught);
      if (cause.code === 'DEADLINE_EXCEEDED') throw cause;
      try { kill(pid, 0); } catch (caught) { const error = /** @type {import('../types.js').Failure} */ (caught); if (error.code === 'ESRCH') return null; }
      throw relayError('PROCESS_UNAVAILABLE', 'cannot read process', cause);
    }
    if (typeof output !== 'string') throw relayError('PROCESS_UNREADABLE', 'invalid process output');
    if (!output.trim()) return null;
    const lines = output.trimEnd().split('\n');
    if (lines.length !== 1) throw relayError('PROCESS_UNREADABLE', 'ambiguous process output');
    const row = parseDarwinRow(lines[0], { terminal });
    if (row.pid !== pid) throw relayError('PROCESS_UNREADABLE', 'process identity mismatch');
    return row;
  };
  /** @param {number} pid @param {import("../types.js").Deadline} [options] */
  const core = (pid, options) => platform === 'linux' ? linuxRow(pid) : darwinRow(pid, options, false);
  /** @param {number} pid @param {import("../types.js").Deadline} [options] */
  const recordOf = (pid, options) => platform === 'linux' ? linuxRow(pid) : darwinRow(pid, options, true);
  /** @param {number} pid @param {import("../types.js").Deadline} [options] */
  const ancestors = (pid, options) => {
    if (platform === 'linux') return walk(pid, (id) => recordOf(id, options));
    const output = runPs(['-axo', 'pid=,ppid=,tty=,lstart=,comm='], options);
    if (typeof output !== 'string' || !output.trim()) throw relayError('PROCESS_UNREADABLE', 'invalid process table');
    const records = new Map();
    for (const line of output.trimEnd().split('\n')) {
      try { const row = parseDarwinRow(line); records.set(row.pid, row); }
      catch (caught) { const error = /** @type {import('../types.js').Failure} */ (caught);
        const id = Number(/^\s*(\d+)/.exec(line)?.[1]);
        if (Number.isInteger(id) && id > 0 && !records.has(id)) records.set(id, error);
      }
    }
    return walk(pid, (id) => { const row = records.get(id); if (row instanceof Error) throw row; return row ?? null; });
  };
  /** @param {number} pid @param {import("../types.js").Deadline} [options] */
  const handleOf = (pid, options) => {
    const identity = local();
    if (!identity) return null;
    const row = core(pid, options);
    if (!row?.starttime) return null;
    return validateHandle({ ...identity, pid, start: row.starttime });
  };
  /** @param {import("../types.js").Liveness["status"]} status @param {string} reason */
  const verdict = (status, reason) => ({ status, reason });
  /** @param {unknown} value @param {import("../types.js").Deadline} [options] */
  const probe = (value, options) => {
    let handle;
    try { handle = validateHandle(value); } catch { return verdict('unknown', 'missing or invalid handle'); }
    if (handle.platform !== platform) return verdict('unknown', 'foreign platform');
    const identity = local();
    if (!identity) return verdict('unknown', 'local identity unavailable');
    if (handle.host !== identity.host) return verdict('unknown', 'foreign host');
    if (platform === 'linux') {
      if (!handle.bootId || !identity.bootId) return verdict('unknown', 'boot identity unavailable');
      if (handle.bootId !== identity.bootId) return verdict('dead', 'boot changed');
    }
    let row;
    try { row = core(handle.pid, options); }
    catch (caught) { const error = /** @type {import('../types.js').Failure} */ (caught); return verdict('unknown', error.code === 'DEADLINE_EXCEEDED' ? DEADLINE_CUT : 'process unreadable'); }
    if (!row) return verdict('dead', 'process absent');
    if ((/** @type {unknown[]} */ (['Z', 'X', 'x'])).includes(row.state)) return verdict('dead', 'process exited');
    if (row.starttime !== handle.start) return verdict('dead', 'process replaced');
    return verdict('alive', 'qualified process matches');
  };
  // Read for one process at a time, only when an adapter's rule needs it (docs/adapters.md,
  // "Session process handle and ancestry"). The kernel suffixes the exe link of a binary
  // replaced on disk, which a Claude Code auto-update does to every running session.
  const DELETED = ' (deleted)';
  /** @param {number} pid @param {import("../types.js").Deadline} [options] */
  const executableOf = (pid, options) => {
    if (platform === 'linux') {
      let target;
      try { target = readLink(`/proc/${pid}/exe`); }
      catch (cause) { throw relayError('PROCESS_UNREADABLE', 'cannot read process executable', cause); }
      if (typeof target !== 'string' || !target.startsWith('/')) throw relayError('PROCESS_UNREADABLE', 'invalid process executable');
      return target.endsWith(DELETED) ? target.slice(0, -DELETED.length) : target;
    }
    // Darwin's ps comm column is argv[0], which the process chooses, so it names no file.
    return parseDarwinExecutable(runLsof(['-w', '-a', '-p', String(pid), '-d', 'txt', '-Fn'], options), pid);
  };
  /** @param {number} pid @param {import("../types.js").Deadline} [options] */
  const argvOf = (pid, options) => {
    if (platform === 'linux') {
      let text;
      try { text = readText(`/proc/${pid}/cmdline`); }
      catch (cause) { throw relayError('PROCESS_UNREADABLE', 'cannot read process arguments', cause); }
      if (typeof text !== 'string') throw relayError('PROCESS_UNREADABLE', 'invalid process arguments');
      if (text === '') return [];
      return (text.endsWith('\0') ? text.slice(0, -1) : text).split('\0');
    }
    // ps joins arguments with spaces, so boundaries cannot be recovered from args alone.
    // argv[0] is located by the comm column, which Darwin's ps reports as argv[0] itself; when
    // args does not start with it, the boundary is unknown and the read fails closed. Later
    // arguments split on whitespace: a role is one exact word, so a split can only make a role
    // appear (no handle), never hide one.
    const comm = runPs(['-p', String(pid), '-o', 'comm='], options);
    const args = runPs(['-p', String(pid), '-o', 'args='], options);
    const first = typeof comm === 'string' ? comm.trim() : '', line = typeof args === 'string' ? args.trim() : '';
    if (!first || !line) throw relayError('PROCESS_UNREADABLE', 'invalid process arguments');
    const argv0 = line === first || line.startsWith(`${first} `) ? first : null;
    if (argv0 === null) throw relayError('PROCESS_UNREADABLE', 'argument boundaries unknown');
    const rest = line.slice(argv0.length).trim();
    return rest ? [argv0, ...rest.split(/\s+/)] : [argv0];
  };
  // A start as a wall-clock interval (docs/registry.md, "Start-time interval"). Linux keeps a start only on the boot clock, in USER_HZ (100) ticks;
  // one uptime sample bracketed by two wall-clock reads places it however long a pause lasts.
  // Darwin's lstart is the recorded wall-clock start, truncated to the second.
  /** @param {import("../types.js").ProcessRow} row @returns {{lo: number, hi: number}} */
  const startedAtMs = (row) => {
    const start = Number(row.starttime);
    if (!Number.isSafeInteger(start)) throw relayError('PROCESS_UNREADABLE', 'process start out of range');
    if (platform === 'darwin') return { lo: start * 1000, hi: start * 1000 + 999 };
    const w0 = now();
    let text;
    try { text = readText('/proc/uptime'); }
    catch (cause) { throw relayError('PROCESS_UNREADABLE', 'cannot read uptime', cause); }
    const w1 = now();
    const match = typeof text === 'string' ? /^([0-9]+)\.([0-9]{2}) /.exec(text) : null;
    if (!match) throw relayError('PROCESS_UNREADABLE', 'invalid uptime');
    const ageMs = Number(match[1]) * 1000 + Number(match[2]) * 10 - start * 10;
    return { lo: w0 - ageMs - 11, hi: w1 - ageMs + 11 };
  };
  return { recordOf, ancestors, handleOf, probe, executableOf, argvOf, startedAtMs };
}
