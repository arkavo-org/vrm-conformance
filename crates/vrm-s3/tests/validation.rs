use vrm_s3::manifest::{
    ManifestEntry, ManifestEntryKind, SequenceManifest, SequenceManifestFrame, SubmissionMetadata,
};
use vrm_s3::validation::validate_entries;

fn meta() -> SubmissionMetadata {
    SubmissionMetadata {
        os: "macos".into(),
        os_version: "14".into(),
        gpu_vendor: "Apple".into(),
        gpu_model: "M2".into(),
        driver_version: "Metal 3".into(),
        build_flags: "release".into(),
    }
}

fn good_image_entry() -> ManifestEntry {
    ManifestEntry {
        test_id: "img_ok".into(),
        renderer_name: "vmk".into(),
        renderer_version: "0.15.2".into(),
        git_hash: "abcdef1".into(),
        metadata: meta(),
        kind: ManifestEntryKind::Image,
        image_url: Some("s3://bucket/x.png".into()),
        image_blake3: Some(format!("blake3:{}", "a".repeat(64))),
        byte_size: Some(1024),
        submitted_at: "2026-01-01T00:00:00Z".into(),
        positions_url: None,
        positions_blake3: None,
        vrma_url: None,
        vrma_blake3: None,
        sequence: None,
    }
}

fn good_sequence_entry() -> ManifestEntry {
    ManifestEntry {
        test_id: "seq_ok".into(),
        renderer_name: "vmk".into(),
        renderer_version: "0.15.2".into(),
        git_hash: "abcdef1".into(),
        metadata: meta(),
        kind: ManifestEntryKind::Sequence,
        image_url: None,
        image_blake3: None,
        byte_size: None,
        submitted_at: "2026-01-01T00:00:00Z".into(),
        positions_url: None,
        positions_blake3: None,
        vrma_url: None,
        vrma_blake3: None,
        sequence: Some(SequenceManifest {
            frame_count: 2,
            frame_hz: 30.0,
            frames: vec![
                SequenceManifestFrame {
                    index: 0,
                    image_url: "s3://bucket/0000.png".into(),
                    blake3: format!("blake3:{}", "a".repeat(64)),
                },
                SequenceManifestFrame {
                    index: 1,
                    image_url: "s3://bucket/0001.png".into(),
                    blake3: format!("blake3:{}", "b".repeat(64)),
                },
            ],
            muxed_url: None,
            muxed_blake3: None,
        }),
    }
}

#[test]
fn valid_image_entry_passes() {
    let errs = validate_entries(&[good_image_entry()]);
    assert!(errs.is_empty(), "{errs:?}");
}

#[test]
fn valid_sequence_entry_passes() {
    let errs = validate_entries(&[good_sequence_entry()]);
    assert!(errs.is_empty(), "{errs:?}");
}

#[test]
fn sequence_with_image_url_at_top_level_fails() {
    let mut e = good_sequence_entry();
    e.image_url = Some("s3://bucket/should_not_be_here.png".into());
    let errs = validate_entries(&[e]);
    assert!(
        errs.iter().any(|s| s.contains("image_url is set")),
        "{errs:?}",
    );
}

#[test]
fn sequence_with_empty_frames_fails() {
    let mut e = good_sequence_entry();
    e.sequence.as_mut().unwrap().frames.clear();
    e.sequence.as_mut().unwrap().frame_count = 0;
    let errs = validate_entries(&[e]);
    assert!(errs.iter().any(|s| s.contains("0 frames")), "{errs:?}",);
}

#[test]
fn sequence_frame_count_mismatch_fails() {
    let mut e = good_sequence_entry();
    e.sequence.as_mut().unwrap().frame_count = 5; // frames has 2
    let errs = validate_entries(&[e]);
    assert!(errs.iter().any(|s| s.contains("frame_count")), "{errs:?}",);
}

#[test]
fn sequence_frame_index_mismatch_fails() {
    let mut e = good_sequence_entry();
    e.sequence.as_mut().unwrap().frames[1].index = 99;
    let errs = validate_entries(&[e]);
    assert!(errs.iter().any(|s| s.contains("index field")), "{errs:?}",);
}

#[test]
fn sequence_bad_image_url_fails() {
    let mut e = good_sequence_entry();
    e.sequence.as_mut().unwrap().frames[0].image_url = "http://bad".into();
    let errs = validate_entries(&[e]);
    assert!(errs.iter().any(|s| s.contains("s3://")), "{errs:?}",);
}

#[test]
fn sequence_bad_blake3_fails() {
    let mut e = good_sequence_entry();
    e.sequence.as_mut().unwrap().frames[0].blake3 = "blake3:short".into();
    let errs = validate_entries(&[e]);
    assert!(
        errs.iter().any(|s| s.contains("blake3 malformed")),
        "{errs:?}",
    );
}

#[test]
fn sequence_missing_block_fails() {
    let mut e = good_image_entry();
    e.kind = ManifestEntryKind::Sequence;
    e.image_url = None;
    e.image_blake3 = None;
    e.byte_size = None;
    e.sequence = None;
    let errs = validate_entries(&[e]);
    assert!(
        errs.iter().any(|s| s.contains("sequence block is missing")),
        "{errs:?}",
    );
}

#[test]
fn muxed_url_without_blake3_fails() {
    let mut e = good_sequence_entry();
    e.sequence.as_mut().unwrap().muxed_url = Some("s3://bucket/out.mp4".into());
    // muxed_blake3 still None
    let errs = validate_entries(&[e]);
    assert!(errs.iter().any(|s| s.contains("muxed_url")), "{errs:?}",);
}

#[test]
fn muxed_blake3_without_url_fails() {
    let mut e = good_sequence_entry();
    e.sequence.as_mut().unwrap().muxed_blake3 = Some(format!("blake3:{}", "c".repeat(64)));
    // muxed_url still None
    let errs = validate_entries(&[e]);
    assert!(
        errs.iter()
            .any(|s| s.contains("muxed_blake3 set without muxed_url")),
        "{errs:?}",
    );
}
