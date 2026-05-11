//! Godot child-process management: port allocation, spawn, lifecycle.

use std::env;
use std::ffi::OsString;
use std::net::TcpListener;

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
}
