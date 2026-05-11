// Contract tests for the babylon-vrm adapter at the L1+L2 scaffold milestone.
//
// Spawns the built `dist/main.js` as a subprocess, sends framed JSON-RPC
// requests, and asserts the response shape. Phase 1 ops should return
// -32000 Unimplemented with `data.phase = "L3 (babylon-vrm integration
// deferred)"`. Phase 2+ reserved ops should return -32000 with their
// canonical phase labels. Unknown methods should return -32601.
//
// L3 will replace the Phase 1 expectations here with real-render assertions
// against a generated VRM (same pattern as three-vrm's render.test.ts).

import { test } from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { readMessage, writeMessage } from "../src/framing.ts";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const BIN = path.resolve(__dirname, "..", "dist", "main.js");

interface ChildHandle {
  child: ReturnType<typeof spawn>;
  stdin: NodeJS.WritableStream;
  stdout: NodeJS.ReadableStream;
}

function spawnAdapter(): ChildHandle {
  const child = spawn(process.execPath, [BIN], {
    stdio: ["pipe", "pipe", "ignore"],
  });
  if (!child.stdin || !child.stdout) {
    throw new Error("spawn failed: no stdio pipes");
  }
  return { child, stdin: child.stdin, stdout: child.stdout };
}

async function rpc(
  h: ChildHandle,
  id: number,
  method: string,
  params: unknown,
): Promise<{
  result?: unknown;
  error?: { code: number; message: string; data?: unknown };
  id?: unknown;
}> {
  const reqBody = Buffer.from(
    JSON.stringify({ jsonrpc: "2.0", id, method, params }),
    "utf8",
  );
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  await writeMessage(h.stdin as any, reqBody);
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const respBody = await readMessage(h.stdout as any);
  return JSON.parse(respBody.toString("utf8"));
}

test("unknown method returns -32601", async () => {
  const h = spawnAdapter();
  try {
    const resp = await rpc(h, 1, "definitely_not_a_method", {});
    assert.equal(resp.error?.code, -32601);
  } finally {
    h.stdin.end();
    await new Promise((r) => h.child.on("exit", r));
  }
});

test("Phase 1 op load_vrm returns L3 deferral phase label", async () => {
  const h = spawnAdapter();
  try {
    const resp = await rpc(h, 2, "load_vrm", { path: "/tmp/x.vrm" });
    assert.equal(resp.error?.code, -32000);
    assert.equal(resp.error?.message, "Unimplemented");
    assert.equal(
      (resp.error?.data as { phase?: string } | undefined)?.phase,
      "L3 (babylon-vrm integration deferred)",
    );
  } finally {
    h.stdin.end();
    await new Promise((r) => h.child.on("exit", r));
  }
});

test("Phase 1 op render returns L3 deferral phase label", async () => {
  const h = spawnAdapter();
  try {
    const resp = await rpc(h, 3, "render", {});
    assert.equal(resp.error?.code, -32000);
    assert.equal(
      (resp.error?.data as { phase?: string } | undefined)?.phase,
      "L3 (babylon-vrm integration deferred)",
    );
  } finally {
    h.stdin.end();
    await new Promise((r) => h.child.on("exit", r));
  }
});

test("Reserved Phase 2 op set_humanoid_pose returns Phase 2 label", async () => {
  const h = spawnAdapter();
  try {
    const resp = await rpc(h, 4, "set_humanoid_pose", {});
    assert.equal(resp.error?.code, -32000);
    assert.equal(
      (resp.error?.data as { phase?: string } | undefined)?.phase,
      "Phase 2",
    );
  } finally {
    h.stdin.end();
    await new Promise((r) => h.child.on("exit", r));
  }
});

test("Reserved v1.x op set_environment returns v1.x label", async () => {
  const h = spawnAdapter();
  try {
    const resp = await rpc(h, 5, "set_environment", {});
    assert.equal(resp.error?.code, -32000);
    assert.equal(
      (resp.error?.data as { phase?: string } | undefined)?.phase,
      "v1.x",
    );
  } finally {
    h.stdin.end();
    await new Promise((r) => h.child.on("exit", r));
  }
});

test("Reserved Phase 3 op set_expression returns Phase 3 label", async () => {
  const h = spawnAdapter();
  try {
    const resp = await rpc(h, 6, "set_expression", {});
    assert.equal(resp.error?.code, -32000);
    assert.equal(
      (resp.error?.data as { phase?: string } | undefined)?.phase,
      "Phase 3",
    );
  } finally {
    h.stdin.end();
    await new Promise((r) => h.child.on("exit", r));
  }
});

test("Malformed JSON returns -32700 with null id", async () => {
  const h = spawnAdapter();
  try {
    const garbage = Buffer.from("not json at all }}}", "utf8");
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    await writeMessage(h.stdin as any, garbage);
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const respBody = await readMessage(h.stdout as any);
    const resp = JSON.parse(respBody.toString("utf8"));
    assert.equal(resp.error?.code, -32700);
    assert.equal(resp.id, null);
  } finally {
    h.stdin.end();
    await new Promise((r) => h.child.on("exit", r));
  }
});
