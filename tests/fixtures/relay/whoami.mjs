// Serves `relay whoami --json` from the frozen sources, with relay's own stdout and exit
// contract: every status is one stdout line, and only `failed` exits 1.
import { whoami } from './sources/src/identity/whoami.js';

const args = process.argv.slice(2);
if (args.join(' ') !== 'whoami --json') {
  process.stderr.write(`frozen relay serves only whoami --json, not ${args.join(' ')}\n`);
  process.exit(2);
}
try {
  process.stdout.write(`${JSON.stringify(await whoami())}\n`);
} catch (error) {
  const { code, message, native, nativeSource, nativeSession } = error;
  process.stdout.write(`${JSON.stringify({ schema: 1, status: 'failed', code, message, native, nativeSource, nativeSession })}\n`);
  process.exitCode = 1;
}
