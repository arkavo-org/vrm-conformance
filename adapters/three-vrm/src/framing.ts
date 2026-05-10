// LSP-style Content-Length framing over Node Readable/Writable streams.
// Same wire format as `vrm-ops::stdio` (Rust) and Swift's `Framing.swift`.

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
  const header = Buffer.from(
    `Content-Length: ${body.length}${CRLF}${CRLF}`,
    "utf8",
  );
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
