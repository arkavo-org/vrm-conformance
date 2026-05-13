//! Batched-mode execution: builds a JSON manifest of test_ids, invokes
//! the adapter once for the whole batch, ingests the NDJSON results file
//! the adapter writes. See
//! `docs/superpowers/specs/2026-05-12-adapter-univrm-design.md` for the
//! design rationale (engine-idiom divergence; Unity batch mode is
//! idiomatic for "run, do work, exit").

use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use vrm_test_plan::{AnimationConfig, Camera, Lighting, Output, PhysicsConfig, PostProcessing, TestPlan};

/// Top-level JSON document the Rust runner writes for the adapter to
/// consume. Schema version is pinned at the top so future changes can
/// be detected by Unity-side code.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BatchManifest {
    pub manifest_version: u32,
    pub output_dir: Utf8PathBuf,
    pub renderer_name: String,
    pub renderer_version: Option<String>,
    pub tests: Vec<BatchTestEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BatchTestEntry {
    pub test_id: String,
    pub vrm_path: Utf8PathBuf,
    pub spec_section: String,
    pub camera: Camera,
    pub lighting: Lighting,
    pub post_processing: PostProcessing,
    pub output: Output,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub physics: Option<PhysicsConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animation: Option<AnimationConfig>,
}

/// Build the manifest from a slice of `(plan, vrm_path)` pairs.
/// Caller is responsible for plan/.vrm pairing and for ensuring
/// `output_dir` exists; this function only translates types.
pub fn build_manifest(
    pairs: &[(TestPlan, Utf8PathBuf)],
    output_dir: Utf8PathBuf,
    renderer_name: String,
) -> BatchManifest {
    let tests = pairs
        .iter()
        .map(|(plan, vrm_path)| BatchTestEntry {
            test_id: plan.id.clone(),
            vrm_path: absolutize(vrm_path),
            spec_section: plan.spec_section.clone(),
            camera: plan.camera,
            lighting: plan.lighting.clone(),
            post_processing: plan.post_processing.clone(),
            output: plan.output,
            physics: plan.physics.clone(),
            animation: plan.animation.clone(),
        })
        .collect();

    BatchManifest {
        manifest_version: 1,
        output_dir: absolutize(&output_dir),
        renderer_name,
        renderer_version: None,
        tests,
    }
}

fn absolutize(p: &Utf8PathBuf) -> Utf8PathBuf {
    let std_path = p.as_std_path();
    let abs = if std_path.is_absolute() {
        std_path.to_path_buf()
    } else {
        std::env::current_dir()
            .expect("current_dir")
            .join(std_path)
    };
    Utf8PathBuf::from_path_buf(abs).expect("absolute path is utf-8")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolutize_handles_already_absolute_path() {
        let p = Utf8PathBuf::from("/tmp/already_abs");
        let out = absolutize(&p);
        assert_eq!(out, Utf8PathBuf::from("/tmp/already_abs"));
    }

    #[test]
    fn absolutize_joins_relative_path_with_cwd() {
        let p = Utf8PathBuf::from("relative/path");
        let out = absolutize(&p);
        assert!(out.as_str().starts_with('/'));
        assert!(out.as_str().ends_with("relative/path"));
    }
}
