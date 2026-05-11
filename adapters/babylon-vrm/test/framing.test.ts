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
