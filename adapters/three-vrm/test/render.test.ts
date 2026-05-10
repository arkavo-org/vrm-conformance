import { test } from "node:test";
import assert from "node:assert/strict";
import { spawn, execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { readMessage, writeMessage } from "../src/framing.ts";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ADAPTER_BIN = path.resolve(__dirname, "..", "dist", "main.js");
const REPO_ROOT = path.resolve(__dirname, "..", "..", "..");

interface Handle {
  child: ReturnType<typeof spawn>;
  stdin: NodeJS.WritableStream;
  stdout: NodeJS.ReadableStream;
}

function spawnAdapter(): Handle {
  const child = spawn(process.execPath, [ADAPTER_BIN], {
    stdio: ["pipe", "pipe", "ignore"],
  });
  if (!child.stdin || !child.stdout) {
    throw new Error("spawn failed");
  }
  return { child, stdin: child.stdin, stdout: child.stdout };
}

async function rpc(
  h: Handle,
  id: number,
  method: string,
  params: unknown,
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
): Promise<{ result?: any; error?: any; id?: any }> {
  const body = Buffer.from(
    JSON.stringify({ jsonrpc: "2.0", id, method, params }),
    "utf8",
  );
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  await writeMessage(h.stdin as any, body);
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const respBody = await readMessage(h.stdout as any);
  return JSON.parse(respBody.toString("utf8"));
}

function emitDefaultAsset(outDir: string, id: string): string {
  const generator = path.join(
    REPO_ROOT,
    "target",
    "release",
    "vrm-asset-generator",
  );
  if (!existsSync(generator)) {
    throw new Error(
      `vrm-asset-generator binary not found at ${generator}. ` +
        "Run 'cargo build --release -p vrm-asset-generator' from the repo root first.",
    );
  }
  execFileSync(
    generator,
    ["emit-default", "--id", id, "--output-dir", outDir],
    { stdio: "inherit" },
  );
  return path.join(outDir, `${id}.vrm`);
}

test(
  "full session against a real .vrm produces a non-magenta PNG",
  { timeout: 60_000 },
  async () => {
    const dir = mkdtempSync(path.join(tmpdir(), "three-vrm-render-"));
    const vrmPath = emitDefaultAsset(dir, "render_test");
    const outPath = path.join(dir, "out.png");

    const h = spawnAdapter();
    try {
      const loadResp = await rpc(h, 1, "load_vrm", { path: vrmPath });
      if (loadResp.error) {
        throw new Error(
          `load_vrm failed: ${JSON.stringify(loadResp.error, null, 2)}`,
        );
      }
      const sessionId = loadResp.result.session_id;
      assert.match(sessionId, /^three-vrm-/);

      const cameraResp = await rpc(h, 2, "set_camera", {
        session_id: sessionId,
        position: [0.0, 1.4, 1.5],
        target: [0.0, 1.4, 0.0],
        up: [0.0, 1.0, 0.0],
        fov_degrees: 30.0,
      });
      assert.ok(cameraResp.result, `set_camera: ${JSON.stringify(cameraResp)}`);

      const lightingResp = await rpc(h, 3, "set_lighting", {
        session_id: sessionId,
        directional: {
          dir: [-0.3, -0.6, -0.7],
          color: [1.0, 1.0, 1.0],
          intensity: 1.0,
        },
        ambient: { color: [0.5, 0.5, 0.5], intensity: 0.3 },
        cast_shadows: false,
        receive_shadows: false,
      });
      assert.ok(lightingResp.result);

      const postResp = await rpc(h, 4, "set_post_processing", {
        session_id: sessionId,
        tone_mapping: "None",
        exposure: 1.0,
      });
      assert.ok(postResp.result);

      const renderResp = await rpc(h, 5, "render", {
        session_id: sessionId,
        width: 256,
        height: 256,
        output_path: outPath,
        color_space: "Linear",
        msaa: 4,
        output_type: "Color",
      });
      assert.ok(renderResp.result, `render: ${JSON.stringify(renderResp)}`);
      assert.equal(renderResp.result.output_path, outPath);

      const disposeResp = await rpc(h, 6, "dispose", {
        session_id: sessionId,
      });
      assert.ok(disposeResp.result);

      assert.ok(existsSync(outPath), "render must produce a PNG file");
      const bytes = readFileSync(outPath);
      assert.deepEqual(
        Array.from(bytes.subarray(0, 8)),
        [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a],
        "PNG signature",
      );
      // "Something rendered" heuristic: an all-magenta 256×256 PNG encodes
      // to ~700 bytes after deflate; a real avatar render carries far more
      // entropy. If this ever flakes in CI, decode and count non-magenta
      // pixels instead.
      assert.ok(
        bytes.length > 1500,
        `PNG too small (${bytes.length} bytes) — likely all-magenta (renderer drew nothing)`,
      );
    } finally {
      h.stdin.end();
      await new Promise((r) => h.child.on("exit", r));
    }
  },
);
