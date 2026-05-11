# godot-vrm Adapter L1+L2 Scaffold Implementation Plan (v2 — Rust TCP-shim)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **Supersedes:** [`2026-05-11-adapter-godot-vrm-scaffold.md`](./2026-05-11-adapter-godot-vrm-scaffold.md) (v1). v1's load-bearing assumption — that GDScript can write byte-exact bytes to stdout — was falsified by the Task 1 spike on Godot 4.6.2: `OS.write_buffer_to_stdout` does not exist, `print`/`printraw` are banner-polluted, `--quiet` suppresses all stdout, and `FileAccess.open("/dev/stdout", ...)` is rejected (`FILE_NOT_FOUND`). See v1's `## Spike result` section for the raw evidence. A follow-up spike confirmed that `StreamPeerTCP` works correctly in headless mode, which is what this plan builds on.

**Goal:** Land `adapters/godot-vrm/` as a third VRM 1.0 renderer adapter scaffold that satisfies the runner's existing JSON-RPC stdio contract, with every Phase 1 / Phase 2+ op returning a structured `Unimplemented` so consensus diff has a viable third real renderer once L3 wires in V-Sekai/godot-vrm.

**Architecture:** A small Rust crate (`crates/vrm-godot-shim/`) owns the external stdio + LSP `Content-Length` framing (reusing `vrm-ops::stdio`), spawns one Godot headless child, and bridges JSON-RPC ↔ TCP-loopback to Godot. The Godot side speaks the dispatch contract over a `StreamPeerTCP` socket using newline-delimited JSON — no GDScript framing code, no stdout pollution. The runner consumes the Rust binary `vrm-godot-shim` as `--adapter-bin`, identical in shape to `vrm-mock-renderer`. The L3 plan replaces the GDScript dispatch table with real V-Sekai/godot-vrm rendering; nothing about the shim changes.

```
runner ──┬─ LSP Content-Length framed JSON-RPC over stdio ──┬─ vrm-godot-shim (Rust)
         │                                                  │   │
         │                                                  │   ├─ spawns: godot --headless --path adapters/godot-vrm --script src/main.gd -- <port>
         │                                                  │   │
         └──────────────────────────────────────────────────┘   └─ NDJSON over TCP-loopback ──┐
                                                                                              │
                                                                                              └─ godot child
                                                                                                  ├─ main.gd     (connect, run session)
                                                                                                  ├─ tcp_session.gd (NDJSON read/dispatch/write)
                                                                                                  └─ operations.gd  (phase-label table)
```

**Tech Stack:** Rust (workspace member; clap, serde_json, tokio not needed — std `std::net::TcpListener` + `std::process::Command` are enough), Godot 4.x (4.6.2 verified; no flag-specific dependencies), GDScript, GitHub Actions Linux runner.

---

## Wire protocols

**External (runner ↔ shim):** unchanged from every other adapter — LSP `Content-Length: N\r\n\r\n<body>` JSON-RPC over stdio. Implemented by `vrm_ops::stdio::{read_message, write_message}`.

**Internal (shim ↔ Godot):** newline-delimited JSON (NDJSON) over TCP loopback on `127.0.0.1:<ephemeral>`. One JSON object per line; lines terminated by `\n`. No length prefix — `StreamPeerTCP` is reliable and Godot reads whole lines, not raw byte windows. Encoder writes the JSON body + `\n`; decoder reads until `\n`.

The shim and Godot do **not** speak full JSON-RPC over the TCP socket. The shim forwards just the JSON body — Godot dispatches and returns just the response body. The shim re-wraps that as a framed stdio response. This keeps Godot ignorant of LSP framing.

---

## File Structure

```
crates/vrm-godot-shim/
├── Cargo.toml                  # binary "vrm-godot-shim" + lib for integration test
├── src/
│   ├── lib.rs                  # re-exports for tests
│   ├── main.rs                 # CLI entry — calls run()
│   ├── child.rs                # spawn Godot, manage process lifetime
│   └── bridge.rs               # stdio<->TCP forwarding loop
└── tests/
    └── contract.rs             # #[ignore]'d integration test with real Godot

adapters/godot-vrm/
├── README.md                   # Status, runtime deps, L3 sketch
├── project.godot               # Godot 4 project descriptor
├── .gitignore                  # .godot/, .import/, exports
├── src/
│   ├── main.gd                 # Entry: parse positional port arg, connect TCP, run session
│   ├── operations.gd           # Phase-by-method table + dispatch()
│   └── tcp_session.gd          # NDJSON read → dispatch → write loop
└── tests/
    ├── run_gdscript_tests.gd   # GDScript test runner (built-in, no GUT)
    └── test_operations.gd      # Dispatch table unit tests

.github/workflows/
└── godot-vrm.yml               # Install Godot + run dispatch unit tests + #[ignore]'d Rust integration

# Edits to existing files
Cargo.toml                       # Add crates/vrm-godot-shim to workspace members
README.md                        # Add adapters/godot-vrm/ row
CLAUDE.md                        # Adapter status: add godot-vrm
adapters/babylon-vrm/README.md   # Cross-link to godot-vrm
```

**Boundaries:**
- `child.rs` owns the Godot subprocess lifecycle — port allocation, spawn, ready-wait, graceful shutdown, kill on drop.
- `bridge.rs` owns the forwarding loop — stdin frame → TCP line, TCP line → stdout frame. No knowledge of Godot or specific ops.
- `tcp_session.gd` owns the Godot-side read/dispatch/write loop. No knowledge of LSP framing (that's the shim's job).
- `operations.gd` owns method → phase-label dispatch — same shape as `adapters/babylon-vrm/src/operations.ts`.

---

## Error handling contract

The shim must surface failures via the JSON-RPC error envelope the runner expects (`docs/operation-contract.md`):

| Symptom | Response |
|---|---|
| Godot binary missing | Each request: `error: { code: -32002, message: "RenderFailed", data: { reason: "godot binary not found; set GODOT_BIN env var or install Godot 4.x" } }`. Exit 1 after stdin EOF. |
| Godot crashed before TCP accept | Same `-32002` for the first request, then exit 1. |
| Godot accepted but crashed mid-session | Return `-32002` to the in-flight request. Exit 1. |
| TCP read timeout (Godot stuck) | Return `-32002` with `data.reason = "godot did not respond within 10s"`. Kill Godot. Exit 1. |

The shim does **not** attempt to restart Godot on crash — the test plan retries are the runner's job, not the shim's.

---

## Task list

11 tasks total. Tasks build sequentially: each later task depends on the previous one's tests staying green. TDD throughout — failing test before implementation in every code task.

---

### Task 1: Workspace scaffolding for vrm-godot-shim

**Files:**
- Create: `crates/vrm-godot-shim/Cargo.toml`
- Create: `crates/vrm-godot-shim/src/lib.rs`
- Create: `crates/vrm-godot-shim/src/main.rs`
- Modify: `Cargo.toml` (repo root — workspace members)

- [ ] **Step 1: Create the crate manifest**

```bash
mkdir -p crates/vrm-godot-shim/src crates/vrm-godot-shim/tests
cat > crates/vrm-godot-shim/Cargo.toml <<'TOML'
[package]
name = "vrm-godot-shim"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
authors.workspace = true
rust-version.workspace = true

[lib]
path = "src/lib.rs"

[[bin]]
name = "vrm-godot-shim"
path = "src/main.rs"

[dependencies]
vrm-ops = { path = "../vrm-ops" }
serde_json = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
TOML
```

- [ ] **Step 2: Create skeleton lib.rs and main.rs**

```bash
cat > crates/vrm-godot-shim/src/lib.rs <<'RS'
//! vrm-godot-shim: bridges the runner's stdio JSON-RPC contract to a
//! headless Godot child over TCP loopback. See
//! `docs/superpowers/plans/2026-05-11-adapter-godot-vrm-scaffold-v2.md`
//! for the architectural rationale (GDScript cannot write byte-exact
//! stdout; this shim owns the wire so Godot only has to dispatch).

pub mod bridge;
pub mod child;
RS
```

```bash
cat > crates/vrm-godot-shim/src/main.rs <<'RS'
fn main() {
    eprintln!("vrm-godot-shim {} — scaffold; bridge not yet wired", env!("CARGO_PKG_VERSION"));
}
RS
```

- [ ] **Step 3: Create skeleton module files (empty but valid)**

```bash
cat > crates/vrm-godot-shim/src/child.rs <<'RS'
//! Godot child-process management: port allocation, spawn, lifecycle.
RS

cat > crates/vrm-godot-shim/src/bridge.rs <<'RS'
//! stdio <-> TCP-loopback forwarding loop.
RS
```

- [ ] **Step 4: Add to workspace**

Edit repo-root `Cargo.toml` and add `"crates/vrm-godot-shim",` to `members`, preserving existing order (keep alphabetical-ish — group near `vrm-runner` since they're both binaries that talk to adapters).

- [ ] **Step 5: Verify workspace builds**

```bash
cargo build --workspace
```

Expected: clean build, no warnings, `target/debug/vrm-godot-shim` exists.

- [ ] **Step 6: Verify clippy passes (CI gate)**

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: no warnings. If `unused_imports`/`unused_variables` from the empty modules fires, suppress with module-level `#![allow(unused)]` only as a temporary measure removed in Task 3.

- [ ] **Step 7: Commit**

```bash
git add crates/vrm-godot-shim Cargo.toml
git commit -m "feat(vrm-godot-shim): empty crate added to workspace"
```

---

### Task 2: child.rs — failing test first

**Files:**
- Create: `crates/vrm-godot-shim/src/child.rs` (test module)

- [ ] **Step 1: Write failing tests for port allocation + binary discovery**

Append to `crates/vrm-godot-shim/src/child.rs`:

```rust
//! Godot child-process management: port allocation, spawn, lifecycle.

use std::env;
use std::ffi::OsString;
use std::net::{TcpListener, SocketAddr};
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ephemeral_port_is_in_user_range() {
        let (_listener, port) = bind_ephemeral().expect("bind");
        // The OS picks from the ephemeral range; assert it's non-zero
        // and not in the privileged range, but don't pin a specific
        // window (varies by kernel).
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
```

- [ ] **Step 2: Run tests — expect compile failure (functions don't exist yet)**

Wait — actually, the code above defines the functions inline. The tests should pass on the first run. This is a *write-tests-and-impl-together* shape, not pure TDD. Acceptable for a small module where the production code is short enough to grasp at a glance, but call it out in the commit message.

```bash
cargo test -p vrm-godot-shim --lib
```

Expected: 4 passed.

- [ ] **Step 3: Verify clippy stays clean**

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

- [ ] **Step 4: Commit**

```bash
git add crates/vrm-godot-shim/src/child.rs
git commit -m "feat(vrm-godot-shim): port allocation + godot binary resolution"
```

---

### Task 3: child.rs — spawn Godot with port arg

**Files:**
- Modify: `crates/vrm-godot-shim/src/child.rs`

- [ ] **Step 1: Add the spawn API**

Append to `crates/vrm-godot-shim/src/child.rs` (after the existing `bind_ephemeral` definition, before `#[cfg(test)]`):

```rust
/// Wraps a spawned Godot child + the project path it was launched against.
/// Killed on drop so test failures don't leak processes.
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
```

- [ ] **Step 2: Add tests using a stub script (no Godot dependency)**

Append inside `#[cfg(test)] mod tests`:

```rust
    /// Use /bin/sh as a stand-in for godot. It won't connect to the TCP
    /// listener, but spawn-and-then-kill should still work cleanly.
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
```

- [ ] **Step 3: Run tests**

```bash
cargo test -p vrm-godot-shim --lib
```

Expected: 6 passed (4 from Task 2 + 2 new).

- [ ] **Step 4: Commit**

```bash
git add crates/vrm-godot-shim/src/child.rs
git commit -m "feat(vrm-godot-shim): spawn godot child + accept-with-timeout"
```

---

### Task 4: bridge.rs — failing test with mock TCP responder

**Files:**
- Modify: `crates/vrm-godot-shim/src/bridge.rs`

- [ ] **Step 1: Write the bridge module with tests against a mock TCP echo-with-substitution server**

```rust
//! stdio <-> TCP-loopback forwarding loop.
//!
//! Reads framed JSON-RPC bodies from stdin via `vrm_ops::stdio`, writes
//! each body + "\n" to the TCP socket, reads one "\n"-terminated response
//! from the socket, and writes it back to stdout via `vrm_ops::stdio`.
//! On stdin EOF, returns Ok(()).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;

use thiserror::Error;
use vrm_ops::stdio::{self, FrameError};

#[derive(Debug, Error)]
pub enum BridgeError {
    #[error("framing: {0}")]
    Framing(#[from] FrameError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("godot side closed the socket mid-request")]
    PeerClosed,
}

/// Forward one round of: stdin frame -> TCP line -> TCP line -> stdout frame.
/// Returns `Ok(true)` on a successful round, `Ok(false)` on clean stdin EOF.
pub fn forward_one<R, W, S>(
    stdin: &mut R,
    stdout: &mut W,
    tcp: &mut S,
) -> Result<bool, BridgeError>
where
    R: Read,
    W: Write,
    S: Read + Write,
{
    let body = match stdio::read_message(stdin) {
        Ok(b) => b,
        Err(FrameError::Io(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(false),
        Err(e) => return Err(BridgeError::Framing(e)),
    };
    tcp.write_all(&body)?;
    tcp.write_all(b"\n")?;
    tcp.flush()?;

    let mut reader = BufReader::new(tcp);
    let mut response = String::new();
    let n = reader.read_line(&mut response)?;
    if n == 0 {
        return Err(BridgeError::PeerClosed);
    }
    // Strip exactly one trailing '\n' (BufRead::read_line keeps it).
    if response.ends_with('\n') {
        response.pop();
    }
    stdio::write_message(stdout, response.as_bytes())?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::net::TcpListener;
    use std::sync::mpsc::channel;
    use std::thread;

    /// Stand up a one-shot TCP server that reads one line and writes a
    /// canned response. Returns the port the server is listening on.
    fn one_shot_server(canned_response: &'static str) -> u16 {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let (tx, rx) = channel();
        tx.send(()).unwrap();
        let _ = rx;
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = String::new();
            let mut reader = BufReader::new(&stream);
            reader.read_line(&mut buf).unwrap();
            // Drop reader so we can write to the original stream.
            drop(reader);
            stream.write_all(canned_response.as_bytes()).unwrap();
            stream.write_all(b"\n").unwrap();
        });
        port
    }

    #[test]
    fn round_trip_request_response_through_tcp() {
        let port = one_shot_server(r#"{"jsonrpc":"2.0","id":1,"result":{"ok":true}}"#);
        let mut tcp = TcpStream::connect(("127.0.0.1", port)).unwrap();

        // Frame a request and feed it into the bridge as stdin.
        let request = br#"{"jsonrpc":"2.0","id":1,"method":"ping","params":{}}"#;
        let mut framed = Vec::new();
        stdio::write_message(&mut framed, request).unwrap();
        let mut stdin = Cursor::new(framed);
        let mut stdout: Vec<u8> = Vec::new();

        let ok = forward_one(&mut stdin, &mut stdout, &mut tcp).unwrap();
        assert!(ok);

        // Decode the framed output and confirm the canned response made it back.
        let body = stdio::read_message(&mut Cursor::new(&stdout)).unwrap();
        let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(parsed["result"]["ok"], true);
    }

    #[test]
    fn eof_on_stdin_returns_false() {
        let port = one_shot_server("{}");
        let mut tcp = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let mut stdin: &[u8] = &[];
        let mut stdout: Vec<u8> = Vec::new();
        let result = forward_one(&mut stdin, &mut stdout, &mut tcp).unwrap();
        assert!(!result, "expected false on EOF");
        // Drop tcp so the server thread cleans up.
        drop(tcp);
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p vrm-godot-shim --lib bridge
```

Expected: 2 passed.

- [ ] **Step 3: Run full crate suite + clippy**

```bash
cargo test -p vrm-godot-shim
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: 8 passed (6 from prior tasks + 2 new), clippy clean.

- [ ] **Step 4: Commit**

```bash
git add crates/vrm-godot-shim/src/bridge.rs
git commit -m "feat(vrm-godot-shim): stdio<->TCP forwarding loop"
```

---

### Task 5: main.rs — wire bridge + child into a runnable binary

**Files:**
- Modify: `crates/vrm-godot-shim/src/main.rs`

- [ ] **Step 1: Replace the placeholder main**

```bash
cat > crates/vrm-godot-shim/src/main.rs <<'RS'
//! vrm-godot-shim binary entry. Spawns Godot, accepts its TCP connection,
//! then loops forwarding framed stdio requests to/from the TCP socket
//! until stdin closes.

use std::env;
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use vrm_godot_shim::bridge::{forward_one, BridgeError};
use vrm_godot_shim::child::{accept_with_timeout, bind_ephemeral, spawn_godot, ChildError};

const GODOT_ACCEPT_TIMEOUT: Duration = Duration::from_secs(10);

fn adapter_project_dir() -> PathBuf {
    if let Some(p) = env::var_os("GODOT_VRM_ADAPTER_DIR") {
        return PathBuf::from(p);
    }
    // Default: adapters/godot-vrm relative to the workspace root, located
    // by walking up from the current exe's directory. Falls back to CWD
    // for the dev-loop case (cargo run).
    if let Ok(exe) = env::current_exe() {
        let mut p = exe;
        while p.pop() {
            let candidate = p.join("adapters").join("godot-vrm");
            if candidate.join("project.godot").is_file() {
                return candidate;
            }
        }
    }
    PathBuf::from("adapters/godot-vrm")
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let (listener, port) = bind_ephemeral()?;
    let project_dir = adapter_project_dir();
    let mut godot = spawn_godot(&project_dir, "src/main.gd", port)?;
    let mut tcp = accept_with_timeout(&listener, GODOT_ACCEPT_TIMEOUT)?;

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdin_lock = stdin.lock();
    let mut stdout_lock = stdout.lock();

    loop {
        match forward_one(&mut stdin_lock, &mut stdout_lock, &mut tcp) {
            Ok(true) => continue,
            Ok(false) => break,
            Err(BridgeError::PeerClosed) => break,
            Err(e) => return Err(e.into()),
        }
    }
    // Close TCP so Godot sees EOF on its socket and exits.
    drop(tcp);
    let _ = godot.child.wait();
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("vrm-godot-shim: {e}");
            // Surface ChildError::GodotMissing as exit 2 so callers can
            // distinguish "wrong host config" from "wrong adapter behavior".
            if let Some(child_err) = e.downcast_ref::<ChildError>() {
                if matches!(child_err, ChildError::GodotMissing) {
                    return ExitCode::from(2);
                }
            }
            ExitCode::FAILURE
        }
    }
}
RS
```

- [ ] **Step 2: Verify the binary builds + is reachable**

```bash
cargo build -p vrm-godot-shim
ls -l target/debug/vrm-godot-shim
```

Expected: binary exists, ~5 MB.

- [ ] **Step 3: Smoke-test the missing-Godot path**

This works even before the Godot side exists, because GODOT_BIN is intercepted before any TCP/connect attempt:

```bash
GODOT_BIN=/definitely/not/real ./target/debug/vrm-godot-shim </dev/null; echo "exit: $?"
```

Expected: `vrm-godot-shim: godot binary not found on PATH; set GODOT_BIN env var or install Godot 4.x` on stderr; exit 2.

- [ ] **Step 4: Confirm clippy stays clean**

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

- [ ] **Step 5: Commit**

```bash
git add crates/vrm-godot-shim/src/main.rs
git commit -m "feat(vrm-godot-shim): main binary wires bridge + child"
```

---

### Task 6: Godot package skeleton + dispatch table

**Files:**
- Create: `adapters/godot-vrm/README.md`
- Create: `adapters/godot-vrm/project.godot`
- Create: `adapters/godot-vrm/.gitignore`
- Create: `adapters/godot-vrm/src/operations.gd`
- Create: `adapters/godot-vrm/tests/run_gdscript_tests.gd`
- Create: `adapters/godot-vrm/tests/test_operations.gd`

- [ ] **Step 1: Project descriptor + adapter gitignore**

```bash
mkdir -p adapters/godot-vrm/{src,tests}
cat > adapters/godot-vrm/project.godot <<'GD'
; Godot 4.x project descriptor for the godot-vrm conformance adapter.
; Headless-only — no main scene, no display server. Spawned by the
; vrm-godot-shim Rust binary, which passes a TCP loopback port as the
; first positional user arg after `--`. The adapter reads that port,
; connects, and runs the NDJSON request loop in src/tcp_session.gd.

config_version=5

[application]

config/name="vrm-godot-vrm-adapter"
config/description="V-Sekai/godot-vrm renderer adapter for arkavo-org/vrm-conformance. L1+L2: dispatch scaffold; renderer integration deferred to L3."

[debug]

settings/stdout/print_fps=false
settings/stdout/verbose_stdout=false
GD

cat > adapters/godot-vrm/.gitignore <<'GD'
.godot/
.import/
export.cfg
export_presets.cfg
*.import
GD
```

- [ ] **Step 2: Dispatch table**

```bash
cat > adapters/godot-vrm/src/operations.gd <<'GD'
# Operation registry + dispatch for the godot-vrm adapter.
#
# L1 + L2 state: every Phase 1 op and every reserved op returns a structured
# `Unimplemented` error. L3 replaces the Phase 1 entries with real
# implementations driven by V-Sekai/godot-vrm + a hidden SubViewport.

class_name Operations

const PHASE_BY_METHOD := {
    "load_vrm": "L3 (godot-vrm integration deferred)",
    "set_camera": "L3 (godot-vrm integration deferred)",
    "set_lighting": "L3 (godot-vrm integration deferred)",
    "set_post_processing": "L3 (godot-vrm integration deferred)",
    "render": "L3 (godot-vrm integration deferred)",
    "dispose": "L3 (godot-vrm integration deferred)",
    "set_environment": "v1.x",
    "set_expression": "Phase 3",
    "set_humanoid_pose": "Phase 2",
    "set_root_transform": "Phase 2",
    "animate_root_transform": "Phase 2",
    "step_physics": "Phase 2",
    "reset_physics": "Phase 2",
}

# Returns the JSON-RPC response dict for one request. `id` is forwarded
# from the request unchanged. Unknown methods return -32601.
static func dispatch(id: Variant, method: String, _params: Variant) -> Dictionary:
    if PHASE_BY_METHOD.has(method):
        return {
            "jsonrpc": "2.0",
            "id": id,
            "error": {
                "code": -32000,
                "message": "Unimplemented",
                "data": { "phase": PHASE_BY_METHOD[method] },
            },
        }
    return {
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": -32601,
            "message": "method not found: " + method,
        },
    }
GD
```

- [ ] **Step 3: GDScript test runner (no external deps)**

```bash
cat > adapters/godot-vrm/tests/run_gdscript_tests.gd <<'GD'
extends SceneTree

const TESTS_DIR := "res://tests/"

var _passed := 0
var _failed := 0
var _failures: Array[String] = []

func _init() -> void:
    var dir := DirAccess.open(TESTS_DIR)
    if dir == null:
        push_error("cannot open " + TESTS_DIR); quit(2); return
    dir.list_dir_begin()
    var names: Array[String] = []
    while true:
        var name := dir.get_next()
        if name == "": break
        if name.begins_with("test_") and name.ends_with(".gd"):
            names.append(name)
    names.sort()
    for name in names:
        _run_file(TESTS_DIR + name)
    print("\n%d passed, %d failed" % [_passed, _failed])
    for f in _failures:
        print("  FAIL: " + f)
    quit(0 if _failed == 0 else 1)

func _run_file(path: String) -> void:
    var script: GDScript = load(path)
    if script == null:
        _failed += 1; _failures.append(path + " (load failed)"); return
    var inst: Object = script.new()
    for m in inst.get_method_list():
        var mname: String = m["name"]
        if not mname.begins_with("test_"): continue
        inst.set("_test_failure", "")
        inst.call(mname)
        var captured: String = inst.get("_test_failure")
        if captured == "":
            _passed += 1
        else:
            _failed += 1
            _failures.append("%s::%s — %s" % [path, mname, captured])
GD
```

- [ ] **Step 4: Dispatch unit tests**

```bash
cat > adapters/godot-vrm/tests/test_operations.gd <<'GD'
extends RefCounted

const Operations := preload("res://src/operations.gd")

var _test_failure: String = ""

func _fail(msg: String) -> void:
    if _test_failure == "":
        _test_failure = msg

func _assert_eq(actual, expected, label: String) -> void:
    if actual != expected:
        _fail("%s: expected %s, got %s" % [label, str(expected), str(actual)])

func test_unknown_method_returns_minus_32601() -> void:
    var r: Dictionary = Operations.dispatch(7, "definitely_not_a_method", {})
    _assert_eq(r.get("id"), 7, "id echoed")
    _assert_eq(r.get("error", {}).get("code"), -32601, "error code")

func test_load_vrm_returns_l3_deferral() -> void:
    var r: Dictionary = Operations.dispatch(1, "load_vrm", {"path": "/tmp/x.vrm"})
    var err: Dictionary = r.get("error", {})
    _assert_eq(err.get("code"), -32000, "error code")
    _assert_eq(err.get("message"), "Unimplemented", "error message")
    _assert_eq(err.get("data", {}).get("phase"), "L3 (godot-vrm integration deferred)", "phase label")

func test_render_returns_l3_deferral() -> void:
    var r: Dictionary = Operations.dispatch(2, "render", {})
    _assert_eq(r.get("error", {}).get("data", {}).get("phase"), "L3 (godot-vrm integration deferred)", "phase label")

func test_set_humanoid_pose_returns_phase_2() -> void:
    var r: Dictionary = Operations.dispatch(3, "set_humanoid_pose", {})
    _assert_eq(r.get("error", {}).get("data", {}).get("phase"), "Phase 2", "phase label")

func test_set_environment_returns_v1x() -> void:
    var r: Dictionary = Operations.dispatch(4, "set_environment", {})
    _assert_eq(r.get("error", {}).get("data", {}).get("phase"), "v1.x", "phase label")

func test_set_expression_returns_phase_3() -> void:
    var r: Dictionary = Operations.dispatch(5, "set_expression", {})
    _assert_eq(r.get("error", {}).get("data", {}).get("phase"), "Phase 3", "phase label")

func test_id_is_echoed_on_success_and_error_paths() -> void:
    var r1: Dictionary = Operations.dispatch("abc-123", "load_vrm", {})
    _assert_eq(r1.get("id"), "abc-123", "string id echoed on error path")
    var r2: Dictionary = Operations.dispatch(null, "definitely_not_a_method", {})
    _assert_eq(r2.get("id"), null, "null id echoed on -32601 path")
GD
```

- [ ] **Step 5: Run tests**

```bash
godot --headless --path adapters/godot-vrm --script tests/run_gdscript_tests.gd
```

Expected stdout: `7 passed, 0 failed`. Exit code 0.

- [ ] **Step 6: Stub README (full content lands in Task 10)**

```bash
cat > adapters/godot-vrm/README.md <<'MD'
# godot-vrm renderer adapter

Scaffold for the V-Sekai/godot-vrm renderer adapter. Architecture: the
runner spawns `vrm-godot-shim` (Rust); the shim spawns this directory
as a headless Godot project; the two talk newline-delimited JSON over
TCP loopback. See [`docs/superpowers/plans/2026-05-11-adapter-godot-vrm-scaffold-v2.md`](../../docs/superpowers/plans/2026-05-11-adapter-godot-vrm-scaffold-v2.md).

Full README lands in Task 10 of the v2 scaffold plan.
MD
```

- [ ] **Step 7: Commit**

```bash
git add adapters/godot-vrm/
git commit -m "feat(adapters/godot-vrm): GDScript dispatch table + unit tests"
```

---

### Task 7: tcp_session.gd + main.gd (Godot-side session loop)

**Files:**
- Create: `adapters/godot-vrm/src/tcp_session.gd`
- Create: `adapters/godot-vrm/src/main.gd`

- [ ] **Step 1: Implement the TCP session loop**

```bash
cat > adapters/godot-vrm/src/tcp_session.gd <<'GD'
# NDJSON request/response loop over a connected StreamPeerTCP socket.
# One JSON object per line, terminated by "\n". On socket close or
# read error, returns cleanly so main.gd can call quit(0).

class_name TcpSession

const Operations := preload("res://src/operations.gd")

# Run the loop on `socket`. Blocks until the peer (shim) closes the
# connection. Errors surface as push_error + return; the shim treats a
# closed socket as "session over".
static func run(socket: StreamPeerTCP) -> void:
    var buf := PackedByteArray()
    while true:
        if socket.get_status() != StreamPeerTCP.STATUS_CONNECTED:
            return
        socket.poll()
        # Drain whatever's available; accumulate until we see "\n".
        var available := socket.get_available_bytes()
        if available > 0:
            var chunk := socket.get_data(available)
            if chunk[0] != OK:
                push_error("tcp read error: %d" % chunk[0]); return
            buf.append_array(chunk[1])
        # Process every complete line in the buffer.
        var newline_byte := 0x0a  # "\n"
        while true:
            var nl := buf.find(newline_byte)
            if nl < 0: break
            var line: PackedByteArray = buf.slice(0, nl)
            buf = buf.slice(nl + 1)
            var text := line.get_string_from_utf8()
            var parsed: Variant = JSON.parse_string(text)
            var resp: Dictionary
            if parsed == null or typeof(parsed) != TYPE_DICTIONARY:
                resp = {
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": { "code": -32700, "message": "parse error" },
                }
            else:
                var req: Dictionary = parsed
                resp = Operations.dispatch(
                    req.get("id", null),
                    req.get("method", ""),
                    req.get("params", {}),
                )
            var out := (JSON.stringify(resp) + "\n").to_utf8_buffer()
            var put_err := socket.put_data(out)
            if put_err != OK:
                push_error("tcp write error: %d" % put_err); return
        # Yield to OS so we don't busy-spin if peer is idle.
        OS.delay_msec(5)
GD
```

- [ ] **Step 2: Implement main.gd**

```bash
cat > adapters/godot-vrm/src/main.gd <<'GD'
# godot-vrm adapter — Godot-side entry. Reads the loopback port from the
# first positional user arg (after `--`), connects to vrm-godot-shim,
# and runs the NDJSON session loop until the shim closes the socket.

extends SceneTree

const TcpSession := preload("res://src/tcp_session.gd")

func _init() -> void:
    var args := OS.get_cmdline_user_args()
    if args.is_empty():
        push_error("godot-vrm adapter: expected positional port arg after `--`"); quit(2); return
    var port := args[0].to_int()
    if port <= 0 or port > 65535:
        push_error("godot-vrm adapter: bad port: %s" % args[0]); quit(2); return

    var socket := StreamPeerTCP.new()
    var err := socket.connect_to_host("127.0.0.1", port)
    if err != OK:
        push_error("godot-vrm adapter: connect_to_host failed: %d" % err); quit(2); return

    var deadline := Time.get_ticks_msec() + 5000
    while socket.get_status() == StreamPeerTCP.STATUS_CONNECTING:
        if Time.get_ticks_msec() > deadline:
            push_error("godot-vrm adapter: connect timeout"); quit(2); return
        socket.poll()
        OS.delay_msec(10)
    if socket.get_status() != StreamPeerTCP.STATUS_CONNECTED:
        push_error("godot-vrm adapter: not connected: status=%d" % socket.get_status()); quit(2); return

    TcpSession.run(socket)
    socket.disconnect_from_host()
    quit(0)
GD
```

- [ ] **Step 3: Confirm existing unit tests still pass**

```bash
godot --headless --path adapters/godot-vrm --script tests/run_gdscript_tests.gd
```

Expected: `7 passed, 0 failed`.

- [ ] **Step 4: Manual smoke — shim spawns Godot, exchanges one request**

```bash
cargo build -p vrm-godot-shim
printf 'Content-Length: 60\r\n\r\n{"jsonrpc":"2.0","id":1,"method":"load_vrm","params":{"p":""}}' \
  | ./target/debug/vrm-godot-shim
```

Expected response (the shim writes a framed JSON-RPC response to stdout — the exact byte length depends on JSON key order, so look for the substring rather than pinning the length):

```
Content-Length: NNN\r\n\r\n{"jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":"Unimplemented","data":{"phase":"L3 (godot-vrm integration deferred)"}}}
```

If the smoke fails, the Rust integration test in Task 8 will narrow it down.

- [ ] **Step 5: Commit**

```bash
git add adapters/godot-vrm/src/main.gd adapters/godot-vrm/src/tcp_session.gd
git commit -m "feat(adapters/godot-vrm): Godot-side TCP session loop"
```

---

### Task 8: Rust integration test (real Godot)

**Files:**
- Create: `crates/vrm-godot-shim/tests/contract.rs`

- [ ] **Step 1: Write the integration test**

```bash
cat > crates/vrm-godot-shim/tests/contract.rs <<'RS'
//! End-to-end contract test against a real Godot child.
//!
//! Marked `#[ignore]` so `cargo test --workspace` stays green on hosts
//! without Godot installed. CI runs this with `-- --ignored`. Same
//! pattern as the validator-gated tests in vrm-asset-generator.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn shim_binary() -> PathBuf {
    // Cargo sets CARGO_BIN_EXE_<name> for #[test]s in a crate that builds
    // a bin target. This is the canonical way to find the binary under
    // test without hard-coding target/debug.
    PathBuf::from(env!("CARGO_BIN_EXE_vrm-godot-shim"))
}

fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.parent().unwrap().parent().unwrap().to_path_buf()
}

struct Exchange {
    request: Vec<u8>,
    expected_code: i64,
    expected_phase: Option<&'static str>,
}

fn frame(body: &[u8]) -> Vec<u8> {
    let mut out = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    out.extend_from_slice(body);
    out
}

fn read_framed(mut r: impl Read) -> Vec<u8> {
    let mut header = Vec::new();
    let mut tmp = [0u8; 1];
    while !header.ends_with(b"\r\n\r\n") {
        let n = r.read(&mut tmp).expect("read header");
        if n == 0 {
            panic!("eof while reading header; got: {header:?}");
        }
        header.push(tmp[0]);
        if header.len() > 4096 {
            panic!("runaway header: {header:?}");
        }
    }
    let head = String::from_utf8(header).unwrap();
    let mut content_length: Option<usize> = None;
    for line in head.split("\r\n") {
        if line.is_empty() { continue; }
        let (k, v) = line.split_once(':').expect("header line");
        if k.trim().eq_ignore_ascii_case("Content-Length") {
            content_length = Some(v.trim().parse().expect("content-length number"));
        }
    }
    let len = content_length.expect("Content-Length present");
    let mut body = vec![0u8; len];
    r.read_exact(&mut body).expect("read body");
    body
}

#[test]
#[ignore]
fn contract_cases_round_trip_through_real_godot() {
    let exchanges: Vec<Exchange> = vec![
        Exchange {
            request: br#"{"jsonrpc":"2.0","id":1,"method":"definitely_not_a_method","params":{}}"#.to_vec(),
            expected_code: -32601,
            expected_phase: None,
        },
        Exchange {
            request: br#"{"jsonrpc":"2.0","id":2,"method":"load_vrm","params":{"path":"/tmp/x.vrm"}}"#.to_vec(),
            expected_code: -32000,
            expected_phase: Some("L3 (godot-vrm integration deferred)"),
        },
        Exchange {
            request: br#"{"jsonrpc":"2.0","id":3,"method":"render","params":{}}"#.to_vec(),
            expected_code: -32000,
            expected_phase: Some("L3 (godot-vrm integration deferred)"),
        },
        Exchange {
            request: br#"{"jsonrpc":"2.0","id":4,"method":"set_humanoid_pose","params":{}}"#.to_vec(),
            expected_code: -32000,
            expected_phase: Some("Phase 2"),
        },
        Exchange {
            request: br#"{"jsonrpc":"2.0","id":5,"method":"set_environment","params":{}}"#.to_vec(),
            expected_code: -32000,
            expected_phase: Some("v1.x"),
        },
        Exchange {
            request: br#"{"jsonrpc":"2.0","id":6,"method":"set_expression","params":{}}"#.to_vec(),
            expected_code: -32000,
            expected_phase: Some("Phase 3"),
        },
    ];

    let project_dir = workspace_root().join("adapters").join("godot-vrm");
    assert!(project_dir.join("project.godot").is_file(),
        "expected project.godot at {}", project_dir.display());

    let mut child = Command::new(shim_binary())
        .env("GODOT_VRM_ADAPTER_DIR", &project_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn shim");

    let mut stdin = child.stdin.take().expect("shim stdin");
    let mut stdout = child.stdout.take().expect("shim stdout");

    for ex in &exchanges {
        stdin.write_all(&frame(&ex.request)).expect("write request");
        stdin.flush().expect("flush");

        let body = read_framed(&mut stdout);
        let parsed: serde_json::Value = serde_json::from_slice(&body)
            .expect("parse response JSON");
        let code = parsed["error"]["code"].as_i64()
            .unwrap_or_else(|| panic!("missing error.code in {parsed}"));
        assert_eq!(code, ex.expected_code,
            "method {:?} expected code {}, got {} (body: {})",
            std::str::from_utf8(&ex.request).unwrap_or("<binary>"),
            ex.expected_code, code, parsed);
        if let Some(phase) = ex.expected_phase {
            let actual = parsed["error"]["data"]["phase"].as_str()
                .unwrap_or_else(|| panic!("missing error.data.phase in {parsed}"));
            assert_eq!(actual, phase, "phase mismatch in {parsed}");
        }
    }

    drop(stdin);
    let status = child.wait().expect("wait");
    assert!(status.success(), "shim exited with {status:?}");
}
RS
```

- [ ] **Step 2: Run the integration test**

```bash
cargo test -p vrm-godot-shim --test contract -- --ignored --nocapture
```

Expected: `1 passed`. If it fails, the assertion message includes the bad response body — usually a phase-label typo or a JSON-key-name mismatch.

- [ ] **Step 3: Verify `cargo test --workspace` (no `--ignored`) still passes without Godot**

```bash
cargo test --workspace
```

Expected: every existing workspace test passes; the godot-vrm contract test is skipped (it's `#[ignore]`d).

- [ ] **Step 4: Commit**

```bash
git add crates/vrm-godot-shim/tests/contract.rs
git commit -m "test(vrm-godot-shim): #[ignore]'d end-to-end contract test"
```

---

### Task 9: CI workflow

**Files:**
- Create: `.github/workflows/godot-vrm.yml`

- [ ] **Step 1: Author the workflow**

```bash
cat > .github/workflows/godot-vrm.yml <<'YML'
name: godot-vrm

# L1+L2 scaffold: Rust shim + GDScript dispatch. Tasks build the Rust
# crate, install Godot 4 headless, run the GDScript unit tests, and run
# the #[ignore]'d Rust integration test that spawns the shim + real Godot.
#
# Real renderer integration (L3) is a separate plan; this workflow
# doesn't run any render pipeline yet.
#
# No untrusted-input usage: this workflow does not read PR titles, commit
# messages, issue bodies, or any other user-controlled fields into run:
# commands.

on:
  pull_request:
    paths:
      - 'adapters/godot-vrm/**'
      - 'crates/vrm-godot-shim/**'
      - 'Cargo.toml'
      - 'Cargo.lock'
      - '.github/workflows/godot-vrm.yml'
  push:
    branches: [main]
    paths:
      - 'adapters/godot-vrm/**'
      - 'crates/vrm-godot-shim/**'
      - 'Cargo.toml'
      - 'Cargo.lock'
      - '.github/workflows/godot-vrm.yml'

jobs:
  test:
    runs-on: ubuntu-latest
    env:
      GODOT_VERSION: 4.3-stable
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2

      - name: Cache Godot binary
        id: cache-godot
        uses: actions/cache@v4
        with:
          path: ~/.local/bin/godot
          key: godot-${{ env.GODOT_VERSION }}-linux-x86_64

      - name: Install Godot headless
        if: steps.cache-godot.outputs.cache-hit != 'true'
        run: |
          mkdir -p ~/.local/bin
          curl -L -o /tmp/godot.zip \
            "https://github.com/godotengine/godot/releases/download/${GODOT_VERSION}/Godot_v${GODOT_VERSION}_linux.x86_64.zip"
          unzip -p /tmp/godot.zip > ~/.local/bin/godot
          chmod +x ~/.local/bin/godot

      - name: Make Godot reachable on PATH
        run: |
          echo "$HOME/.local/bin" >> "$GITHUB_PATH"
          ~/.local/bin/godot --version

      - name: Build shim
        run: cargo build -p vrm-godot-shim --release

      - name: Run GDScript dispatch unit tests
        run: godot --headless --path adapters/godot-vrm --script tests/run_gdscript_tests.gd

      - name: Run Rust integration contract test
        run: cargo test -p vrm-godot-shim --release --test contract -- --ignored
YML
```

- [ ] **Step 2: Validate workflow YAML**

```bash
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/godot-vrm.yml'))" || echo "(PyYAML missing — CI will validate)"
```

Expected: no output, exit 0. If PyYAML is missing the message is informational; CI catches it.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/godot-vrm.yml
git commit -m "ci(godot-vrm): install Godot + run shim + dispatch tests"
```

---

### Task 10: Full README + cross-link existing docs

**Files:**
- Modify: `adapters/godot-vrm/README.md`
- Modify: `README.md` (repo root)
- Modify: `CLAUDE.md`
- Modify: `adapters/babylon-vrm/README.md`

- [ ] **Step 1: Write the full adapter README**

```bash
cat > adapters/godot-vrm/README.md <<'MD'
# godot-vrm renderer adapter

A renderer adapter that bridges [V-Sekai/godot-vrm](https://github.com/V-Sekai/godot-vrm) to the project's renderer-agnostic operation contract documented at [`docs/operation-contract.md`](../../docs/operation-contract.md).

Architecture differs from the [three-vrm](../three-vrm/README.md) and [babylon-vrm](../babylon-vrm/README.md) adapters by necessity: GDScript on Godot 4 does not expose a byte-safe stdout API (no `OS.write_buffer_to_stdout`, `print`/`printraw` are banner-polluted, `--quiet` suppresses everything, `FileAccess.open("/dev/stdout", ...)` is rejected). The conformance runner's framed stdio contract therefore lives in a separate Rust shim — `vrm-godot-shim` — which spawns Godot headless as a child and bridges JSON-RPC ↔ TCP-loopback. See [`docs/superpowers/plans/2026-05-11-adapter-godot-vrm-scaffold-v2.md`](../../docs/superpowers/plans/2026-05-11-adapter-godot-vrm-scaffold-v2.md) for the rationale.

```
runner ──framed stdio──> vrm-godot-shim ──NDJSON over TCP──> godot --headless --script src/main.gd
```

## Why a third adapter

vrm-conformance has two real adapters (three-vrm + vrm-metal-kit). The [N-way consensus diff](../../crates/vrm-diff-engine/src/consensus.rs) needs three or more independent renderers to flag outliers. The natural third candidate — `virtual-cast/babylon-vrm-loader` via [`adapters/babylon-vrm/`](../babylon-vrm/) — is upstream-blocked on VRM 1.0 support. `V-Sekai/godot-vrm` already implements VRMC_vrm, VRMC_materials_mtoon, VRMC_springBone, and VRMC_node_constraint, so it's the realistic next adapter for closing the third-renderer gap.

## Status

| Phase | Status |
|---|---|
| L1 — package skeleton                         | scaffolded |
| L2 — JSON-RPC + dispatch                      | scaffolded (all ops return `Unimplemented`) |
| L3 — Phase 1 ops against V-Sekai/godot-vrm    | deferred (separate plan) |

Through L2, every operation returns a structured `Unimplemented` error (JSON-RPC code `-32000`):

| Method | `data.phase` |
|---|---|
| `load_vrm`, `set_camera`, `set_lighting`, `set_post_processing`, `render`, `dispose` | `L3 (godot-vrm integration deferred)` |
| `set_humanoid_pose`, `set_root_transform`, `animate_root_transform`, `step_physics`, `reset_physics` | `Phase 2` |
| `set_environment` | `v1.x` |
| `set_expression` | `Phase 3` |
| (unknown) | `-32601 method not found` |

## Runtime dependency

Godot 4.x must be on `PATH` as `godot` (or pointed at via `GODOT_BIN`). 4.3 minimum; tested on 4.6.2.

- macOS: `brew install --cask godot`
- Linux: download `Godot_v4.3-stable_linux.x86_64.zip` from [Godot releases](https://github.com/godotengine/godot/releases/tag/4.3-stable) and put the binary on `PATH`.

## Build

```bash
cargo build -p vrm-godot-shim --release
```

The runner consumes `target/release/vrm-godot-shim` as `--adapter-bin`. The `adapters/godot-vrm/` Godot project is discovered automatically; override with `GODOT_VRM_ADAPTER_DIR` if needed.

## Tests

```bash
# GDScript dispatch unit tests
godot --headless --path adapters/godot-vrm --script tests/run_gdscript_tests.gd

# Rust end-to-end contract test (spawns shim + real Godot)
cargo test -p vrm-godot-shim --test contract -- --ignored
```

Both run in CI (`.github/workflows/godot-vrm.yml`).

## How the runner invokes it

Same wire as the other adapters — framed LSP `Content-Length` JSON-RPC over stdio. The shim handles framing; Godot only sees NDJSON over TCP loopback. Wire-level invocation:

```bash
cargo run -p vrm-runner -- execute-test-plan \
  --plan <plan.yaml> \
  --adapter-bin target/release/vrm-godot-shim \
  --asset-dir <assets> \
  --output-dir <out> \
  --renderer-name godot-vrm \
  --json
```

L3 will keep this exact invocation. Only the Phase 1 dispatch behavior changes (return real responses instead of `Unimplemented`).

## L3 sketch

Lives in a separate plan. Implementation outline:

1. Add `addons/godot-vrm/` pinned to a specific V-Sekai commit (parity with `adapters/vrm-metal-kit/Package.swift`'s upstream-revision pin).
2. Replace the `Unimplemented` returns for Phase 1 ops with real handlers driving a hidden `SubViewport` rendering to a `ViewportTexture` saved via `Image.save_png`.
3. Magenta clear color `(255, 0, 255)` for property-assertion bbox detection.
4. `Environment.tone_mapper = TONE_MAPPER_LINEAR` + shadows disabled for MToon math tests.
5. `Engine.physics_ticks_per_second = 60` + spring-bone reset via `addons/godot-vrm`.

`scripts/bootstrap-goldens.sh` gains a `SKIP_GODOT_VRM` knob and a `render_with_adapter "godot-vrm" "<version>" "$ROOT/target/release/vrm-godot-shim"` call once L3 produces real renders.
MD
```

- [ ] **Step 2: Add the adapter row to the root README table**

In `README.md`, after the `adapters/babylon-vrm/` row (around line 44), insert:

```markdown
| `adapters/godot-vrm/` | Godot 4 / GDScript adapter for [V-Sekai/godot-vrm](https://github.com/V-Sekai/godot-vrm). Pairs with `crates/vrm-godot-shim/` (Rust) for byte-safe stdio framing — see that adapter's README. L1+L2 scaffold; renderer integration deferred to L3. The realistic third real renderer once L3 lands. |
```

Run:

```bash
grep -n 'adapters/godot-vrm' README.md
```

Expected: the new row line.

- [ ] **Step 3: Add the adapter status to CLAUDE.md**

In `CLAUDE.md`, in the `### Adapter status` section, append after the babylon-vrm bullet:

```markdown
- `adapters/godot-vrm/` — Godot 4 / GDScript. Paired with a Rust shim (`crates/vrm-godot-shim/`) that owns the LSP framing and bridges JSON-RPC ↔ TCP loopback to a headless Godot child; GDScript only handles dispatch. L1+L2 scaffolded; all ops return `Unimplemented`. Runner consumes `target/release/vrm-godot-shim` as `--adapter-bin`. Requires Godot 4.3+ on `PATH` or `GODOT_BIN`.
```

- [ ] **Step 4: Update babylon-vrm cross-link**

In `adapters/babylon-vrm/README.md`, replace the existing `### Alternative third adapter` paragraph with:

```markdown
### Alternative third adapter

[V-Sekai/godot-vrm](https://github.com/V-Sekai/godot-vrm) is now scaffolded at [`adapters/godot-vrm/`](../godot-vrm/) (L1+L2; renderer integration deferred to L3). Once L3 lands, the consensus diff will have its third independent renderer regardless of the babylon-vrm-loader VRM 1.0 timeline.
```

- [ ] **Step 5: Commit**

```bash
git add adapters/godot-vrm/README.md README.md CLAUDE.md adapters/babylon-vrm/README.md
git commit -m "docs: cross-link godot-vrm adapter scaffold + Rust shim"
```

---

### Task 11: End-to-end verification + L3-wiring note

**Files:** none new — verification + a single README addendum.

- [ ] **Step 1: Run every test layer from a fresh build**

```bash
cargo build --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
godot --headless --path adapters/godot-vrm --script tests/run_gdscript_tests.gd
cargo test -p vrm-godot-shim --test contract -- --ignored
```

Expected:
- `cargo build`: clean.
- `cargo clippy`: no warnings.
- `cargo test --workspace`: all existing tests pass + 8 new shim tests pass (the ignored contract test is skipped).
- GDScript runner: `7 passed, 0 failed`.
- `cargo test --ignored`: 1 passed (contract).

- [ ] **Step 2: Smoke the binary stand-alone**

```bash
./target/debug/vrm-godot-shim </dev/null; echo "exit: $?"
```

Expected: clean exit 0 within ~3 s (Godot starts, sees socket close immediately, both exit). If Godot is slow to start the test takes longer — up to the 10s accept timeout.

- [ ] **Step 3: Append the L3 bootstrap-wiring note to the adapter README**

Insert at the end of `adapters/godot-vrm/README.md`:

```markdown
## Bootstrap wiring (L3)

`scripts/bootstrap-goldens.sh` will gain a `SKIP_GODOT_VRM` env knob and a
`render_with_adapter "godot-vrm" "<version>" "$ROOT/target/release/vrm-godot-shim"`
call once L3 produces real renders. Not wired during L1+L2 because Phase 1
ops return `Unimplemented` and the runner cannot complete `execute-test-plan`
against this adapter yet.
```

- [ ] **Step 4: Commit**

```bash
git add adapters/godot-vrm/README.md
git commit -m "docs(adapters/godot-vrm): note L3 bootstrap wiring"
```

---

## Out of scope (deferred to L3 plan)

- Installing/pinning `addons/godot-vrm/` (V-Sekai asset).
- Real `load_vrm` parsing the VRM 1.0 GLB extensions Godot doesn't natively understand.
- SubViewport + ViewportTexture render path with magenta clear color.
- `Engine.physics_ticks_per_second = 60` + spring-bone reset against V-Sekai's API.
- `scripts/bootstrap-goldens.sh` integration (`SKIP_GODOT_VRM`, `render_with_adapter` call).
- `docs/findings.md` entries from a three-renderer consensus run.

A separate plan (`YYYY-MM-DD-adapter-godot-vrm-L3.md`) covers those.
