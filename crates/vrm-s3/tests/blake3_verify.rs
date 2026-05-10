use vrm_s3::push_pull::verify_blake3;

#[test]
fn matching_hash_returns_ok() {
    let bytes = b"hello world";
    let hash = blake3::hash(bytes);
    let expected = format!("blake3:{}", hash.to_hex());
    verify_blake3(bytes, &expected).expect("matching hash should verify");
}

#[test]
fn mismatching_hash_errors() {
    let bytes = b"hello world";
    let wrong = "blake3:0000000000000000000000000000000000000000000000000000000000000000";
    let err = verify_blake3(bytes, wrong).expect_err("mismatched hash must error");
    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("hash mismatch") || msg.contains("blake3"),
        "expected mismatch/blake3 in error, got: {msg}"
    );
}

#[test]
fn malformed_prefix_errors() {
    let bytes = b"x";
    let err = verify_blake3(bytes, "sha256:abc").expect_err("non-blake3 prefix must error");
    assert!(err.to_string().contains("blake3:"), "got: {err}");
}

#[test]
fn missing_prefix_errors() {
    let bytes = b"x";
    let err = verify_blake3(bytes, "abc").expect_err("missing prefix must error");
    assert!(err.to_string().contains("blake3:"), "got: {err}");
}
