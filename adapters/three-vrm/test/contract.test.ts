import { test } from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { readMessage, writeMessage } from "../src/framing.ts";
import { knownMethods } from "../src/operations.ts";
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

test(
  "render_sequence returns frames on disk with blake3 sentinel",
  { timeout: 120_000 },
  async () => {
    const { mkdtempSync, existsSync } = await import("node:fs");
    const { execFileSync } = await import("node:child_process");
    const { tmpdir } = await import("node:os");
    const generator = path.resolve(
      __dirname,
      "..",
      "..",
      "..",
      "target",
      "release",
      "vrm-asset-generator",
    );
    if (!existsSync(generator)) {
      // Skip gracefully if release binary absent (CI doesn't build release for TS tests).
      return;
    }
    const assetDir = mkdtempSync(path.join(tmpdir(), "three-vrm-seq-asset-"));
    execFileSync(generator, ["emit-default", "--id", "smoke", "--output-dir", assetDir], {
      stdio: "inherit",
    });
    const vrmPath = path.join(assetDir, "smoke.vrm");
    const outDir = mkdtempSync(path.join(tmpdir(), "three-vrm-seq-out-"));

    const h = spawnAdapter();
    try {
      // Phase 1 setup: load, camera, lighting, post-processing.
      await rpc(h, 1, "load_vrm", { path: vrmPath });
      await rpc(h, 2, "set_camera", {
        position: [0, 1, 3],
        target: [0, 1, 0],
        fov_degrees: 30,
      });
      await rpc(h, 3, "set_lighting", {
        ambient_intensity: 0.3,
        directional_intensity: 1.0,
        directional_direction: [0, 1, 0],
      });
      await rpc(h, 4, "set_post_processing", { tone_mapping: "none" });

      const seqResp = await rpc(h, 5, "render_sequence", {
        output_dir: outDir,
        width: 64,
        height: 64,
        color_space: "Srgb",
        frame_count: 2,
        frame_hz: 30.0,
        physics_dt_seconds: 1.0 / 60.0,
        output_format: "png_sequence",
      });

      assert.ok(
        !seqResp.error,
        `render_sequence failed: ${JSON.stringify(seqResp.error)}`,
      );
      const result = seqResp.result as {
        frames: Array<{ path: string; blake3: string }>;
      };
      assert.strictEqual(result.frames.length, 2, "expected 2 frames");
      for (const frame of result.frames) {
        assert.ok(existsSync(frame.path), `frame file missing: ${frame.path}`);
        assert.ok(
          frame.blake3.startsWith("blake3:"),
          `blake3 must start with "blake3:": ${frame.blake3}`,
        );
        assert.strictEqual(
          frame.blake3.length,
          7 + 64,
          `blake3 sentinel must be "blake3:" + 64 hex chars`,
        );
      }
    } finally {
      h.stdin.end();
      await new Promise((r) => h.child.on("exit", r));
    }
  },
);

test("render_sequence rejects mutual exclusion of animate_root_transform and apply_vrma", async () => {
  const h = spawnAdapter();
  try {
    const resp = await rpc(h, 6, "render_sequence", {
      output_dir: "/tmp/seq-reject",
      frame_count: 1,
      output_format: "png_sequence",
      animate_root_transform: { translation_start: [0, 0, 0], translation_end: [1, 0, 0] },
      apply_vrma: { vrma_handle: 0, time_seconds: 0 },
    });
    assert.equal(resp.error?.code, -32602, `expected -32602, got: ${JSON.stringify(resp)}`);
    assert.ok(
      (resp.error?.message ?? "").includes("mutually exclusive"),
      `expected "mutually exclusive" in message: ${resp.error?.message}`,
    );
  } finally {
    h.stdin.end();
    await new Promise((r) => h.child.on("exit", r));
  }
});

test("render_sequence rejects physics_dt_seconds above 60 Hz floor", async () => {
  const h = spawnAdapter();
  try {
    const resp = await rpc(h, 7, "render_sequence", {
      output_dir: "/tmp/seq-reject-dt",
      frame_count: 1,
      output_format: "png_sequence",
      physics_dt_seconds: 0.02,
    });
    assert.equal(resp.error?.code, -32602, `expected -32602, got: ${JSON.stringify(resp)}`);
    assert.ok(
      (resp.error?.message ?? "").includes("60 Hz"),
      `expected "60 Hz" in message: ${resp.error?.message}`,
    );
  } finally {
    h.stdin.end();
    await new Promise((r) => h.child.on("exit", r));
  }
});

test("knownMethods() catalog includes all 5 VRMA ops", () => {
  const methods = new Set(knownMethods());
  for (const op of [
    "load_vrma",
    "apply_vrma_at_time",
    "dump_humanoid_pose",
    "dump_expression_weights",
    "dump_look_at_state",
  ]) {
    assert.ok(methods.has(op), `knownMethods missing ${op}`);
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

test("benchmark_plan returns estimated_frames = warmup + measured", async () => {
  const h = spawnAdapter();
  try {
    const resp = await rpc(h, 1, "benchmark_plan", {
      warmup_frames: 5,
      measured_frames: 10,
      width: 64,
      height: 64,
    });
    assert.ok(!resp.error, `benchmark_plan failed: ${JSON.stringify(resp.error)}`);
    const result = resp.result as {
      estimated_frames: number;
      estimated_seconds: number;
      scene_summary: string;
    };
    assert.strictEqual(result.estimated_frames, 15, "estimated_frames must equal warmup + measured");
    assert.ok(result.estimated_seconds > 0, "estimated_seconds must be > 0");
    assert.ok(typeof result.scene_summary === "string", "scene_summary must be a string");
  } finally {
    h.stdin.end();
    await new Promise((r) => h.child.on("exit", r));
  }
});

test(
  "benchmark_execute returns timing/structural/geometry capabilities with correct shape",
  { timeout: 120_000 },
  async () => {
    const { mkdtempSync, existsSync } = await import("node:fs");
    const { execFileSync } = await import("node:child_process");
    const { tmpdir } = await import("node:os");
    const generator = path.resolve(
      __dirname,
      "..",
      "..",
      "..",
      "target",
      "release",
      "vrm-asset-generator",
    );
    if (!existsSync(generator)) {
      // Skip gracefully if release binary absent (CI doesn't build release for TS tests).
      return;
    }
    const assetDir = mkdtempSync(path.join(tmpdir(), "three-vrm-bench-asset-"));
    execFileSync(generator, ["emit-default", "--id", "smoke", "--output-dir", assetDir], {
      stdio: "inherit",
    });
    const vrmPath = path.join(assetDir, "smoke.vrm");

    const h = spawnAdapter();
    try {
      // Phase 1 setup: load, camera, lighting, post-processing.
      await rpc(h, 1, "load_vrm", { path: vrmPath });
      await rpc(h, 2, "set_camera", {
        position: [0, 1, 3],
        target: [0, 1, 0],
        fov_degrees: 30,
        up: [0, 1, 0],
      });
      await rpc(h, 3, "set_lighting", {
        directional: { dir: [0, -1, 0], color: [1, 1, 1], intensity: 1.0 },
        ambient: { color: [1, 1, 1], intensity: 0.3 },
        cast_shadows: false,
      });
      await rpc(h, 4, "set_post_processing", { tone_mapping: "None", exposure: 1.0 });

      const benchResp = await rpc(h, 5, "benchmark_execute", {
        width: 64,
        height: 64,
        color_space: "Srgb",
        warmup_frames: 2,
        measured_frames: 4,
      });

      assert.ok(
        !benchResp.error,
        `benchmark_execute failed: ${JSON.stringify(benchResp.error)}`,
      );
      const result = benchResp.result as {
        capabilities: string[];
        timing: {
          frame_time_ms: { p50: number; p95: number; p99: number };
          fps_mean: number;
          clock: string;
        };
        structural: { draw_calls: number; state_changes?: number; texture_bindings?: number };
        geometry: { triangles: number; vertices?: number };
      };

      // Capabilities must include timing, structural, geometry.
      assert.ok(Array.isArray(result.capabilities), "capabilities must be an array");
      assert.ok(result.capabilities.includes("timing"), 'capabilities must include "timing"');
      assert.ok(result.capabilities.includes("structural"), 'capabilities must include "structural"');
      assert.ok(result.capabilities.includes("geometry"), 'capabilities must include "geometry"');

      // Timing shape and values.
      assert.ok(result.timing.frame_time_ms.p50 >= 0, "p50 must be >= 0");
      assert.ok(result.timing.frame_time_ms.p95 >= 0, "p95 must be >= 0");
      assert.ok(result.timing.frame_time_ms.p99 >= 0, "p99 must be >= 0");
      assert.strictEqual(result.timing.clock, "cpu", 'clock must be "cpu"');

      // Structural: draw_calls present; state_changes / texture_bindings OMITTED.
      assert.ok(result.structural.draw_calls >= 0, "draw_calls must be >= 0");
      assert.strictEqual(
        result.structural.state_changes,
        undefined,
        "state_changes must be omitted (three.js does not instrument it)",
      );
      assert.strictEqual(
        result.structural.texture_bindings,
        undefined,
        "texture_bindings must be omitted (three.js does not instrument it)",
      );

      // Geometry: triangles present; vertices OMITTED.
      assert.ok(result.geometry.triangles >= 0, "triangles must be >= 0");
      assert.strictEqual(
        result.geometry.vertices,
        undefined,
        "vertices must be omitted (three.js does not instrument it)",
      );
    } finally {
      h.stdin.end();
      await new Promise((r) => h.child.on("exit", r));
    }
  },
);

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
