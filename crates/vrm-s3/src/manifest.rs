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
        }
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
