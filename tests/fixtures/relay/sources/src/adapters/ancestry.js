// @ts-check
// Two lookups over one nearest-first ancestor chain whose index zero is the hook itself.
// The session process is the registry's identity handle: the harness process that fired the
// hook, terminal or not. The terminal process is where a presentation layer paints: the
// measured familiar predicate. They coincide for an interactive session and differ for every
// session without a terminal of its own. See docs/adapters.md ("Session process handle and ancestry").
export const VERSION_COMM = /^[0-9]+(\.[0-9]+)+$/;
// The walk never continues past the nearest harness process: a pid borrowed from an outer
// session is worse than none.
/** @param {import("../types.js").ProcessRow[]} chain @param {{isHarness: (row: import("../types.js").ProcessRow) => boolean, roles?: readonly string[], argvOf?: (pid: number) => string[]}} options */
export function nearestSessionPid(chain, { isHarness, roles = [], argvOf }) {
  const nearest = chain.find((row, index) => index > 0 && isHarness(row));
  if (!nearest) return null;
  if (roles.length > 0 && roles.includes((/** @type {(pid: number) => string[]} */ (argvOf))(nearest.pid)[1])) return null;
  return nearest.pid;
}
// The predicate sees only processes that own a terminal, so any read it makes is gated by it.
/** @param {import("../types.js").ProcessRow[]} chain @param {(row: import("../types.js").ProcessRow, index: number) => boolean} isTerminal */
export function nearestTerminalPid(chain, isTerminal) {
  return chain.find((row, index) => index > 0 && row.tty !== null && isTerminal(row, index))?.pid ?? null;
}
