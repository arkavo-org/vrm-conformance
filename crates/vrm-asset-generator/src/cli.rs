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
