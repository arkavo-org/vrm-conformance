//! Spawns a renderer adapter subprocess, sends JSON-RPC requests over stdin,
//! reads framed responses from stdout. The adapter binary path is given by
//! the test plan or runner CLI arg; the protocol is `vrm-ops`.

use camino::Utf8PathBuf;
use serde::{de::DeserializeOwned, Serialize};
use std::io::{BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use vrm_ops::{
    stdio::{read_message, write_message},
    JsonRpcRequest, JsonRpcResponse, RpcError,
};

#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("frame: {0}")]
    Frame(#[from] vrm_ops::stdio::FrameError),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("rpc error: {0}")]
    Rpc(#[from] RpcError),
}

pub struct Adapter {
    child: Child,
    // `Option` so we can `take()` and drop stdin on shutdown despite `Drop`
    // forbidding moves out of fields. `None` means stdin has been closed.
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl Adapter {
    pub fn spawn(adapter_bin: &Utf8PathBuf, args: &[String]) -> Result<Self, AdapterError> {
        let mut child = Command::new(adapter_bin.as_std_path())
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit()) // adapter logs go to operator's stderr
            .spawn()?;

        let stdin = child.stdin.take().expect("stdin piped");
        let stdout = BufReader::new(child.stdout.take().expect("stdout piped"));
        Ok(Adapter {
            child,
            stdin: Some(stdin),
            stdout,
            next_id: 1,
        })
    }

    pub fn call<P: Serialize, R: DeserializeOwned>(
        &mut self,
        method: &str,
        params: P,
    ) -> Result<R, AdapterError> {
        let id = self.next_id;
        self.next_id += 1;

        let req = JsonRpcRequest::new(id, method, params);
        let body = serde_json::to_vec(&req)?;
        let stdin = self.stdin.as_mut().ok_or_else(|| {
            AdapterError::Io(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "adapter stdin already closed",
            ))
        })?;
        write_message(stdin, &body)?;
        stdin.flush()?;

        let resp_bytes = read_message(&mut self.stdout)?;
        let resp: JsonRpcResponse<R> = serde_json::from_slice(&resp_bytes)?;
        Ok(resp.into_result()?)
    }

    pub fn shutdown(mut self) -> Result<(), AdapterError> {
        // Closing stdin signals adapters to exit gracefully.
        // We give the adapter a bounded grace period (5 seconds) to exit on its
        // own; if it ignores stdin EOF and hangs, we kill it so CI can never
        // deadlock here. The exact bound is an implementation detail — tests
        // must not depend on this timing.
        self.stdin.take();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            match self.child.try_wait()? {
                Some(_status) => return Ok(()),
                None => {
                    if std::time::Instant::now() >= deadline {
                        // Adapter is hung; kill and reap. Ignore errors —
                        // best-effort cleanup, child may exit between calls.
                        let _ = self.child.kill();
                        let _ = self.child.wait();
                        return Ok(());
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
            }
        }
    }
}

impl Drop for Adapter {
    fn drop(&mut self) {
        // Best-effort subprocess cleanup. Ensures that if `execute_plan` (or
        // any caller) bails via `?` between `spawn` and an explicit
        // `shutdown()`, the renderer subprocess is killed and reaped instead
        // of being orphaned/zombied.
        //
        // When `shutdown()` ran in the happy path, the child has already
        // exited; `kill()` will then return Err, which we deliberately
        // swallow. The same applies to `wait()` if the child is already
        // reaped.
        //
        // Drop stdin first (if still held) so the child sees EOF; then kill
        // unconditionally as a safety net. `kill()` on an already-exited
        // child returns Err on some platforms — we deliberately swallow it.
        // `wait()` reaps the process so we don't leave a zombie.
        self.stdin.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
