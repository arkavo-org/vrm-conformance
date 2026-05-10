//! Integration tests for the validator wrapper. Require the validator shim
//! installed via `scripts/install-validator.sh`. If the shim isn't present,
//! tests are skipped with a printed warning rather than failing.

use vrm_validator_wrap::{validate, ValidatorConfig};

fn config_or_skip() -> Option<ValidatorConfig> {
    match ValidatorConfig::from_env() {
        Ok(c) => Some(c),
        Err(e) => {
            eprintln!(
                "SKIP: validator shim not installed ({e}). Run scripts/install-validator.sh."
            );
            None
        }
    }
}

#[test]
fn validate_returns_error_for_nonexistent_file() {
    let Some(config) = config_or_skip() else {
        return;
    };
    let result = validate(&config, camino::Utf8Path::new("/nonexistent/file.vrm"));
    assert!(result.is_err(), "validation of missing file should error");
}

#[test]
fn validate_minimal_glb_returns_clean_report() {
    let Some(config) = config_or_skip() else {
        return;
    };

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("minimal.glb");
    let path = camino::Utf8PathBuf::from_path_buf(path).unwrap();

    let glb_bytes = build_minimal_glb();
    std::fs::write(&path, &glb_bytes).unwrap();

    let report = validate(&config, &path).expect("validator should produce a report");
    assert_eq!(
        report.issues.num_errors, 0,
        "minimal valid GLB should have no errors"
    );
    assert_eq!(
        report.mime_type.as_deref(),
        Some("model/gltf-binary"),
        "validator should detect GLB binary"
    );
}

#[test]
fn validate_garbage_input_produces_internal_error() {
    let Some(config) = config_or_skip() else {
        return;
    };

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("garbage.glb");
    let path = camino::Utf8PathBuf::from_path_buf(path).unwrap();
    std::fs::write(&path, b"not a vrm file at all").unwrap();

    // Validator's exit code 3 = internal failure (couldn't parse as glTF).
    // Our wrapper surfaces that as ValidatorError::Internal, not a successful
    // empty report.
    let err = validate(&config, &path).expect_err("garbage input must error");
    let s = err.to_string();
    assert!(
        s.to_lowercase().contains("internal") || s.to_lowercase().contains("could not"),
        "expected internal-error variant, got: {s}"
    );
}

fn build_minimal_glb() -> Vec<u8> {
    let json = br#"{"asset":{"version":"2.0"}}"#;
    let json_padded_len = (json.len() + 3) & !3;
    let mut json_chunk = json.to_vec();
    json_chunk.resize(json_padded_len, b' ');

    let total_len = 12 + 8 + json_padded_len;

    let mut out = Vec::with_capacity(total_len);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total_len as u32).to_le_bytes());
    out.extend_from_slice(&(json_padded_len as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&json_chunk);
    out
}
