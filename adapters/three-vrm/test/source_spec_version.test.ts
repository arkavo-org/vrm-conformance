// Integration tests for source_spec_version detection (Task 23).
//
// These tests require humanoid fixture VRM files installed via
// scripts/install-humanoid-fixtures.sh. When the fixture files are absent the
// tests skip cleanly — they will not fail in a clean checkout that hasn't run
// the fixture-install script.
//
// Fixture paths (relative to repo root):
//   assets/humanoid/avatarA_0_0.vrm     — canonical VRM 0.x asset
//   assets/humanoid/vroid_default_F_1_0.vrm — canonical VRM 1.0 asset
//
// The adapter is exercised as a real subprocess over JSON-RPC stdio framing,
// mirroring the pattern used in contract.test.ts.

import { test } from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { readMessage, writeMessage } from "../src/framing.ts";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const BIN = path.resolve(__dirname, "..", "dist", "main.js");
const REPO_ROOT = path.resolve(__dirname, "..", "..", "..");

const FIXTURE_0X = path.join(REPO_ROOT, "assets", "humanoid", "avatarA_0_0.vrm");
const FIXTURE_1_0 = path.join(REPO_ROOT, "assets", "humanoid", "vroid_default_F_1_0.vrm");

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

// ---------------------------------------------------------------------------
// VRM 0.x fixture tests
// ---------------------------------------------------------------------------

test(
  "dump_expression_weights: reports source_spec_version=0.x for VRM 0.x asset",
  { timeout: 60_000 },
  async (t) => {
    if (!existsSync(FIXTURE_0X)) {
      t.skip("fixture not installed; run scripts/install-humanoid-fixtures.sh");
      return;
    }
    const h = spawnAdapter();
    try {
      const loadResp = await rpc(h, 1, "load_vrm", { path: FIXTURE_0X });
      assert.ok(!loadResp.error, `load_vrm failed: ${JSON.stringify(loadResp.error)}`);
      const sessionId = (loadResp.result as { session_id: string }).session_id;

      const dumpResp = await rpc(h, 2, "dump_expression_weights", { session_id: sessionId });
      assert.ok(!dumpResp.error, `dump_expression_weights failed: ${JSON.stringify(dumpResp.error)}`);

      const result = dumpResp.result as { source_spec_version?: string };
      assert.strictEqual(result.source_spec_version, "0.x",
        `expected source_spec_version="0.x", got: ${JSON.stringify(result.source_spec_version)}`);
    } finally {
      h.stdin.end();
      await new Promise((r) => h.child.on("exit", r));
    }
  },
);

test(
  "dump_humanoid_pose: reports source_spec_version=0.x for VRM 0.x asset",
  { timeout: 60_000 },
  async (t) => {
    if (!existsSync(FIXTURE_0X)) {
      t.skip("fixture not installed; run scripts/install-humanoid-fixtures.sh");
      return;
    }
    const h = spawnAdapter();
    try {
      const loadResp = await rpc(h, 1, "load_vrm", { path: FIXTURE_0X });
      assert.ok(!loadResp.error, `load_vrm failed: ${JSON.stringify(loadResp.error)}`);
      const sessionId = (loadResp.result as { session_id: string }).session_id;

      const dumpResp = await rpc(h, 2, "dump_humanoid_pose", { session_id: sessionId });
      assert.ok(!dumpResp.error, `dump_humanoid_pose failed: ${JSON.stringify(dumpResp.error)}`);

      const result = dumpResp.result as { source_spec_version?: string };
      assert.strictEqual(result.source_spec_version, "0.x",
        `expected source_spec_version="0.x", got: ${JSON.stringify(result.source_spec_version)}`);
    } finally {
      h.stdin.end();
      await new Promise((r) => h.child.on("exit", r));
    }
  },
);

test(
  "dump_look_at_state: reports source_spec_version=0.x for VRM 0.x asset",
  { timeout: 60_000 },
  async (t) => {
    if (!existsSync(FIXTURE_0X)) {
      t.skip("fixture not installed; run scripts/install-humanoid-fixtures.sh");
      return;
    }
    const h = spawnAdapter();
    try {
      const loadResp = await rpc(h, 1, "load_vrm", { path: FIXTURE_0X });
      assert.ok(!loadResp.error, `load_vrm failed: ${JSON.stringify(loadResp.error)}`);
      const sessionId = (loadResp.result as { session_id: string }).session_id;

      const dumpResp = await rpc(h, 2, "dump_look_at_state", { session_id: sessionId });
      assert.ok(!dumpResp.error, `dump_look_at_state failed: ${JSON.stringify(dumpResp.error)}`);

      const result = dumpResp.result as { source_spec_version?: string };
      assert.strictEqual(result.source_spec_version, "0.x",
        `expected source_spec_version="0.x", got: ${JSON.stringify(result.source_spec_version)}`);
    } finally {
      h.stdin.end();
      await new Promise((r) => h.child.on("exit", r));
    }
  },
);

// ---------------------------------------------------------------------------
// VRM 1.0 fixture tests
// ---------------------------------------------------------------------------

test(
  "dump_expression_weights: reports source_spec_version=1.0 for VRM 1.0 asset",
  { timeout: 60_000 },
  async (t) => {
    if (!existsSync(FIXTURE_1_0)) {
      t.skip("fixture not installed; run scripts/install-humanoid-fixtures.sh");
      return;
    }
    const h = spawnAdapter();
    try {
      const loadResp = await rpc(h, 1, "load_vrm", { path: FIXTURE_1_0 });
      assert.ok(!loadResp.error, `load_vrm failed: ${JSON.stringify(loadResp.error)}`);
      const sessionId = (loadResp.result as { session_id: string }).session_id;

      const dumpResp = await rpc(h, 2, "dump_expression_weights", { session_id: sessionId });
      assert.ok(!dumpResp.error, `dump_expression_weights failed: ${JSON.stringify(dumpResp.error)}`);

      const result = dumpResp.result as { source_spec_version?: string };
      assert.strictEqual(result.source_spec_version, "1.0",
        `expected source_spec_version="1.0", got: ${JSON.stringify(result.source_spec_version)}`);
    } finally {
      h.stdin.end();
      await new Promise((r) => h.child.on("exit", r));
    }
  },
);

test(
  "dump_humanoid_pose: reports source_spec_version=1.0 for VRM 1.0 asset",
  { timeout: 60_000 },
  async (t) => {
    if (!existsSync(FIXTURE_1_0)) {
      t.skip("fixture not installed; run scripts/install-humanoid-fixtures.sh");
      return;
    }
    const h = spawnAdapter();
    try {
      const loadResp = await rpc(h, 1, "load_vrm", { path: FIXTURE_1_0 });
      assert.ok(!loadResp.error, `load_vrm failed: ${JSON.stringify(loadResp.error)}`);
      const sessionId = (loadResp.result as { session_id: string }).session_id;

      const dumpResp = await rpc(h, 2, "dump_humanoid_pose", { session_id: sessionId });
      assert.ok(!dumpResp.error, `dump_humanoid_pose failed: ${JSON.stringify(dumpResp.error)}`);

      const result = dumpResp.result as { source_spec_version?: string };
      assert.strictEqual(result.source_spec_version, "1.0",
        `expected source_spec_version="1.0", got: ${JSON.stringify(result.source_spec_version)}`);
    } finally {
      h.stdin.end();
      await new Promise((r) => h.child.on("exit", r));
    }
  },
);

test(
  "dump_look_at_state: reports source_spec_version=1.0 for VRM 1.0 asset",
  { timeout: 60_000 },
  async (t) => {
    if (!existsSync(FIXTURE_1_0)) {
      t.skip("fixture not installed; run scripts/install-humanoid-fixtures.sh");
      return;
    }
    const h = spawnAdapter();
    try {
      const loadResp = await rpc(h, 1, "load_vrm", { path: FIXTURE_1_0 });
      assert.ok(!loadResp.error, `load_vrm failed: ${JSON.stringify(loadResp.error)}`);
      const sessionId = (loadResp.result as { session_id: string }).session_id;

      const dumpResp = await rpc(h, 2, "dump_look_at_state", { session_id: sessionId });
      assert.ok(!dumpResp.error, `dump_look_at_state failed: ${JSON.stringify(dumpResp.error)}`);

      const result = dumpResp.result as { source_spec_version?: string };
      assert.strictEqual(result.source_spec_version, "1.0",
        `expected source_spec_version="1.0", got: ${JSON.stringify(result.source_spec_version)}`);
    } finally {
      h.stdin.end();
      await new Promise((r) => h.child.on("exit", r));
    }
  },
);

// ---------------------------------------------------------------------------
// as_spec_version ignored gracefully
// ---------------------------------------------------------------------------

test(
  "dump_expression_weights: as_spec_version param is accepted without error",
  { timeout: 60_000 },
  async (t) => {
    // Use a generated VRM 1.0 asset (from asset-generator) to avoid fixture
    // dependency for this behavioral correctness check.
    const { mkdtempSync, existsSync: fsExists } = await import("node:fs");
    const { execFileSync } = await import("node:child_process");
    const { tmpdir } = await import("node:os");
    const generator = path.resolve(REPO_ROOT, "target", "release", "vrm-asset-generator");
    if (!fsExists(generator)) {
      t.skip("vrm-asset-generator release binary absent; run: cargo build --release -p vrm-asset-generator");
      return;
    }
    const assetDir = mkdtempSync(path.join(tmpdir(), "three-vrm-ssv-"));
    execFileSync(generator, ["emit-default", "--id", "ssv_test", "--output-dir", assetDir], {
      stdio: "inherit",
    });
    const vrmPath = path.join(assetDir, "ssv_test.vrm");

    const h = spawnAdapter();
    try {
      const loadResp = await rpc(h, 1, "load_vrm", { path: vrmPath });
      assert.ok(!loadResp.error, `load_vrm failed: ${JSON.stringify(loadResp.error)}`);
      const sessionId = (loadResp.result as { session_id: string }).session_id;

      // Pass as_spec_version — adapter must NOT reject this with an error.
      const dumpResp = await rpc(h, 2, "dump_expression_weights", {
        session_id: sessionId,
        as_spec_version: "1.0",
      });
      assert.ok(!dumpResp.error,
        `dump_expression_weights rejected as_spec_version param: ${JSON.stringify(dumpResp.error)}`);

      const result = dumpResp.result as { source_spec_version?: string };
      // Generated asset uses VRMC_vrm, so must report 1.0.
      assert.strictEqual(result.source_spec_version, "1.0",
        `expected source_spec_version="1.0", got: ${JSON.stringify(result.source_spec_version)}`);
    } finally {
      h.stdin.end();
      await new Promise((r) => h.child.on("exit", r));
    }
  },
);
