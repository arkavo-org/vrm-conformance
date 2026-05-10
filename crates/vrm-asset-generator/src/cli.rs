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
                        }
                    }
                }
            });
            match format {
                DescribeFormat::Json => println!("{}", serde_json::to_string_pretty(&catalog)?),
                DescribeFormat::Text => println!("{:#?}", catalog),
            }
            Ok(())
        }
    }
}
