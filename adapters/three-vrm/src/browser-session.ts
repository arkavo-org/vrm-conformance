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

  async start(): Promise<void> {
    if (this.browser) return;
    this.browser = await chromium.launch({ headless: true });
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
    await this.page.evaluate(
      ({ url }: { url: string }) =>
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        (window as any).__loadVrm(url),
      { url: "https://app.local/asset" },
    );
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
