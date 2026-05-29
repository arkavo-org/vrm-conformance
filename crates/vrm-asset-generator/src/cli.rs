use crate::emit::{emit_with_sidecars, emit_with_sidecars_v0};
use crate::params::MToonParams;
use anyhow::Result;
use camino::Utf8PathBuf;
use clap::{Parser, Subcommand, ValueEnum};
use serde_json::json;

fn parse_spec_version(s: &str) -> Result<vrm_ops::SpecVersion, String> {
    match s {
        "0.x" => Ok(vrm_ops::SpecVersion::V0),
        "1.0" => Ok(vrm_ops::SpecVersion::V1),
        other => Err(format!(
            "unsupported spec_version {other:?}; expected \"0.x\" or \"1.0\""
        )),
    }
}

#[derive(Debug, Parser)]
#[command(version, about = "Parametric VRM 1.0 test asset generator")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Cmd,
}

#[derive(Debug, Subcommand)]
pub enum Cmd {
    /// Emit a `.vrm` + `.meta.json` + `.test.yaml` triplet using the
    /// VRMC_materials_mtoon spec defaults.
    EmitDefault {
        #[arg(long)]
        id: String,
        #[arg(long)]
        output_dir: Utf8PathBuf,
        /// VRM spec version target: "0.x" or "1.0". Defaults to 1.0.
        #[arg(long, default_value = "1.0", value_parser = parse_spec_version)]
        spec_version: vrm_ops::SpecVersion,
        /// Emit JSON status to stdout instead of human text.
        #[arg(long)]
        json: bool,
    },

    /// Emit the full MToon basic sweep (~50 assets) into output_dir/.
    EmitSweep {
        #[arg(long)]
        output_dir: Utf8PathBuf,
        /// VRM spec version target: "0.x" or "1.0". Defaults to 1.0.
        #[arg(long, default_value = "1.0", value_parser = parse_spec_version)]
        spec_version: vrm_ops::SpecVersion,
        /// Emit JSON progress on stderr (NDJSON) and a final JSON summary on stdout.
        #[arg(long)]
        json: bool,
    },

    /// Emit the MToon emissive sweep (14 assets) covering
    /// `material.emissiveFactor` + the (archived but still in-spec)
    /// `VRMC_materials_hdr_emissiveMultiplier-1.0` extension. Sweeps
    /// multiplier ∈ {0, 0.25, 0.5, 0.75, 1, 2, 4} crossed with white
    /// emissive, plus per-channel RGB color variants at multiplier=1
    /// and multiplier=2, plus a zero-emissive baseline that exercises
    /// the conditional-emit code path (extension must NOT be emitted
    /// when `emissiveFactor == [0,0,0]`).
    EmitEmissiveSweep {
        #[arg(long)]
        output_dir: Utf8PathBuf,
        /// VRM spec version target: "0.x" or "1.0". Defaults to 1.0.
        #[arg(long, default_value = "1.0", value_parser = parse_spec_version)]
        spec_version: vrm_ops::SpecVersion,
        #[arg(long)]
        json: bool,
    },

    /// Emit the VRMC_vrm.firstPerson sweep (4 assets, one per spec
    /// `meshAnnotations[*].type` enum value: auto, both, thirdPersonOnly,
    /// firstPersonOnly). Standard third-person camera. Conformant
    /// renderers cull the firstPersonOnly variant's head mesh and
    /// render the other three identically; renderers that ignore
    /// firstPerson annotations produce 4 identical PNGs.
    EmitFirstPersonSweep {
        #[arg(long)]
        output_dir: Utf8PathBuf,
        /// VRM spec version target: "0.x" or "1.0". Defaults to 1.0.
        #[arg(long, default_value = "1.0", value_parser = parse_spec_version)]
        spec_version: vrm_ops::SpecVersion,
        #[arg(long)]
        json: bool,
    },

    /// Emit the KHR_texture_transform sweep (8 assets covering offset,
    /// rotation, scale, and combined transforms). Each asset uses the
    /// same procedural 16x16 quadrant checkerboard texture (red/green/
    /// blue/yellow) on the MToon baseColorTexture; conformant renderers
    /// produce 8 visually-distinct PNGs corresponding to the declared
    /// UV transforms. Renderers that ignore KHR_texture_transform
    /// produce 7 identical PNGs (the 7 non-identity variants render
    /// like identity) plus the identity baseline.
    EmitTextureTransformSweep {
        #[arg(long)]
        output_dir: Utf8PathBuf,
        #[arg(long)]
        json: bool,
    },

    /// Emit the MToon shadeMultiplyTexture sweep (6 assets covering
    /// the spec's shaded-color path: shadeColorTerm = shadeColorFactor *
    /// texture(shadeMultiplyTexture, uv)). One untextured baseline +
    /// five textured variants crossing shadeColorFactor (white, red
    /// tint) and shadingShiftFactor (-0.5, default, +0.5). Conformant
    /// renderers display the checkerboard pattern wherever the sphere
    /// is in shadow; non-conformant renderers fall back to plain
    /// shadeColorFactor.
    EmitShadeMultiplyTextureSweep {
        #[arg(long)]
        output_dir: Utf8PathBuf,
        /// VRM spec version target: "0.x" or "1.0". Defaults to 1.0.
        #[arg(long, default_value = "1.0", value_parser = parse_spec_version)]
        spec_version: vrm_ops::SpecVersion,
        #[arg(long)]
        json: bool,
    },

    /// Emit the MToon matcapTexture sweep (5 assets covering the spec's
    /// rim-lighting matcap term: matcapFactor.rgb *
    /// texture(matcapTexture, matcapUv), where matcapUv is derived
    /// from the view-space surface normal — distinct from mesh UVs).
    /// Baseline (no matcap) + 4 textured variants crossing
    /// matcapFactor (default white, red tint, blue tint, half-
    /// intensity dim). Every variant sets near-black base and shade
    /// colors to isolate the matcap signal in the rendered output.
    EmitMatcapTextureSweep {
        #[arg(long)]
        output_dir: Utf8PathBuf,
        /// VRM spec version target: "0.x" or "1.0". Defaults to 1.0.
        #[arg(long, default_value = "1.0", value_parser = parse_spec_version)]
        spec_version: vrm_ops::SpecVersion,
        #[arg(long)]
        json: bool,
    },

    /// Emit the MToon shadingShiftTexture sweep (5 assets covering
    /// per-pixel shading-boundary modulation: the texture's R-channel
    /// value, multiplied by `scale`, is ADDED to shadingShiftFactor).
    /// Baseline + 4 textured variants crossing scale (1.0, 0.5, 2.0)
    /// and one combined-factor case to test additive composition.
    EmitShadingShiftTextureSweep {
        #[arg(long)]
        output_dir: Utf8PathBuf,
        #[arg(long)]
        json: bool,
    },

    /// Emit the MToon rimMultiplyTexture sweep (4 assets covering
    /// per-pixel modulation of the parametric rim contribution: the
    /// texture's RGB multiplies into the rim term). Baseline + 3
    /// textured variants crossing rim color (white, red) and
    /// rimLightingMixFactor (1.0, 0.5).
    EmitRimMultiplyTextureSweep {
        #[arg(long)]
        output_dir: Utf8PathBuf,
        #[arg(long)]
        json: bool,
    },

    /// Emit the MToon outlineWidthMultiplyTexture sweep (5 assets
    /// covering per-vertex outline-width modulation by the texture's
    /// G-channel, per spec). Baseline + 3 textured variants (world
    /// outline / screen outline / wider base width) + 1 regression
    /// guard with mode=none that must NOT produce outlines.
    EmitOutlineWidthMultiplyTextureSweep {
        #[arg(long)]
        output_dir: Utf8PathBuf,
        /// VRM spec version target: "0.x" or "1.0". Defaults to 1.0.
        #[arg(long, default_value = "1.0", value_parser = parse_spec_version)]
        spec_version: vrm_ops::SpecVersion,
        #[arg(long)]
        json: bool,
    },

    /// Emit the glTF-core PBR textures sweep (6 assets covering
    /// `normalTexture` (tangent-space normal map) + `occlusionTexture`
    /// (R-channel AO modulation) applied to MToon materials).
    /// Surfaces whether MToon renderers integrate with the glTF-core
    /// PBR texture pipeline or override it entirely.
    EmitPbrTexturesSweep {
        #[arg(long)]
        output_dir: Utf8PathBuf,
        /// VRM spec version target: "0.x" or "1.0". Defaults to 1.0.
        #[arg(long, default_value = "1.0", value_parser = parse_spec_version)]
        spec_version: vrm_ops::SpecVersion,
        #[arg(long)]
        json: bool,
    },

    /// Emit one `.vrm` carrying both default MToon material and a default
    /// VRMC_springBone chain attached to the head.
    EmitSpringbone {
        #[arg(long)]
        id: String,
        #[arg(long)]
        output_dir: Utf8PathBuf,
        #[arg(long)]
        json: bool,
    },

    /// Emit the full VRMC_springBone parameter sweep (~20 assets) into
    /// output_dir/. Each asset pairs default MToon material with a
    /// single-axis spring-bone variant; baseline included.
    EmitSpringboneSweep {
        #[arg(long)]
        output_dir: Utf8PathBuf,
        #[arg(long)]
        json: bool,
    },

    /// Emit one .vrm with default MToon + default VRMC_springBone chain,
    /// whose test.yaml triggers animate_root_transform mid-render so the
    /// chain is captured mid-swing rather than at the gravity settle.
    EmitSpringboneSwing {
        #[arg(long)]
        id: String,
        #[arg(long)]
        output_dir: Utf8PathBuf,
        #[arg(long)]
        json: bool,
    },

    /// Emit the swing-variant spring-bone sweep (~20 assets). Same axes
    /// as emit-springbone-sweep; every emitted plan carries both a
    /// physics block and an animation.root_transform block.
    EmitSpringboneSwingSweep {
        #[arg(long)]
        output_dir: Utf8PathBuf,
        #[arg(long)]
        json: bool,
    },

    /// Emit the VMK#162 coupling-detection sweep (8 assets = 4 variants
    /// × settle + swing). Variants: vmk162_baseline + vmk162_gravity_{0,1,2}.
    /// Designed to surface the parser substitution `gravityPower=0 → 1.0`
    /// at `VRMExtensionParser.swift:1061` via the matrix runner: under
    /// substitution, gravity_0 (asset value 0.0) and gravity_1 (asset
    /// value 1.0) produce identical chain dynamics. Without it, gravity_0
    /// decouples from gravity_1 and the matrix surfaces a per-joint drift.
    /// Emit both settle (`vmk162_*`) and swing (`swing_vmk162_*`) plans;
    /// the swing variant is the load-bearing one for matrix runner usage
    /// because static settle on a 4-joint stiff chain barely moves.
    EmitSpringboneCouplingSweep {
        #[arg(long)]
        output_dir: Utf8PathBuf,
        #[arg(long)]
        json: bool,
    },

    /// Emit the spring-bone sequence-mode sweep (~20 assets). Each asset's
    /// `.test.yaml` carries a `render_sequence:` block instead of an
    /// `animation:` block, dispatching the runner's render_sequence path
    /// (multi-frame capture) instead of the single-frame render path.
    /// 60 frames @ 30 Hz with `physics_dt_seconds = 1/60`, root translation
    /// `[0,0,0] → [0.15,0,0]` linearly across all frames.
    ///
    /// Asset IDs are prefixed `swing_seq_` to keep them distinct from the
    /// existing single-frame `swing_` variants in the cross-renderer
    /// goldens manifest (both can coexist).
    EmitSequenceSweep {
        #[arg(long)]
        output_dir: Utf8PathBuf,
        #[arg(long)]
        json: bool,
    },

    /// Emit the VRMC_springBone collider sweep (48 assets = 24 Cartesian
    /// variants × settle + swing). Each asset has one collider (sphere or
    /// capsule) attached to the head node in the chain's path. The settle
    /// plan uses 60-step settle; the swing plan adds animate_root_transform.
    EmitSpringboneColliderSweep {
        #[arg(long)]
        output_dir: Utf8PathBuf,
        #[arg(long)]
        json: bool,
    },

    /// Emit the VRMC_springBone_extended_collider sweep (36 assets = 18 variants
    /// × settle + swing). Variants: 3 shapes (plane, inside-sphere, inside-capsule)
    /// × 3 placements + 3 shapes × 3 angle-limits (30°, 60°, 90°) = 18 base ×
    /// settle/swing = 36 plans. Uses VRMC_springBone_extended_collider-1.0
    /// extension shapes and per-joint angleLimit.
    EmitSpringboneExtendedSweep {
        #[arg(long)]
        output_dir: Utf8PathBuf,
        #[arg(long)]
        json: bool,
    },

    /// Emit the gravity_dir sweep (8 assets = 4 directions × settle + swing).
    /// Directions: default (-Y), anti (+Y), sideways (+X), oblique (+0.7, -0.7, 0).
    /// All other SpringBoneParams held at defaults so the gravity-direction axis
    /// is unconfounded. Flushes adapters that hard-code gravity_dir = [0,-1,0].
    EmitSpringboneGravityDirSweep {
        #[arg(long)]
        output_dir: Utf8PathBuf,
        #[arg(long)]
        json: bool,
    },

    /// Emit the per-joint taper sweep (14 assets = 7 variants × settle + swing).
    /// Variants: 4 stiffness shapes (flat, high→low, low→high, exp-decay) +
    /// 3 drag shapes (flat, high→low, exp-decay). All use joint_count=4; the
    /// per-joint vector overrides the scalar for the swept axis. Exercises
    /// adapter-level discretization on non-uniform chains.
    EmitSpringboneTaperSweep {
        #[arg(long)]
        output_dir: Utf8PathBuf,
        #[arg(long)]
        json: bool,
    },

    /// Emit the multi-chain spring-bone sweep (36 assets = 18 variants × settle + swing).
    /// Variants: 3 chain counts (2, 3, 5) × 2 spacings (0.02, 0.05 m) × 3 sharing modes
    /// (share_all, share_none, share_alt). Each variant emits both a settle plan (60-step
    /// settle) and a swing plan (animate_root_transform). Exercises per-chain collider-group
    /// assignment semantics (VMK#162-class coupling bugs).
    EmitSpringboneMultichainSweep {
        #[arg(long)]
        output_dir: Utf8PathBuf,
        #[arg(long)]
        json: bool,
    },

    /// Emit the VRMA humanoid bone sweep (~15 plans). Each variant
    /// rotates one humanoid bone through a single axis arc over 1 s.
    EmitVrmaHumanoidSweep {
        #[arg(long)]
        output_dir: Utf8PathBuf,
        #[arg(long)]
        json: bool,
    },

    /// Emit the VRMA expression sweep (12 plans). Each variant animates a
    /// single expression 0 → 1 → 0 over 1 s; test plans sample at peak
    /// (t=0.5). Presets: happy, angry, sad, relaxed, surprised, aa, ih, ou,
    /// ee, blink. Custom: smug, drowsy.
    EmitVrmaExpressionSweep {
        #[arg(long)]
        output_dir: Utf8PathBuf,
        #[arg(long)]
        json: bool,
    },

    /// Emit the VRMA lookAt sweep (10 plans = 5 directions x 2 avatar configs).
    /// Directions: yaw +-60 deg, pitch +-30 deg, neutral. Avatar configs: bone vs expression.
    /// Each plan emits a .vrm (with matching lookAt.type) + .vrma + .test.yaml triplet.
    EmitVrmaLookatSweep {
        #[arg(long)]
        output_dir: Utf8PathBuf,
        #[arg(long)]
        json: bool,
    },

    /// Emit the VRM 0.x MToon basic sweep (3 variants, slice 1 of the 0.x conformance corpus).
    ///
    /// Two Applicable variants produce `.vrm`, `.meta.json`, and `.test.yaml` triplets via
    /// the v0 emit pipeline. The NotApplicable variant (`mtoon_basic_v0_outline_lighting_mix`)
    /// produces a `.skipped.json` marker; outlineLightingMix is a v1-only axis absent from 0.x.
    EmitMtoonBasicV0Sweep {
        #[arg(long)]
        output_dir: Utf8PathBuf,
        /// Emit JSON progress on stderr (NDJSON) and a final JSON summary on stdout.
        #[arg(long)]
        json: bool,
    },

    /// Emit the expressions_preset_basic v0+v1 pair — 4 assets total.
    ///
    /// This is the canonical normalization test pair that validates
    /// `vrm-normalize::expressions` mapping (joy → happy, neutral → neutral)
    /// and drives the cross-renderer round-trip property test.
    ///
    /// Outputs (2 v0 + 2 v1):
    /// - `expressions_preset_basic_v0_joy.vrm` — VRM 0.x with `blendShapeMaster`
    ///   preset=joy, weight=100, mesh/morph index 0.
    /// - `expressions_preset_basic_v0_neutral.vrm` — VRM 0.x, preset=neutral.
    /// - `expressions_preset_basic_happy.vrm` — VRM 1.0 with
    ///   `VRMC_vrm.expressions.preset.happy`, weight=1.0, morph index 0.
    /// - `expressions_preset_basic_neutral.vrm` — VRM 1.0, preset=neutral.
    EmitExpressionsPresetBasic {
        #[arg(long)]
        output_dir: Utf8PathBuf,
        #[arg(long)]
        json: bool,
    },

    /// Emit the material-name render-classification sweep (6 variants).
    ///
    /// One MToon material under heuristic-tripping names (`matname_clothing_*`,
    /// `matname_skirt_*`) vs control names (`matname_plain_*`, `matname_body_*`)
    /// crossed with the glTF `doubleSided` flag. Every variant is byte-identical
    /// except its material name and `doubleSided`; a conformant renderer's output
    /// is invariant to the name, while a name-heuristic renderer (VMK forces
    /// `cloth`-named materials double-sided + applies an overlay depth bias) diverges
    /// on the trip-token variants. Deterministic reproducer for the `Vita_clothing`
    /// z-fighting artifact. See
    /// docs/superpowers/specs/2026-05-28-material-name-classification-reproducer.md.
    EmitMaterialNameClassificationSweep {
        #[arg(long)]
        output_dir: Utf8PathBuf,
        #[arg(long)]
        json: bool,
    },

    /// Emit the doubleSided back-face-culling spec-test pair (2 triplets).
    ///
    /// `doublesided_quad_{false,true}`: an open quad viewed from BEHIND (camera
    /// on the −Z side), so back-face culling is observable. doubleSided=false
    /// culls the quad (all-background frame); doubleSided=true renders it. The
    /// false variant's plan carries a cross_variant block requiring the two
    /// renders to diverge (cross-variant SSIM ≤ 0.85). See
    /// docs/superpowers/specs/2026-05-28-doublesided-cross-variant-spec-test-design.md.
    EmitDoublesidedSpecTest {
        #[arg(long)]
        output_dir: Utf8PathBuf,
        #[arg(long)]
        json: bool,
    },

    /// Print the operation catalog (JSON Schema by default).
    Describe {
        #[arg(long, value_enum, default_value_t = DescribeFormat::Json)]
        format: DescribeFormat,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum DescribeFormat {
    Json,
    Text,
}

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Cmd::EmitDefault {
            id,
            output_dir,
            spec_version,
            json: emit_json,
        } => {
            std::fs::create_dir_all(&output_dir)?;
            let stem = output_dir.join(&id);
            let params = MToonParams::defaults(&id);
            match spec_version {
                vrm_ops::SpecVersion::V0 => emit_with_sidecars_v0(&params, &stem)?,
                vrm_ops::SpecVersion::V1 => emit_with_sidecars(&params, &stem)?,
            }

            if emit_json {
                let result = json!({
                    "ok": true,
                    "outputs": {
                        "vrm": stem.with_extension("vrm"),
                        "meta": stem.with_extension("meta.json"),
                        "test_plan": stem.with_extension("test.yaml")
                    }
                });
                println!("{}", serde_json::to_string(&result)?);
            } else {
                println!("emitted: {}", stem.with_extension("vrm"));
                println!("emitted: {}", stem.with_extension("meta.json"));
                println!("emitted: {}", stem.with_extension("test.yaml"));
            }
            Ok(())
        }
        Cmd::EmitSweep {
            output_dir,
            spec_version,
            json: emit_json,
        } => {
            use crate::sweep::mtoon_basic_sweep;
            std::fs::create_dir_all(&output_dir)?;
            let assets = mtoon_basic_sweep();
            let total = assets.len();

            let mut emitted = Vec::new();
            for (i, p) in assets.iter().enumerate() {
                if emit_json {
                    // NDJSON progress on stderr
                    let evt = json!({
                        "event": "progress",
                        "op": "emit-sweep",
                        "index": i,
                        "total": total,
                        "id": p.id
                    });
                    eprintln!("{}", serde_json::to_string(&evt)?);
                } else {
                    eprintln!("[{:3}/{}] {}", i + 1, total, p.id);
                }

                let stem = output_dir.join(&p.id);
                match spec_version {
                    vrm_ops::SpecVersion::V0 => emit_with_sidecars_v0(p, &stem)?,
                    vrm_ops::SpecVersion::V1 => emit_with_sidecars(p, &stem)?,
                }
                emitted.push(stem);
            }

            if emit_json {
                let summary = json!({
                    "ok": true,
                    "count": emitted.len(),
                    "output_dir": output_dir,
                    "assets": emitted
                });
                println!("{}", serde_json::to_string(&summary)?);
            } else {
                println!("emitted {} assets to {}", emitted.len(), output_dir);
            }
            Ok(())
        }
        Cmd::EmitPbrTexturesSweep {
            output_dir,
            spec_version,
            json: emit_json,
        } => {
            use crate::sweep::mtoon_pbr_textures_sweep;
            std::fs::create_dir_all(&output_dir)?;
            let assets = mtoon_pbr_textures_sweep();
            let total = assets.len();
            let mut emitted = Vec::new();
            for (i, p) in assets.iter().enumerate() {
                if emit_json {
                    let evt = json!({
                        "event": "progress",
                        "op": "emit-pbr-textures-sweep",
                        "index": i,
                        "total": total,
                        "id": p.id
                    });
                    eprintln!("{}", serde_json::to_string(&evt)?);
                } else {
                    eprintln!("[{:3}/{}] {}", i + 1, total, p.id);
                }
                let stem = output_dir.join(&p.id);
                match spec_version {
                    vrm_ops::SpecVersion::V0 => emit_with_sidecars_v0(p, &stem)?,
                    vrm_ops::SpecVersion::V1 => emit_with_sidecars(p, &stem)?,
                }
                emitted.push(stem);
            }
            if emit_json {
                let summary = json!({"ok": true, "count": emitted.len(), "output_dir": output_dir, "assets": emitted});
                println!("{}", serde_json::to_string(&summary)?);
            } else {
                println!(
                    "emitted {} PBR-textures sweep assets to {}",
                    emitted.len(),
                    output_dir
                );
            }
            Ok(())
        }
        Cmd::EmitOutlineWidthMultiplyTextureSweep {
            output_dir,
            spec_version,
            json: emit_json,
        } => {
            use crate::sweep::mtoon_outline_width_multiply_texture_sweep;
            std::fs::create_dir_all(&output_dir)?;
            let assets = mtoon_outline_width_multiply_texture_sweep();
            let total = assets.len();
            let mut emitted = Vec::new();
            for (i, p) in assets.iter().enumerate() {
                if emit_json {
                    let evt = json!({
                        "event": "progress",
                        "op": "emit-outline-width-multiply-texture-sweep",
                        "index": i,
                        "total": total,
                        "id": p.id
                    });
                    eprintln!("{}", serde_json::to_string(&evt)?);
                } else {
                    eprintln!("[{:3}/{}] {}", i + 1, total, p.id);
                }
                let stem = output_dir.join(&p.id);
                match spec_version {
                    vrm_ops::SpecVersion::V0 => emit_with_sidecars_v0(p, &stem)?,
                    vrm_ops::SpecVersion::V1 => emit_with_sidecars(p, &stem)?,
                }
                emitted.push(stem);
            }
            if emit_json {
                let summary = json!({"ok": true, "count": emitted.len(), "output_dir": output_dir, "assets": emitted});
                println!("{}", serde_json::to_string(&summary)?);
            } else {
                println!(
                    "emitted {} outlineWidthMultiplyTexture sweep assets to {}",
                    emitted.len(),
                    output_dir
                );
            }
            Ok(())
        }
        Cmd::EmitShadingShiftTextureSweep {
            output_dir,
            json: emit_json,
        } => {
            use crate::sweep::mtoon_shading_shift_texture_sweep;
            std::fs::create_dir_all(&output_dir)?;
            let assets = mtoon_shading_shift_texture_sweep();
            let total = assets.len();
            let mut emitted = Vec::new();
            for (i, p) in assets.iter().enumerate() {
                if emit_json {
                    let evt = json!({
                        "event": "progress",
                        "op": "emit-shading-shift-texture-sweep",
                        "index": i,
                        "total": total,
                        "id": p.id
                    });
                    eprintln!("{}", serde_json::to_string(&evt)?);
                } else {
                    eprintln!("[{:3}/{}] {}", i + 1, total, p.id);
                }
                let stem = output_dir.join(&p.id);
                emit_with_sidecars(p, &stem)?;
                emitted.push(stem);
            }
            if emit_json {
                let summary = json!({"ok": true, "count": emitted.len(), "output_dir": output_dir, "assets": emitted});
                println!("{}", serde_json::to_string(&summary)?);
            } else {
                println!(
                    "emitted {} shadingShiftTexture sweep assets to {}",
                    emitted.len(),
                    output_dir
                );
            }
            Ok(())
        }
        Cmd::EmitRimMultiplyTextureSweep {
            output_dir,
            json: emit_json,
        } => {
            use crate::sweep::mtoon_rim_multiply_texture_sweep;
            std::fs::create_dir_all(&output_dir)?;
            let assets = mtoon_rim_multiply_texture_sweep();
            let total = assets.len();
            let mut emitted = Vec::new();
            for (i, p) in assets.iter().enumerate() {
                if emit_json {
                    let evt = json!({
                        "event": "progress",
                        "op": "emit-rim-multiply-texture-sweep",
                        "index": i,
                        "total": total,
                        "id": p.id
                    });
                    eprintln!("{}", serde_json::to_string(&evt)?);
                } else {
                    eprintln!("[{:3}/{}] {}", i + 1, total, p.id);
                }
                let stem = output_dir.join(&p.id);
                emit_with_sidecars(p, &stem)?;
                emitted.push(stem);
            }
            if emit_json {
                let summary = json!({"ok": true, "count": emitted.len(), "output_dir": output_dir, "assets": emitted});
                println!("{}", serde_json::to_string(&summary)?);
            } else {
                println!(
                    "emitted {} rimMultiplyTexture sweep assets to {}",
                    emitted.len(),
                    output_dir
                );
            }
            Ok(())
        }
        Cmd::EmitMatcapTextureSweep {
            output_dir,
            spec_version,
            json: emit_json,
        } => {
            use crate::sweep::mtoon_matcap_texture_sweep;
            std::fs::create_dir_all(&output_dir)?;
            let assets = mtoon_matcap_texture_sweep();
            let total = assets.len();
            let mut emitted = Vec::new();
            for (i, p) in assets.iter().enumerate() {
                if emit_json {
                    let evt = json!({
                        "event": "progress",
                        "op": "emit-matcap-texture-sweep",
                        "index": i,
                        "total": total,
                        "id": p.id
                    });
                    eprintln!("{}", serde_json::to_string(&evt)?);
                } else {
                    eprintln!("[{:3}/{}] {}", i + 1, total, p.id);
                }
                let stem = output_dir.join(&p.id);
                match spec_version {
                    vrm_ops::SpecVersion::V0 => emit_with_sidecars_v0(p, &stem)?,
                    vrm_ops::SpecVersion::V1 => emit_with_sidecars(p, &stem)?,
                }
                emitted.push(stem);
            }
            if emit_json {
                let summary = json!({
                    "ok": true,
                    "count": emitted.len(),
                    "output_dir": output_dir,
                    "assets": emitted
                });
                println!("{}", serde_json::to_string(&summary)?);
            } else {
                println!(
                    "emitted {} matcapTexture sweep assets to {}",
                    emitted.len(),
                    output_dir
                );
            }
            Ok(())
        }
        Cmd::EmitShadeMultiplyTextureSweep {
            output_dir,
            spec_version,
            json: emit_json,
        } => {
            use crate::sweep::mtoon_shade_multiply_texture_sweep;
            std::fs::create_dir_all(&output_dir)?;
            let assets = mtoon_shade_multiply_texture_sweep();
            let total = assets.len();
            let mut emitted = Vec::new();
            for (i, p) in assets.iter().enumerate() {
                if emit_json {
                    let evt = json!({
                        "event": "progress",
                        "op": "emit-shade-multiply-texture-sweep",
                        "index": i,
                        "total": total,
                        "id": p.id
                    });
                    eprintln!("{}", serde_json::to_string(&evt)?);
                } else {
                    eprintln!("[{:3}/{}] {}", i + 1, total, p.id);
                }
                let stem = output_dir.join(&p.id);
                match spec_version {
                    vrm_ops::SpecVersion::V0 => emit_with_sidecars_v0(p, &stem)?,
                    vrm_ops::SpecVersion::V1 => emit_with_sidecars(p, &stem)?,
                }
                emitted.push(stem);
            }
            if emit_json {
                let summary = json!({
                    "ok": true,
                    "count": emitted.len(),
                    "output_dir": output_dir,
                    "assets": emitted
                });
                println!("{}", serde_json::to_string(&summary)?);
            } else {
                println!(
                    "emitted {} shadeMultiplyTexture sweep assets to {}",
                    emitted.len(),
                    output_dir
                );
            }
            Ok(())
        }
        Cmd::EmitTextureTransformSweep {
            output_dir,
            json: emit_json,
        } => {
            use crate::sweep::mtoon_texture_transform_sweep;
            std::fs::create_dir_all(&output_dir)?;
            let assets = mtoon_texture_transform_sweep();
            let total = assets.len();
            let mut emitted = Vec::new();
            for (i, p) in assets.iter().enumerate() {
                if emit_json {
                    let evt = json!({
                        "event": "progress",
                        "op": "emit-texture-transform-sweep",
                        "index": i,
                        "total": total,
                        "id": p.id
                    });
                    eprintln!("{}", serde_json::to_string(&evt)?);
                } else {
                    eprintln!("[{:3}/{}] {}", i + 1, total, p.id);
                }
                let stem = output_dir.join(&p.id);
                emit_with_sidecars(p, &stem)?;
                emitted.push(stem);
            }
            if emit_json {
                let summary = json!({
                    "ok": true,
                    "count": emitted.len(),
                    "output_dir": output_dir,
                    "assets": emitted
                });
                println!("{}", serde_json::to_string(&summary)?);
            } else {
                println!(
                    "emitted {} texture-transform sweep assets to {}",
                    emitted.len(),
                    output_dir
                );
            }
            Ok(())
        }
        Cmd::EmitFirstPersonSweep {
            output_dir,
            spec_version,
            json: emit_json,
        } => {
            use crate::sweep::mtoon_first_person_sweep;
            std::fs::create_dir_all(&output_dir)?;
            let assets = mtoon_first_person_sweep();
            let total = assets.len();

            let mut emitted = Vec::new();
            for (i, p) in assets.iter().enumerate() {
                if emit_json {
                    let evt = json!({
                        "event": "progress",
                        "op": "emit-first-person-sweep",
                        "index": i,
                        "total": total,
                        "id": p.id
                    });
                    eprintln!("{}", serde_json::to_string(&evt)?);
                } else {
                    eprintln!("[{:3}/{}] {}", i + 1, total, p.id);
                }
                let stem = output_dir.join(&p.id);
                match spec_version {
                    vrm_ops::SpecVersion::V0 => emit_with_sidecars_v0(p, &stem)?,
                    vrm_ops::SpecVersion::V1 => emit_with_sidecars(p, &stem)?,
                }
                emitted.push(stem);
            }

            if emit_json {
                let summary = json!({
                    "ok": true,
                    "count": emitted.len(),
                    "output_dir": output_dir,
                    "assets": emitted
                });
                println!("{}", serde_json::to_string(&summary)?);
            } else {
                println!(
                    "emitted {} firstPerson sweep assets to {}",
                    emitted.len(),
                    output_dir
                );
            }
            Ok(())
        }
        Cmd::EmitEmissiveSweep {
            output_dir,
            spec_version,
            json: emit_json,
        } => {
            use crate::sweep::mtoon_emissive_sweep;
            std::fs::create_dir_all(&output_dir)?;
            let assets = mtoon_emissive_sweep();
            let total = assets.len();

            let mut emitted = Vec::new();
            for (i, p) in assets.iter().enumerate() {
                if emit_json {
                    let evt = json!({
                        "event": "progress",
                        "op": "emit-emissive-sweep",
                        "index": i,
                        "total": total,
                        "id": p.id
                    });
                    eprintln!("{}", serde_json::to_string(&evt)?);
                } else {
                    eprintln!("[{:3}/{}] {}", i + 1, total, p.id);
                }

                let stem = output_dir.join(&p.id);
                match spec_version {
                    vrm_ops::SpecVersion::V0 => emit_with_sidecars_v0(p, &stem)?,
                    vrm_ops::SpecVersion::V1 => emit_with_sidecars(p, &stem)?,
                }
                emitted.push(stem);
            }

            if emit_json {
                let summary = json!({
                    "ok": true,
                    "count": emitted.len(),
                    "output_dir": output_dir,
                    "assets": emitted
                });
                println!("{}", serde_json::to_string(&summary)?);
            } else {
                println!(
                    "emitted {} emissive sweep assets to {}",
                    emitted.len(),
                    output_dir
                );
            }
            Ok(())
        }
        Cmd::EmitSpringbone {
            id,
            output_dir,
            json: emit_json,
        } => {
            use crate::emit::emit_with_sidecars_spring_bone;
            use crate::spring_bone::SpringBoneParams;

            std::fs::create_dir_all(&output_dir)?;
            let stem = output_dir.join(&id);
            let mtoon = MToonParams::defaults(&id);
            let spring = SpringBoneParams::defaults(&id);
            emit_with_sidecars_spring_bone(&mtoon, &spring, &stem)?;

            if emit_json {
                let result = json!({
                    "ok": true,
                    "outputs": {
                        "vrm": stem.with_extension("vrm"),
                        "meta": stem.with_extension("meta.json"),
                        "test_plan": stem.with_extension("test.yaml")
                    }
                });
                println!("{}", serde_json::to_string(&result)?);
            } else {
                println!("emitted: {}", stem.with_extension("vrm"));
                println!("emitted: {}", stem.with_extension("meta.json"));
                println!("emitted: {}", stem.with_extension("test.yaml"));
            }
            Ok(())
        }
        Cmd::EmitSpringboneSweep {
            output_dir,
            json: emit_json,
        } => {
            use crate::emit::emit_with_sidecars_spring_bone;
            use crate::spring_bone::spring_bone_basic_sweep;

            std::fs::create_dir_all(&output_dir)?;
            let variants = spring_bone_basic_sweep();
            let total = variants.len();

            let mut emitted = Vec::new();
            for (i, spring) in variants.iter().enumerate() {
                if emit_json {
                    let evt = json!({
                        "event": "progress",
                        "op": "emit-springbone-sweep",
                        "index": i,
                        "total": total,
                        "id": spring.id
                    });
                    eprintln!("{}", serde_json::to_string(&evt)?);
                } else {
                    eprintln!("[{:3}/{}] {}", i + 1, total, spring.id);
                }

                let stem = output_dir.join(&spring.id);
                // Paired MToon material is held at default — the spring-bone
                // axis is what's under test, and a varying material would
                // confound the comparison.
                let mtoon = MToonParams::defaults(&spring.id);
                emit_with_sidecars_spring_bone(&mtoon, spring, &stem)?;
                emitted.push(stem);
            }

            if emit_json {
                let summary = json!({
                    "ok": true,
                    "count": emitted.len(),
                    "output_dir": output_dir,
                    "assets": emitted
                });
                println!("{}", serde_json::to_string(&summary)?);
            } else {
                println!(
                    "emitted {} spring-bone assets to {}",
                    emitted.len(),
                    output_dir
                );
            }
            Ok(())
        }
        Cmd::EmitSpringboneSwing {
            id,
            output_dir,
            json: emit_json,
        } => {
            use crate::emit::emit_with_sidecars_spring_bone_swing;
            use crate::spring_bone::SpringBoneParams;

            // Swing variants share semantics with settle variants — same
            // chain config, different test-plan kinematics — so the
            // emitted ID is prefixed `swing_` to keep them distinct in
            // the cross-renderer goldens manifest. Without this, the
            // bootstrap loop overwrites the settle render for every
            // (test_id, renderer) pair.
            let id = format!("swing_{id}");
            std::fs::create_dir_all(&output_dir)?;
            let stem = output_dir.join(&id);
            let mtoon = MToonParams::defaults(&id);
            let spring = SpringBoneParams::defaults(&id);
            emit_with_sidecars_spring_bone_swing(&mtoon, &spring, &stem)?;

            if emit_json {
                let result = json!({
                    "ok": true,
                    "outputs": {
                        "vrm": stem.with_extension("vrm"),
                        "meta": stem.with_extension("meta.json"),
                        "test_plan": stem.with_extension("test.yaml")
                    }
                });
                println!("{}", serde_json::to_string(&result)?);
            } else {
                println!("emitted: {}", stem.with_extension("vrm"));
                println!("emitted: {}", stem.with_extension("meta.json"));
                println!("emitted: {}", stem.with_extension("test.yaml"));
            }
            Ok(())
        }
        Cmd::EmitSpringboneSwingSweep {
            output_dir,
            json: emit_json,
        } => {
            use crate::emit::emit_with_sidecars_spring_bone_swing;
            use crate::spring_bone::spring_bone_basic_sweep;

            std::fs::create_dir_all(&output_dir)?;
            let variants = spring_bone_basic_sweep();
            let total = variants.len();

            let mut emitted = Vec::new();
            for (i, spring) in variants.iter().enumerate() {
                // Each cell ID gets a `swing_` prefix so it can't collide
                // with the settle-sweep variant of the same parameter
                // axis when both are rendered into the same goldens
                // manifest. The underlying SpringBoneParams stays as the
                // sweep generator produced it (the chain config is
                // identical between settle and swing — only the test plan
                // differs); only the emitted asset's filename + the
                // test plan's `id` field carry the prefix.
                let swing_id = format!("swing_{}", spring.id);
                if emit_json {
                    let evt = json!({
                        "event": "progress",
                        "op": "emit-springbone-swing-sweep",
                        "index": i,
                        "total": total,
                        "id": swing_id
                    });
                    eprintln!("{}", serde_json::to_string(&evt)?);
                } else {
                    eprintln!("[{:3}/{}] {}", i + 1, total, swing_id);
                }

                // Underlying SpringBoneParams stays as the sweep generator
                // produced it (chain config is identical between settle and
                // swing). Only the emitted asset's filename + the test
                // plan's `id` field get the `swing_` prefix — that's what
                // keeps the cross-renderer goldens manifest from collapsing
                // settle and swing into the same entry.
                let mut prefixed = spring.clone();
                prefixed.id = swing_id.clone();
                prefixed.spring_name = format!("{swing_id}_chain");
                let stem = output_dir.join(&swing_id);
                let mtoon = MToonParams::defaults(&swing_id);
                emit_with_sidecars_spring_bone_swing(&mtoon, &prefixed, &stem)?;
                emitted.push(stem);
            }

            if emit_json {
                let summary = json!({
                    "ok": true,
                    "count": emitted.len(),
                    "output_dir": output_dir,
                    "assets": emitted
                });
                println!("{}", serde_json::to_string(&summary)?);
            } else {
                println!(
                    "emitted {} swing spring-bone assets to {}",
                    emitted.len(),
                    output_dir
                );
            }
            Ok(())
        }
        Cmd::EmitSpringboneCouplingSweep {
            output_dir,
            json: emit_json,
        } => {
            use crate::emit::{
                emit_with_sidecars_spring_bone, emit_with_sidecars_spring_bone_swing,
            };
            use crate::spring_bone::spring_bone_coupling_sweep;

            std::fs::create_dir_all(&output_dir)?;
            let variants = spring_bone_coupling_sweep();
            let total = variants.len() * 2;

            let mut emitted = Vec::new();
            for (i, spring) in variants.iter().enumerate() {
                // Settle variant.
                if emit_json {
                    eprintln!(
                        "{}",
                        serde_json::to_string(&json!({
                            "event": "progress",
                            "op": "emit-springbone-coupling-sweep",
                            "index": i * 2,
                            "total": total,
                            "id": spring.id
                        }))?
                    );
                } else {
                    eprintln!("[{:3}/{}] {}", i * 2 + 1, total, spring.id);
                }
                let stem = output_dir.join(&spring.id);
                let mtoon = MToonParams::defaults(&spring.id);
                emit_with_sidecars_spring_bone(&mtoon, spring, &stem)?;
                emitted.push(stem);

                // Swing variant (load-bearing for the matrix runner —
                // static settle on a stiff 4-joint chain doesn't excite
                // gravity enough to differentiate gravity_0 from gravity_1).
                let swing_id = format!("swing_{}", spring.id);
                if emit_json {
                    eprintln!(
                        "{}",
                        serde_json::to_string(&json!({
                            "event": "progress",
                            "op": "emit-springbone-coupling-sweep",
                            "index": i * 2 + 1,
                            "total": total,
                            "id": swing_id
                        }))?
                    );
                } else {
                    eprintln!("[{:3}/{}] {}", i * 2 + 2, total, swing_id);
                }
                let mut prefixed = spring.clone();
                prefixed.id = swing_id.clone();
                prefixed.spring_name = format!("{swing_id}_chain");
                let stem = output_dir.join(&swing_id);
                let mtoon = MToonParams::defaults(&swing_id);
                emit_with_sidecars_spring_bone_swing(&mtoon, &prefixed, &stem)?;
                emitted.push(stem);
            }

            if emit_json {
                println!(
                    "{}",
                    serde_json::to_string(&json!({
                        "ok": true,
                        "count": emitted.len(),
                        "output_dir": output_dir,
                        "assets": emitted
                    }))?
                );
            } else {
                println!(
                    "emitted {} coupling-sweep assets (4 variants × settle + swing) to {}",
                    emitted.len(),
                    output_dir
                );
            }
            Ok(())
        }
        Cmd::EmitSequenceSweep {
            output_dir,
            json: emit_json,
        } => {
            use crate::emit::emit_with_sidecars_spring_bone_swing_sequence;
            use crate::spring_bone::spring_bone_basic_sweep;

            std::fs::create_dir_all(&output_dir)?;
            let variants = spring_bone_basic_sweep();
            let total = variants.len();

            let mut emitted = Vec::new();
            for (i, spring) in variants.iter().enumerate() {
                let seq_id = format!("swing_seq_{}", spring.id);
                if emit_json {
                    let evt = json!({
                        "event": "progress",
                        "op": "emit-sequence-sweep",
                        "index": i,
                        "total": total,
                        "id": seq_id
                    });
                    eprintln!("{}", serde_json::to_string(&evt)?);
                } else {
                    eprintln!("[{:3}/{}] {}", i + 1, total, seq_id);
                }

                let mut prefixed = spring.clone();
                prefixed.id = seq_id.clone();
                prefixed.spring_name = format!("{seq_id}_chain");
                let stem = output_dir.join(&seq_id);
                let mtoon = MToonParams::defaults(&seq_id);
                emit_with_sidecars_spring_bone_swing_sequence(&mtoon, &prefixed, &stem)?;
                emitted.push(stem);
            }

            if emit_json {
                let summary = json!({
                    "ok": true,
                    "count": emitted.len(),
                    "output_dir": output_dir,
                    "assets": emitted
                });
                println!("{}", serde_json::to_string(&summary)?);
            } else {
                println!(
                    "emitted {} sequence-mode spring-bone assets to {}",
                    emitted.len(),
                    output_dir
                );
            }
            Ok(())
        }
        Cmd::EmitSpringboneColliderSweep {
            output_dir,
            json: emit_json,
        } => {
            use crate::emit::{
                emit_with_sidecars_spring_bone_colliders,
                emit_with_sidecars_spring_bone_colliders_swing,
            };
            use crate::sweep::spring_bone_collider_sweep;

            std::fs::create_dir_all(&output_dir)?;
            let variants = spring_bone_collider_sweep();
            // Each variant emits BOTH a settle and a swing plan — 24 × 2 = 48 plans.
            let total = variants.len() * 2;
            let mut emitted = Vec::new();
            let mut idx = 0;

            for (mtoon, scene) in &variants {
                // Settle variant: ID unchanged (matches the `springbone_collider_*` prefix)
                let settle_id = mtoon.id.clone();
                if emit_json {
                    let evt = json!({
                        "event": "progress",
                        "op": "emit-springbone-collider-sweep",
                        "index": idx,
                        "total": total,
                        "id": settle_id
                    });
                    eprintln!("{}", serde_json::to_string(&evt)?);
                } else {
                    eprintln!("[{:3}/{}] {}", idx + 1, total, settle_id);
                }
                let stem = output_dir.join(&settle_id);
                // Clone mtoon with settle ID (already correct), but ensure scene's spring ID matches.
                emit_with_sidecars_spring_bone_colliders(mtoon, scene, &stem)?;
                emitted.push(stem);
                idx += 1;

                // Swing variant: prefix `swing_` to avoid manifest collisions.
                let swing_id = format!("swing_{}", mtoon.id);
                if emit_json {
                    let evt = json!({
                        "event": "progress",
                        "op": "emit-springbone-collider-sweep",
                        "index": idx,
                        "total": total,
                        "id": swing_id
                    });
                    eprintln!("{}", serde_json::to_string(&evt)?);
                } else {
                    eprintln!("[{:3}/{}] {}", idx + 1, total, swing_id);
                }

                // Build a swing-specific MToonParams with the swing_* id so the
                // test plan's `id` field matches the filename.
                let swing_mtoon = {
                    let mut m = mtoon.clone();
                    m.id = swing_id.clone();
                    m
                };
                // Build a swing-specific scene so the spring name reflects the swing id.
                let swing_scene = {
                    let mut s = scene.clone();
                    s.springs[0].id = swing_id.clone();
                    s.springs[0].spring_name = format!("{swing_id}_chain");
                    s
                };
                let stem = output_dir.join(&swing_id);
                emit_with_sidecars_spring_bone_colliders_swing(&swing_mtoon, &swing_scene, &stem)?;
                emitted.push(stem);
                idx += 1;
            }

            if emit_json {
                let summary = json!({
                    "ok": true,
                    "count": emitted.len(),
                    "output_dir": output_dir,
                    "assets": emitted
                });
                println!("{}", serde_json::to_string(&summary)?);
            } else {
                println!(
                    "emitted {} collider spring-bone assets to {}",
                    emitted.len(),
                    output_dir
                );
            }
            Ok(())
        }
        Cmd::EmitSpringboneExtendedSweep {
            output_dir,
            json: emit_json,
        } => {
            use crate::emit::{
                emit_with_sidecars_spring_bone_extended,
                emit_with_sidecars_spring_bone_extended_swing,
            };
            use crate::sweep::spring_bone_extended_collider_sweep;

            std::fs::create_dir_all(&output_dir)?;
            let variants = spring_bone_extended_collider_sweep();
            // Each variant emits BOTH a settle and a swing plan — 18 × 2 = 36 plans.
            let total = variants.len() * 2;
            let mut emitted = Vec::new();
            let mut idx = 0;

            for (mtoon, scene) in &variants {
                // Settle variant: ID unchanged (matches the `springbone_extended_*` prefix)
                let settle_id = mtoon.id.clone();
                if emit_json {
                    let evt = json!({
                        "event": "progress",
                        "op": "emit-springbone-extended-sweep",
                        "index": idx,
                        "total": total,
                        "id": settle_id
                    });
                    eprintln!("{}", serde_json::to_string(&evt)?);
                } else {
                    eprintln!("[{:3}/{}] {}", idx + 1, total, settle_id);
                }
                let stem = output_dir.join(&settle_id);
                emit_with_sidecars_spring_bone_extended(mtoon, scene, &stem)?;
                emitted.push(stem);
                idx += 1;

                // Swing variant: prefix `swing_` to avoid manifest collisions.
                let swing_id = format!("swing_{}", mtoon.id);
                if emit_json {
                    let evt = json!({
                        "event": "progress",
                        "op": "emit-springbone-extended-sweep",
                        "index": idx,
                        "total": total,
                        "id": swing_id
                    });
                    eprintln!("{}", serde_json::to_string(&evt)?);
                } else {
                    eprintln!("[{:3}/{}] {}", idx + 1, total, swing_id);
                }

                let swing_mtoon = {
                    let mut m = mtoon.clone();
                    m.id = swing_id.clone();
                    m
                };
                let swing_scene = {
                    let mut s = scene.clone();
                    s.springs[0].id = swing_id.clone();
                    s.springs[0].spring_name = format!("{swing_id}_chain");
                    s
                };
                let stem = output_dir.join(&swing_id);
                emit_with_sidecars_spring_bone_extended_swing(&swing_mtoon, &swing_scene, &stem)?;
                emitted.push(stem);
                idx += 1;
            }

            if emit_json {
                let summary = json!({
                    "ok": true,
                    "count": emitted.len(),
                    "output_dir": output_dir,
                    "assets": emitted
                });
                println!("{}", serde_json::to_string(&summary)?);
            } else {
                println!(
                    "emitted {} extended collider spring-bone assets to {}",
                    emitted.len(),
                    output_dir
                );
            }
            Ok(())
        }
        Cmd::EmitSpringboneGravityDirSweep {
            output_dir,
            json: emit_json,
        } => {
            use crate::emit::{
                emit_with_sidecars_spring_bone, emit_with_sidecars_spring_bone_swing,
            };
            use crate::sweep::spring_bone_gravity_dir_sweep;

            std::fs::create_dir_all(&output_dir)?;
            let variants = spring_bone_gravity_dir_sweep();
            // Each variant emits BOTH a settle and a swing plan — 4 × 2 = 8 plans.
            let total = variants.len() * 2;
            let mut emitted = Vec::new();
            let mut idx = 0;

            for spring in &variants {
                // Settle variant: ID unchanged (matches the `springbone_gravity_dir_*` prefix)
                let settle_id = spring.id.clone();
                if emit_json {
                    let evt = json!({
                        "event": "progress",
                        "op": "emit-springbone-gravity-dir-sweep",
                        "index": idx,
                        "total": total,
                        "id": settle_id
                    });
                    eprintln!("{}", serde_json::to_string(&evt)?);
                } else {
                    eprintln!("[{:3}/{}] {}", idx + 1, total, settle_id);
                }
                let stem = output_dir.join(&settle_id);
                let mtoon = MToonParams::defaults(&settle_id);
                emit_with_sidecars_spring_bone(&mtoon, spring, &stem)?;
                emitted.push(stem);
                idx += 1;

                // Swing variant: prefix `swing_` to avoid manifest collisions.
                let swing_id = format!("swing_{}", spring.id);
                if emit_json {
                    let evt = json!({
                        "event": "progress",
                        "op": "emit-springbone-gravity-dir-sweep",
                        "index": idx,
                        "total": total,
                        "id": swing_id
                    });
                    eprintln!("{}", serde_json::to_string(&evt)?);
                } else {
                    eprintln!("[{:3}/{}] {}", idx + 1, total, swing_id);
                }
                let mut prefixed = spring.clone();
                prefixed.id = swing_id.clone();
                prefixed.spring_name = format!("{swing_id}_chain");
                let stem = output_dir.join(&swing_id);
                let swing_mtoon = MToonParams::defaults(&swing_id);
                emit_with_sidecars_spring_bone_swing(&swing_mtoon, &prefixed, &stem)?;
                emitted.push(stem);
                idx += 1;
            }

            if emit_json {
                let summary = json!({
                    "ok": true,
                    "count": emitted.len(),
                    "output_dir": output_dir,
                    "assets": emitted
                });
                println!("{}", serde_json::to_string(&summary)?);
            } else {
                println!(
                    "emitted {} gravity-dir spring-bone assets to {}",
                    emitted.len(),
                    output_dir
                );
            }
            Ok(())
        }
        Cmd::EmitSpringboneTaperSweep {
            output_dir,
            json: emit_json,
        } => {
            use crate::emit::{
                emit_with_sidecars_spring_bone, emit_with_sidecars_spring_bone_swing,
            };
            use crate::sweep::spring_bone_taper_sweep;

            std::fs::create_dir_all(&output_dir)?;
            let variants = spring_bone_taper_sweep();
            // Each variant emits BOTH a settle and a swing plan — 7 × 2 = 14 plans.
            let total = variants.len() * 2;
            let mut emitted = Vec::new();
            let mut idx = 0;

            for spring in &variants {
                // Settle variant: ID unchanged (matches the `springbone_taper_*` prefix)
                let settle_id = spring.id.clone();
                if emit_json {
                    let evt = json!({
                        "event": "progress",
                        "op": "emit-springbone-taper-sweep",
                        "index": idx,
                        "total": total,
                        "id": settle_id
                    });
                    eprintln!("{}", serde_json::to_string(&evt)?);
                } else {
                    eprintln!("[{:3}/{}] {}", idx + 1, total, settle_id);
                }
                let stem = output_dir.join(&settle_id);
                let mtoon = MToonParams::defaults(&settle_id);
                emit_with_sidecars_spring_bone(&mtoon, spring, &stem)?;
                emitted.push(stem);
                idx += 1;

                // Swing variant: prefix `swing_` to avoid manifest collisions.
                let swing_id = format!("swing_{}", spring.id);
                if emit_json {
                    let evt = json!({
                        "event": "progress",
                        "op": "emit-springbone-taper-sweep",
                        "index": idx,
                        "total": total,
                        "id": swing_id
                    });
                    eprintln!("{}", serde_json::to_string(&evt)?);
                } else {
                    eprintln!("[{:3}/{}] {}", idx + 1, total, swing_id);
                }
                let mut prefixed = spring.clone();
                prefixed.id = swing_id.clone();
                prefixed.spring_name = format!("{swing_id}_chain");
                let stem = output_dir.join(&swing_id);
                let swing_mtoon = MToonParams::defaults(&swing_id);
                emit_with_sidecars_spring_bone_swing(&swing_mtoon, &prefixed, &stem)?;
                emitted.push(stem);
                idx += 1;
            }

            if emit_json {
                let summary = json!({
                    "ok": true,
                    "count": emitted.len(),
                    "output_dir": output_dir,
                    "assets": emitted
                });
                println!("{}", serde_json::to_string(&summary)?);
            } else {
                println!(
                    "emitted {} taper spring-bone assets to {}",
                    emitted.len(),
                    output_dir
                );
            }
            Ok(())
        }
        Cmd::EmitSpringboneMultichainSweep {
            output_dir,
            json: emit_json,
        } => {
            use crate::emit::{
                emit_with_sidecars_spring_bone_multichain,
                emit_with_sidecars_spring_bone_multichain_swing,
            };
            use crate::sweep::spring_bone_multichain_sweep;

            std::fs::create_dir_all(&output_dir)?;
            let variants = spring_bone_multichain_sweep();
            // Each variant emits BOTH a settle and a swing plan — 18 × 2 = 36 plans.
            let total = variants.len() * 2;
            let mut emitted = Vec::new();
            let mut idx = 0;

            for (mtoon, scene) in &variants {
                // Settle variant: ID unchanged.
                let settle_id = mtoon.id.clone();
                if emit_json {
                    let evt = json!({
                        "event": "progress",
                        "op": "emit-springbone-multichain-sweep",
                        "index": idx,
                        "total": total,
                        "id": settle_id
                    });
                    eprintln!("{}", serde_json::to_string(&evt)?);
                } else {
                    eprintln!("[{:3}/{}] {}", idx + 1, total, settle_id);
                }
                let stem = output_dir.join(&settle_id);
                emit_with_sidecars_spring_bone_multichain(mtoon, scene, &stem)?;
                emitted.push(stem);
                idx += 1;

                // Swing variant: prefix `swing_` to avoid manifest collisions.
                let swing_id = format!("swing_{}", mtoon.id);
                if emit_json {
                    let evt = json!({
                        "event": "progress",
                        "op": "emit-springbone-multichain-sweep",
                        "index": idx,
                        "total": total,
                        "id": swing_id
                    });
                    eprintln!("{}", serde_json::to_string(&evt)?);
                } else {
                    eprintln!("[{:3}/{}] {}", idx + 1, total, swing_id);
                }
                let swing_mtoon = {
                    let mut m = mtoon.clone();
                    m.id = swing_id.clone();
                    m
                };
                let swing_scene = {
                    let mut s = scene.clone();
                    for (i, sp) in s.springs.iter_mut().enumerate() {
                        sp.id = format!("{swing_id}_chain_{i}");
                        sp.spring_name = format!("{swing_id}_chain_{i}_chain");
                    }
                    s
                };
                let stem = output_dir.join(&swing_id);
                emit_with_sidecars_spring_bone_multichain_swing(&swing_mtoon, &swing_scene, &stem)?;
                emitted.push(stem);
                idx += 1;
            }

            if emit_json {
                let summary = json!({
                    "ok": true,
                    "count": emitted.len(),
                    "output_dir": output_dir,
                    "assets": emitted
                });
                println!("{}", serde_json::to_string(&summary)?);
            } else {
                println!(
                    "emitted {} multichain spring-bone assets to {}",
                    emitted.len(),
                    output_dir
                );
            }
            Ok(())
        }
        Cmd::EmitVrmaHumanoidSweep {
            output_dir,
            json: emit_json,
        } => {
            use crate::emit::emit_vrma_humanoid_triplet;
            use crate::sweep::vrma_humanoid_sweep;

            std::fs::create_dir_all(&output_dir)?;
            let sweep = vrma_humanoid_sweep();
            let total = sweep.len();
            for (i, params) in sweep.iter().enumerate() {
                emit_vrma_humanoid_triplet(&output_dir, params)?;
                if emit_json {
                    eprintln!(
                        r#"{{"event":"progress","op":"emit-vrma-humanoid-sweep","index":{i},"total":{total},"id":"{id}"}}"#,
                        id = params.id,
                    );
                } else {
                    eprintln!("[{:3}/{}] {}", i + 1, total, params.id);
                }
            }
            println!("emitted {total} VRMA humanoid sweep plans to {output_dir}");
            Ok(())
        }
        Cmd::EmitVrmaExpressionSweep {
            output_dir,
            json: emit_json,
        } => {
            use crate::emit::emit_vrma_expression_triplet;
            use crate::sweep::vrma_expression_sweep;

            std::fs::create_dir_all(&output_dir)?;
            let sweep = vrma_expression_sweep();
            let total = sweep.len();
            for (i, params) in sweep.iter().enumerate() {
                emit_vrma_expression_triplet(&output_dir, params)?;
                if emit_json {
                    eprintln!(
                        r#"{{"event":"progress","op":"emit-vrma-expression-sweep","index":{i},"total":{total},"id":"{id}"}}"#,
                        id = params.id,
                    );
                } else {
                    eprintln!("[{:3}/{}] {}", i + 1, total, params.id);
                }
            }
            if emit_json {
                let summary = serde_json::json!({
                    "ok": true,
                    "count": total,
                    "output_dir": output_dir,
                });
                println!("{}", serde_json::to_string(&summary)?);
            } else {
                println!("emitted {total} VRMA expression sweep plans to {output_dir}");
            }
            Ok(())
        }
        Cmd::EmitVrmaLookatSweep {
            output_dir,
            json: emit_json,
        } => {
            use crate::emit::emit_vrma_lookat_triplet;
            use crate::sweep::vrma_lookat_sweep;

            std::fs::create_dir_all(&output_dir)?;
            let sweep = vrma_lookat_sweep();
            let total = sweep.len();
            for (i, params) in sweep.iter().enumerate() {
                emit_vrma_lookat_triplet(&output_dir, params)?;
                if emit_json {
                    eprintln!(
                        r#"{{"event":"progress","op":"emit-vrma-lookat-sweep","index":{i},"total":{total},"id":"{id}"}}"#,
                        id = params.id,
                    );
                } else {
                    eprintln!("[{:3}/{}] {}", i + 1, total, params.id);
                }
            }
            if emit_json {
                let summary = serde_json::json!({
                    "ok": true,
                    "count": total,
                    "output_dir": output_dir,
                });
                println!("{}", serde_json::to_string(&summary)?);
            } else {
                println!("emitted {total} VRMA lookAt sweep plans to {output_dir}");
            }
            Ok(())
        }
        Cmd::EmitMtoonBasicV0Sweep {
            output_dir,
            json: emit_json,
        } => {
            use crate::emit::{emit_not_applicable_marker, emit_with_sidecars_v0};
            use crate::sweep::mtoon_basic_v0_sweep;
            use crate::SweepApplicability;

            std::fs::create_dir_all(&output_dir)?;
            let variants = mtoon_basic_v0_sweep();
            let total = variants.len();
            let mut emitted: Vec<String> = Vec::new();
            let mut skipped: Vec<String> = Vec::new();

            for (i, (params, applicability)) in variants.iter().enumerate() {
                match applicability {
                    SweepApplicability::Applicable => {
                        if emit_json {
                            let evt = json!({
                                "event": "progress",
                                "op": "emit-mtoon-basic-v0-sweep",
                                "index": i,
                                "total": total,
                                "id": params.id,
                                "status": "applicable"
                            });
                            eprintln!("{}", serde_json::to_string(&evt)?);
                        } else {
                            eprintln!("[{:3}/{}] {} (Applicable)", i + 1, total, params.id);
                        }
                        let stem = output_dir.join(&params.id);
                        emit_with_sidecars_v0(params, &stem)?;
                        emitted.push(params.id.clone());
                    }
                    SweepApplicability::NotApplicable { reason } => {
                        if emit_json {
                            let evt = json!({
                                "event": "progress",
                                "op": "emit-mtoon-basic-v0-sweep",
                                "index": i,
                                "total": total,
                                "id": params.id,
                                "status": "not_applicable",
                                "reason": format!("{reason:?}")
                            });
                            eprintln!("{}", serde_json::to_string(&evt)?);
                        } else {
                            eprintln!(
                                "[{:3}/{}] {} (NotApplicable: {:?})",
                                i + 1,
                                total,
                                params.id,
                                reason
                            );
                        }
                        emit_not_applicable_marker(&params.id, *reason, &output_dir)?;
                        skipped.push(params.id.clone());
                    }
                }
            }

            if emit_json {
                let summary = json!({
                    "ok": true,
                    "emitted": emitted.len(),
                    "skipped": skipped.len(),
                    "output_dir": output_dir,
                    "assets": emitted,
                    "not_applicable": skipped
                });
                println!("{}", serde_json::to_string(&summary)?);
            } else {
                println!(
                    "emitted {} v0 MToon basic sweep assets ({} skipped) to {}",
                    emitted.len(),
                    skipped.len(),
                    output_dir
                );
            }
            Ok(())
        }
        Cmd::EmitExpressionsPresetBasic {
            output_dir,
            json: emit_json,
        } => {
            use crate::emit::{
                emit_with_sidecars_v0_with_expressions, emit_with_sidecars_v1_with_expressions,
            };
            use crate::sweep::{
                expressions_preset_basic_v0_sweep, expressions_preset_basic_v1_sweep,
            };

            std::fs::create_dir_all(&output_dir)?;

            let v0_variants = expressions_preset_basic_v0_sweep();
            let v1_variants = expressions_preset_basic_v1_sweep();
            let total = v0_variants.len() + v1_variants.len();
            let mut emitted: Vec<String> = Vec::new();
            let mut idx = 0;

            // Emit v0 variants (blendShapeMaster with real binds).
            for (id, expressions) in &v0_variants {
                if emit_json {
                    let evt = json!({
                        "event": "progress",
                        "op": "emit-expressions-preset-basic",
                        "index": idx,
                        "total": total,
                        "id": id,
                        "spec_version": "0.x"
                    });
                    eprintln!("{}", serde_json::to_string(&evt)?);
                } else {
                    eprintln!("[{:3}/{}] {} (v0)", idx + 1, total, id);
                }
                let params = MToonParams::defaults(id);
                let stem = output_dir.join(id);
                emit_with_sidecars_v0_with_expressions(&params, expressions, &stem)?;
                emitted.push(id.clone());
                idx += 1;
            }

            // Emit v1 variants (VRMC_vrm.expressions.preset with real binds).
            // The mesh node index is not known at sweep-definition time; it is
            // determined here (after the skeleton layout is fixed). For the
            // standard minimal_skeleton, the mesh node is always appended after
            // all skeleton nodes — currently index 13 (12 skeleton + root) for
            // the default skeleton. However, rather than hard-coding that, we
            // let emit_with_sidecars_v1_with_expressions handle it internally:
            // it already builds the geometry and appends the mesh node at the
            // right index. The `node` field in the sweep params is updated
            // at dispatch time.
            for (id, expr_params) in &v1_variants {
                if emit_json {
                    let evt = json!({
                        "event": "progress",
                        "op": "emit-expressions-preset-basic",
                        "index": idx,
                        "total": total,
                        "id": id,
                        "spec_version": "1.0"
                    });
                    eprintln!("{}", serde_json::to_string(&evt)?);
                } else {
                    eprintln!("[{:3}/{}] {} (v1)", idx + 1, total, id);
                }
                let params = MToonParams::defaults(id);
                let stem = output_dir.join(id);
                emit_with_sidecars_v1_with_expressions(&params, expr_params, &stem)?;
                emitted.push(id.clone());
                idx += 1;
            }

            if emit_json {
                let summary = json!({
                    "ok": true,
                    "count": emitted.len(),
                    "output_dir": output_dir,
                    "assets": emitted
                });
                println!("{}", serde_json::to_string(&summary)?);
            } else {
                println!(
                    "emitted {} expressions-preset-basic assets ({} v0 + {} v1) to {}",
                    emitted.len(),
                    v0_variants.len(),
                    v1_variants.len(),
                    output_dir
                );
            }
            Ok(())
        }
        Cmd::EmitMaterialNameClassificationSweep {
            output_dir,
            json: emit_json,
        } => {
            use crate::sweep::material_name_classification_sweep;
            std::fs::create_dir_all(&output_dir)?;
            let assets = material_name_classification_sweep();
            let total = assets.len();

            let mut emitted = Vec::new();
            for (i, p) in assets.iter().enumerate() {
                if emit_json {
                    let evt = json!({
                        "event": "progress",
                        "op": "emit-material-name-classification-sweep",
                        "index": i,
                        "total": total,
                        "id": p.id
                    });
                    eprintln!("{}", serde_json::to_string(&evt)?);
                } else {
                    eprintln!("[{:3}/{}] {}", i + 1, total, p.id);
                }

                let stem = output_dir.join(&p.id);
                emit_with_sidecars(p, &stem)?;
                emitted.push(stem);
            }

            if emit_json {
                let summary = json!({
                    "ok": true,
                    "count": emitted.len(),
                    "output_dir": output_dir,
                    "assets": emitted
                });
                println!("{}", serde_json::to_string(&summary)?);
            } else {
                println!(
                    "emitted {} material-name-classification assets to {}",
                    emitted.len(),
                    output_dir
                );
            }
            Ok(())
        }
        Cmd::EmitDoublesidedSpecTest {
            output_dir,
            json: emit_json,
        } => {
            use crate::emit::emit_doublesided_spec_test_pair;
            let stems = emit_doublesided_spec_test_pair(&output_dir)?;
            if emit_json {
                let summary = json!({
                    "ok": true,
                    "count": stems.len(),
                    "output_dir": output_dir,
                    "assets": stems
                });
                println!("{}", serde_json::to_string(&summary)?);
            } else {
                println!(
                    "emitted {} doubleSided spec-test assets to {}",
                    stems.len(),
                    output_dir
                );
            }
            Ok(())
        }
        Cmd::Describe { format } => {
            let catalog = json!({
                "name": "vrm-asset-generator",
                "version": env!("CARGO_PKG_VERSION"),
                "operations": {
                    "emit-default": {
                        "summary": "Emit a default-MToon asset triplet (.vrm + .meta.json + .test.yaml)",
                        "input_schema": {
                            "type": "object",
                            "required": ["id", "output_dir"],
                            "properties": {
                                "id": { "type": "string" },
                                "output_dir": { "type": "string" }
                            }
                        },
                        "output_schema": {
                            "type": "object",
                            "properties": {
                                "ok": { "type": "boolean" },
                                "outputs": {
                                    "type": "object",
                                    "properties": {
                                        "vrm": { "type": "string" },
                                        "meta": { "type": "string" },
                                        "test_plan": { "type": "string" }
                                    }
                                }
                            }
                        }
                    },
                    "emit-sweep": {
                        "summary": "Emit the full MToon basic sweep (~50 assets) into output_dir/",
                        "input_schema": {
                            "type": "object",
                            "required": ["output_dir"],
                            "properties": {
                                "output_dir": { "type": "string" },
                                "json": {
                                    "type": "boolean",
                                    "description": "Emit NDJSON progress on stderr and a JSON summary on stdout"
                                }
                            }
                        },
                        "output_schema": {
                            "type": "object",
                            "properties": {
                                "ok": { "type": "boolean" },
                                "count": { "type": "integer" },
                                "output_dir": { "type": "string" },
                                "assets": { "type": "array", "items": { "type": "string" } }
                            }
                        }
                    },
                    "emit-springbone": {
                        "summary": "Emit one .vrm with default MToon + default VRMC_springBone chain",
                        "input_schema": {
                            "type": "object",
                            "required": ["id", "output_dir"],
                            "properties": {
                                "id": { "type": "string" },
                                "output_dir": { "type": "string" }
                            }
                        },
                        "output_schema": {
                            "type": "object",
                            "properties": {
                                "ok": { "type": "boolean" },
                                "outputs": {
                                    "type": "object",
                                    "properties": {
                                        "vrm": { "type": "string" },
                                        "meta": { "type": "string" },
                                        "test_plan": { "type": "string" }
                                    }
                                }
                            }
                        }
                    },
                    "emit-springbone-sweep": {
                        "summary": "Emit the full VRMC_springBone parameter sweep (~20 assets)",
                        "input_schema": {
                            "type": "object",
                            "required": ["output_dir"],
                            "properties": {
                                "output_dir": { "type": "string" },
                                "json": {
                                    "type": "boolean",
                                    "description": "Emit NDJSON progress on stderr and a JSON summary on stdout"
                                }
                            }
                        },
                        "output_schema": {
                            "type": "object",
                            "properties": {
                                "ok": { "type": "boolean" },
                                "count": { "type": "integer" },
                                "output_dir": { "type": "string" },
                                "assets": { "type": "array", "items": { "type": "string" } }
                            }
                        }
                    },
                    "emit-springbone-swing": {
                        "summary": "Emit one .vrm with default MToon + spring-bone chain, with a swing animation block in the test plan",
                        "input_schema": {
                            "type": "object",
                            "required": ["id", "output_dir"],
                            "properties": {
                                "id": { "type": "string" },
                                "output_dir": { "type": "string" }
                            }
                        },
                        "output_schema": {
                            "type": "object",
                            "properties": {
                                "ok": { "type": "boolean" },
                                "outputs": {
                                    "type": "object",
                                    "properties": {
                                        "vrm": { "type": "string" },
                                        "meta": { "type": "string" },
                                        "test_plan": { "type": "string" }
                                    }
                                }
                            }
                        }
                    },
                    "emit-springbone-swing-sweep": {
                        "summary": "Swing-variant spring-bone sweep (~20 assets). Same axes as emit-springbone-sweep; every test.yaml carries an animation.root_transform block. Use a different --output-dir than emit-springbone-sweep to avoid overwriting the settle-only test.yaml files.",
                        "input_schema": {
                            "type": "object",
                            "required": ["output_dir"],
                            "properties": {
                                "output_dir": { "type": "string" },
                                "json": {
                                    "type": "boolean",
                                    "description": "Emit NDJSON progress on stderr and a JSON summary on stdout"
                                }
                            }
                        },
                        "output_schema": {
                            "type": "object",
                            "properties": {
                                "ok": { "type": "boolean" },
                                "count": { "type": "integer" },
                                "output_dir": { "type": "string" },
                                "assets": { "type": "array", "items": { "type": "string" } }
                            }
                        }
                    },
                    "emit-springbone-collider-sweep": {
                        "summary": "VRMC_springBone collider sweep (48 assets = 24 Cartesian variants × settle + swing). Axes: shape (sphere, capsule), offset_y (-0.08, -0.04, 0, +0.04), radius (0.03, 0.05, 0.10). Each settle plan uses 60-step settle; swing plans add animate_root_transform.",
                        "input_schema": {
                            "type": "object",
                            "required": ["output_dir"],
                            "properties": {
                                "output_dir": { "type": "string" },
                                "json": {
                                    "type": "boolean",
                                    "description": "Emit NDJSON progress on stderr and a JSON summary on stdout"
                                }
                            }
                        },
                        "output_schema": {
                            "type": "object",
                            "properties": {
                                "ok": { "type": "boolean" },
                                "count": { "type": "integer" },
                                "output_dir": { "type": "string" },
                                "assets": { "type": "array", "items": { "type": "string" } }
                            }
                        }
                    },
                    "emit-springbone-extended-sweep": {
                        "summary": "VRMC_springBone_extended_collider sweep (36 assets = 18 variants × settle + swing). Variants: 3 shapes (plane, inside-sphere, inside-capsule) × 3 placements + 3 shapes × 3 angle-limits (30°, 60°, 90°) = 18 base. Each plan uses 60-step settle; swing plans add animate_root_transform.",
                        "input_schema": {
                            "type": "object",
                            "required": ["output_dir"],
                            "properties": {
                                "output_dir": { "type": "string" },
                                "json": {
                                    "type": "boolean",
                                    "description": "Emit NDJSON progress on stderr and a JSON summary on stdout"
                                }
                            }
                        },
                        "output_schema": {
                            "type": "object",
                            "properties": {
                                "ok": { "type": "boolean" },
                                "count": { "type": "integer" },
                                "output_dir": { "type": "string" },
                                "assets": { "type": "array", "items": { "type": "string" } }
                            }
                        }
                    },
                    "emit-springbone-gravity-dir-sweep": {
                        "summary": "gravity_dir 4-direction sweep (8 assets = 4 directions × settle + swing). Directions: default (-Y), anti (+Y), sideways (+X), oblique (+0.7, -0.7, 0). All other SpringBoneParams held at defaults. Flushes adapters that hard-code gravity_dir = [0,-1,0].",
                        "input_schema": {
                            "type": "object",
                            "required": ["output_dir"],
                            "properties": {
                                "output_dir": { "type": "string" },
                                "json": {
                                    "type": "boolean",
                                    "description": "Emit NDJSON progress on stderr and a JSON summary on stdout"
                                }
                            }
                        },
                        "output_schema": {
                            "type": "object",
                            "properties": {
                                "ok": { "type": "boolean" },
                                "count": { "type": "integer" },
                                "output_dir": { "type": "string" },
                                "assets": { "type": "array", "items": { "type": "string" } }
                            }
                        }
                    },
                    "emit-springbone-taper-sweep": {
                        "summary": "Per-joint taper sweep (14 assets = 7 variants x settle + swing). 4 stiffness shapes (flat, high-to-low, low-to-high, exp-decay) + 3 drag shapes (flat, high-to-low, exp-decay). All use joint_count=4; per-joint vector overrides scalar for each swept axis.",
                        "input_schema": {
                            "type": "object",
                            "required": ["output_dir"],
                            "properties": {
                                "output_dir": { "type": "string" },
                                "json": {
                                    "type": "boolean",
                                    "description": "Emit NDJSON progress on stderr and a JSON summary on stdout"
                                }
                            }
                        },
                        "output_schema": {
                            "type": "object",
                            "properties": {
                                "ok": { "type": "boolean" },
                                "count": { "type": "integer" },
                                "output_dir": { "type": "string" },
                                "assets": { "type": "array", "items": { "type": "string" } }
                            }
                        }
                    },
                    "emit-springbone-multichain-sweep": {
                        "summary": "Multi-chain spring-bone sweep (36 assets = 18 variants × settle + swing). Axes: chain count (2, 3, 5), spacing (0.02, 0.05 m encoded in ID; emit uses 0.05 m), sharing mode (share_all, share_none, share_alt). Exercises per-chain collider-group assignment semantics (VMK#162-class regressions).",
                        "input_schema": {
                            "type": "object",
                            "required": ["output_dir"],
                            "properties": {
                                "output_dir": { "type": "string" },
                                "json": {
                                    "type": "boolean",
                                    "description": "Emit NDJSON progress on stderr and a JSON summary on stdout"
                                }
                            }
                        },
                        "output_schema": {
                            "type": "object",
                            "properties": {
                                "ok": { "type": "boolean" },
                                "count": { "type": "integer" },
                                "output_dir": { "type": "string" },
                                "assets": { "type": "array", "items": { "type": "string" } }
                            }
                        }
                    },
                    "emit-vrma-humanoid-sweep": {
                        "summary": "VRMA humanoid bone sweep (15 plans). Each variant rotates one humanoid bone through a single axis arc over 1 s. Bones: hips, spine, head (3 axes), leftUpperArm (3 axes), rightUpperArm (3 axes), leftUpperLeg (2 axes), leftLowerLeg, neck. Each plan emits a .vrm + .vrma + .test.yaml triplet.",
                        "input_schema": {
                            "type": "object",
                            "required": ["output_dir"],
                            "properties": {
                                "output_dir": { "type": "string" },
                                "json": {
                                    "type": "boolean",
                                    "description": "Emit NDJSON progress on stderr"
                                }
                            }
                        },
                        "output_schema": {
                            "type": "object",
                            "properties": {
                                "ok": { "type": "boolean" },
                                "count": { "type": "integer" },
                                "output_dir": { "type": "string" }
                            }
                        }
                    },
                    "emit-vrma-expression-sweep": {
                        "summary": "VRMA expression sweep (12 plans). Each variant animates a single expression 0 → 1 → 0 over 1 s; test plans sample at peak (t=0.5). Presets: happy, angry, sad, relaxed, surprised, aa, ih, ou, ee, blink. Custom: smug, drowsy. Each plan emits a .vrm + .vrma + .test.yaml triplet.",
                        "input_schema": {
                            "type": "object",
                            "required": ["output_dir"],
                            "properties": {
                                "output_dir": { "type": "string" },
                                "json": {
                                    "type": "boolean",
                                    "description": "Emit NDJSON progress on stderr and a JSON summary on stdout"
                                }
                            }
                        },
                        "output_schema": {
                            "type": "object",
                            "properties": {
                                "ok": { "type": "boolean" },
                                "count": { "type": "integer" },
                                "output_dir": { "type": "string" }
                            }
                        }
                    },
                    "emit-vrma-lookat-sweep": {
                        "summary": "VRMA lookAt sweep (10 plans = 5 directions x 2 avatar configs). Directions: yaw +-60deg, pitch +-30deg, neutral. Avatar configs: bone (VRMC_vrm.lookAt.type: bone) vs expression. Same .vrma gaze tested against both avatar rendering paths. Each plan emits a .vrm + .vrma + .test.yaml triplet.",
                        "input_schema": {
                            "type": "object",
                            "required": ["output_dir"],
                            "properties": {
                                "output_dir": { "type": "string" },
                                "json": {
                                    "type": "boolean",
                                    "description": "Emit NDJSON progress on stderr and a JSON summary on stdout"
                                }
                            }
                        },
                        "output_schema": {
                            "type": "object",
                            "properties": {
                                "ok": { "type": "boolean" },
                                "count": { "type": "integer" },
                                "output_dir": { "type": "string" }
                            }
                        }
                    },
                    "emit-doublesided-spec-test": {
                        "summary": "Emit the doubleSided back-face-culling spec-test pair (doublesided_quad_false/true): an open quad viewed from behind so culling is observable. The false variant's plan carries a cross_variant SSIM assertion requiring the two renders to diverge.",
                        "input_schema": {
                            "type": "object",
                            "required": ["output_dir"],
                            "properties": {
                                "output_dir": { "type": "string" },
                                "json": { "type": "boolean" }
                            }
                        },
                        "output_schema": {
                            "type": "object",
                            "properties": {
                                "ok": { "type": "boolean" },
                                "count": { "type": "integer" },
                                "output_dir": { "type": "string" },
                                "assets": { "type": "array", "items": { "type": "string" } }
                            }
                        }
                    }
                }
            });
            match format {
                DescribeFormat::Json => println!("{}", serde_json::to_string_pretty(&catalog)?),
                DescribeFormat::Text => println!("{catalog:#?}"),
            }
            Ok(())
        }
    }
}
