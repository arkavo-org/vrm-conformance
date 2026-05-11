// Stdio JSON-RPC server. One request → one response. EOF on the input
// stream ends the loop cleanly. Wire format matches the project's framing
// contract (LSP-style `Content-Length:`), identical to the three-vrm and
// vrm-metal-kit adapters so the same runner code drives all three.

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
          `babylon-vrm: read failed: ${(e as Error).message}\n`,
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
