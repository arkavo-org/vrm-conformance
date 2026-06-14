//! Batched-mode execution: builds a JSON manifest of test_ids, invokes
//! the adapter once for the whole batch, ingests the NDJSON results file
//! the adapter writes. See
//! `docs/superpowers/specs/2026-05-12-adapter-univrm-design.md` for the
//! design rationale (engine-idiom divergence; Unity batch mode is
//! idiomatic for "run, do work, exit").

use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use vrm_ops::tools as ops;
use vrm_test_plan::{
    AnimationConfig, Camera, Lighting, Output, PhysicsConfig, PostProcessing, RenderSequenceBlock,
    TestPlan,
};

/// Per-entry benchmark request carried in the batch manifest. Present only when
/// `execute-test-batch --benchmark` was used; Unity runs a warmup+measured
/// render loop for the entry and returns a `measurement` block.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BatchBenchmarkParams {
    pub warmup_frames: u32,
    pub measured_frames: u32,
    pub animate: bool,
}

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub render_sequence: Option<RenderSequenceBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub benchmark: Option<BatchBenchmarkParams>,
}

/// Build the manifest from a slice of `(plan, vrm_path)` pairs.
/// Caller is responsible for plan/.vrm pairing and for ensuring
/// `output_dir` exists; this function only translates types.
pub fn build_manifest(
    pairs: &[(TestPlan, Utf8PathBuf)],
    output_dir: Utf8PathBuf,
    renderer_name: String,
    benchmark: Option<BatchBenchmarkParams>,
) -> BatchManifest {
    let tests = pairs
        .iter()
        .map(|(plan, vrm_path)| {
            let abs_vrm = absolutize(vrm_path);
            // Resolve animation.vrma.path relative to the VRM asset's parent
            // directory so that relative sibling paths (e.g. "foo.vrma") become
            // absolute before the manifest is handed to the Unity adapter. Unity
            // has no concept of a "plans dir"; it only sees the manifest JSON.
            let animation = resolve_animation_paths(&plan.animation, &abs_vrm);
            BatchTestEntry {
                test_id: plan.id.clone(),
                vrm_path: abs_vrm,
                spec_section: plan.spec_section.clone(),
                camera: plan.camera,
                lighting: plan.lighting.clone(),
                post_processing: plan.post_processing.clone(),
                output: plan.output,
                physics: plan.physics.clone(),
                animation,
                render_sequence: plan.render_sequence.clone(),
                benchmark: benchmark.clone(),
            }
        })
        .collect();

    BatchManifest {
        manifest_version: 2,
        output_dir: absolutize(&output_dir),
        renderer_name,
        renderer_version: None,
        tests,
    }
}

/// Resolve any relative paths inside `AnimationConfig` to absolute paths,
/// using `vrm_path`'s parent directory as the base.
fn resolve_animation_paths(
    animation: &Option<AnimationConfig>,
    vrm_path: &Utf8PathBuf,
) -> Option<AnimationConfig> {
    let anim = animation.as_ref()?;
    let base = vrm_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| Utf8PathBuf::from("."));

    let vrma = anim.vrma.as_ref().map(|v| {
        let resolved = absolutize_str_path(&v.path, &base);
        vrm_test_plan::VrmaAnimation {
            path: resolved,
            apply_at_time: v.apply_at_time,
        }
    });
    Some(AnimationConfig {
        vrma,
        root_transform: anim.root_transform.clone(),
    })
}

/// Absolutize a path given as a `String`, using `base` as the parent
/// directory for relative paths.
fn absolutize_str_path(path: &str, base: &Utf8PathBuf) -> String {
    let p = std::path::Path::new(path);
    if p.is_absolute() {
        return path.to_string();
    }
    let joined = base.as_std_path().join(p);
    // Canonicalize if the file exists; otherwise just join without
    // resolving symlinks (the file may not exist yet at plan-build time).
    let abs = joined.canonicalize().unwrap_or(joined);
    abs.to_string_lossy().into_owned()
}

fn absolutize(p: &Utf8PathBuf) -> Utf8PathBuf {
    let std_path = p.as_std_path();
    let abs = if std_path.is_absolute() {
        std_path.to_path_buf()
    } else {
        std::env::current_dir().expect("current_dir").join(std_path)
    };
    Utf8PathBuf::from_path_buf(abs).expect("absolute path is utf-8")
}

// =====================================================================
// Results parsing (Unity → runner)
// =====================================================================

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct BatchResultsMeta {
    pub manifest_version: u32,
    pub renderer_name: String,
    pub renderer_version: String,
    pub unity_version: String,
    pub render_pipeline: String,
    pub total_tests: usize,
}

/// One rendered frame inside a sequence result entry.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct SequenceFrameEntry {
    pub index: u32,
    pub timestamp_seconds: f32,
    pub path: String,
    pub blake3: String,
    /// Per-spring joint world positions at this frame, present only when the
    /// adapter ran with `capture_positions`. Joint positions are FLAT
    /// (`[x0,y0,z0, x1,y1,z1, ...]`) — the batch adapter's JsonUtility vec3
    /// convention (Unity JsonUtility cannot emit nested arrays). The runner
    /// reshapes to canonical `[[x,y,z], ...]` when persisting the positions
    /// JSON, so the on-disk schema matches the per-op path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spring_positions: Option<Vec<BatchSpringPositions>>,
}

/// Adapter-wire spring positions for batch mode: flat `joint_positions`
/// (`[x0,y0,z0, ...]`). Reshaped to `vrm_ops::tools::SpringPositions` (with
/// `[f32;3]` triples) before the runner writes the canonical positions JSON.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct BatchSpringPositions {
    pub name: String,
    pub joint_positions: Vec<f32>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ResultEntry {
    pub test_id: String,
    pub status: ResultStatus,
    #[serde(default)]
    pub output_path: Option<String>,
    #[serde(default)]
    pub blake3: Option<String>,
    #[serde(default)]
    pub actual_color_space: Option<String>,
    #[serde(default)]
    pub render_seconds: Option<f32>,
    /// Task 28: source spec version reported by the adapter, detected from the
    /// GLB `extensionsUsed` before any auto-migration. "0.x" for VRM 0.x
    /// ("VRM" extension), "1.0" for VRM 1.0 ("VRMC_vrm" extension).
    /// `None` for adapters that predate Task 28 or if detection failed.
    #[serde(default)]
    pub source_spec_version: Option<String>,
    #[serde(default)]
    pub error: Option<ResultError>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frames: Option<Vec<SequenceFrameEntry>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_hz_achieved: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measurement: Option<ops::PerfMeasurement>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ResultStatus {
    Ok,
    Error,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ResultError {
    pub code: i32,
    pub message: String,
    #[serde(default)]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct BatchResults {
    pub meta: BatchResultsMeta,
    pub entries: Vec<ResultEntry>,
}

/// Parse the NDJSON results file. Line 1 must be the `_meta` envelope
/// (a JSON object with `"_meta": true`); subsequent non-blank lines
/// are per-test result entries. Blank lines are tolerated.
///
/// Returns an error if (a) the file is empty, (b) line 1 is not a
/// `_meta` envelope, or (c) any line fails to parse as JSON. Does
/// **not** validate that `entries.len() == meta.total_tests` —
/// partial output is a defined success condition; the caller
/// reconciles it.
pub fn parse_results_ndjson(s: &str) -> anyhow::Result<BatchResults> {
    let mut lines = s.lines().filter(|l| !l.trim().is_empty());
    let meta_line = lines
        .next()
        .ok_or_else(|| anyhow::anyhow!("results file is empty; expected _meta envelope"))?;
    let meta_value: serde_json::Value = serde_json::from_str(meta_line).map_err(|e| {
        anyhow::anyhow!("failed to parse first line as JSON: {e}; line={meta_line}")
    })?;
    if meta_value.get("_meta").and_then(|v| v.as_bool()) != Some(true) {
        anyhow::bail!(
            "first line must be a _meta envelope (object with \"_meta\": true); got: {meta_line}"
        );
    }
    let meta: BatchResultsMeta = serde_json::from_value(meta_value)
        .map_err(|e| anyhow::anyhow!("_meta envelope deserialization failed: {e}"))?;
    let mut entries = Vec::new();
    for (i, line) in lines.enumerate() {
        let entry: ResultEntry = serde_json::from_str(line).map_err(|e| {
            anyhow::anyhow!("failed to parse entry line {}: {e}; line={line}", i + 2)
        })?;
        entries.push(entry);
    }
    Ok(BatchResults { meta, entries })
}

// =====================================================================
// Local manifest writer (per-renderer goldens-cache output)
// =====================================================================

use std::path::Path;

/// One entry in `goldens-cache/<renderer>/local-manifest.json`.
/// Format mirrors what `scripts/bootstrap-goldens.sh` already produces
/// for the per-test adapters; UniVRM writes the same shape so the
/// downstream consensus tooling doesn't need a UniVRM-specific path.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LocalManifestEntry {
    pub test_id: String,
    pub renderer_name: String,
    pub renderer_version: String,
    pub output_path: String,
    /// If `None` at write time, the writer hashes the file at
    /// `output_path` and fills this in.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blake3: Option<String>,
    pub actual_color_space: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct LocalManifestEnvelope<'a> {
    manifest_version: u32,
    entries: &'a [LocalManifestEntry],
}

/// Write the per-renderer local manifest. Computes BLAKE3 for any
/// entry whose `blake3` field is `None` by reading the PNG bytes at
/// `output_path`.
pub fn write_local_manifest(path: &Path, entries: &[LocalManifestEntry]) -> anyhow::Result<()> {
    let mut materialized: Vec<LocalManifestEntry> = Vec::with_capacity(entries.len());
    for entry in entries {
        let mut e = entry.clone();
        // Single-PNG rehash: only when output_path actually points at a file.
        // Sequence-shape entries (Phase 7) have empty output_path because
        // their per-frame paths live in ResultEntry.frames; those frames
        // were already rehashed in run() before reconcile_to_local_manifest.
        if e.blake3.is_none() && e.status == "ok" && !e.output_path.is_empty() {
            let bytes = std::fs::read(&e.output_path)
                .map_err(|err| anyhow::anyhow!("read {}: {err}", e.output_path))?;
            let hash = blake3::hash(&bytes);
            e.blake3 = Some(format!("blake3:{}", hash.to_hex()));
        }
        materialized.push(e);
    }
    let envelope = LocalManifestEnvelope {
        manifest_version: 2,
        entries: &materialized,
    };
    let bytes = serde_json::to_vec_pretty(&envelope)?;
    std::fs::write(path, bytes)?;
    Ok(())
}

// =====================================================================
// Top-level `run` — discovery, manifest emission, adapter invocation,
// results ingestion, local-manifest writing
// =====================================================================

use std::process::Command;

#[derive(Debug, Clone)]
pub struct RunOptions {
    pub plans_dir: Utf8PathBuf,
    pub adapter_bin: Utf8PathBuf,
    pub output_dir: Utf8PathBuf,
    pub renderer_name: String,
    pub benchmark: Option<BatchBenchmarkParams>,
}

#[derive(Debug, Clone)]
pub struct RunSummary {
    pub total_tests: usize,
    pub ok_count: usize,
    pub error_count: usize,
    pub local_manifest_path: Utf8PathBuf,
}

/// Discover `*.test.yaml` files in `opts.plans_dir`, pair each with a
/// sibling `.vrm` (same stem), build the manifest, invoke the adapter
/// binary, parse the results NDJSON, and write the per-renderer local
/// manifest. Returns a summary; does not panic on per-test errors
/// (those land in the local manifest as `status: "error"`).
pub fn run(opts: &RunOptions) -> anyhow::Result<RunSummary> {
    let pairs = discover_plan_vrm_pairs(&opts.plans_dir)?;
    if pairs.is_empty() {
        anyhow::bail!(
            "no *.test.yaml files found in {}; nothing to run",
            opts.plans_dir
        );
    }

    std::fs::create_dir_all(opts.output_dir.as_std_path())?;

    let manifest = build_manifest(
        &pairs,
        opts.output_dir.clone(),
        opts.renderer_name.clone(),
        opts.benchmark.clone(),
    );

    // Write manifest + results paths into the output dir so they end up
    // in a stable location (helps debugging; not protocol-load-bearing).
    let manifest_path = opts.output_dir.join("manifest.json");
    let results_path = opts.output_dir.join("results.ndjson");
    std::fs::write(
        manifest_path.as_std_path(),
        serde_json::to_vec_pretty(&manifest)?,
    )?;

    // Invoke the adapter. Two positional args: manifest path, results
    // path. The mock fixtures and the real launcher.sh both use this
    // contract.
    let status = Command::new(opts.adapter_bin.as_std_path())
        .arg(manifest_path.as_std_path())
        .arg(results_path.as_std_path())
        .status()
        .map_err(|e| anyhow::anyhow!("failed to spawn adapter {}: {e}", opts.adapter_bin))?;

    // Read whatever the adapter wrote, even if the exit was non-zero —
    // partial output is a defined success condition.
    let parsed = match std::fs::read_to_string(results_path.as_std_path()) {
        Ok(s) => parse_results_ndjson(&s)?,
        Err(e) => {
            anyhow::bail!(
                "adapter ({}) did not produce a readable results file at {}: {e}",
                opts.adapter_bin,
                results_path
            );
        }
    };
    // The adapter's exit code is informational at this layer. We don't
    // fail the run on non-zero exit because partial output is expected
    // to surface as per-test errors. Future scope: surface the exit
    // code in the summary so callers can choose to gate on it.
    let _ = status;

    // Re-hash per-frame BLAKE3 from on-disk PNG bytes when the result entry
    // carries a frames list. The runner is the authority for the manifest's
    // content-addressed column (Phase 5 Task 2 contract); adapter-returned
    // hashes are advisory and may be placeholder values.
    let mut parsed = parsed;
    for entry in parsed.entries.iter_mut() {
        if let Some(frames) = entry.frames.as_mut() {
            for frame in frames.iter_mut() {
                match std::fs::read(&frame.path) {
                    Ok(bytes) => {
                        let hash = blake3::hash(&bytes);
                        frame.blake3 = format!("blake3:{}", hash.to_hex());
                    }
                    Err(e) => {
                        tracing::warn!(
                            "sequence frame re-hash: cannot read {}: {e}; hash unchanged",
                            frame.path
                        );
                    }
                }
            }
        }
    }

    // Benchmark: write a PerfReport per entry that returned a measurement.
    {
        use std::collections::HashMap;
        let vrm_by_id: HashMap<&str, &Utf8PathBuf> =
            pairs.iter().map(|(p, v)| (p.id.as_str(), v)).collect();
        for entry in parsed.entries.iter() {
            if entry.status != ResultStatus::Ok {
                continue;
            }
            let Some(measurement) = entry.measurement.clone() else {
                continue;
            };
            let hash = vrm_by_id
                .get(entry.test_id.as_str())
                .and_then(|p| crate::benchmark::asset_blake3(p).ok())
                .unwrap_or_else(|| "blake3:unknown".to_string());
            let report = crate::benchmark::compose_report(
                &entry.test_id,
                &opts.renderer_name,
                &hash,
                measurement,
            );
            let path = crate::benchmark::report_path(
                &opts.output_dir,
                &entry.test_id,
                &opts.renderer_name,
            );
            match serde_json::to_string_pretty(&report) {
                Ok(json) => {
                    if let Err(e) = std::fs::write(path.as_std_path(), json) {
                        tracing::warn!("failed to write perf report {path}: {e}");
                    }
                }
                Err(e) => {
                    tracing::warn!("failed to serialize perf report for {}: {e}", entry.test_id)
                }
            }
        }
    }

    // Persist per-plan positions JSON when the adapter captured spring-bone
    // positions (render_sequence + capture_positions). Mirrors the per-op
    // path (`execute::persist_positions_json`): one
    // `<test_id>_<renderer>_positions.json` of `FramePositionsEntry` values,
    // the canonical schema `penetration-diff` consumes.
    for entry in parsed.entries.iter() {
        let Some(frames) = entry.frames.as_ref() else {
            continue;
        };
        let positions: Vec<crate::execute::FramePositionsEntry> = frames
            .iter()
            .filter_map(|f| {
                f.spring_positions
                    .as_ref()
                    .map(|sp| crate::execute::FramePositionsEntry {
                        frame_index: f.index,
                        timestamp_seconds: f.timestamp_seconds,
                        // Reshape flat [x,y,z,...] → canonical [[x,y,z], ...].
                        springs: sp
                            .iter()
                            .map(|s| vrm_ops::tools::SpringPositions {
                                name: s.name.clone(),
                                joint_positions: s
                                    .joint_positions
                                    .chunks_exact(3)
                                    .map(|c| [c[0], c[1], c[2]])
                                    .collect(),
                            })
                            .collect(),
                    })
            })
            .collect();
        if positions.is_empty() {
            continue;
        }
        let path = opts.output_dir.join(format!(
            "{}_{}_positions.json",
            entry.test_id, opts.renderer_name
        ));
        match serde_json::to_string_pretty(&positions) {
            Ok(json) => {
                if let Err(e) = std::fs::write(path.as_std_path(), json) {
                    tracing::warn!("failed to write positions JSON {path}: {e}");
                }
            }
            Err(e) => tracing::warn!("failed to serialize positions for {}: {e}", entry.test_id),
        }
    }

    // Build local-manifest entries from the parsed results, padding
    // missing test_ids (declared in _meta but not present in entries)
    // as errors.
    let local_entries = reconcile_to_local_manifest(&parsed, &pairs, &opts.renderer_name);
    let local_manifest_path = opts.output_dir.join("local-manifest.json");
    write_local_manifest(local_manifest_path.as_std_path(), &local_entries)?;

    let ok_count = local_entries.iter().filter(|e| e.status == "ok").count();
    let error_count = local_entries.len() - ok_count;
    Ok(RunSummary {
        total_tests: local_entries.len(),
        ok_count,
        error_count,
        local_manifest_path,
    })
}

fn discover_plan_vrm_pairs(
    plans_dir: &Utf8PathBuf,
) -> anyhow::Result<Vec<(TestPlan, Utf8PathBuf)>> {
    let mut pairs = Vec::new();
    for entry in std::fs::read_dir(plans_dir.as_std_path())? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("yaml") {
            continue;
        }
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) if n.ends_with(".test.yaml") => n.to_owned(),
            _ => continue,
        };
        let stem = name.trim_end_matches(".test.yaml");
        let vrm_path = plans_dir.join(format!("{stem}.vrm"));
        if !vrm_path.exists() {
            tracing::warn!(
                "skipping {}: sibling .vrm not found at {vrm_path}",
                path.display()
            );
            continue;
        }
        let plan_bytes = std::fs::read(&path)?;
        let plan: TestPlan = serde_yml::from_slice(&plan_bytes)
            .map_err(|e| anyhow::anyhow!("parse {}: {e}", path.display()))?;
        pairs.push((plan, vrm_path));
    }
    // Stable ordering — discovery order is filesystem-dependent.
    pairs.sort_by(|a, b| a.0.id.cmp(&b.0.id));
    Ok(pairs)
}

fn reconcile_to_local_manifest(
    parsed: &BatchResults,
    pairs: &[(TestPlan, Utf8PathBuf)],
    renderer_name: &str,
) -> Vec<LocalManifestEntry> {
    use std::collections::HashMap;
    let by_id: HashMap<&str, &ResultEntry> = parsed
        .entries
        .iter()
        .map(|e| (e.test_id.as_str(), e))
        .collect();
    let mut out = Vec::with_capacity(pairs.len());
    for (plan, _vrm) in pairs {
        if let Some(entry) = by_id.get(plan.id.as_str()) {
            match entry.status {
                ResultStatus::Ok => out.push(LocalManifestEntry {
                    test_id: entry.test_id.clone(),
                    renderer_name: renderer_name.to_string(),
                    renderer_version: parsed.meta.renderer_version.clone(),
                    output_path: entry.output_path.clone().unwrap_or_default(),
                    blake3: entry.blake3.clone(),
                    actual_color_space: entry
                        .actual_color_space
                        .clone()
                        .unwrap_or_else(|| "unknown".into()),
                    status: "ok".into(),
                    error_message: None,
                }),
                ResultStatus::Error => out.push(LocalManifestEntry {
                    test_id: entry.test_id.clone(),
                    renderer_name: renderer_name.to_string(),
                    renderer_version: parsed.meta.renderer_version.clone(),
                    output_path: String::new(),
                    blake3: None,
                    actual_color_space: "n/a".into(),
                    status: "error".into(),
                    error_message: entry
                        .error
                        .as_ref()
                        .map(|e| format!("code={} message={}", e.code, e.message)),
                }),
            }
        } else {
            // Declared in the plan dir; missing from the results file.
            // Treat as "batch terminated before this test ran."
            out.push(LocalManifestEntry {
                test_id: plan.id.clone(),
                renderer_name: renderer_name.to_string(),
                renderer_version: parsed.meta.renderer_version.clone(),
                output_path: String::new(),
                blake3: None,
                actual_color_space: "n/a".into(),
                status: "error".into(),
                error_message: Some("batch terminated before this test ran".into()),
            });
        }
    }
    out
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

    // ── Benchmark serialization tests ────────────────────────────────────────

    /// A BatchTestEntry with benchmark params serializes with the `benchmark` key;
    /// a None benchmark produces no `benchmark` key.
    #[test]
    fn batch_test_entry_benchmark_serialization() {
        use vrm_test_plan::{
            AmbientLight, Camera, ColorSpace, DirectionalLight, Lighting, Output, PostProcessing,
        };

        let make_entry = |benchmark: Option<BatchBenchmarkParams>| BatchTestEntry {
            test_id: "smoke".to_string(),
            vrm_path: Utf8PathBuf::from("/tmp/a.vrm"),
            spec_section: "1.0".to_string(),
            camera: Camera {
                position: [0.0, 1.0, 3.0],
                target: [0.0, 1.0, 0.0],
                up: [0.0, 1.0, 0.0],
                fov_degrees: 30.0,
            },
            lighting: Lighting {
                directional: DirectionalLight {
                    dir: [0.0, -1.0, 0.0],
                    color: [1.0, 1.0, 1.0],
                    intensity: 1.0,
                },
                ambient: AmbientLight {
                    color: [1.0, 1.0, 1.0],
                    intensity: 0.5,
                },
                cast_shadows: false,
                receive_shadows: false,
            },
            post_processing: PostProcessing::default(),
            output: Output {
                width: 512,
                height: 512,
                color_space: ColorSpace::Srgb,
                msaa: 1,
            },
            physics: None,
            animation: None,
            render_sequence: None,
            benchmark,
        };

        // With benchmark params present
        let with_bench = make_entry(Some(BatchBenchmarkParams {
            warmup_frames: 5,
            measured_frames: 10,
            animate: false,
        }));
        let json_with = serde_json::to_string(&with_bench).unwrap();
        let val: serde_json::Value = serde_json::from_str(&json_with).unwrap();
        let b = &val["benchmark"];
        assert_eq!(b["warmup_frames"], 5);
        assert_eq!(b["measured_frames"], 10);
        assert_eq!(b["animate"], false);

        // With no benchmark — key must be absent
        let without_bench = make_entry(None);
        let json_without = serde_json::to_string(&without_bench).unwrap();
        let val_without: serde_json::Value = serde_json::from_str(&json_without).unwrap();
        assert!(
            val_without.get("benchmark").is_none() || val_without["benchmark"].is_null(),
            "benchmark key should be absent when None"
        );
    }

    /// ResultEntry deserializes measurement when present, and leaves it None
    /// when absent. Confirms structural.state_changes is None (not declared
    /// in a minimal UniVRM measurement block).
    #[test]
    fn result_entry_measurement_deserialization() {
        // A line with a measurement block (minimal PerfMeasurement shape)
        let with_measurement = r#"{
            "test_id": "smoke",
            "status": "ok",
            "output_path": "/tmp/smoke.png",
            "measurement": {
                "protocol": { "warmup_frames": 3, "measured_frames": 10, "animated": false },
                "host": {
                    "os": "macOS", "os_version": "15.0",
                    "gpu_vendor": "Apple", "gpu_model": "M1",
                    "driver_version": "1.0", "build_flags": ""
                },
                "capabilities": ["timing", "structural", "geometry", "resources"],
                "structural": { "draw_calls": 42.0 }
            }
        }"#;

        let entry: ResultEntry = serde_json::from_str(with_measurement).unwrap();
        let measurement = entry.measurement.expect("measurement should be Some");
        // structural.draw_calls present
        let structural = measurement.structural.expect("structural should be Some");
        assert!((structural.draw_calls - 42.0).abs() < f32::EPSILON);
        // state_changes and texture_bindings absent (None) — UniVRM doesn't emit them
        assert!(
            structural.state_changes.is_none(),
            "state_changes should be None when absent from JSON"
        );
        assert!(
            structural.texture_bindings.is_none(),
            "texture_bindings should be None when absent from JSON"
        );

        // A line without a measurement block
        let without_measurement = r#"{
            "test_id": "smoke",
            "status": "ok",
            "output_path": "/tmp/smoke.png"
        }"#;
        let entry2: ResultEntry = serde_json::from_str(without_measurement).unwrap();
        assert!(
            entry2.measurement.is_none(),
            "measurement should be None when absent"
        );
    }
}
