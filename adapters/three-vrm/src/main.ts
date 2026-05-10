// Entry point: wire stdin/stdout into the JSON-RPC server loop.

import { run } from "./server.js";

process.stderr.write("three-vrm adapter starting\n");

run(process.stdin, process.stdout).catch((err) => {
  process.stderr.write(`three-vrm adapter fatal: ${err}\n`);
  process.exit(1);
});
