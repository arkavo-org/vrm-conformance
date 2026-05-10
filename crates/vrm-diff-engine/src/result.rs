use crate::property::PropertyResult;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffResult {
    pub test_id: String,
    pub renderer: String,
    pub reference_renderer: String,

    pub ssim: f32,
    pub ssim_threshold: f32,
    pub ssim_passed: bool,

    #[serde(default)]
    pub properties: Vec<PropertyResult>,
}

impl DiffResult {
    pub fn overall_passed(&self) -> bool {
        self.ssim_passed && self.properties.iter().all(|p| p.passed)
    }
}
