# Phase 2C-a — three-vrm Adapter Scaffold (TS)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the TypeScript scaffold for a `three-vrm` renderer adapter — Node project, JSON-RPC stdio server, full Phase 1 op contract returning `Unimplemented` (Phase 2C-b will plug in real headless-WebGL rendering). Mirrors how the Swift adapter landed in Phase 1 (L1+L2 framing scaffold first, L3 real rendering deferred). The runner can now spawn this binary just like the mock; ops error cleanly until 2C-b.

**Architecture:** A new `adapters/three-vrm/` Node + TypeScript project with its own `package.json` (independent from the existing `site/` project). Single executable entrypoint reads framed JSON-RPC requests on stdin via the LSP-style Content-Length convention (same framing as Swift, Rust mock, vrm-ops::stdio), routes to per-op handlers, writes framed responses. All ops return `Unimplemented` for now with the appropriate phase label (`v1.x` / `Phase 2` / `Phase 3` per the operation contract). Tests use `node:test` (built-in, no extra deps) to spawn the adapter as a subprocess and assert framed responses, mirroring the mock crate's `contract.rs`.

**Tech Stack:** Node 20+, TypeScript 5.4+, `tsx` for test/dev runs (so we don't need a build step in the inner loop), no other runtime deps. The build step uses plain `tsc` to emit `dist/main.js` for distribution. Phase 2C-b will add `three`, `@pixiv/three-vrm`, and `playwright`.

**Why scaffold-first:**
- Establishes the JSON-RPC contract on the TS side before the renderer integration adds complexity.
- Lets the runner spawn the three-vrm adapter and verify error envelopes (`-32000` Unimplemented, `-32601` not-found, `-32700` parse-error) end-to-end.
- Matches the cadence of the Swift adapter: L1+L2 first (framing + Unimplemented dispatch), L3 (real rendering) later. That separation worked well in Phase 1.

**YAGNI scope guards:**
- No three.js, no three-vrm, no Playwright, no GL — those land in 2C-b.
- No `describe`-style operation catalog endpoint — that's a runner concern, not adapter.
- Reserved Phase 2+ ops (`set_environment`, `step_physics`, etc.) get the same Unimplemented path as Phase 1 ops in this scaffold; 2C-b will keep the reserved-op stubs and only swap the Phase 1 implementations.

---

## File Layout

| File | Status | Responsibility |
|---|---|---|
| `adapters/three-vrm/package.json` | Create | Node project metadata, scripts (`build`, `test`, `start`), TS deps. |
| `adapters/three-vrm/tsconfig.json` | Create | Strict TypeScript, ES2022, NodeNext module resolution. |
| `adapters/three-vrm/.gitignore` | Create | `node_modules/`, `dist/`, `*.tgz`. |
| `adapters/three-vrm/README.md` | Create | Status table, build instructions, Phase 2C-b TODO. |
| `adapters/three-vrm/src/framing.ts` | Create | LSP-style `readMessage` / `writeMessage` over `Readable` / `Writable` streams. |
| `adapters/three-vrm/src/operations.ts` | Create | Phase mapping + dispatch table (currently all paths → Unimplemented). |
| `adapters/three-vrm/src/server.ts` | Create | Read-loop: framed requests → dispatch → framed responses. EOF on stdin = clean exit. |
| `adapters/three-vrm/src/main.ts` | Create | Entry point: wires `process.stdin`/`stdout` into `server.run()`. |
| `adapters/three-vrm/test/framing.test.ts` | Create | Round-trip framing primitives, malformed-header behavior. |
| `adapters/three-vrm/test/contract.test.ts` | Create | Subprocess-based contract test mirroring `vrm-mock-renderer/tests/contract.rs`. |
| `.gitignore` (repo root) | Modify | Add `adapters/*/node_modules/` and `adapters/*/dist/` (the existing pattern only covers Swift `.build/`). |
| `.github/workflows/three-vrm.yml` | Create | Node CI: `npm ci`, `npm run build`, `npm test`. Triggers on changes to `adapters/three-vrm/**`. |
| `docs/operation-contract.md` | Modify | Add `three-vrm` to "Reference implementations" with Phase 2C-a status. |

---

## Section A — Project scaffold

### Task A1: TypeScript project skeleton

**Files:**
- Create: `adapters/three-vrm/package.json`
- Create: `adapters/three-vrm/tsconfig.json`
- Create: `adapters/three-vrm/.gitignore`
- Create: `adapters/three-vrm/README.md`
- Modify: repo root `.gitignore`

- [ ] **Step 1: Update repo root `.gitignore`**

In `/Users/arkavo/Projects/vrm-conformance/.gitignore`, find the existing Swift block:

```
# Swift / Xcode
adapters/*/.build/
adapters/*/.swiftpm/
adapters/*/Package.resolved
```

Replace with:

```
# Swift / Xcode (adapters/<adapter>/.build/ is also Swift)
adapters/*/.build/
adapters/*/.swiftpm/
adapters/*/Package.resolved

# Node (TS adapter projects under adapters/)
adapters/*/node_modules/
adapters/*/dist/
adapters/*/package-lock.json.bak
```

- [ ] **Step 2: Create `adapters/three-vrm/package.json`**

```json
{
  "name": "vrm-three-vrm-adapter",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "description": "three-vrm renderer adapter for arkavo-org/vrm-conformance. Phase 2C-a: scaffold + JSON-RPC framing; renderer integration deferred to 2C-b.",
  "license": "Apache-2.0",
  "engines": {
    "node": ">=20.0.0"
  },
  "scripts": {
    "build": "tsc",
    "start": "tsx src/main.ts",
    "test": "node --import tsx --test test/*.test.ts"
  },
  "devDependencies": {
    "@types/node": "^22.10.0",
    "tsx": "^4.19.0",
    "typescript": "^5.6.0"
  }
}
```

- [ ] **Step 3: Create `tsconfig.json`**

`adapters/three-vrm/tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true,
    "outDir": "dist",
    "declaration": true,
    "sourceMap": true,
    "rootDir": "src"
  },
  "include": ["src"]
}
```

- [ ] **Step 4: Create `adapters/three-vrm/.gitignore`** (project-local, in addition to repo root)

```
node_modules/
dist/
*.tgz
package-lock.json.bak
```

- [ ] **Step 5: Create `adapters/three-vrm/README.md`**

````markdown
# three-vrm renderer adapter

TypeScript renderer adapter for the [VRM conformance suite](https://github.com/arkavo-org/vrm-conformance). Speaks the [operation contract](../../docs/operation-contract.md) over stdio JSON-RPC, satisfying the same contract as the Rust mock and the Swift VRMMetalKit scaffold.

## Status

| Phase | Scope | State |
|---|---|---|
| 2C-a | TS scaffold + JSON-RPC framing + Unimplemented dispatch | (this commit) |
| 2C-b | three-vrm + Playwright headless WebGL2 + real Phase 1 ops | not started |

Until Phase 2C-b lands, every Phase 1 op returns `Unimplemented` with `data.phase = "v1.x"`. Reserved Phase 2+ ops return `Unimplemented` with the appropriate phase label.

## Build

```bash
cd adapters/three-vrm
npm install
npm run build
node dist/main.js
```

Or, for the inner-loop dev experience without a build step:

```bash
npm start  # runs src/main.ts via tsx
```

## Run via the conformance runner

```bash
cargo run -p vrm-runner -- execute-test-plan \
  --plan ../../assets/generated/smoke_default.test.yaml \
  --adapter-bin "$(npm bin)/tsx" \
  --adapter-args "$(pwd)/src/main.ts" \
  --asset-dir ../../assets/generated \
  --output-dir /tmp/three-vrm-out \
  --renderer-name three-vrm \
  --json
```

Phase 1 ops will return `LoadFailed` or `Unimplemented` until 2C-b lands; the runner exits with the structured error in JSON form.

## Tests

```bash
npm test
```

Two test files:
- `test/framing.test.ts` — Content-Length round-trip primitives.
- `test/contract.test.ts` — spawn `dist/main.js` as a subprocess, exchange framed JSON-RPC, assert error envelopes.

## Phase 1 ops (must implement in 2C-b)

`load_vrm`, `set_camera`, `set_lighting`, `set_post_processing`, `render`, `dispose`.

## Reserved (Unimplemented in v0.1, v1.x, Phase 2, Phase 3)

`set_environment` (v1.x), `set_humanoid_pose` (Phase 2), `set_root_transform` (Phase 2), `animate_root_transform` (Phase 2), `step_physics` (Phase 2), `reset_physics` (Phase 2), `set_expression` (Phase 3).
````

- [ ] **Step 6: Install deps + verify TypeScript compiles (empty src will fail; use `--noEmit` to verify config)**

```bash
cd adapters/three-vrm
npm install
npx tsc --noEmit
```

The `--noEmit` will succeed even with no source files yet, validating the tsconfig.

- [ ] **Step 7: Commit**

```bash
git add .gitignore adapters/three-vrm/
git commit -m "chore(three-vrm): TypeScript project scaffold + Node 20+ harness"
```

---

## Section B — Framing primitives

### Task B1: LSP-style Content-Length framing (TDD)

**Files:**
- Create: `adapters/three-vrm/src/framing.ts`
- Create: `adapters/three-vrm/test/framing.test.ts`

Mirrors `vrm-ops::stdio::{read_message, write_message}` and the Swift adapter's `Framing.swift`. Reads `Content-Length: N\r\n\r\n` headers + N body bytes from a `Readable`; writes the same shape to a `Writable`. Header parsing is case-insensitive and tolerates additional headers (e.g., `X-Trace-Id`).

- [ ] **Step 1: Write the failing test**

`adapters/three-vrm/test/framing.test.ts`:

```ts
import { test } from "node:test";
import assert from "node:assert/strict";
import { Readable, Writable } from "node:stream";
import { readMessage, writeMessage } from "../src/framing.ts";

function bufferStream(): { stream: Writable; chunks: Buffer[] } {
  const chunks: Buffer[] = [];
  const stream = new Writable({
    write(chunk, _enc, cb) {
      chunks.push(Buffer.from(chunk));
      cb();
    },
  });
  return { stream, chunks };
}

function readableFromBuffer(buf: Buffer): Readable {
  return Readable.from([buf], { objectMode: false });
}

test("writeMessage emits Content-Length + CRLF + body", async () => {
  const { stream, chunks } = bufferStream();
  await writeMessage(stream, Buffer.from('{"ok":true}'));
  const out = Buffer.concat(chunks).toString("utf8");
  assert.equal(out, "Content-Length: 11\r\n\r\n{\"ok\":true}");
});

test("readMessage round-trips through write+read", async () => {
  const payload = Buffer.from(
    '{"jsonrpc":"2.0","id":1,"method":"ping","params":{}}',
  );

  const { stream, chunks } = bufferStream();
  await writeMessage(stream, payload);
  const wireBytes = Buffer.concat(chunks);

  const body = await readMessage(readableFromBuffer(wireBytes));
  assert.deepEqual(body, payload);
});

test("readMessage tolerates case-insensitive Content-Length and extra headers", async () => {
  const wire = Buffer.from(
    'content-length: 5\r\nX-Trace-Id: abc\r\n\r\nhello',
  );
  const body = await readMessage(readableFromBuffer(wire));
  assert.equal(body.toString("utf8"), "hello");
});

test("readMessage rejects missing Content-Length", async () => {
  const wire = Buffer.from("\r\n\r\n{}");
  await assert.rejects(readMessage(readableFromBuffer(wire)), /content-length/i);
});

test("writeMessage handles empty body", async () => {
  const { stream, chunks } = bufferStream();
  await writeMessage(stream, Buffer.alloc(0));
  const out = Buffer.concat(chunks).toString("utf8");
  assert.equal(out, "Content-Length: 0\r\n\r\n");
});
```

- [ ] **Step 2: Run failing test**

```bash
cd adapters/three-vrm
npm test
```

Expected: TypeScript can't resolve `../src/framing.ts`. Test errors out.

- [ ] **Step 3: Implement the framing module**

`adapters/three-vrm/src/framing.ts`:

```ts
//! LSP-style Content-Length framing over Node Readable/Writable streams.
//! Same wire format as `vrm-ops::stdio` (Rust) and Swift's `Framing.swift`.

import type { Readable, Writable } from "node:stream";

const CRLF = "\r\n";
const HEADER_TERMINATOR = Buffer.from(CRLF + CRLF, "utf8");

export class FrameError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "FrameError";
  }
}

/**
 * Write a JSON-RPC body to the wire, prefixed by `Content-Length: N\r\n\r\n`.
 * Resolves once the body has been flushed to the writable.
 */
export function writeMessage(stream: Writable, body: Buffer): Promise<void> {
  const header = Buffer.from(`Content-Length: ${body.length}${CRLF}${CRLF}`, "utf8");
  return new Promise((resolve, reject) => {
    stream.write(header, (err) => {
      if (err) {
        reject(err);
        return;
      }
      stream.write(body, (err2) => {
        if (err2) reject(err2);
        else resolve();
      });
    });
  });
}

/**
 * Read one framed message from `stream`. Headers are parsed case-insensitively;
 * additional headers (e.g., X-Trace-Id) are accepted and ignored. Throws
 * `FrameError` if Content-Length is missing or the stream ends mid-body.
 */
export async function readMessage(stream: Readable): Promise<Buffer> {
  const reader = new BufferedReader(stream);
  const headerBytes = await reader.readUntil(HEADER_TERMINATOR);
  const headerText = headerBytes
    .subarray(0, headerBytes.length - HEADER_TERMINATOR.length)
    .toString("utf8");

  let contentLength: number | undefined;
  for (const line of headerText.split(CRLF)) {
    if (line.length === 0) continue;
    const colon = line.indexOf(":");
    if (colon < 0) {
      throw new FrameError(`invalid header line: ${line}`);
    }
    const key = line.slice(0, colon).trim().toLowerCase();
    const value = line.slice(colon + 1).trim();
    if (key === "content-length") {
      const parsed = Number.parseInt(value, 10);
      if (!Number.isFinite(parsed) || parsed < 0) {
        throw new FrameError(`invalid Content-Length: ${value}`);
      }
      contentLength = parsed;
    }
  }
  if (contentLength === undefined) {
    throw new FrameError("missing Content-Length header");
  }
  return reader.readExact(contentLength);
}

/**
 * Pull-based byte reader over a Node Readable. Buffers data internally so
 * callers can ask for delimiter-bounded or fixed-length chunks regardless of
 * how the underlying stream chunks bytes.
 */
class BufferedReader {
  private buffer = Buffer.alloc(0);
  private waiters: Array<() => void> = [];
  private ended = false;
  private error: Error | null = null;

  constructor(stream: Readable) {
    stream.on("data", (chunk: Buffer | string) => {
      const buf = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
      this.buffer = Buffer.concat([this.buffer, buf]);
      this.notifyWaiters();
    });
    stream.on("end", () => {
      this.ended = true;
      this.notifyWaiters();
    });
    stream.on("error", (e) => {
      this.error = e;
      this.notifyWaiters();
    });
  }

  private notifyWaiters() {
    const waiters = this.waiters.splice(0);
    for (const w of waiters) w();
  }

  private waitForData(): Promise<void> {
    return new Promise((resolve) => {
      this.waiters.push(resolve);
    });
  }

  async readUntil(delimiter: Buffer): Promise<Buffer> {
    while (true) {
      if (this.error) throw this.error;
      const idx = this.buffer.indexOf(delimiter);
      if (idx >= 0) {
        const out = this.buffer.subarray(0, idx + delimiter.length);
        this.buffer = this.buffer.subarray(idx + delimiter.length);
        return out;
      }
      if (this.ended) {
        throw new FrameError("stream ended before delimiter found");
      }
      await this.waitForData();
    }
  }

  async readExact(length: number): Promise<Buffer> {
    while (this.buffer.length < length) {
      if (this.error) throw this.error;
      if (this.ended) {
        throw new FrameError(
          `stream ended before reading ${length} bytes (got ${this.buffer.length})`,
        );
      }
      await this.waitForData();
    }
    const out = this.buffer.subarray(0, length);
    this.buffer = this.buffer.subarray(length);
    return Buffer.from(out);
  }
}
```

- [ ] **Step 4: Run tests**

```bash
cd adapters/three-vrm
npm test
```

Expected: 5 tests pass.

- [ ] **Step 5: Commit**

```bash
git add adapters/three-vrm/src/framing.ts adapters/three-vrm/test/framing.test.ts
git commit -m "feat(three-vrm): LSP-style JSON-RPC framing primitives"
```

---

## Section C — Operation dispatch + server

### Task C1: Operation phase table + dispatch returning Unimplemented

**Files:**
- Create: `adapters/three-vrm/src/operations.ts`

Single source of truth for which methods exist and which phase each belongs to. Until 2C-b, every method (Phase 1 + reserved) returns `Unimplemented` — the difference is only the phase label.

- [ ] **Step 1: Implement**

`adapters/three-vrm/src/operations.ts`:

```ts
//! Operation registry + dispatch. Until Phase 2C-b lands, every op returns
//! Unimplemented with the appropriate phase label. Phase 1 ops report
//! `v1.x` because they're scheduled to be implemented in 2C-b (the
//! still-unbuilt next phase). Reserved Phase 2+ ops report their target
//! phase.

export interface RpcError {
  code: number;
  message: string;
  data?: unknown;
}

const PHASE_BY_METHOD: Record<string, string> = {
  // Phase 1 ops — to be implemented in 2C-b
  load_vrm: "v1.x",
  set_camera: "v1.x",
  set_lighting: "v1.x",
  set_post_processing: "v1.x",
  render: "v1.x",
  dispose: "v1.x",

  // Reserved
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

/**
 * Dispatch one method invocation. In Phase 2C-a all known methods return
 * Unimplemented; the dispatch table exists to ensure unknown methods get
 * `-32601` while known-but-deferred methods get `-32000`.
 */
export function dispatch(method: string, _params: unknown): DispatchOutcome {
  const phase = PHASE_BY_METHOD[method];
  if (phase === undefined) {
    return {
      ok: false,
      error: {
        code: -32601,
        message: `method not found: ${method}`,
      },
    };
  }
  return {
    ok: false,
    error: {
      code: -32000,
      message: `${method}: not implemented in this adapter version`,
      data: { phase },
    },
  };
}

export function knownMethods(): string[] {
  return Object.keys(PHASE_BY_METHOD);
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
git add adapters/three-vrm/src/operations.ts
git commit -m "feat(three-vrm): operation phase table + dispatch (all Unimplemented for now)"
```

---

### Task C2: Server read-loop

**Files:**
- Create: `adapters/three-vrm/src/server.ts`
- Create: `adapters/three-vrm/src/main.ts`

Read framed requests, parse JSON, dispatch by method, write framed responses. Malformed JSON → `-32700` parse-error response with `id: null`. EOF on stdin → resolve cleanly (clean exit).

- [ ] **Step 1: Implement `server.ts`**

`adapters/three-vrm/src/server.ts`:

```ts
//! Stdio JSON-RPC server. One request → one response. EOF on input stream
//! ends the loop cleanly.

import type { Readable, Writable } from "node:stream";
import { readMessage, writeMessage, FrameError } from "./framing.js";
import { dispatch } from "./operations.js";

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

export async function run(input: Readable, output: Writable): Promise<void> {
  while (true) {
    let body: Buffer;
    try {
      body = await readMessage(input);
    } catch (e) {
      // EOF or framing error → exit cleanly. We treat any read failure here
      // as "the runner closed the pipe"; partial-frame errors are surfaced
      // via console.error for diagnostics but don't keep the loop alive.
      if (e instanceof FrameError && /content-length/i.test(e.message)) {
        // Likely just EOF on a clean shutdown.
      } else {
        console.error(`three-vrm: read failed: ${(e as Error).message}`);
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
    const outcome = dispatch(req.method, req.params);

    const resp: JsonRpcResponse = outcome.ok
      ? { jsonrpc: "2.0", id, result: outcome.result }
      : { jsonrpc: "2.0", id, error: outcome.error };

    await writeMessage(output, Buffer.from(JSON.stringify(resp), "utf8"));
  }
}
```

- [ ] **Step 2: Implement `main.ts`**

`adapters/three-vrm/src/main.ts`:

```ts
//! Entry point: wire stdin/stdout into the JSON-RPC server loop.

import { run } from "./server.js";

process.stderr.write("three-vrm adapter starting\n");

run(process.stdin, process.stdout).catch((err) => {
  process.stderr.write(`three-vrm adapter fatal: ${err}\n`);
  process.exit(1);
});
```

- [ ] **Step 3: Build**

```bash
cd adapters/three-vrm
npm run build
ls dist/
```

Expected: `dist/main.js`, `dist/server.js`, `dist/operations.js`, `dist/framing.js`, plus `.d.ts` and `.js.map` for each.

- [ ] **Step 4: Smoke run (manual)**

```bash
cd adapters/three-vrm
# In one terminal:
echo -ne 'Content-Length: 80\r\n\r\n{"jsonrpc":"2.0","id":1,"method":"unknown_op","params":{}}            ' | node dist/main.js
```

(The trailing spaces pad the body to 80 bytes — anything that decodes as JSON works.) Expected: stderr shows `three-vrm adapter starting`, stdout emits a Content-Length-framed response with `error.code = -32601`. The exact framing on stdout is `Content-Length: NN\r\n\r\n{...}`.

This manual check is nice-to-have; the contract tests in D1 cover it more rigorously.

- [ ] **Step 5: Commit**

```bash
git add adapters/three-vrm/src/server.ts adapters/three-vrm/src/main.ts
git commit -m "feat(three-vrm): JSON-RPC stdio server + main entrypoint"
```

---

## Section D — Subprocess contract test

### Task D1: Contract test mirroring the Rust mock's pattern

**Files:**
- Create: `adapters/three-vrm/test/contract.test.ts`

Spawns the built binary as a subprocess, exchanges framed JSON-RPC, asserts error envelopes for: unknown method (`-32601`), Phase 1 op (`-32000` + phase `v1.x`), reserved Phase 2 op (`-32000` + phase `Phase 2`), parse error (`-32700`), clean exit on stdin close.

- [ ] **Step 1: Write the test**

`adapters/three-vrm/test/contract.test.ts`:

```ts
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
): Promise<{ result?: unknown; error?: { code: number; message: string; data?: unknown } }> {
  const reqBody = Buffer.from(
    JSON.stringify({ jsonrpc: "2.0", id, method, params }),
    "utf8",
  );
  await writeMessage(h.stdin as any, reqBody);
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

test("phase 1 op (load_vrm) returns -32000 with phase v1.x", async () => {
  const h = spawnAdapter();
  try {
    const resp = await rpc(h, 2, "load_vrm", { path: "/tmp/whatever.vrm" });
    assert.equal(resp.error?.code, -32000);
    assert.equal((resp.error?.data as { phase?: string } | undefined)?.phase, "v1.x");
  } finally {
    h.stdin.end();
    await new Promise((r) => h.child.on("exit", r));
  }
});

test("reserved phase-2 op (step_physics) returns phase Phase 2", async () => {
  const h = spawnAdapter();
  try {
    const resp = await rpc(h, 3, "step_physics", { dt_seconds: 0.016, count: 1 });
    assert.equal(resp.error?.code, -32000);
    assert.equal((resp.error?.data as { phase?: string } | undefined)?.phase, "Phase 2");
  } finally {
    h.stdin.end();
    await new Promise((r) => h.child.on("exit", r));
  }
});

test("reserved phase-3 op (set_expression) returns phase Phase 3", async () => {
  const h = spawnAdapter();
  try {
    const resp = await rpc(h, 4, "set_expression", { name: "happy", weight: 1.0 });
    assert.equal(resp.error?.code, -32000);
    assert.equal((resp.error?.data as { phase?: string } | undefined)?.phase, "Phase 3");
  } finally {
    h.stdin.end();
    await new Promise((r) => h.child.on("exit", r));
  }
});

test("malformed JSON returns -32700 parse error with id null", async () => {
  const h = spawnAdapter();
  try {
    const garbage = Buffer.from("not json at all }}}", "utf8");
    await writeMessage(h.stdin as any, garbage);
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
    const r1 = await rpc(h, 1, "load_vrm", {});
    const r2 = await rpc(h, 2, "render", {});
    assert.equal(r1.error?.code, -32000);
    assert.equal(r2.error?.code, -32000);
  } finally {
    h.stdin.end();
    await new Promise((r) => h.child.on("exit", r));
  }
});
```

- [ ] **Step 2: Run tests**

```bash
cd adapters/three-vrm
npm run build
npm test
```

Expected: 5 framing tests + 6 contract tests = 11 pass. The contract tests depend on `dist/main.js` existing, hence the explicit `npm run build` first.

- [ ] **Step 3: Commit**

```bash
git add adapters/three-vrm/test/contract.test.ts
git commit -m "test(three-vrm): subprocess contract test (unknown/Phase 1/reserved/parse)"
```

---

## Section E — CI + docs

### Task E1: GitHub Actions workflow for the TS adapter

**Files:**
- Create: `.github/workflows/three-vrm.yml`

Mirrors the rust/swift workflow shapes. Path-filter triggers; ubuntu-latest; setup-node@v4 with Node 20; `npm ci`, `npm run build`, `npm test`.

- [ ] **Step 1: Implement**

```yaml
name: three-vrm

on:
  pull_request:
    paths:
      - 'adapters/three-vrm/**'
      - '.github/workflows/three-vrm.yml'
  push:
    branches: [main]
    paths:
      - 'adapters/three-vrm/**'
      - '.github/workflows/three-vrm.yml'

jobs:
  build:
    runs-on: ubuntu-latest
    defaults:
      run:
        working-directory: adapters/three-vrm
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: '20'
      - run: npm ci
      - run: npm run build
      - run: npm test
```

- [ ] **Step 2: Commit**

```bash
git add .github/workflows/three-vrm.yml
git commit -m "ci: three-vrm workflow (Node 20, npm ci/build/test)"
```

---

### Task E2: Document the new adapter

**Files:**
- Modify: `docs/operation-contract.md`

Add `three-vrm` to the "Reference implementations" section (added in Phase 2B) so the catalog reflects all three adapters.

- [ ] **Step 1: Update operation-contract.md**

Find the Reference implementations section, currently:

```markdown
## Reference implementations

- **`vrm-mock-renderer`** (in-tree, Rust). A deterministic CPU adapter that satisfies the Phase 1 op contract. Renders are a stable function of `MToonParams` — identical params produce byte-identical PNGs, so self-diff is SSIM 1.0 by construction. Used as the default smoke-test adapter; not a real renderer.
- **`adapters/vrm-metal-kit/`** (in-tree, Swift). Real macOS / Metal renderer scaffold. JSON-RPC framing is implemented; the actual VRMMetalKit integration (L3) is deferred.
```

Replace with:

```markdown
## Reference implementations

- **`vrm-mock-renderer`** (in-tree, Rust). A deterministic CPU adapter that satisfies the Phase 1 op contract. Renders are a stable function of `MToonParams` — identical params produce byte-identical PNGs, so self-diff is SSIM 1.0 by construction. Used as the default smoke-test adapter; not a real renderer.
- **`adapters/vrm-metal-kit/`** (in-tree, Swift). Real macOS / Metal renderer scaffold. JSON-RPC framing is implemented; the actual VRMMetalKit integration (L3) is deferred.
- **`adapters/three-vrm/`** (in-tree, TypeScript). Node-based renderer scaffold for the [pixiv/three-vrm](https://github.com/pixiv/three-vrm) library. JSON-RPC framing is implemented (Phase 2C-a); real headless-WebGL2 rendering via Playwright lands in Phase 2C-b. All ops currently return `Unimplemented`.
```

- [ ] **Step 2: Commit**

```bash
git add docs/operation-contract.md
git commit -m "docs: list three-vrm adapter in Reference implementations"
```

---

## Self-Review

**Spec coverage:**

| 2C-a goal | Task |
|---|---|
| TS project scaffold | A1 |
| LSP-style JSON-RPC framing | B1 |
| Operation phase table + dispatch | C1 |
| Server read-loop + entrypoint | C2 |
| Subprocess contract test | D1 |
| CI workflow | E1 |
| Documentation | E2 |

**Placeholder scan:** none. Every code block is complete; tests assert behavior, not just structure.

**Type consistency:**

- `RpcError`, `DispatchOutcome`, `DispatchSuccess`, `DispatchFailure` defined in C1; consumed in C2's `server.run`.
- `readMessage` / `writeMessage` signatures (`Readable`/`Writable` arguments, `Buffer` body) consistent across B1 and D1.
- Phase labels (`v1.x`, `Phase 2`, `Phase 3`) match the Swift adapter's labels and the operation-contract doc.

**YAGNI guards:**

- ✅ No `three`, `@pixiv/three-vrm`, or `playwright` deps yet — those are 2C-b's introduction.
- ✅ No `describe`-style operation catalog endpoint on the adapter (runner exposes that, not adapter).
- ✅ No file output / no PNG generation — every op returns Unimplemented.
- ✅ No bundling step beyond `tsc`; the runtime entrypoint is `dist/main.js` for distribution and `tsx src/main.ts` for dev.

**Risk register:**

- **Node version drift.** Pin `engines.node >= 20`; CI uses Node 20 explicitly. Local devs on older Node will see clear errors from the engines field.
- **`tsx` for tests.** Using `node --import tsx --test test/*.test.ts` runs TS directly without a build step. Worst case, if `tsx` ever breaks, fall back to `npm run build && node --test dist/test/*.test.js`. The contract test always builds first because it spawns `dist/main.js`.
- **Stream backpressure.** The framing implementation in B1 doesn't honor backpressure on `writeMessage`. For the small request/response sizes here it doesn't matter; if 2C-b ever streams larger payloads (it shouldn't — we use file paths) revisit.

---

## Execution Handoff

Plan saved to `docs/superpowers/plans/2026-05-10-phase2c-three-vrm-adapter-scaffold.md`. Two execution options:

1. **Subagent-Driven (recommended)** — fresh subagent per task, two-stage review per task. 7 tasks; mostly sequential (B1 must precede C2 and D1; A1 must come first).
2. **Inline Execution** — execute tasks in this session via `superpowers:executing-plans`.

For inline, expect ~20 minutes given the size and the established Swift/Rust adapter pattern as a reference.
