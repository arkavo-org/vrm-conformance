//! vrm-godot-shim binary entry. Spawns Godot, accepts its TCP connection,
//! then loops forwarding framed stdio requests to/from the TCP socket
//! until stdin closes.

use std::env;
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use vrm_godot_shim::bridge::forward_one;
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
