// One Playwright Chromium browser + one page held for the lifetime of the
// adapter process. Routes intercepted to serve .vrm files from disk.

import { chromium, type Browser, type Page, type Route } from "playwright";
import { fileURLToPath, pathToFileURL } from "node:url";
import path from "node:path";
import fs from "node:fs/promises";
import fsSync from "node:fs";

const READY_FLAG = "window.__rendererReady === true";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// In `dist/`, renderer-host.html sits next to this file. In dev (tsx), the
// .ts file lives in `src/` and the html is alongside.
function findRendererHost(): string {
  const here = __dirname;
  const candidates = [
    path.join(here, "renderer-host.html"),
    path.join(here, "..", "src", "renderer-host.html"),
  ];
  for (const c of candidates) {
    if (fsSync.existsSync(c)) return c;
  }
  throw new Error(
    `renderer-host.html not found. Tried: ${candidates.join(", ")}`,
  );
}

interface LoadedAsset {
  /** The original disk path the adapter received for `load_vrm`. */
  diskPath: string;
}

export class BrowserSession {
  private browser: Browser | null = null;
  private page: Page | null = null;
  // We don't load multiple VRMs into the same page — three-vrm renders one
  // avatar at a time — so loading a new asset implicitly clears the previous.
  private currentAsset: LoadedAsset | null = null;
  private currentVrma: LoadedAsset | null = null;
  /** Node-side wall-clock milliseconds for the last loadVrm call. */
  private loadMs = 0;

  async start(): Promise<void> {
    if (this.browser) return;
    this.browser = await chromium.launch({
      headless: true,
      args: ["--enable-precise-memory-info"],
    });
    this.page = await this.browser.newPage();

    // Intercept all requests; serve `https://app.local/asset` from `currentAsset.diskPath`,
    // let everything else fall through (including CDN imports).
    await this.page.route("**/*", (route: Route) => {
      const url = route.request().url();
      if (url.startsWith("https://app.local/asset")) {
        const asset = this.currentAsset;
        if (!asset) {
          return route.fulfill({
            status: 404,
            contentType: "text/plain",
            body: "no asset loaded",
          });
        }
        return fs
          .readFile(asset.diskPath)
          .then((buffer) =>
            route.fulfill({
              status: 200,
              contentType: "model/gltf-binary",
              body: buffer,
            }),
          )
          .catch((err: unknown) =>
            route.fulfill({
              status: 500,
              contentType: "text/plain",
              body: `read asset failed: ${(err as Error).message}`,
            }),
          );
      }
      if (url.startsWith("https://app.local/vrma")) {
        const vrma = this.currentVrma;
        if (!vrma) {
          return route.fulfill({
            status: 404,
            contentType: "text/plain",
            body: "no vrma loaded",
          });
        }
        return fs
          .readFile(vrma.diskPath)
          .then((buffer) =>
            route.fulfill({
              status: 200,
              contentType: "model/gltf-binary",
              body: buffer,
            }),
          )
          .catch((err: unknown) =>
            route.fulfill({
              status: 500,
              contentType: "text/plain",
              body: `read vrma failed: ${(err as Error).message}`,
            }),
          );
      }
      return route.continue();
    });

    const hostPath = findRendererHost();
    const hostUrl = pathToFileURL(hostPath).toString();
    await this.page.goto(hostUrl, { waitUntil: "domcontentloaded" });
    await this.page.waitForFunction(READY_FLAG, undefined, { timeout: 30_000 });
  }

  async loadVrm(diskPath: string): Promise<void> {
    if (!this.page) throw new Error("BrowserSession not started");
    if (!fsSync.existsSync(diskPath)) {
      throw new Error(`vrm not found: ${diskPath}`);
    }
    this.currentAsset = { diskPath };
    const t0 = performance.now();
    await this.page.evaluate(
      ({ url }: { url: string }) =>
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        (window as any).__loadVrm(url),
      { url: "https://app.local/asset" },
    );
    this.loadMs = performance.now() - t0;
  }

  async setCamera(params: unknown): Promise<void> {
    if (!this.page) throw new Error("BrowserSession not started");
    await this.page.evaluate(
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (p) => (window as any).__setCamera(p),
      params,
    );
  }

  async setLighting(params: unknown): Promise<void> {
    if (!this.page) throw new Error("BrowserSession not started");
    await this.page.evaluate(
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (p) => (window as any).__setLighting(p),
      params,
    );
  }

  async setPostProcessing(params: unknown): Promise<void> {
    if (!this.page) throw new Error("BrowserSession not started");
    await this.page.evaluate(
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (p) => (window as any).__setPostProcessing(p),
      params,
    );
  }

  async stepPhysics(params: unknown): Promise<void> {
    if (!this.page) throw new Error("BrowserSession not started");
    await this.page.evaluate(
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (p) => (window as any).__stepPhysics(p),
      params,
    );
  }

  async resetPhysics(params: unknown): Promise<void> {
    if (!this.page) throw new Error("BrowserSession not started");
    await this.page.evaluate(
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (p) => (window as any).__resetPhysics(p),
      params,
    );
  }

  async animateRootTransform(params: unknown): Promise<void> {
    if (!this.page) throw new Error("BrowserSession not started");
    await this.page.evaluate(
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (p) => (window as any).__animateRootTransform(p),
      params,
    );
  }

  async dumpBonePositions(
    params: unknown,
  ): Promise<{ springs: Array<{ name: string; joint_positions: number[][] }> }> {
    if (!this.page) throw new Error("BrowserSession not started");
    return await this.page.evaluate(
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (p) => (window as any).__dumpBonePositions(p),
      params,
    );
  }

  async loadVrma(diskPath: string): Promise<{
    vrma_handle: number;
    channel_summary: {
      humanoid_bones: number;
      expressions: number;
      has_look_at: boolean;
      duration_seconds: number;
    };
  }> {
    if (!this.page) throw new Error("BrowserSession not started");
    if (!fsSync.existsSync(diskPath)) {
      throw new Error(`vrma not found: ${diskPath}`);
    }
    this.currentVrma = { diskPath };
    return await this.page.evaluate(
      ({ url }: { url: string }) =>
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        (window as any).__loadVrma(url),
      { url: "https://app.local/vrma" },
    );
  }

  async applyVrmaAtTime(params: unknown): Promise<{
    channels_applied: {
      humanoid_bones: number;
      expressions: number;
      look_at: boolean;
    };
  }> {
    if (!this.page) throw new Error("BrowserSession not started");
    return await this.page.evaluate(
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (p) => (window as any).__applyVrmaAtTime(p),
      params,
    );
  }

  async dumpHumanoidPose(): Promise<{
    source_spec_version: string | null;
    bones: Array<{ name: string; local_rotation_quat: number[] }>;
    hips_translation: number[];
    bones_missing: string[];
  }> {
    if (!this.page) throw new Error("BrowserSession not started");
    return await this.page.evaluate(
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      () => (window as any).__dumpHumanoidPose(),
    );
  }

  async dumpExpressionWeights(): Promise<{
    source_spec_version: string | null;
    presets: Record<string, number>;
    custom: Record<string, number>;
  }> {
    if (!this.page) throw new Error("BrowserSession not started");
    return await this.page.evaluate(
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      () => (window as any).__dumpExpressionWeights(),
    );
  }

  async dumpLookAtState(): Promise<{
    source_spec_version: string | null;
    gaze_direction_quat: number[];
    yaw_deg: number;
    pitch_deg: number;
    applied_via: string;
    offset_from_head_bone: number[];
  }> {
    if (!this.page) throw new Error("BrowserSession not started");
    return await this.page.evaluate(
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      () => (window as any).__dumpLookAtState(),
    );
  }

  async renderSequence(params: {
    output_dir: string;
    width: number;
    height: number;
    color_space: string;
    frame_count: number;
    frame_hz: number;
    physics_dt_seconds: number;
    animate_root_transform: unknown;
    capture_positions: boolean;
  }): Promise<{
    frames: Array<{
      index: number;
      timestamp_seconds: number;
      path: string;
      blake3: string;
      spring_positions?: Array<{ name: string; joint_positions: number[][] }>;
    }>;
    duration_seconds: number;
    actual_color_space: string;
    frame_hz_achieved: number;
  }> {
    if (!this.page) throw new Error("BrowserSession not started");
    await fs.mkdir(params.output_dir, { recursive: true });

    // __renderSequence returns one object per frame: { dataUrl, positions }.
    // `positions` is the dump_bone_positions result ({springs:[...]}) when
    // capture_positions was set, else null.
    type FrameOut = {
      dataUrl: string;
      positions: { springs: Array<{ name: string; joint_positions: number[][] }> } | null;
    };
    const frameOuts: FrameOut[] = await this.page.evaluate(
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (p) => (window as any).__renderSequence(p),
      {
        width: params.width,
        height: params.height,
        color_space: params.color_space,
        frame_count: params.frame_count,
        physics_dt_seconds: params.physics_dt_seconds,
        animate_root_transform: params.animate_root_transform,
        capture_positions: params.capture_positions,
      },
    );

    const ZERO_BLAKE3 = `blake3:${"0".repeat(64)}`;
    const frames: Array<{
      index: number;
      timestamp_seconds: number;
      path: string;
      blake3: string;
      spring_positions?: Array<{ name: string; joint_positions: number[][] }>;
    }> = [];
    for (let i = 0; i < frameOuts.length; i++) {
      const dataUrl = frameOuts[i].dataUrl;
      const comma = dataUrl.indexOf(",");
      if (comma < 0) throw new Error(`frame ${i}: malformed data URL`);
      const b64 = dataUrl.slice(comma + 1);
      const png = Buffer.from(b64, "base64");
      const frameIndex = String(i).padStart(4, "0");
      const framePath = path.join(params.output_dir, `${frameIndex}.png`);
      await fs.writeFile(framePath, png);
      const frame: {
        index: number;
        timestamp_seconds: number;
        path: string;
        blake3: string;
        spring_positions?: Array<{ name: string; joint_positions: number[][] }>;
      } = {
        index: i,
        timestamp_seconds: i / params.frame_hz,
        path: framePath,
        blake3: ZERO_BLAKE3,
      };
      const pos = frameOuts[i].positions;
      if (pos && pos.springs) frame.spring_positions = pos.springs;
      frames.push(frame);
    }

    return {
      frames,
      duration_seconds: params.frame_count / params.frame_hz,
      actual_color_space: params.color_space,
      frame_hz_achieved: params.frame_hz,
    };
  }

  async render(params: {
    width: number;
    height: number;
    color_space: string;
    output_path: string;
  }): Promise<void> {
    if (!this.page) throw new Error("BrowserSession not started");
    const dataUrl: string = await this.page.evaluate(
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (p) => (window as any).__render(p),
      {
        width: params.width,
        height: params.height,
        color_space: params.color_space,
      },
    );
    const comma = dataUrl.indexOf(",");
    if (comma < 0) throw new Error("renderer returned malformed data URL");
    const b64 = dataUrl.slice(comma + 1);
    const png = Buffer.from(b64, "base64");
    const dir = path.dirname(params.output_path);
    if (dir && dir !== ".") {
      await fs.mkdir(dir, { recursive: true });
    }
    await fs.writeFile(params.output_path, png);
  }

  async benchmarkPlan(p: {
    warmup_frames?: number;
    measured_frames?: number;
    width?: number;
    height?: number;
  }): Promise<{ estimated_frames: number; estimated_seconds: number; scene_summary: string }> {
    const warmup = p.warmup_frames ?? 30;
    const measured = p.measured_frames ?? 300;
    const total = warmup + measured;
    return {
      estimated_frames: total,
      estimated_seconds: total / 60.0,
      scene_summary: `three-vrm ${p.width ?? 0}x${p.height ?? 0}`,
    };
  }

  async benchmarkExecute(p: {
    width?: number;
    height?: number;
    color_space?: string;
    warmup_frames?: number;
    measured_frames?: number;
    animate_root_transform?: unknown;
  }): Promise<unknown> {
    if (!this.page) throw new Error("no session loaded");
    const warmup = p.warmup_frames ?? 30;
    const measured = p.measured_frames ?? 300;
    const raw = (await this.page.evaluate(
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      (q) => (window as any).__benchmarkRender(q),
      {
        width: p.width ?? 256,
        height: p.height ?? 256,
        color_space: p.color_space ?? "Linear",
        warmup_frames: warmup,
        measured_frames: measured,
        animate_root_transform: p.animate_root_transform ?? null,
      },
    )) as {
      frame_times_ms: number[];
      draw_calls_mean: number;
      triangles_mean: number;
      js_heap_bytes: number | null;
      first_frame_ms: number;
      animated: boolean;
      gpu_model: string;
    };

    // Compute percentiles on Node side.
    const sorted = [...raw.frame_times_ms].sort((a, b) => a - b);
    const pct = (q: number) =>
      sorted.length
        ? sorted[Math.min(sorted.length - 1, Math.floor(q * (sorted.length - 1)))]
        : 0;
    const mean =
      sorted.length ? sorted.reduce((a, b) => a + b, 0) / sorted.length : 0;

    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const measurement: any = {
      protocol: { warmup_frames: warmup, measured_frames: measured, animated: raw.animated },
      timing: {
        frame_time_ms: { p50: pct(0.5), p95: pct(0.95), p99: pct(0.99) },
        fps_mean: mean > 0 ? 1000 / mean : 0,
        clock: "cpu",
      },
      // state_changes and texture_bindings omitted — three.js does not instrument them.
      structural: { draw_calls: raw.draw_calls_mean },
      // vertices omitted — three.js does not expose a per-frame vertex counter.
      geometry: { triangles: Math.round(raw.triangles_mean) },
      host: {
        os: process.platform,
        os_version: process.version,
        gpu_vendor: "Google",
        gpu_model: raw.gpu_model,
        driver_version: "0",
        build_flags: "",
      },
      capabilities: ["timing", "structural", "geometry"],
    };

    if (raw.js_heap_bytes != null) {
      measurement.resources = {
        peak_memory_bytes: raw.js_heap_bytes,
        memory_kind: "host",
        load_ms: this.loadMs,
        first_frame_ms: raw.first_frame_ms,
      };
      measurement.capabilities.push("resources");
    }

    return measurement;
  }

  async clearVrm(): Promise<void> {
    if (!this.page) return;
    try {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      await this.page.evaluate(() => (window as any).__dispose());
    } catch {
      // ignore — page may be unhealthy
    }
    this.currentAsset = null;
    this.currentVrma = null;
  }

  async dispose(): Promise<void> {
    await this.clearVrm();
    if (this.browser) {
      try {
        await this.browser.close();
      } catch {
        // ignore
      }
    }
    this.browser = null;
    this.page = null;
  }
}
