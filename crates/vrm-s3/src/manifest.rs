use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub version: u32,
    pub entries: Vec<ManifestEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub test_id: String,
    pub renderer_name: String,
    pub renderer_version: String,
    pub git_hash: String,

    #[serde(flatten)]
    pub metadata: SubmissionMetadata,

    pub image_url: String,
    pub image_blake3: String,
    pub byte_size: u64,
    pub submitted_at: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub positions_url: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub positions_blake3: Option<String>,
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
            renderer_name: renderer_name.into(),
            renderer_version: "0.1.0".into(),
            git_hash: "deadbeef".into(),
            metadata: sample_metadata(),
            image_url: format!("s3://b/{test_id}_{renderer_name}.png"),
            image_blake3: image_blake3.into(),
            byte_size: 100,
            submitted_at: "2026-05-10T12:00:00Z".into(),
            positions_url: None,
            positions_blake3: None,
        }
    }

    #[test]
    fn entry_with_positions_roundtrips() {
        let e = ManifestEntry {
            test_id: "springbone_default".into(),
            renderer_name: "three-vrm".into(),
            renderer_version: "0.1.0".into(),
            git_hash: "deadbeef".into(),
            metadata: sample_metadata(),
            image_url: "s3://b/x.png".into(),
            image_blake3: "blake3:aaa".into(),
            byte_size: 100,
            submitted_at: "2026-05-15T12:00:00Z".into(),
            positions_url: Some("s3://b/x.positions.json".into()),
            positions_blake3: Some("blake3:bbb".into()),
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
            renderer_name: "r".into(),
            renderer_version: "v".into(),
            git_hash: "g".into(),
            metadata: sample_metadata(),
            image_url: "s3://b/x.png".into(),
            image_blake3: "blake3:aaa".into(),
            byte_size: 1,
            submitted_at: "2026-05-15T12:00:00Z".into(),
            positions_url: None,
            positions_blake3: None,
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
        assert_eq!(m.entries[0].image_blake3, "blake3:ccc");
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
}
