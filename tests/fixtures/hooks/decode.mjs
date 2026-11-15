// The frozen producer is the only decoder used by replay.
import { readFileSync } from 'node:fs';
import { normalizeHookEvent, projectEnvelope } from './sources/relay/src/adapters/hook-event.js';
const [harness, nativeEvent, cwd, role] = process.argv.slice(2);
try {
  const event = normalizeHookEvent(harness, nativeEvent, readFileSync(0, 'utf8'), { invokingCwd: cwd });
  process.stdout.write(JSON.stringify(projectEnvelope(event, { name: 'tasks-fixture', role }, { status: 'not-requested' })));
} catch (error) {
  process.stderr.write(`decoder: ${error.code ?? 'INVALID_INPUT'}\n`);
  process.exitCode = 1;
}
