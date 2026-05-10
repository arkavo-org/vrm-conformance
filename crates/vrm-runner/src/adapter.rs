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
    stdin: ChildStdin,
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
            stdin,
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
        write_message(&mut self.stdin, &body)?;
        self.stdin.flush()?;

        let resp_bytes = read_message(&mut self.stdout)?;
        let resp: JsonRpcResponse<R> = serde_json::from_slice(&resp_bytes)?;
        Ok(resp.into_result()?)
    }

    pub fn shutdown(mut self) -> Result<(), AdapterError> {
        // Closing stdin signals adapters to exit gracefully.
        drop(self.stdin);
        let _ = self.child.wait()?;
        Ok(())
    }
}
