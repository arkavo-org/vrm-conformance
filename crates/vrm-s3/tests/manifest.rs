use vrm_s3::manifest::{Manifest, ManifestEntry, SubmissionMetadata};

#[test]
fn manifest_round_trips_json() {
    let m = Manifest {
        version: 1,
        entries: vec![ManifestEntry {
            test_id: "mtoon_default".into(),
            renderer_name: "vrm-metal-kit".into(),
            renderer_version: "0.5.2".into(),
            git_hash: "deadbeef".into(),
            metadata: SubmissionMetadata {
                os: "macos".into(),
                os_version: "14.4.1".into(),
                gpu_vendor: "Apple".into(),
                gpu_model: "M2 Pro".into(),
                driver_version: "Metal 3".into(),
                build_flags: "release".into(),
            },
            image_url: "s3://arkavo-vrm-conformance/test/mtoon_default.png".into(),
            image_blake3: "blake3:abcdef".into(),
            byte_size: 12345,
            submitted_at: "2026-05-10T12:00:00Z".into(),
        }],
    };
    let s = serde_json::to_string(&m).unwrap();
    let parsed: Manifest = serde_json::from_str(&s).unwrap();
    assert_eq!(parsed.entries.len(), 1);
    assert_eq!(parsed.entries[0].test_id, "mtoon_default");
}

#[test]
fn manifest_rejects_missing_required_fields() {
    let raw = r#"{
        "version": 1,
        "entries": [
            { "test_id": "x", "renderer_name": "r" }
        ]
    }"#;
    let result: Result<Manifest, _> = serde_json::from_str(raw);
    assert!(result.is_err(), "should reject missing required fields");
}
