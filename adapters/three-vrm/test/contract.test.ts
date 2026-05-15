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

test("animate_root_transform without a loaded VRM returns ok (no longer reserved)", async () => {
  const h = spawnAdapter();
  try {
    const resp = await rpc(h, 5, "animate_root_transform", {
      session_id: "no-vrm-yet",
      translation_start: [0, 0, 0],
      translation_end: [0.2, 0, 0],
      duration_seconds: 0.1,
      fps: 60,
    });
    assert.equal(
      resp.error,
      undefined,
      `expected ok; got: ${JSON.stringify(resp)}`,
    );
    assert.ok(resp.result, `expected result; got: ${JSON.stringify(resp)}`);
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

test("dump_bone_positions with missing session_id returns InvalidParams (-32602)", async () => {
  const h = spawnAdapter();
  try {
    const resp = await rpc(h, 7, "dump_bone_positions", {});
    assert.equal(resp.error?.code, -32602);
  } finally {
    h.stdin.end();
    await new Promise((r) => h.child.on("exit", r));
  }
});

test(
  "dump_bone_positions after load_vrm returns springs array with correct shape",
  { timeout: 60_000 },
  async () => {
    const { mkdtempSync, existsSync } = await import("node:fs");
    const { execFileSync } = await import("node:child_process");
    const { tmpdir } = await import("node:os");
    const generator = path.resolve(__dirname, "..", "..", "..", "target", "release", "vrm-asset-generator");
    if (!existsSync(generator)) {
      // Skip gracefully if release binary is absent (CI doesn't build release for TS adapter tests).
      return;
    }
    const dir = mkdtempSync(path.join(tmpdir(), "three-vrm-dump-"));
    execFileSync(generator, ["emit-springbone-sweep", "--output-dir", dir], { stdio: "inherit" });
    const vrmPath = path.join(dir, "springbone_default.vrm");

    const h = spawnAdapter();
    try {
      const loadResp = await rpc(h, 1, "load_vrm", { path: vrmPath });
      assert.ok(!loadResp.error, `load_vrm failed: ${JSON.stringify(loadResp.error)}`);
      const sessionId: string = (loadResp.result as { session_id: string }).session_id;

      const dumpResp = await rpc(h, 2, "dump_bone_positions", { session_id: sessionId });
      assert.ok(!dumpResp.error, `dump_bone_positions failed: ${JSON.stringify(dumpResp.error)}`);

      const result = dumpResp.result as { springs: Array<{ name: string; joint_positions: number[][] }> };
      assert.ok(Array.isArray(result.springs), "result.springs must be an array");
      // springbone_default.vrm has exactly 1 spring chain of 4 joints.
      // Each springs[] entry maps 1:1 to VRMC_springBone.springs[i] per the op
      // contract, and joint_positions[] contains world positions head-to-tail.
      assert.strictEqual(result.springs.length, 1, `expected 1 spring chain (one entry per VRM chain), got ${result.springs.length}`);
      assert.ok(typeof result.springs[0].name === "string", "chain entry must have a string name");
      assert.strictEqual(
        result.springs[0].joint_positions.length,
        4,
        `expected 4 joint positions (joint_count=4 for springbone_default), got ${result.springs[0].joint_positions.length}`,
      );
      for (const s of result.springs) {
        assert.ok(typeof s.name === "string", "each entry must have a string name");
        assert.ok(Array.isArray(s.joint_positions), "joint_positions must be an array");
        for (const pos of s.joint_positions) {
          assert.strictEqual(pos.length, 3, "each position must be [x, y, z]");
          for (const v of pos) {
            assert.ok(typeof v === "number", "position component must be a number");
          }
        }
      }
    } finally {
      h.stdin.end();
      await new Promise((r) => h.child.on("exit", r));
    }
  },
);
