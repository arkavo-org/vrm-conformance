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
