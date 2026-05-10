# Phase 2C-b — three-vrm Real Headless Renderer

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the Phase 2C-a Unimplemented stubs in the three-vrm adapter with real Phase 1 op implementations driven by [pixiv/three-vrm](https://github.com/pixiv/three-vrm) running in a headless Chromium WebGL2 context. After this plan, the runner can spawn `adapters/three-vrm/` and produce actual cross-renderer-comparison-grade PNGs of generated VRM assets.

**Architecture:** The Node adapter process owns one Playwright Chromium browser instance for the lifetime of the adapter (~50MB RAM, ~500ms one-time launch cost). Inside the browser context, a static "renderer host" HTML page loads three.js + @pixiv/three-vrm via importmap, exposes a small API on `window` (`__loadVrm`, `__setCamera`, `__setLighting`, `__setPostProcessing`, `__render`), and the adapter drives that API via Playwright's `page.evaluate`. The .vrm bytes flow into the page through Playwright's `page.route()` interception — no separate HTTP server needed. Each `render` call screenshots the canvas via `canvas.toDataURL("image/png")`, decodes, and writes to `output_path`. Magenta `[255,0,255]` background matches the diff-engine bbox sentinel.

**Tech Stack:** Adds runtime deps to `adapters/three-vrm/package.json`: `playwright` (Chromium driver, ~10MB code + 200MB bundled browser), `three`, `@pixiv/three-vrm`. Browser-side three.js loaded from CDN via importmap inside the renderer-host HTML — keeps the Node side dep small and the page self-contained.

**Why this scope:**
- 2C-a proved the JSON-RPC framing; this swaps stubs for real functionality.
- The runner already drives the mock through the same contract; three-vrm must satisfy that contract identically.
- Once this lands, the diff loop has *two* real renderers to compare (mock vs three-vrm), unblocking real cross-renderer fidelity work.

**YAGNI scope guards:**
- No HDRI lighting (set_environment stays Unimplemented; v1.x).
- No spring bones (`step_physics`/`reset_physics` stay Unimplemented; Phase 2D).
- No expressions, humanoid pose, root transform — all reserved ops stay Unimplemented.
- No animation timeline; one render per `render` call.
- MSAA: best-effort. three.js's WebGLRenderer takes `antialias: true`; the actual sample count is browser-determined. Don't over-engineer.
- Color space: declare what was asked for (`Linear` or `Srgb`), but don't try to be color-managed beyond three.js's `outputColorSpace` setting.

---

## File Layout

| File | Status | Responsibility |
|---|---|---|
| `adapters/three-vrm/package.json` | Modify | Add `playwright`, `three`, `@pixiv/three-vrm` to runtime `dependencies` (not devDependencies — the production adapter needs them). |
| `adapters/three-vrm/src/renderer-host.html` | Create | Static HTML loaded inside the browser. importmap → three / three-vrm. Defines a `<canvas>` and the `window.__*` API the adapter calls. |
| `adapters/three-vrm/src/browser-session.ts` | Create | `BrowserSession` class wrapping Playwright launch + page setup + `page.route()` hookup for serving .vrm bytes from disk. |
| `adapters/three-vrm/src/operations.ts` | Modify | Replace the `dispatch()` stub with real handlers calling `BrowserSession`. Phase 1 ops become real; reserved ops stay Unimplemented. |
| `adapters/three-vrm/src/server.ts` | Modify | Pass an `AdapterContext` (holding the browser session) into dispatch so handlers have access. |
| `adapters/three-vrm/src/main.ts` | Modify | Construct + dispose the BrowserSession; the loop and EOF-handling stay the same. |
| `adapters/three-vrm/test/render.test.ts` | Create | Integration test: emit a default MToon asset via the asset-generator binary, spawn the three-vrm adapter, run the full op sequence, assert the PNG exists with expected dimensions and a non-magenta avatar bbox (proves something was rendered, not just the sentinel). |
| `adapters/three-vrm/README.md` | Modify | Status table flips 2C-b to "shipped." Add a "Browser dependency" caveat. |
| `.github/workflows/three-vrm.yml` | Modify | Add `npx playwright install --with-deps chromium` before `npm test` so CI has the browser. |
| `scripts/smoke.sh` | Modify | Add an optional `RUN_THREE_VRM=1` step that exercises three-vrm alongside the mock. |
| `docs/operation-contract.md` | Modify | Reference-implementations entry for three-vrm flips to "shipped." |

---

## Section A — Dependencies + browser bootstrap

### Task A1: Add runtime deps + verify Playwright launches a browser

**Files:**
- Modify: `adapters/three-vrm/package.json`

This is the Playwright fitness check. Front-loaded so we know early if headless Chromium can launch on this machine. If the install or browser launch fails, stop and escalate before anything else.

- [ ] **Step 1: Add runtime deps**

In `adapters/three-vrm/package.json`, add a `dependencies` section (sibling to `devDependencies`):

```json
"dependencies": {
  "@pixiv/three-vrm": "^3.5.0",
  "playwright": "^1.49.0",
  "three": "^0.171.0"
}
```

Note: `three` and `@pixiv/three-vrm` are bundled with the Node adapter only because the page loads them via importmap from a CDN at runtime; the Node-side install pins versions for reproducibility and lets the adapter optionally load from `node_modules` if a CDN-blocked environment ever needs it. Phase 2C-b uses CDN; the local copies stay as fallback.

- [ ] **Step 2: Install + download Chromium**

```bash
cd adapters/three-vrm
npm install
npx playwright install chromium
```

Expected: `npm install` completes; `playwright install chromium` downloads ~200 MB into a Playwright cache (typically `~/Library/Caches/ms-playwright/` on macOS). If the download fails (rate limit / network), retry. If it fails for an environment reason (corporate proxy, no internet), STOP and escalate — there is no useful fallback to write code against.

- [ ] **Step 3: Smoke-test that Chromium launches headless**

```bash
cd adapters/three-vrm
node --input-type=module -e "
import { chromium } from 'playwright';
const b = await chromium.launch({ headless: true });
const p = await b.newPage();
await p.setContent('<html><body><canvas id=c width=64 height=64></canvas></body></html>');
const has = await p.evaluate(() => {
  const c = document.getElementById('c');
  const gl = c.getContext('webgl2');
  return !!gl;
});
console.log('webgl2:', has);
await b.close();
"
```

Expected output: `webgl2: true`. If `false`, the headless build doesn't have WebGL2 (rare; would mean a Playwright bug or platform issue). Stop and escalate.

- [ ] **Step 4: Commit**

```bash
git add adapters/three-vrm/package.json adapters/three-vrm/package-lock.json
git commit -m "chore(three-vrm): add playwright + three + @pixiv/three-vrm runtime deps"
```

---

### Task A2: Renderer-host HTML page

**Files:**
- Create: `adapters/three-vrm/src/renderer-host.html`

A static page the browser loads once at session startup. It declares an importmap pointing at three.js + three-vrm on a CDN, sets up a `<canvas>`, and exposes a small global API the adapter calls via `page.evaluate`. No bundler — pure ES modules.

- [ ] **Step 1: Implement**

`adapters/three-vrm/src/renderer-host.html`:

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <title>three-vrm renderer host</title>
    <style>
      html, body { margin: 0; padding: 0; background: #ff00ff; overflow: hidden; }
      #c { display: block; }
    </style>
    <script type="importmap">
      {
        "imports": {
          "three": "https://unpkg.com/three@0.171.0/build/three.module.js",
          "three/addons/": "https://unpkg.com/three@0.171.0/examples/jsm/",
          "@pixiv/three-vrm": "https://unpkg.com/@pixiv/three-vrm@3.5.0/lib/three-vrm.module.min.js"
        }
      }
    </script>
  </head>
  <body>
    <canvas id="c"></canvas>
    <script type="module">
      import * as THREE from "three";
      import { GLTFLoader } from "three/addons/loaders/GLTFLoader.js";
      import { VRMLoaderPlugin } from "@pixiv/three-vrm";

      const MAGENTA = 0xff00ff;

      const state = {
        canvas: null,
        renderer: null,
        scene: null,
        camera: null,
        directional: null,
        ambient: null,
        vrm: null,
      };

      function ensureRenderer(width, height) {
        if (!state.canvas) {
          state.canvas = document.getElementById("c");
        }
        state.canvas.width = width;
        state.canvas.height = height;
        if (!state.renderer) {
          state.renderer = new THREE.WebGLRenderer({
            canvas: state.canvas,
            antialias: true,
            alpha: false,
            preserveDrawingBuffer: true,
          });
          state.renderer.setClearColor(MAGENTA, 1.0);
          state.renderer.outputColorSpace = THREE.SRGBColorSpace;
          state.renderer.toneMapping = THREE.NoToneMapping;
        }
        state.renderer.setSize(width, height, false);

        if (!state.scene) {
          state.scene = new THREE.Scene();
          state.scene.background = new THREE.Color(MAGENTA);
        }
        if (!state.camera) {
          state.camera = new THREE.PerspectiveCamera(30, width / height, 0.01, 100);
        } else {
          state.camera.aspect = width / height;
          state.camera.updateProjectionMatrix();
        }
        if (!state.directional) {
          state.directional = new THREE.DirectionalLight(0xffffff, 1.0);
          state.scene.add(state.directional);
        }
        if (!state.ambient) {
          state.ambient = new THREE.AmbientLight(0x808080, 0.3);
          state.scene.add(state.ambient);
        }
      }

      // The adapter calls this once per load_vrm. The .vrm bytes are served
      // from a Playwright-intercepted route (handled in browser-session.ts).
      window.__loadVrm = async function (url) {
        ensureRenderer(1024, 1024);
        if (state.vrm) {
          state.scene.remove(state.vrm.scene);
          state.vrm = null;
        }
        const loader = new GLTFLoader();
        loader.register((parser) => new VRMLoaderPlugin(parser));
        const gltf = await loader.loadAsync(url);
        const vrm = gltf.userData.vrm;
        state.vrm = vrm;
        state.scene.add(vrm.scene);
      };

      window.__setCamera = function (params) {
        ensureRenderer(state.canvas?.width ?? 1024, state.canvas?.height ?? 1024);
        state.camera.position.set(...params.position);
        state.camera.up.set(...params.up);
        state.camera.lookAt(new THREE.Vector3(...params.target));
        state.camera.fov = params.fov_degrees;
        state.camera.updateProjectionMatrix();
      };

      window.__setLighting = function (params) {
        ensureRenderer(state.canvas?.width ?? 1024, state.canvas?.height ?? 1024);
        const d = params.directional;
        // three.js DirectionalLight is positioned at (x,y,z); aim is from
        // position toward (0,0,0). We set the position to the negation of
        // the requested incoming-light direction so that the *direction* of
        // travel through the scene matches the requested dir.
        state.directional.position.set(-d.dir[0], -d.dir[1], -d.dir[2]).multiplyScalar(5);
        state.directional.color.setRGB(d.color[0], d.color[1], d.color[2]);
        state.directional.intensity = d.intensity;
        state.directional.castShadow = !!params.cast_shadows;
        if (state.renderer) {
          state.renderer.shadowMap.enabled = !!params.cast_shadows;
        }
        const a = params.ambient;
        state.ambient.color.setRGB(a.color[0], a.color[1], a.color[2]);
        state.ambient.intensity = a.intensity;
      };

      window.__setPostProcessing = function (params) {
        ensureRenderer(state.canvas?.width ?? 1024, state.canvas?.height ?? 1024);
        const map = {
          None: THREE.NoToneMapping,
          Linear: THREE.LinearToneMapping,
          Reinhard: THREE.ReinhardToneMapping,
          Aces: THREE.ACESFilmicToneMapping,
        };
        state.renderer.toneMapping = map[params.tone_mapping] ?? THREE.NoToneMapping;
        state.renderer.toneMappingExposure = params.exposure ?? 1.0;
      };

      // Render N frames (typically 1) and return the canvas as a base64 PNG.
      window.__render = async function (params) {
        ensureRenderer(params.width, params.height);
        // Color space: respect the runner's request as a hint to the
        // outputColorSpace. Pixel byte determinism is the renderer's job.
        if (params.color_space === "Linear") {
          state.renderer.outputColorSpace = THREE.LinearSRGBColorSpace;
        } else {
          state.renderer.outputColorSpace = THREE.SRGBColorSpace;
        }
        // Update VRM time so any default animations advance one tick.
        if (state.vrm && typeof state.vrm.update === "function") {
          state.vrm.update(1 / 60);
        }
        state.renderer.render(state.scene, state.camera);
        // Read back to a PNG data URL.
        const dataUrl = state.canvas.toDataURL("image/png");
        return dataUrl;
      };

      window.__dispose = function () {
        if (state.vrm) {
          state.scene.remove(state.vrm.scene);
          state.vrm = null;
        }
      };

      // Signal readiness — the adapter waits for this before any op.
      window.__rendererReady = true;
    </script>
  </body>
</html>
```

- [ ] **Step 2: Verify the file is well-formed HTML**

```bash
cd adapters/three-vrm
node -e "console.log(require('fs').readFileSync('src/renderer-host.html', 'utf8').length, 'bytes')"
```

Expected: ~4000 bytes printed (sanity check that the file exists and is readable).

- [ ] **Step 3: Commit**

```bash
git add adapters/three-vrm/src/renderer-host.html
git commit -m "feat(three-vrm): renderer-host HTML with three.js + three-vrm importmap and window API"
```

---

### Task A3: Make the HTML survive `tsc` build

**Files:**
- Modify: `adapters/three-vrm/package.json`
- Modify: `adapters/three-vrm/tsconfig.json`

`tsc` only emits .js files; `renderer-host.html` needs to be copied alongside `dist/main.js` so the browser-session can serve it. We'll add a copy step to the build script.

- [ ] **Step 1: Update build script**

In `adapters/three-vrm/package.json` change `"build": "tsc"` to:

```json
"build": "tsc && cp src/renderer-host.html dist/renderer-host.html"
```

- [ ] **Step 2: Verify build copies the HTML**

```bash
cd adapters/three-vrm
rm -rf dist && npm run build
ls dist/renderer-host.html
```

Expected: file exists.

- [ ] **Step 3: Commit**

```bash
git add adapters/three-vrm/package.json
git commit -m "build(three-vrm): copy renderer-host.html into dist after tsc"
```

---

## Section B — Browser session lifecycle

### Task B1: `BrowserSession` class

**Files:**
- Create: `adapters/three-vrm/src/browser-session.ts`

Wraps Playwright launch, page setup, route interception for `.vrm` files, and `page.evaluate` calls into ergonomic methods. One instance per adapter process.

- [ ] **Step 1: Implement**

`adapters/three-vrm/src/browser-session.ts`:

```ts
// One Playwright Chromium browser + one page held for the lifetime of the
// adapter process. Routes intercepted to serve .vrm files from disk.

import { chromium, type Browser, type Page, type Route } from "playwright";
import { fileURLToPath, pathToFileURL } from "node:url";
import path from "node:path";
import fs from "node:fs/promises";

// The renderer-host page registers a `__rendererReady` global; we wait for
// it before sending ops. This avoids a race where load_vrm fires before
// the importmap modules have evaluated.
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
    try {
      // Sync-stat is fine — we're in startup.
      // eslint-disable-next-line @typescript-eslint/no-require-imports
      const fsSync = require("node:fs");
      if (fsSync.existsSync(c)) return c;
    } catch {
      // ignore
    }
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
  // session_id → asset state. We don't load multiple VRMs into the same
  // page — three-vrm renders one avatar at a time — so loading a new
  // session implicitly clears the previous one's bytes from memory.
  private currentAsset: LoadedAsset | null = null;

  /**
   * Launch the browser, open one page, and load the renderer-host. Callers
   * await this before sending any op.
   */
  async start(): Promise<void> {
    if (this.browser) return;
    this.browser = await chromium.launch({ headless: true });
    this.page = await this.browser.newPage();

    // Intercept ALL requests; allow the renderer-host file: URL through, and
    // serve `app://asset` from `currentAsset.diskPath`. Everything else
    // (CDN imports for three.js + three-vrm) falls through.
    await this.page.route("**/*", (route: Route) => {
      const url = route.request().url();
      if (url.startsWith("app://asset")) {
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
      return route.continue();
    });

    const hostPath = findRendererHost();
    const hostUrl = pathToFileURL(hostPath).toString();
    await this.page.goto(hostUrl, { waitUntil: "domcontentloaded" });
    await this.page.waitForFunction(READY_FLAG, undefined, { timeout: 30_000 });
  }

  async loadVrm(diskPath: string): Promise<void> {
    if (!this.page) throw new Error("BrowserSession not started");
    // Make the bytes available to the page via the route interceptor.
    this.currentAsset = { diskPath };
    await this.page.evaluate(
      ({ url }: { url: string }) => (window as any).__loadVrm(url),
      { url: "app://asset" },
    );
  }

  async setCamera(params: unknown): Promise<void> {
    if (!this.page) throw new Error("BrowserSession not started");
    await this.page.evaluate(
      (p) => (window as any).__setCamera(p),
      params,
    );
  }

  async setLighting(params: unknown): Promise<void> {
    if (!this.page) throw new Error("BrowserSession not started");
    await this.page.evaluate(
      (p) => (window as any).__setLighting(p),
      params,
    );
  }

  async setPostProcessing(params: unknown): Promise<void> {
    if (!this.page) throw new Error("BrowserSession not started");
    await this.page.evaluate(
      (p) => (window as any).__setPostProcessing(p),
      params,
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
      (p) => (window as any).__render(p),
      { width: params.width, height: params.height, color_space: params.color_space },
    );
    // dataUrl is "data:image/png;base64,...."
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

  async dispose(): Promise<void> {
    this.currentAsset = null;
    if (this.page) {
      try {
        await this.page.evaluate(() => (window as any).__dispose());
      } catch {
        // ignore — page may be already closed
      }
    }
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
```

- [ ] **Step 2: Verify it compiles**

```bash
cd adapters/three-vrm
npx tsc --noEmit
```

Expected: clean.

- [ ] **Step 3: Commit**

```bash
git add adapters/three-vrm/src/browser-session.ts
git commit -m "feat(three-vrm): BrowserSession wrapping Playwright + page route interception"
```

---

## Section C — Hook real handlers into dispatch

### Task C1: `AdapterContext` + revised `dispatch` signature

**Files:**
- Modify: `adapters/three-vrm/src/operations.ts`

Today's `dispatch()` is pure (no state). To call into `BrowserSession`, dispatch needs a context handle. Refactor: keep `DispatchOutcome` shape, add `AdapterContext` parameter.

- [ ] **Step 1: Replace `operations.ts`**

Full new content:

```ts
// Operation registry + dispatch. Phase 2C-b: load_vrm, set_*, render,
// dispose call into the browser session; reserved Phase 2+ ops still
// return Unimplemented with the appropriate phase label.

import type { BrowserSession } from "./browser-session.js";

export interface RpcError {
  code: number;
  message: string;
  data?: unknown;
}

export interface AdapterContext {
  session: BrowserSession;
  /** Map from session_id → { ... } if/when we need per-session state. v0.1
   *  is single-session: load_vrm overwrites whatever was loaded prior, and
   *  the session_id we hand back is "three-vrm-<n>" purely for protocol
   *  compliance — three-vrm itself only holds one VRM at a time. */
  nextSessionId: { value: number };
}

const PHASE_BY_RESERVED_METHOD: Record<string, string> = {
  set_environment: "v1.x",
  set_expression: "Phase 3",
  set_humanoid_pose: "Phase 2",
  set_root_transform: "Phase 2",
  animate_root_transform: "Phase 2",
  step_physics: "Phase 2",
  reset_physics: "Phase 2",
};

export interface DispatchSuccess<T = unknown> {
  ok: true;
  result: T;
}

export interface DispatchFailure {
  ok: false;
  error: RpcError;
}

export type DispatchOutcome<T = unknown> = DispatchSuccess<T> | DispatchFailure;

export async function dispatch(
  ctx: AdapterContext,
  method: string,
  params: unknown,
): Promise<DispatchOutcome> {
  try {
    switch (method) {
      case "load_vrm": {
        const p = params as { path: string };
        if (!p?.path) return badParams("missing path");
        await ctx.session.loadVrm(p.path);
        const id = `three-vrm-${++ctx.nextSessionId.value}`;
        return { ok: true, result: { session_id: id } };
      }
      case "set_camera": {
        await ctx.session.setCamera(params);
        return { ok: true, result: {} };
      }
      case "set_lighting": {
        await ctx.session.setLighting(params);
        return { ok: true, result: {} };
      }
      case "set_post_processing": {
        await ctx.session.setPostProcessing(params);
        return { ok: true, result: {} };
      }
      case "render": {
        const p = params as {
          width?: number;
          height?: number;
          output_path?: string;
          color_space?: string;
        };
        if (!p?.output_path) return badParams("missing output_path");
        const width = p.width ?? 1024;
        const height = p.height ?? 1024;
        const colorSpace = p.color_space ?? "Srgb";
        await ctx.session.render({
          width,
          height,
          color_space: colorSpace,
          output_path: p.output_path,
        });
        return {
          ok: true,
          result: {
            output_path: p.output_path,
            actual_color_space: colorSpace,
          },
        };
      }
      case "dispose": {
        // Note: we don't tear the browser down on dispose — that happens in
        // main.ts's process-exit handler. dispose just clears the loaded VRM.
        await ctx.session.loadVrm.bind(ctx.session); // ensure session ok
        return { ok: true, result: {} };
      }
      default: {
        const phase = PHASE_BY_RESERVED_METHOD[method];
        if (phase) {
          return {
            ok: false,
            error: {
              code: -32000,
              message: `${method}: not implemented in this adapter version`,
              data: { phase },
            },
          };
        }
        return {
          ok: false,
          error: {
            code: -32601,
            message: `method not found: ${method}`,
          },
        };
      }
    }
  } catch (e) {
    const msg = (e as Error).message ?? String(e);
    // load_vrm-specific failures map to LoadFailed; other render-time
    // failures to RenderFailed. Cheap heuristic on method name.
    if (method === "load_vrm") {
      return {
        ok: false,
        error: {
          code: -32001,
          message: "LoadFailed",
          data: { reason: msg },
        },
      };
    }
    if (method === "render") {
      return {
        ok: false,
        error: {
          code: -32002,
          message: "RenderFailed",
          data: { reason: msg },
        },
      };
    }
    return {
      ok: false,
      error: {
        code: -32603,
        message: `internal error: ${msg}`,
      },
    };
  }
}

function badParams(reason: string): DispatchFailure {
  return {
    ok: false,
    error: { code: -32602, message: `invalid params: ${reason}` },
  };
}

export function knownMethods(): string[] {
  return [
    "load_vrm",
    "set_camera",
    "set_lighting",
    "set_post_processing",
    "render",
    "dispose",
    ...Object.keys(PHASE_BY_RESERVED_METHOD),
  ];
}
```

> **Caveat for the implementing engineer:** the `dispose` arm has `await ctx.session.loadVrm.bind(ctx.session)` as a placeholder. That's wrong — it just produces a bound method reference and awaits it (a no-op since binding a function returns a function, not a promise). The intent is "no-op other than telling the page to drop the loaded VRM." Replace with a small new method on `BrowserSession`:
>
> ```ts
> // browser-session.ts: add method
> async clearVrm(): Promise<void> {
>     if (!this.page) return;
>     try {
>         await this.page.evaluate(() => (window as any).__dispose());
>     } catch {}
>     this.currentAsset = null;
> }
> ```
>
> And in `operations.ts`'s dispose arm:
>
> ```ts
> case "dispose": {
>     await ctx.session.clearVrm();
>     return { ok: true, result: {} };
> }
> ```
>
> Apply that fix before committing.

- [ ] **Step 2: Apply the dispose fix**

In `browser-session.ts`, add the `clearVrm` method (insert after `dispose`):

```ts
async clearVrm(): Promise<void> {
    if (!this.page) return;
    try {
        await this.page.evaluate(() => (window as any).__dispose());
    } catch {
        // ignore — page may be unhealthy
    }
    this.currentAsset = null;
}
```

In `operations.ts`'s `dispose` arm, replace the broken line with:

```ts
case "dispose": {
    await ctx.session.clearVrm();
    return { ok: true, result: {} };
}
```

- [ ] **Step 3: Compile**

```bash
cd adapters/three-vrm
npx tsc --noEmit
```

Expected: clean.

- [ ] **Step 4: Update existing tests for the new dispatch signature**

The Phase 2C-a `test/contract.test.ts` calls `dispatch()` directly with two args (`method`, `params`). It now needs three (`ctx`, `method`, `params`). Since the test currently spawns the binary as a subprocess and exercises dispatch via JSON-RPC, it doesn't actually call dispatch directly — only the binary does. But the test still expects all known methods to return Unimplemented. After this commit, Phase 1 ops will return real results (or `LoadFailed` because no real .vrm exists at `/tmp/whatever.vrm`).

Update `test/contract.test.ts`:

- The "phase 1 op (load_vrm) returns -32000 with phase v1.x" test must be replaced with one that asserts `LoadFailed` (`-32001`) when given a non-existent path. The Phase 1 ops are no longer Unimplemented.

Replace that specific test:

```ts
test("phase 1 op load_vrm with missing file returns LoadFailed", async () => {
  const h = spawnAdapter();
  try {
    const resp = await rpc(h, 2, "load_vrm", { path: "/nonexistent/file.vrm" });
    assert.equal(resp.error?.code, -32001);
  } finally {
    h.stdin.end();
    await new Promise((r) => h.child.on("exit", r));
  }
});
```

The remaining tests (unknown method, reserved Phase 2/3 ops, parse error, multi-request) still apply unchanged — but the multi-request test currently uses two Phase 1 ops that previously returned -32000. Update it to use two reserved ops to keep the assertion stable:

```ts
test("multiple framed requests handled back-to-back", async () => {
  const h = spawnAdapter();
  try {
    const r1 = await rpc(h, 1, "step_physics", {});
    const r2 = await rpc(h, 2, "set_humanoid_pose", {});
    assert.equal(r1.error?.code, -32000);
    assert.equal(r2.error?.code, -32000);
  } finally {
    h.stdin.end();
    await new Promise((r) => h.child.on("exit", r));
  }
});
```

- [ ] **Step 5: Commit**

```bash
git add adapters/three-vrm/src/operations.ts adapters/three-vrm/src/browser-session.ts adapters/three-vrm/test/contract.test.ts
git commit -m "feat(three-vrm): real Phase 1 op handlers wired through BrowserSession"
```

(Don't run tests yet — server.ts still needs updating in C2 and main.ts in C3, so compile/test won't pass cleanly until C3 lands.)

---

### Task C2: Server reads context for dispatch

**Files:**
- Modify: `adapters/three-vrm/src/server.ts`

`server.run()` previously called `dispatch(method, params)`. It now needs an `AdapterContext`. Take it as a parameter and pass through.

- [ ] **Step 1: Replace `server.ts`**

```ts
// Stdio JSON-RPC server. One request → one response. EOF on input stream
// ends the loop cleanly.

import type { Readable, Writable } from "node:stream";
import { readMessage, writeMessage, FrameError } from "./framing.js";
import { dispatch, type AdapterContext } from "./operations.js";

interface JsonRpcRequest {
  jsonrpc: string;
  id: number | string | null;
  method: string;
  params?: unknown;
}

interface JsonRpcResponse {
  jsonrpc: "2.0";
  id: number | string | null;
  result?: unknown;
  error?: { code: number; message: string; data?: unknown };
}

export async function run(
  ctx: AdapterContext,
  input: Readable,
  output: Writable,
): Promise<void> {
  while (true) {
    let body: Buffer;
    try {
      body = await readMessage(input);
    } catch (e) {
      if (e instanceof FrameError && /content-length/i.test(e.message)) {
        // EOF on a clean shutdown.
      } else {
        process.stderr.write(
          `three-vrm: read failed: ${(e as Error).message}\n`,
        );
      }
      return;
    }

    let req: JsonRpcRequest;
    try {
      req = JSON.parse(body.toString("utf8")) as JsonRpcRequest;
    } catch (e) {
      const resp: JsonRpcResponse = {
        jsonrpc: "2.0",
        id: null,
        error: {
          code: -32700,
          message: `parse error: ${(e as Error).message}`,
        },
      };
      await writeMessage(output, Buffer.from(JSON.stringify(resp), "utf8"));
      continue;
    }

    const id = req.id ?? null;
    const outcome = await dispatch(ctx, req.method, req.params);

    const resp: JsonRpcResponse = outcome.ok
      ? { jsonrpc: "2.0", id, result: outcome.result }
      : { jsonrpc: "2.0", id, error: outcome.error };

    await writeMessage(output, Buffer.from(JSON.stringify(resp), "utf8"));
  }
}
```

- [ ] **Step 2: Compile**

```bash
cd adapters/three-vrm
npx tsc --noEmit
```

Expected: clean. (`main.ts` won't yet pass `ctx` — that's C3.)

- [ ] **Step 3: Commit**

```bash
git add adapters/three-vrm/src/server.ts
git commit -m "refactor(three-vrm): server.run takes AdapterContext for dispatch"
```

---

### Task C3: `main.ts` constructs the BrowserSession + AdapterContext

**Files:**
- Modify: `adapters/three-vrm/src/main.ts`

- [ ] **Step 1: Replace `main.ts`**

```ts
// Entry point: launch the browser session, wire stdin/stdout into the
// JSON-RPC server loop, dispose on exit.

import { run } from "./server.js";
import { BrowserSession } from "./browser-session.js";
import type { AdapterContext } from "./operations.js";

process.stderr.write("three-vrm adapter starting\n");

const session = new BrowserSession();
const ctx: AdapterContext = {
  session,
  nextSessionId: { value: 0 },
};

let exitCode = 0;

try {
  await session.start();
  process.stderr.write("three-vrm adapter ready\n");
  await run(ctx, process.stdin, process.stdout);
} catch (err) {
  process.stderr.write(`three-vrm adapter fatal: ${(err as Error).message}\n`);
  exitCode = 1;
} finally {
  await session.dispose();
}

process.exit(exitCode);
```

> **Top-level `await` requirement:** this file uses `await` at the module top level, which works for ESM modules. Confirm by checking that `package.json` has `"type": "module"` (it does, set in 2C-a) and that `tsconfig.json`'s `module` is `NodeNext` (it is, set in 2C-a).

- [ ] **Step 2: Build the full project**

```bash
cd adapters/three-vrm
npm run build
```

Expected: compiles cleanly; `dist/` contains `main.js`, `server.js`, `operations.js`, `browser-session.js`, `framing.js`, plus the copied `renderer-host.html`.

- [ ] **Step 3: Run existing contract tests**

```bash
cd adapters/three-vrm
npm test
```

Expected: 5 framing tests + 6 contract tests = 11 tests, all green. The contract tests now exercise the *real* dispatch path (with the BrowserSession launching headless Chromium each time the test spawns the adapter), so the test suite startup is slower (~1-2s per test for browser launch). On constrained CI runners this may approach 30s for the full file.

If tests fail because Chromium can't launch under the test environment (e.g. no GPU on CI), the `npx playwright install --with-deps chromium` step in `.github/workflows/three-vrm.yml` (added in Task E) handles the install, but local dev needs `npx playwright install chromium` to have run already (Task A1 Step 2).

- [ ] **Step 4: Commit**

```bash
git add adapters/three-vrm/src/main.ts
git commit -m "feat(three-vrm): main.ts launches BrowserSession + wires AdapterContext"
```

---

## Section D — Integration test against a real .vrm

### Task D1: End-to-end render test

**Files:**
- Create: `adapters/three-vrm/test/render.test.ts`

Build a real .vrm via the asset-generator, drive the adapter through the full op sequence, assert the produced PNG exists, has the right dimensions, and contains non-magenta pixels (proves something was drawn).

This test takes ~5-10 seconds end-to-end (browser launch + .vrm parse + render). Marked appropriately so a developer running `npm test` knows what to expect.

- [ ] **Step 1: Implement**

`adapters/three-vrm/test/render.test.ts`:

```ts
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
): Promise<{ result?: any; error?: any; id?: any }> {
  const body = Buffer.from(
    JSON.stringify({ jsonrpc: "2.0", id, method, params }),
    "utf8",
  );
  await writeMessage(h.stdin as any, body);
  const respBody = await readMessage(h.stdout as any);
  return JSON.parse(respBody.toString("utf8"));
}

function emitDefaultAsset(outDir: string, id: string): string {
  // Use the cargo-built asset-generator binary. This test assumes the
  // workspace has been built (cargo build --release -p vrm-asset-generator).
  // If it hasn't, the test errors with a useful message rather than hanging.
  const generator = path.join(REPO_ROOT, "target", "release", "vrm-asset-generator");
  if (!existsSync(generator)) {
    throw new Error(
      `vrm-asset-generator binary not found at ${generator}. ` +
        "Run 'cargo build --release -p vrm-asset-generator' from the repo root first.",
    );
  }
  execFileSync(generator, ["emit-default", "--id", id, "--output-dir", outDir], {
    stdio: "inherit",
  });
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
      // load_vrm
      const loadResp = await rpc(h, 1, "load_vrm", { path: vrmPath });
      if (loadResp.error) {
        throw new Error(
          `load_vrm failed: ${JSON.stringify(loadResp.error, null, 2)}`,
        );
      }
      const sessionId = loadResp.result.session_id;
      assert.match(sessionId, /^three-vrm-/);

      // set_camera
      const cameraResp = await rpc(h, 2, "set_camera", {
        session_id: sessionId,
        position: [0.0, 1.4, 1.5],
        target: [0.0, 1.4, 0.0],
        up: [0.0, 1.0, 0.0],
        fov_degrees: 30.0,
      });
      assert.ok(cameraResp.result, `set_camera: ${JSON.stringify(cameraResp)}`);

      // set_lighting
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

      // set_post_processing
      const postResp = await rpc(h, 4, "set_post_processing", {
        session_id: sessionId,
        tone_mapping: "None",
        exposure: 1.0,
      });
      assert.ok(postResp.result);

      // render
      const renderResp = await rpc(h, 5, "render", {
        session_id: sessionId,
        width: 256,
        height: 256,
        output_path: outPath,
        color_space: "Linear",
        msaa: 4,
        output_type: "Color",
      });
      assert.ok(
        renderResp.result,
        `render: ${JSON.stringify(renderResp)}`,
      );
      assert.equal(renderResp.result.output_path, outPath);

      // dispose
      const disposeResp = await rpc(h, 6, "dispose", {
        session_id: sessionId,
      });
      assert.ok(disposeResp.result);

      // PNG sanity: file exists, has PNG signature, and not 100% magenta.
      assert.ok(existsSync(outPath), "render must produce a PNG file");
      const bytes = readFileSync(outPath);
      assert.deepEqual(
        Array.from(bytes.subarray(0, 8)),
        [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a],
        "PNG signature",
      );
      // Quick "something rendered" check: count distinct RGB triplets in
      // the IDAT-decoded pixel data via the `image-decode` approach that
      // doesn't pull in another dep — for now, just check that the file
      // is bigger than a plausibly-all-magenta encoded PNG (~700 bytes for
      // 256×256). A real avatar render encodes much more entropy.
      assert.ok(
        bytes.length > 1500,
        `PNG too small (${bytes.length} bytes) — likely all-magenta`,
      );
    } finally {
      h.stdin.end();
      await new Promise((r) => h.child.on("exit", r));
    }
  },
);
```

> **Caveat:** the "non-magenta" check via byte size is a coarse heuristic. A more rigorous check would decode the PNG and count non-`#ff00ff` pixels. We avoid pulling in another image dep here; the byte-size check is good enough to catch the "renderer drew nothing" failure mode. If false positives happen in CI (e.g., a different magenta encoding), upgrade this to a real decode.

- [ ] **Step 2: Build asset-generator first, then run tests**

```bash
cd /Users/arkavo/Projects/vrm-conformance
cargo build --release -p vrm-asset-generator
cd adapters/three-vrm
npm run build
npm test
```

Expected: 5 framing tests + 6 contract tests + 1 render test = 12 tests, all green. The render test takes ~5-10s; the full file run is dominated by per-test browser launches.

> If `npm test` runs *all* test files including render.test.ts and the render test fails because the asset-generator binary isn't built, the failure message points at `cargo build --release -p vrm-asset-generator`. Build it and re-run.

- [ ] **Step 3: Commit**

```bash
git add adapters/three-vrm/test/render.test.ts
git commit -m "test(three-vrm): full-session render produces non-magenta PNG"
```

---

## Section E — CI + smoke + docs

### Task E1: CI installs Playwright Chromium

**Files:**
- Modify: `.github/workflows/three-vrm.yml`

CI runners need the Chromium binary; `npm ci` doesn't pull it (Playwright keeps it out of npm packages). Add `npx playwright install --with-deps chromium` as a step before `npm test`.

We also need the asset-generator binary available because `render.test.ts` shells out to it. Build it inline.

- [ ] **Step 1: Update workflow**

Replace `.github/workflows/three-vrm.yml`:

```yaml
name: three-vrm

on:
  pull_request:
    paths:
      - 'adapters/three-vrm/**'
      - '.github/workflows/three-vrm.yml'
      # render.test.ts shells out to the asset-generator, so we re-run on
      # changes to it as well.
      - 'crates/vrm-asset-generator/**'
  push:
    branches: [main]
    paths:
      - 'adapters/three-vrm/**'
      - '.github/workflows/three-vrm.yml'
      - 'crates/vrm-asset-generator/**'

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: '20'
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Build asset-generator (used by render.test.ts)
        run: cargo build --release -p vrm-asset-generator
      - name: Install adapter deps
        run: npm ci
        working-directory: adapters/three-vrm
      - name: Install Playwright Chromium
        run: npx playwright install --with-deps chromium
        working-directory: adapters/three-vrm
      - name: Build adapter
        run: npm run build
        working-directory: adapters/three-vrm
      - name: Run tests
        run: npm test
        working-directory: adapters/three-vrm
```

- [ ] **Step 2: Commit**

```bash
git add .github/workflows/three-vrm.yml
git commit -m "ci(three-vrm): install Playwright Chromium + build asset-generator before tests"
```

---

### Task E2: Smoke script optionally exercises three-vrm

**Files:**
- Modify: `scripts/smoke.sh`

Behind a `RUN_THREE_VRM=1` env var, build the adapter and ask the runner to drive it alongside the mock. Cross-renderer SSIM in this v0.1 will fail (mock and three-vrm produce visually-different output), but that's expected — the smoke just proves the runner can drive both adapters.

- [ ] **Step 1: Add the optional step**

In `scripts/smoke.sh`, find the section after the runner-diff loop and before the S3 upload (just after `==> Running runner diff loop`'s block). Add:

```bash
# ---- step 4c: optional three-vrm exercise ---------------------------------
if [ "${RUN_THREE_VRM:-0}" = "1" ] && [ "$SKIP_RENDER" != "1" ]; then
    echo "==> Running three-vrm adapter (RUN_THREE_VRM=1)"
    THREE_VRM_DIR=$ROOT/adapters/three-vrm
    (cd "$THREE_VRM_DIR" && npm install --silent && npm run build --silent && npx --yes playwright install chromium >/dev/null 2>&1)
    THREE_VRM_OUT="$OUTPUTS/smoke_default_three-vrm.png"
    if cargo run --release -p vrm-runner -- execute-test-plan \
            --plan "$ASSETS/smoke_default.test.yaml" \
            --adapter-bin node \
            --adapter-args "$THREE_VRM_DIR/dist/main.js" \
            --asset-dir "$ASSETS" \
            --output-dir "$OUTPUTS" \
            --renderer-name three-vrm \
            --json; then
        if [ -f "$THREE_VRM_OUT" ]; then
            echo "    three-vrm produced: $THREE_VRM_OUT ($(wc -c < "$THREE_VRM_OUT") bytes)"
        fi
    else
        echo "    three-vrm runner step exited non-zero (continuing)" >&2
    fi
fi
```

Update the prereq comment at the top of the script:

```
# Optional: RUN_THREE_VRM=1 to also exercise the three-vrm adapter (slow:
# requires Playwright Chromium installed via `npx playwright install chromium`).
```

- [ ] **Step 2: Commit**

```bash
git add scripts/smoke.sh
git commit -m "chore(smoke): optional three-vrm exercise behind RUN_THREE_VRM=1"
```

---

### Task E3: Update docs

**Files:**
- Modify: `adapters/three-vrm/README.md`
- Modify: `docs/operation-contract.md`

- [ ] **Step 1: Update the adapter README**

In `adapters/three-vrm/README.md`, replace the status table:

```markdown
## Status

| Phase | Scope | State |
|---|---|---|
| 2C-a | TS scaffold + JSON-RPC framing + Unimplemented dispatch | shipped |
| 2C-b | three-vrm + Playwright headless WebGL2 + real Phase 1 ops | shipped |

Phase 1 ops are real: load_vrm, set_camera, set_lighting, set_post_processing, render, dispose. Reserved Phase 2+ ops still return `Unimplemented` with the appropriate phase label.
```

Add a "Browser dependency" subsection right after Status:

```markdown
## Browser dependency

This adapter spawns a headless Chromium instance via [Playwright](https://playwright.dev/) and runs three.js + three-vrm inside the browser context. The Chromium binary is **not** included in `node_modules`; install it once after `npm install`:

\`\`\`bash
npx playwright install chromium
\`\`\`

Disk: ~250 MB cached at `~/Library/Caches/ms-playwright/` (macOS) or `~/.cache/ms-playwright/` (Linux). RAM at runtime: ~50 MB per running adapter.
```

- [ ] **Step 2: Update operation-contract docs**

In `docs/operation-contract.md`, update the three-vrm bullet in "Reference implementations" from:

> JSON-RPC framing is implemented (Phase 2C-a); real headless-WebGL2 rendering via Playwright lands in Phase 2C-b. All ops currently return `Unimplemented`.

to:

> Phase 1 ops (load_vrm, set_camera, set_lighting, set_post_processing, render, dispose) drive a Playwright headless Chromium WebGL2 context with [pixiv/three-vrm](https://github.com/pixiv/three-vrm) running inside. Reserved Phase 2+ ops return `Unimplemented`. Requires `npx playwright install chromium` after `npm install`.

- [ ] **Step 3: Commit**

```bash
git add adapters/three-vrm/README.md docs/operation-contract.md
git commit -m "docs: three-vrm Phase 2C-b shipped (status + browser dep + contract entry)"
```

---

## Self-Review

**Spec coverage:**

| 2C-b goal | Task |
|---|---|
| Add Playwright + three + three-vrm + smoke-test browser launch | A1 |
| Renderer-host HTML page with importmap + window API | A2 |
| Build copies HTML to `dist/` | A3 |
| BrowserSession (launch, route, evaluate, dispose) | B1 |
| Real dispatch with AdapterContext | C1 |
| Server takes context | C2 |
| main.ts wires browser session lifecycle | C3 |
| End-to-end render integration test | D1 |
| CI installs Chromium + builds asset-generator | E1 |
| Smoke optionally exercises three-vrm | E2 |
| README + contract docs | E3 |

**Placeholder scan:**

- The `dispose` arm in C1's first draft has a flagged-and-fixed bug (`await ctx.session.loadVrm.bind(...)`). The fix is spelled out in C1 Step 2.
- The "non-magenta" check in D1 uses a byte-size heuristic with a documented caveat. Acceptable for v0.1.

**Type consistency:**

- `AdapterContext` defined in C1; consumed in C2 and constructed in C3.
- `BrowserSession` methods (`start`, `loadVrm`, `setCamera`, `setLighting`, `setPostProcessing`, `render`, `clearVrm`, `dispose`) named consistently across B1, C1, C3.
- The renderer-host's `window.__*` API is named in A2; the BrowserSession in B1 calls those exact names via `page.evaluate`.

**YAGNI guards:**

- ✅ No HDRI / env map.
- ✅ No animation timeline; one render per call.
- ✅ Reserved ops still Unimplemented.
- ✅ No bundler — three.js + three-vrm load from CDN via importmap.
- ✅ Smoke step is opt-in (env-var gated) so the default smoke stays fast.

**Risk register:**

- **Playwright in CI.** `npx playwright install --with-deps chromium` on ubuntu-latest reliably works for ~99% of GitHub-hosted runners. If runners ever lack the apt-get permissions `--with-deps` requires, fall back to `npx playwright install chromium` and accept that some shared-libs may be missing.
- **CDN availability.** The renderer-host loads three.js + three-vrm from unpkg.com. If unpkg is rate-limited or down during a CI run, the test fails. Mitigation (Phase 2C-c if it ever bites): vendor three.js + three-vrm into `dist/vendor/` and rewrite the importmap to `./vendor/...`.
- **Browser launch on macOS arm64.** Playwright officially supports it; verified in A1 Step 3.
- **Top-level await in main.ts.** Requires Node 20+ and `"type": "module"`. Both already in place.

---

## Execution Handoff

Plan saved to `docs/superpowers/plans/2026-05-10-phase2cb-three-vrm-real-renderer.md`. Two execution options:

1. **Subagent-Driven** — fresh subagent per task; useful here because Playwright iteration may need debugging cycles a fresh subagent handles cleanly.
2. **Inline Execution (recommended)** — sequential dependencies (A1 must succeed before any B/C; B1 before C1; C1 before C2; C2 before C3; C3 before D1). 12 tasks total. Inline lets us bail early if Playwright proves unviable on this machine; subagent dispatch adds overhead without parallelism payoff here.

Estimated time: ~30-45 minutes if Playwright cooperates; longer if browser-side debugging is needed.
