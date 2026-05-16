use crate::emit::emit_with_sidecars;
use crate::params::MToonParams;
use anyhow::Result;
use camino::Utf8PathBuf;
use clap::{Parser, Subcommand, ValueEnum};
use serde_json::json;

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
        /// Emit JSON status to stdout instead of human text.
        #[arg(long)]
        json: bool,
    },

    /// Emit the full MToon basic sweep (~50 assets) into output_dir/.
    EmitSweep {
        #[arg(long)]
        output_dir: Utf8PathBuf,
        /// Emit JSON progress on stderr (NDJSON) and a final JSON summary on stdout.
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
            json: emit_json,
        } => {
            std::fs::create_dir_all(&output_dir)?;
            let stem = output_dir.join(&id);
            let params = MToonParams::defaults(&id);
            emit_with_sidecars(&params, &stem)?;

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
                println!("emitted {} assets to {}", emitted.len(), output_dir);
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
