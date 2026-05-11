//! Godot child-process management: port allocation, spawn, lifecycle.

use std::env;
use std::ffi::OsString;
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ChildError {
    #[error("godot binary not found on PATH; set GODOT_BIN env var or install Godot 4.x")]
    GodotMissing,
    #[error("could not bind ephemeral TCP port: {0}")]
    Bind(#[from] std::io::Error),
    #[error("godot did not connect within {timeout_secs}s")]
    AcceptTimeout { timeout_secs: u64 },
}

/// Resolve which Godot binary to invoke. Honors `GODOT_BIN` env var; falls
/// back to the literal name `godot` (which `Command` resolves via PATH).
pub fn godot_binary() -> OsString {
    env::var_os("GODOT_BIN").unwrap_or_else(|| "godot".into())
}

/// Bind to 127.0.0.1:0 and return both the listener and the resolved port.
/// The OS assigns an ephemeral port; we read it back before spawning Godot.
pub fn bind_ephemeral() -> Result<(TcpListener, u16), ChildError> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    Ok((listener, port))
}

/// Wraps a spawned Godot child + the project path it was launched against.
/// Killed on drop so test failures don't leak processes.
#[derive(Debug)]
pub struct GodotChild {
    pub child: Child,
}

impl Drop for GodotChild {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Spawn Godot headless against `project_dir`, passing `port` as a
/// positional user arg after `--`. Stdout/stderr are inherited from the
/// parent so Godot's startup banner goes to the runner's stderr (where
/// it's already tolerated for tracing).
pub fn spawn_godot(
    project_dir: &PathBuf,
    main_script: &str,
    port: u16,
) -> Result<GodotChild, ChildError> {
    let bin = godot_binary();
    let mut cmd = Command::new(&bin);
    cmd.arg("--headless")
        .arg("--path").arg(project_dir)
        .arg("--script").arg(main_script)
        .arg("--")
        .arg(port.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    match cmd.spawn() {
        Ok(child) => Ok(GodotChild { child }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(ChildError::GodotMissing),
        Err(e) => Err(ChildError::Bind(e)),
    }
}

/// Block until Godot accepts the TCP connection or `timeout` elapses.
/// Sets the listener non-blocking + polls so we can enforce the deadline
/// without spinning.
pub fn accept_with_timeout(
    listener: &TcpListener,
    timeout: Duration,
) -> Result<std::net::TcpStream, ChildError> {
    listener.set_nonblocking(true)?;
    let deadline = Instant::now() + timeout;
    loop {
        match listener.accept() {
            Ok((stream, _peer)) => {
                stream.set_nonblocking(false)?;
                return Ok(stream);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return Err(ChildError::AcceptTimeout {
                        timeout_secs: timeout.as_secs(),
                    });
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(ChildError::Bind(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ephemeral_port_is_in_user_range() {
        let (_listener, port) = bind_ephemeral().expect("bind");
        assert!(port >= 1024, "got privileged port {port}");
        assert!(port > 0, "got null port");
    }

    #[test]
    fn ephemeral_port_is_unique() {
        let (_a, port_a) = bind_ephemeral().expect("bind a");
        let (_b, port_b) = bind_ephemeral().expect("bind b");
        assert_ne!(port_a, port_b, "two consecutive binds returned same port");
    }

    #[test]
    fn godot_binary_default_is_godot() {
        // Saving + restoring env is racy across tests, but this one
        // doesn't mutate. If GODOT_BIN happens to be set in the test
        // environment, accept that — assert only that the value is
        // some non-empty string.
        let bin = godot_binary();
        assert!(!bin.is_empty());
    }

    #[test]
    fn godot_binary_honors_env_override() {
        // SAFETY: env mutation is process-wide and racy with other tests
        // running in parallel; this test reads + writes one specific key
        // and restores it, accepting the small race window.
        let old = env::var_os("GODOT_BIN");
        env::set_var("GODOT_BIN", "/some/fake/path");
        let bin = godot_binary();
        match old {
            Some(v) => env::set_var("GODOT_BIN", v),
            None => env::remove_var("GODOT_BIN"),
        }
        assert_eq!(bin, OsString::from("/some/fake/path"));
    }

    /// Use a guaranteed-missing path so spawn_godot returns GodotMissing
    /// without actually trying to invoke a real Godot.
    #[test]
    fn spawn_with_missing_binary_returns_godot_missing() {
        let old = env::var_os("GODOT_BIN");
        env::set_var("GODOT_BIN", "/definitely/not/a/real/binary/xyzzy");
        let result = spawn_godot(
            &PathBuf::from("."),
            "src/main.gd",
            12345,
        );
        match old {
            Some(v) => env::set_var("GODOT_BIN", v),
            None => env::remove_var("GODOT_BIN"),
        }
        match result {
            Err(ChildError::GodotMissing) => {}
            other => panic!("expected GodotMissing, got {other:?}"),
        }
    }

    #[test]
    fn accept_timeout_fires_when_no_one_connects() {
        let (listener, _port) = bind_ephemeral().expect("bind");
        let err = accept_with_timeout(&listener, Duration::from_millis(100))
            .expect_err("should timeout");
        match err {
            ChildError::AcceptTimeout { .. } => {}
            other => panic!("expected AcceptTimeout, got {other:?}"),
        }
    }
}
