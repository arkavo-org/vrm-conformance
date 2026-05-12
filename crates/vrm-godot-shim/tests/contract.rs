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
fn phase1_ops_render_a_real_vrm() {
    use std::path::Path;

    let project_dir = workspace_root().join("adapters").join("godot-vrm");
    assert!(project_dir.join("project.godot").is_file());

    // Generate a sample VRM the test will load.
    let tmp = tempfile::tempdir().expect("tempdir");
    let asset_dir = tmp.path();
    let status = std::process::Command::new("cargo")
        .arg("run").arg("--release").arg("-q")
        .arg("-p").arg("vrm-asset-generator").arg("--")
        .arg("emit-default")
        .arg("--id").arg("contract_l3")
        .arg("--output-dir").arg(asset_dir)
        .current_dir(workspace_root())
        .status().expect("emit-default");
    assert!(status.success(), "emit-default failed");
    let vrm_path = asset_dir.join("contract_l3.vrm");
    assert!(vrm_path.exists(), "VRM not generated");
    let out_png = asset_dir.join("contract_l3.png");

    let mut child = Command::new(shim_binary())
        .env("GODOT_VRM_ADAPTER_DIR", &project_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn shim");

    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();

    let calls: Vec<(i64, &str, serde_json::Value)> = vec![
        (1, "load_vrm", serde_json::json!({"path": vrm_path.to_string_lossy()})),
        (2, "set_camera", serde_json::json!({"position":[0,1.4,1.5],"target":[0,1.4,0],"up":[0,1,0],"fov_degrees":30})),
        (3, "set_lighting", serde_json::json!({"directional":{"dir":[-0.3,-0.6,-0.7],"color":[1,1,1],"intensity":1},"ambient":{"color":[0.5,0.5,0.5],"intensity":0.3},"cast_shadows":false,"receive_shadows":false})),
        (4, "set_post_processing", serde_json::json!({"tone_mapping":"None","exposure":1.0})),
        (5, "render", serde_json::json!({"width":1024,"height":1024,"output_path":out_png.to_string_lossy(),"color_space":"Srgb","msaa":4,"output_type":"Color"})),
        (6, "dispose", serde_json::json!({})),
    ];

    for (id, method, params) in &calls {
        let req = serde_json::json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}).to_string();
        stdin.write_all(&frame(req.as_bytes())).unwrap();
        stdin.flush().unwrap();
        let body = read_framed(&mut stdout);
        let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(parsed["error"].is_null(), "{method} failed: {parsed}");
        let resp_id = parsed["id"].as_i64().expect("integer id");
        assert_eq!(resp_id, *id, "id mismatch for {method}");
    }

    drop(stdin);
    assert!(child.wait().unwrap().success());

    // Validate the rendered PNG.
    let png = std::fs::read(&out_png).expect("read PNG");
    let png_len = png.len();
    assert!(png_len > 5000, "PNG too small: {png_len} bytes");
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "bad PNG magic");
    let width = u32::from_be_bytes([png[16], png[17], png[18], png[19]]);
    let height = u32::from_be_bytes([png[20], png[21], png[22], png[23]]);
    assert_eq!((width, height), (1024, 1024), "unexpected dimensions");
    let _ = out_png; let _ = Path::new(asset_dir);
}

#[test]
#[ignore]
fn reserved_ops_still_return_unimplemented() {
    let project_dir = workspace_root().join("adapters").join("godot-vrm");
    let mut child = Command::new(shim_binary())
        .env("GODOT_VRM_ADAPTER_DIR", &project_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn shim");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();

    let cases: Vec<(i64, &str, i64, Option<&str>)> = vec![
        (1, "definitely_not_a_method", -32601, None),
        (2, "set_humanoid_pose", -32000, Some("Phase 2")),
        (3, "set_environment", -32000, Some("v1.x")),
        (4, "set_expression", -32000, Some("Phase 3")),
    ];
    for (id, method, code, phase) in &cases {
        let req = format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"{method}","params":{{}}}}"#);
        stdin.write_all(&frame(req.as_bytes())).unwrap();
        stdin.flush().unwrap();
        let body = read_framed(&mut stdout);
        let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(parsed["error"]["code"].as_i64(), Some(*code), "method {method} expected code {code}, got {parsed}");
        if let Some(p) = phase {
            assert_eq!(parsed["error"]["data"]["phase"].as_str(), Some(*p), "phase mismatch for {method}: {parsed}");
        }
    }
    drop(stdin);
    let _ = child.wait();
}

#[test]
#[ignore]
fn springbone_physics_ops_render_a_real_vrm() {
    let project_dir = workspace_root().join("adapters").join("godot-vrm");
    let tmp = tempfile::tempdir().expect("tempdir");
    let asset_dir = tmp.path();
    let status = std::process::Command::new("cargo")
        .arg("run").arg("--release").arg("-q")
        .arg("-p").arg("vrm-asset-generator").arg("--")
        .arg("emit-springbone")
        .arg("--id").arg("contract_l4")
        .arg("--output-dir").arg(asset_dir)
        .current_dir(workspace_root())
        .status().expect("emit-springbone");
    assert!(status.success(), "emit-springbone failed");
    let vrm_path = asset_dir.join("contract_l4.vrm");
    let out_png = asset_dir.join("contract_l4.png");

    let mut child = Command::new(shim_binary())
        .env("GODOT_VRM_ADAPTER_DIR", &project_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn shim");
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();

    let calls: Vec<(i64, &str, serde_json::Value)> = vec![
        (1, "load_vrm", serde_json::json!({"path": vrm_path.to_string_lossy()})),
        (2, "set_camera", serde_json::json!({"position":[0,1.4,1.5],"target":[0,1.4,0],"up":[0,1,0],"fov_degrees":30})),
        (3, "set_lighting", serde_json::json!({"directional":{"dir":[-0.3,-0.6,-0.7],"color":[1,1,1],"intensity":1},"ambient":{"color":[0.5,0.5,0.5],"intensity":0.3},"cast_shadows":false,"receive_shadows":false})),
        (4, "set_post_processing", serde_json::json!({"tone_mapping":"None","exposure":1.0})),
        (5, "reset_physics", serde_json::json!({"settle_steps":30})),
        (6, "animate_root_transform", serde_json::json!({"translation_start":[0,0,0],"translation_end":[0.15,0,0],"duration_seconds":0.25,"fps":60})),
        (7, "step_physics", serde_json::json!({"dt_seconds":1.0/60.0,"count":5})),
        (8, "render", serde_json::json!({"width":1024,"height":1024,"output_path":out_png.to_string_lossy(),"color_space":"Srgb","msaa":4,"output_type":"Color"})),
        (9, "dispose", serde_json::json!({})),
    ];
    for (id, method, params) in &calls {
        let req = serde_json::json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}).to_string();
        stdin.write_all(&frame(req.as_bytes())).unwrap();
        stdin.flush().unwrap();
        let body = read_framed(&mut stdout);
        let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(parsed["error"].is_null(), "{method} failed: {parsed}");
        let resp_id = parsed["id"].as_i64().expect("integer id");
        assert_eq!(resp_id, *id, "id mismatch for {method}");
    }
    drop(stdin);
    assert!(child.wait().unwrap().success());

    let png = std::fs::read(&out_png).expect("read PNG");
    let png_len = png.len();
    assert!(png_len > 5000, "PNG too small: {png_len} bytes");
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "bad PNG magic");
    let width = u32::from_be_bytes([png[16], png[17], png[18], png[19]]);
    let height = u32::from_be_bytes([png[20], png[21], png[22], png[23]]);
    assert_eq!((width, height), (1024, 1024));
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
