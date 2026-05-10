//! Subprocess-based contract test: spawn the mock binary, feed framed
//! JSON-RPC requests, assert framed responses. Mirrors the runner's
//! production usage of the adapter.

use std::io::{BufReader, Write};
use std::process::{Command, Stdio};
use vrm_ops::stdio::{read_message, write_message};

fn spawn_mock() -> std::process::Child {
    let bin = env!("CARGO_BIN_EXE_vrm-mock-renderer");
    Command::new(bin)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn mock")
}

fn rpc(
    stdin: &mut std::process::ChildStdin,
    stdout: &mut BufReader<std::process::ChildStdout>,
    id: u64,
    method: &str,
    params: serde_json::Value,
) -> serde_json::Value {
    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    });
    let body = serde_json::to_vec(&req).unwrap();
    write_message(stdin, &body).unwrap();
    stdin.flush().unwrap();
    let resp_bytes = read_message(stdout).unwrap();
    serde_json::from_slice(&resp_bytes).unwrap()
}

#[test]
fn full_session_load_render_dispose() {
    use vrm_asset_generator::params::MToonParams;

    // Build a tiny synthetic VRM + sidecar in a tempdir. The mock only
    // needs the meta.json (it ignores the .vrm body), so a stub is fine.
    let dir = tempfile::tempdir().unwrap();
    let vrm_path = dir.path().join("synthetic.vrm");
    let meta_path = dir.path().join("synthetic.meta.json");
    let out_path = dir.path().join("out.png");

    std::fs::write(&vrm_path, b"glTF\x02\x00\x00\x00\x0c\x00\x00\x00").unwrap();
    let meta = serde_json::json!({
        "id": "synthetic",
        "license": "CC0-1.0",
        "params": MToonParams::defaults("synthetic"),
    });
    std::fs::write(&meta_path, serde_json::to_vec(&meta).unwrap()).unwrap();

    let mut child = spawn_mock();
    let mut stdin = child.stdin.take().expect("stdin piped");
    let mut stdout = BufReader::new(child.stdout.take().expect("stdout piped"));

    // load_vrm
    let resp = rpc(
        &mut stdin,
        &mut stdout,
        1,
        "load_vrm",
        serde_json::json!({ "path": vrm_path.to_string_lossy() }),
    );
    let session_id = resp["result"]["session_id"]
        .as_str()
        .expect("load_vrm returns session_id")
        .to_string();
    assert!(session_id.starts_with("mock-"), "got: {session_id}");

    // set_camera (any-shape; mock just stores it)
    let resp = rpc(
        &mut stdin,
        &mut stdout,
        2,
        "set_camera",
        serde_json::json!({
            "session_id": session_id,
            "position": [0.0, 1.4, 1.5],
            "target": [0.0, 1.4, 0.0],
            "up": [0.0, 1.0, 0.0],
            "fov_degrees": 30.0,
        }),
    );
    assert!(resp.get("result").is_some(), "set_camera ok: {resp}");

    // render
    let resp = rpc(
        &mut stdin,
        &mut stdout,
        3,
        "render",
        serde_json::json!({
            "session_id": session_id,
            "width": 64,
            "height": 64,
            "output_path": out_path.to_string_lossy(),
            "color_space": "Linear",
            "msaa": 4,
            "output_type": "Color",
        }),
    );
    assert!(resp.get("result").is_some(), "render ok: {resp:#?}");
    assert!(out_path.exists(), "render must produce a PNG file");

    // The synthesized PNG should be 64×64 RGB.
    let img = image::open(&out_path).unwrap().to_rgb8();
    assert_eq!(img.dimensions(), (64, 64));

    // dispose
    let resp = rpc(
        &mut stdin,
        &mut stdout,
        4,
        "dispose",
        serde_json::json!({ "session_id": session_id }),
    );
    assert!(resp.get("result").is_some());

    // Close stdin so the loop exits cleanly.
    drop(stdin);
    let _ = child.wait();
}

#[test]
fn unknown_method_returns_minus_32601() {
    let mut child = spawn_mock();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    let resp = rpc(
        &mut stdin,
        &mut stdout,
        99,
        "nonexistent_op",
        serde_json::json!({}),
    );
    assert_eq!(resp["error"]["code"], -32601);

    drop(stdin);
    let _ = child.wait();
}

#[test]
fn reserved_phase_2_op_returns_unimplemented_phase_2() {
    let mut child = spawn_mock();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    let resp = rpc(
        &mut stdin,
        &mut stdout,
        1,
        "step_physics",
        serde_json::json!({ "dt_seconds": 0.016, "count": 1 }),
    );
    assert_eq!(resp["error"]["code"], -32000);
    assert_eq!(resp["error"]["data"]["phase"], "Phase 2");

    drop(stdin);
    let _ = child.wait();
}

#[test]
fn load_vrm_missing_meta_returns_load_failed() {
    let dir = tempfile::tempdir().unwrap();
    let vrm_path = dir.path().join("nomerge.vrm");
    std::fs::write(&vrm_path, b"glTF").unwrap();
    // intentionally no .meta.json sidecar

    let mut child = spawn_mock();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    let resp = rpc(
        &mut stdin,
        &mut stdout,
        1,
        "load_vrm",
        serde_json::json!({ "path": vrm_path.to_string_lossy() }),
    );
    assert_eq!(
        resp["error"]["code"], -32001,
        "expect LoadFailed: {resp:#?}"
    );

    drop(stdin);
    let _ = child.wait();
}
