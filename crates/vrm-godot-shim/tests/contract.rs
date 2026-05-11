//! End-to-end contract test against a real Godot child.
//!
//! Marked `#[ignore]` so `cargo test --workspace` stays green on hosts
//! without Godot installed. CI runs this with `-- --ignored`. Same
//! pattern as the validator-gated tests in vrm-asset-generator.
//!
//! Locks down two contract properties beyond the L1+L2 phase labels:
//! (1) integer ids round-trip as integers — GDScript's JSON parser
//!     returns numbers as float, so without explicit coercion the
//!     response would carry `"id":1.0` and fail to deserialize into
//!     `vrm-ops::JsonRpcResponse::id: u64`.
//! (2) malformed JSON produces a -32700 envelope with `id: null` per
//!     JSON-RPC 2.0.

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
    request_id: i64,
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
            request_id: 1,
            request: br#"{"jsonrpc":"2.0","id":1,"method":"definitely_not_a_method","params":{}}"#.to_vec(),
            expected_code: -32601,
            expected_phase: None,
        },
        Exchange {
            request_id: 2,
            request: br#"{"jsonrpc":"2.0","id":2,"method":"load_vrm","params":{"path":"/tmp/x.vrm"}}"#.to_vec(),
            expected_code: -32000,
            expected_phase: Some("L3 (godot-vrm integration deferred)"),
        },
        Exchange {
            request_id: 3,
            request: br#"{"jsonrpc":"2.0","id":3,"method":"render","params":{}}"#.to_vec(),
            expected_code: -32000,
            expected_phase: Some("L3 (godot-vrm integration deferred)"),
        },
        Exchange {
            request_id: 4,
            request: br#"{"jsonrpc":"2.0","id":4,"method":"set_humanoid_pose","params":{}}"#.to_vec(),
            expected_code: -32000,
            expected_phase: Some("Phase 2"),
        },
        Exchange {
            request_id: 5,
            request: br#"{"jsonrpc":"2.0","id":5,"method":"set_environment","params":{}}"#.to_vec(),
            expected_code: -32000,
            expected_phase: Some("v1.x"),
        },
        Exchange {
            request_id: 6,
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

        // Lock down integer-id round-trip — regression guard for the
        // GDScript float-coercion bug fixed in fb7bd689.
        let id = parsed["id"].as_i64()
            .unwrap_or_else(|| panic!("response id was not an integer; got {:?} (body: {parsed})",
                parsed["id"]));
        assert_eq!(id, ex.request_id,
            "id mismatch: expected {}, got {} (body: {parsed})",
            ex.request_id, id);

        let code = parsed["error"]["code"].as_i64()
            .unwrap_or_else(|| panic!("missing error.code in {parsed}"));
        assert_eq!(code, ex.expected_code,
            "method {:?} expected code {}, got {} (body: {parsed})",
            std::str::from_utf8(&ex.request).unwrap_or("<binary>"),
            ex.expected_code, code);
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

#[test]
#[ignore]
fn malformed_json_returns_parse_error_with_null_id() {
    let project_dir = workspace_root().join("adapters").join("godot-vrm");
    let mut child = Command::new(shim_binary())
        .env("GODOT_VRM_ADAPTER_DIR", &project_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn shim");

    let mut stdin = child.stdin.take().expect("shim stdin");
    let mut stdout = child.stdout.take().expect("shim stdout");

    let garbage = b"not json at all }}}";
    stdin.write_all(&frame(garbage)).expect("write");
    stdin.flush().expect("flush");

    let body = read_framed(&mut stdout);
    let parsed: serde_json::Value = serde_json::from_slice(&body)
        .expect("parse response JSON");

    let code = parsed["error"]["code"].as_i64()
        .unwrap_or_else(|| panic!("missing error.code in {parsed}"));
    assert_eq!(code, -32700, "expected -32700 parse error, got {parsed}");
    assert!(parsed["id"].is_null(), "expected null id on parse error, got {parsed}");

    drop(stdin);
    let _ = child.wait();
}
