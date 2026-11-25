// @ts-check
/** @param {Record<string, import("../types.js").Agent>} agents @param {{probe: (handle: import("../types.js").Handle|null) => import("../types.js").Liveness}} options */
export function pruneDead(agents, { probe }) {
  /** @type {Record<string, import("../types.js").Agent>} */
  const kept = {}, reaped = [], liveness = /** @type {Record<string, import("../types.js").Liveness>} */ ({});
  for (const [id,agent] of Object.entries(agents)) {
    const result = probe(agent.process); liveness[id] = result;
    if (result.status === 'dead') reaped.push(id); else kept[id] = agent;
  }
  return { agents:kept, reaped, liveness };
}
