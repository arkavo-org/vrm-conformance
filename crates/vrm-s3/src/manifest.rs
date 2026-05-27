use serde::{Deserialize, Serialize};
use vrm_ops::SpecVersion;

/// Kind of manifest entry. `Image` is the default to preserve back-compat
/// with existing manifest files written before sequence support landed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ManifestEntryKind {
    #[default]
    Image,
    Sequence,
}

/// Sequence manifest block. Present when `ManifestEntry::kind == Sequence`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceManifest {
    pub frame_count: u32,
    pub frame_hz: f32,
    pub frames: Vec<SequenceManifestFrame>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub muxed_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub muxed_blake3: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceManifestFrame {
    pub index: u32,
    pub image_url: String,
    pub blake3: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub version: u32,
    pub entries: Vec<ManifestEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub test_id: String,
    #[serde(default = "default_spec_version_v1")]
    pub spec_version: SpecVersion,
    pub renderer_name: String,
    pub renderer_version: String,
    pub git_hash: String,

    #[serde(flatten)]
    pub metadata: SubmissionMetadata,

    /// Defaults to Image for back-compat with existing flat entries
    /// written before sequence support landed.
    #[serde(default)]
    pub kind: ManifestEntryKind,

    // Image-kind fields (required for kind=Image; absent for kind=Sequence).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_blake3: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_size: Option<u64>,

    pub submitted_at: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub positions_url: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub positions_blake3: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vrma_url: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vrma_blake3: Option<String>,

    // Sequence-kind fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sequence: Option<SequenceManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmissionMetadata {
    pub os: String,
    pub os_version: String,
    pub gpu_vendor: String,
    pub gpu_model: String,
    pub driver_version: String,
    pub build_flags: String,
}

fn default_spec_version_v1() -> SpecVersion {
    SpecVersion::V1
}

impl Manifest {
    pub fn empty() -> Self {
        Self {
            version: 1,
            entries: Vec::new(),
        }
    }

    pub fn upsert(&mut self, entry: ManifestEntry) {
        if let Some(existing) = self
            .entries
            .iter_mut()
            .find(|e| e.test_id == entry.test_id && e.renderer_name == entry.renderer_name)
        {
            *existing = entry;
        } else {
            self.entries.push(entry);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_metadata() -> SubmissionMetadata {
        SubmissionMetadata {
            os: "macos".into(),
            os_version: "14.4.1".into(),
            gpu_vendor: "Apple".into(),
            gpu_model: "M2 Pro".into(),
            driver_version: "Metal 3".into(),
            build_flags: "release".into(),
        }
    }

    fn entry(test_id: &str, renderer_name: &str, image_blake3: &str) -> ManifestEntry {
        ManifestEntry {
            test_id: test_id.into(),
            spec_version: SpecVersion::V1,
            renderer_name: renderer_name.into(),
            renderer_version: "0.1.0".into(),
            git_hash: "deadbeef".into(),
            metadata: sample_metadata(),
            kind: ManifestEntryKind::Image,
            image_url: Some(format!("s3://b/{test_id}_{renderer_name}.png")),
            image_blake3: Some(image_blake3.into()),
            byte_size: Some(100),
            submitted_at: "2026-05-10T12:00:00Z".into(),
            positions_url: None,
            positions_blake3: None,
            vrma_url: None,
            vrma_blake3: None,
            sequence: None,
        }
    }

    #[test]
    fn entry_with_positions_roundtrips() {
        let e = ManifestEntry {
            test_id: "springbone_default".into(),
            spec_version: SpecVersion::V1,
            renderer_name: "three-vrm".into(),
            renderer_version: "0.1.0".into(),
            git_hash: "deadbeef".into(),
            metadata: sample_metadata(),
            kind: ManifestEntryKind::Image,
            image_url: Some("s3://b/x.png".into()),
            image_blake3: Some("blake3:aaa".into()),
            byte_size: Some(100),
            submitted_at: "2026-05-15T12:00:00Z".into(),
            positions_url: Some("s3://b/x.positions.json".into()),
            positions_blake3: Some("blake3:bbb".into()),
            vrma_url: None,
            vrma_blake3: None,
            sequence: None,
        };
        let s = serde_json::to_string(&e).unwrap();
        let back: ManifestEntry = serde_json::from_str(&s).unwrap();
        assert_eq!(
            back.positions_url.as_deref(),
            Some("s3://b/x.positions.json")
        );
        assert_eq!(back.positions_blake3.as_deref(), Some("blake3:bbb"));
    }

    #[test]
    fn entry_without_positions_omits_fields_from_json() {
        let e = ManifestEntry {
            test_id: "t".into(),
            spec_version: SpecVersion::V1,
            renderer_name: "r".into(),
            renderer_version: "v".into(),
            git_hash: "g".into(),
            metadata: sample_metadata(),
            kind: ManifestEntryKind::Image,
            image_url: Some("s3://b/x.png".into()),
            image_blake3: Some("blake3:aaa".into()),
            byte_size: Some(1),
            submitted_at: "2026-05-15T12:00:00Z".into(),
            positions_url: None,
            positions_blake3: None,
            vrma_url: None,
            vrma_blake3: None,
            sequence: None,
        };
        let v: serde_json::Value = serde_json::to_value(&e).unwrap();
        assert!(
            v.get("positions_url").is_none(),
            "None positions_url must be omitted, got {v}"
        );
        assert!(
            v.get("positions_blake3").is_none(),
            "None positions_blake3 must be omitted, got {v}"
        );
    }

    #[test]
    fn entry_existing_json_without_positions_parses() {
        // Backward compat: entries from before this change have no positions
        // fields. Must deserialize cleanly.
        let raw = r#"{
            "test_id": "old",
            "renderer_name": "three-vrm",
            "renderer_version": "0.1.0",
            "git_hash": "deadbeef",
            "os": "macos", "os_version": "14",
            "gpu_vendor": "Apple", "gpu_model": "M2",
            "driver_version": "M3", "build_flags": "rel",
            "image_url": "s3://b/x.png",
            "image_blake3": "blake3:aaa",
            "byte_size": 1,
            "submitted_at": "2026-05-10T12:00:00Z"
        }"#;
        let e: ManifestEntry = serde_json::from_str(raw).unwrap();
        assert!(e.positions_url.is_none());
        assert!(e.positions_blake3.is_none());
    }

    #[test]
    fn upsert_inserts_when_absent() {
        let mut m = Manifest::empty();
        assert_eq!(m.entries.len(), 0);
        m.upsert(entry("t1", "r1", "blake3:aaa"));
        assert_eq!(m.entries.len(), 1);
        assert_eq!(m.entries[0].test_id, "t1");
        assert_eq!(m.entries[0].renderer_name, "r1");
    }

    #[test]
    fn upsert_replaces_in_place_on_match() {
        let mut m = Manifest::empty();
        m.upsert(entry("t1", "r1", "blake3:aaa"));
        m.upsert(entry("t2", "r1", "blake3:bbb"));
        assert_eq!(m.entries.len(), 2);

        // Replace t1/r1 with new hash, expect same length, value updated, order preserved.
        m.upsert(entry("t1", "r1", "blake3:ccc"));
        assert_eq!(m.entries.len(), 2);
        assert_eq!(m.entries[0].test_id, "t1");
        assert_eq!(m.entries[0].renderer_name, "r1");
        assert_eq!(m.entries[0].image_blake3.as_deref(), Some("blake3:ccc"));
        assert_eq!(m.entries[1].test_id, "t2");
    }

    #[test]
    fn upsert_is_idempotent() {
        let mut m = Manifest::empty();
        let e = entry("t1", "r1", "blake3:aaa");
        m.upsert(e.clone());
        m.upsert(e.clone());
        assert_eq!(m.entries.len(), 1);
        assert_eq!(m.entries[0].test_id, e.test_id);
        assert_eq!(m.entries[0].renderer_name, e.renderer_name);
        assert_eq!(m.entries[0].image_blake3, e.image_blake3);
        assert_eq!(m.entries[0].byte_size, e.byte_size);
        assert_eq!(m.entries[0].submitted_at, e.submitted_at);
    }

    #[test]
    fn manifest_entry_roundtrips_vrma_url() {
        let e = ManifestEntry {
            test_id: "vrma_humanoid_x".into(),
            spec_version: SpecVersion::V1,
            renderer_name: "univrm".into(),
            renderer_version: "v0.131.0".into(),
            git_hash: "abc".into(),
            metadata: sample_metadata(),
            kind: ManifestEntryKind::Image,
            image_url: Some("s3://b/x.png".into()),
            image_blake3: Some("blake3:img".into()),
            byte_size: Some(1024),
            submitted_at: "2026-05-10T12:00:00Z".into(),
            positions_url: None,
            positions_blake3: None,
            vrma_url: Some("s3://b/x.vrma".into()),
            vrma_blake3: Some("blake3:vrma".into()),
            sequence: None,
        };
        let json = serde_json::to_string(&e).unwrap();
        let back: ManifestEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.vrma_url.as_deref(), Some("s3://b/x.vrma"));
        assert_eq!(back.vrma_blake3.as_deref(), Some("blake3:vrma"));
        assert!(json.contains(r#""vrma_url":"s3://b/x.vrma""#));
    }

    #[test]
    fn manifest_entry_omits_vrma_fields_when_none() {
        let e = ManifestEntry {
            test_id: "mtoon_default".into(),
            spec_version: SpecVersion::V1,
            renderer_name: "univrm".into(),
            renderer_version: "v0.131.0".into(),
            git_hash: "abc".into(),
            metadata: sample_metadata(),
            kind: ManifestEntryKind::Image,
            image_url: Some("s3://b/x.png".into()),
            image_blake3: Some("blake3:img".into()),
            byte_size: Some(1024),
            submitted_at: "2026-05-10T12:00:00Z".into(),
            positions_url: None,
            positions_blake3: None,
            vrma_url: None,
            vrma_blake3: None,
            sequence: None,
        };
        let v: serde_json::Value = serde_json::to_value(&e).unwrap();
        assert!(
            v.get("vrma_url").is_none(),
            "vrma_url None must be omitted, got {v}"
        );
        assert!(
            v.get("vrma_blake3").is_none(),
            "vrma_blake3 None must be omitted, got {v}"
        );
    }

    #[test]
    fn image_kind_default_when_field_absent() {
        // Existing on-disk entries don't have "kind": "image". Deserialization
        // must default to ManifestEntryKind::Image.
        let raw = r#"{
            "test_id": "x",
            "renderer_name": "vmk",
            "renderer_version": "0.15.2",
            "git_hash": "abcdef1",
            "os": "macos", "os_version": "14", "gpu_vendor": "Apple",
            "gpu_model": "M2", "driver_version": "Metal 3", "build_flags": "release",
            "image_url": "s3://bucket/x.png",
            "image_blake3": "blake3:abc",
            "byte_size": 1024,
            "submitted_at": "2026-01-01T00:00:00Z"
        }"#;
        let e: ManifestEntry = serde_json::from_str(raw).unwrap();
        assert_eq!(e.kind, ManifestEntryKind::Image);
        assert_eq!(e.image_url.as_deref(), Some("s3://bucket/x.png"));
        assert!(e.sequence.is_none());
    }

    #[test]
    fn sequence_kind_roundtrips() {
        let raw = r#"{
            "test_id": "swing_seq",
            "renderer_name": "vmk",
            "renderer_version": "0.15.2",
            "git_hash": "abcdef1",
            "os": "macos", "os_version": "14", "gpu_vendor": "Apple",
            "gpu_model": "M2", "driver_version": "Metal 3", "build_flags": "release",
            "kind": "sequence",
            "submitted_at": "2026-01-01T00:00:00Z",
            "sequence": {
                "frame_count": 2,
                "frame_hz": 30.0,
                "frames": [
                    {"index": 0, "image_url": "s3://b/0000.png", "blake3": "blake3:aaa"},
                    {"index": 1, "image_url": "s3://b/0001.png", "blake3": "blake3:bbb"}
                ]
            }
        }"#;
        let e: ManifestEntry = serde_json::from_str(raw).unwrap();
        assert_eq!(e.kind, ManifestEntryKind::Sequence);
        assert!(e.image_url.is_none());
        assert!(e.image_blake3.is_none());
        assert!(e.byte_size.is_none());
        {
            let seq = e.sequence.as_ref().expect("sequence block required");
            assert_eq!(seq.frame_count, 2);
            assert_eq!(seq.frames.len(), 2);
            assert_eq!(seq.frames[1].image_url, "s3://b/0001.png");
        }

        // Round-trip
        let serialized = serde_json::to_string(&e).unwrap();
        let back: ManifestEntry = serde_json::from_str(&serialized).unwrap();
        assert_eq!(back.kind, ManifestEntryKind::Sequence);
        assert_eq!(back.sequence.as_ref().unwrap().frames.len(), 2);
        // Top-level image_url / image_blake3 must be absent (skip_serializing_if None).
        // Note: "image_url" also appears inside the sequence.frames array, so we
        // check the parsed Value rather than the raw string for the top-level key.
        let v: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert!(
            v.get("image_url").is_none(),
            "top-level image_url must be omitted for Sequence entries, got {v}"
        );
        assert!(
            v.get("image_blake3").is_none(),
            "top-level image_blake3 must be omitted for Sequence entries, got {v}"
        );
    }
}

#[cfg(test)]
mod spec_version_tests {
    use super::*;
    use vrm_ops::SpecVersion;

    /// Build a minimal ManifestEntry JSON string with the given spec_version block
    /// (e.g. `"spec_version": "0.x",` or `""` for absent).
    /// SubmissionMetadata is flattened, so all its required fields appear at top level.
    fn minimal_entry_json(spec_version_block: &str) -> String {
        format!(
            r#"{{
  "test_id": "t",
  "renderer_name": "r",
  "renderer_version": "0",
  "git_hash": "abc",
  "os": "linux",
  "os_version": "22.04",
  "gpu_vendor": "NVIDIA",
  "gpu_model": "RTX 4090",
  "driver_version": "545.0",
  "build_flags": "release",
  {spec_version_block}
  "image_url": "s3://b/r/t.png",
  "image_blake3": "blake3:aaa",
  "submitted_at": "2026-05-26T00:00:00Z"
}}"#
        )
    }

    #[test]
    fn parses_spec_version_v0() {
        let json = minimal_entry_json(r#""spec_version": "0.x","#);
        let e: ManifestEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(e.spec_version, SpecVersion::V0);
    }

    #[test]
    fn parses_spec_version_v1() {
        let json = minimal_entry_json(r#""spec_version": "1.0","#);
        let e: ManifestEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(e.spec_version, SpecVersion::V1);
    }

    #[test]
    fn defaults_to_v1_when_absent() {
        let json = minimal_entry_json("");
        let e: ManifestEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(e.spec_version, SpecVersion::V1);
    }
}
