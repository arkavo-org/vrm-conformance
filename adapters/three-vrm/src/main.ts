// Entry point: launch the browser session, wire stdin/stdout into the
// JSON-RPC server loop, dispose on exit.

import { run } from "./server.js";
import { BrowserSession } from "./browser-session.js";
import type { AdapterContext } from "./operations.js";

process.stderr.write("three-vrm adapter starting\n");

const session = new BrowserSession();
const ctx: AdapterContext = {
  session,
  nextSessionId: { value: 0 },
};

let exitCode = 0;

try {
  await session.start();
  process.stderr.write("three-vrm adapter ready\n");
  await run(ctx, process.stdin, process.stdout);
} catch (err) {
  process.stderr.write(`three-vrm adapter fatal: ${(err as Error).message}\n`);
  exitCode = 1;
} finally {
  await session.dispose();
}

process.exit(exitCode);
