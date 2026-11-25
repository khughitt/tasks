// @ts-check
import { homedir } from 'node:os';
import { join, isAbsolute } from 'node:path';
import { relayError } from '../protocol.js';
export function paths(env = process.env) {
  /** @param {string} value */
  const absolute = (value) => {
    if (typeof value !== 'string' || !value.trim() || !isAbsolute(value)) throw relayError('INVALID_INPUT', 'expected absolute directory');
    return value;
  };
  const home = absolute(env.HOME ?? homedir());
  const stateBase = env.XDG_STATE_HOME ? absolute(env.XDG_STATE_HOME) : join(home, '.local/state');
  const configBase = env.XDG_CONFIG_HOME ? absolute(env.XDG_CONFIG_HOME) : join(home, '.config');
  const stateDir = env.RELAY_STATE_DIR === undefined ? join(stateBase, 'relay') : absolute(env.RELAY_STATE_DIR);
  return { stateDir, agentsPath: join(stateDir, 'agents.json'), lockPath: join(stateDir, 'agents.lock'),
    reclaimPath: join(stateDir, 'agents.lock.reclaim'), configDir: join(configBase, 'relay'), subscribersPath: join(configBase, 'relay/subscribers.json') };
}
