//! Per-load_vrm session state. `Session` holds the parameters extracted
//! from the asset's `.meta.json` sidecar plus the most-recent camera /
//! lighting / post values the runner has set on it. `SessionRegistry`
//! owns sessions for the lifetime of the adapter process.

use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use std::collections::HashMap;
use vrm_asset_generator::params::MToonParams;

#[derive(Debug, Clone)]
pub struct Session {
    pub asset_path: Utf8PathBuf,
    pub params: MToonParams,
    pub camera: Option<vrm_ops::tools::SetCameraParams>,
    pub lighting: Option<vrm_ops::tools::SetLightingParams>,
    pub post_processing: Option<vrm_ops::tools::SetPostProcessingParams>,
}

impl Session {
    pub fn load(asset_path: &Utf8Path) -> Result<Self> {
        let meta_path = asset_path.with_extension("meta.json");
        let meta_bytes = std::fs::read(meta_path.as_std_path())
            .with_context(|| format!("read meta sidecar: {meta_path}"))?;
        // The sidecar shape is `{"id":..., "params": MToonParams, ...}`.
        // We only need params; pull it out with a small envelope.
        #[derive(serde::Deserialize)]
        struct Sidecar {
            params: MToonParams,
        }
        let sidecar: Sidecar = serde_json::from_slice(&meta_bytes)
            .with_context(|| format!("parse meta sidecar: {meta_path}"))?;
        Ok(Self {
            asset_path: asset_path.to_owned(),
            params: sidecar.params,
            camera: None,
            lighting: None,
            post_processing: None,
        })
    }
}

#[derive(Debug, Default)]
pub struct SessionRegistry {
    sessions: HashMap<String, Session>,
    next_id: u64,
}

impl SessionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, session: Session) -> String {
        self.next_id += 1;
        let id = format!("mock-{}", self.next_id);
        self.sessions.insert(id.clone(), session);
        id
    }

    pub fn get(&self, id: &str) -> Option<&Session> {
        self.sessions.get(id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Session> {
        self.sessions.get_mut(id)
    }

    pub fn remove(&mut self, id: &str) -> Option<Session> {
        self.sessions.remove(id)
    }
}
