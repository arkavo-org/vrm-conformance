// babylon-vrm adapter — executable entry point.
//
// L1 + L2: wires stdin/stdout/stderr into the JSON-RPC server. Every known
// method returns -32000 Unimplemented with `data.phase` indicating the L3
// deferral; unknown methods return -32601. No Playwright, no Babylon yet.
// L3 will swap the operations dispatch for one that drives Babylon-VRM-
// Loader inside Playwright Chromium, mirroring three-vrm's architecture.

import { run } from "./server.js";
import type { AdapterContext } from "./operations.js";

process.stderr.write(
  "babylon-vrm adapter: starting (L1+L2 scaffold; ops return Unimplemented)\n",
);

const ctx: AdapterContext = {};
await run(ctx, process.stdin, process.stdout);
