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
    const resp = await rpc(h, 1, "nonexistent_op", {});
    assert.equal(resp.error?.code, -32601);
  } finally {
    h.stdin.end();
    await new Promise((r) => h.child.on("exit", r));
  }
});

test("phase 1 op load_vrm with missing file returns LoadFailed (-32001)", async () => {
  const h = spawnAdapter();
  try {
    const resp = await rpc(h, 2, "load_vrm", { path: "/nonexistent/file.vrm" });
    assert.equal(resp.error?.code, -32001);
  } finally {
    h.stdin.end();
    await new Promise((r) => h.child.on("exit", r));
  }
});

test("reserved phase-2 op (set_humanoid_pose) returns phase Phase 2", async () => {
  const h = spawnAdapter();
  try {
    const resp = await rpc(h, 3, "set_humanoid_pose", {
      session_id: "x",
      bone_rotations: {},
    });
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

test("reserved phase-3 op (set_expression) returns phase Phase 3", async () => {
  const h = spawnAdapter();
  try {
    const resp = await rpc(h, 4, "set_expression", {
      name: "happy",
      weight: 1.0,
    });
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

test("malformed JSON returns -32700 parse error with id null", async () => {
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

test("multiple framed requests handled back-to-back", async () => {
  const h = spawnAdapter();
  try {
    const r1 = await rpc(h, 1, "set_root_transform", {});
    const r2 = await rpc(h, 2, "set_humanoid_pose", {});
    assert.equal(r1.error?.code, -32000);
    assert.equal(r2.error?.code, -32000);
  } finally {
    h.stdin.end();
    await new Promise((r) => h.child.on("exit", r));
  }
});
