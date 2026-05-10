use crate::execute::{execute_plan, load_plan, ExecuteOptions};
use anyhow::Result;
use camino::Utf8PathBuf;
use clap::{Parser, Subcommand, ValueEnum};
use serde_json::json;

#[derive(Debug, Parser)]
#[command(version, about = "VRM conformance runner")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Cmd,
}

#[derive(Debug, Subcommand)]
pub enum Cmd {
    /// Execute a test plan against one renderer adapter.
    ExecuteTestPlan {
        #[arg(long)]
        plan: Utf8PathBuf,
        #[arg(long)]
        adapter_bin: Utf8PathBuf,
        #[arg(long, value_delimiter = ' ', num_args = 0..)]
        adapter_args: Vec<String>,
        #[arg(long)]
        asset_dir: Utf8PathBuf,
        #[arg(long)]
        output_dir: Utf8PathBuf,
        #[arg(long, default_value = "vrm-metal-kit")]
        renderer_name: String,
        #[arg(long)]
        json: bool,
    },
    /// Cost-preview a test plan without executing.
    PlanTestPlan {
        #[arg(long)]
        plan: Utf8PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Print the operation catalog.
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
        Cmd::ExecuteTestPlan {
            plan,
            adapter_bin,
            adapter_args,
            asset_dir,
            output_dir,
            renderer_name,
            json: emit_json,
        } => {
            let plan_value = load_plan(&plan)?;
            let opts = ExecuteOptions {
                adapter_bin,
                adapter_args,
                asset_dir,
                output_dir,
                renderer_name,
                emit_progress_ndjson: emit_json,
            };
            let result = execute_plan(&plan_value, &opts)?;
            if emit_json {
                let summary = json!({
                    "ok": true,
                    "test_id": result.test_id,
                    "renderer": result.renderer,
                    "output_png": result.output_png,
                    "actual_color_space": format!("{:?}", result.actual_color_space)
                });
                println!("{}", serde_json::to_string(&summary)?);
            } else {
                println!("rendered {} → {}", result.test_id, result.output_png);
            }
            Ok(())
        }
        Cmd::PlanTestPlan {
            plan,
            json: emit_json,
        } => {
            let p = load_plan(&plan)?;
            // v0.1 trivial estimate: one render.
            let preview = json!({
                "ok": true,
                "test_id": p.id,
                "estimated_renders": 1,
                "estimated_seconds": 4.0,
                "outputs": [
                    format!("{}_{{renderer}}.png", p.id)
                ]
            });
            if emit_json {
                println!("{}", serde_json::to_string(&preview)?);
            } else {
                println!("would render: {}", p.id);
            }
            Ok(())
        }
        Cmd::Describe { format } => {
            let catalog = json!({
                "name": "vrm-runner",
                "version": env!("CARGO_PKG_VERSION"),
                "operations": {
                    "execute-test-plan": {
                        "summary": "Execute a YAML test plan against one renderer adapter; emit a PNG and JSON status",
                        "input_schema": {
                            "type": "object",
                            "required": ["plan", "adapter_bin", "asset_dir", "output_dir"],
                            "properties": {
                                "plan": { "type": "string" },
                                "adapter_bin": { "type": "string" },
                                "adapter_args": { "type": "array", "items": { "type": "string" } },
                                "asset_dir": { "type": "string" },
                                "output_dir": { "type": "string" }
                            }
                        },
                        "output_schema": {
                            "type": "object",
                            "properties": {
                                "ok": { "type": "boolean" },
                                "test_id": { "type": "string" },
                                "renderer": { "type": "string" },
                                "output_png": { "type": "string" },
                                "actual_color_space": { "type": "string" }
                            }
                        }
                    },
                    "plan-test-plan": {
                        "summary": "Cost-preview a test plan without executing",
                        "input_schema": {
                            "type": "object",
                            "required": ["plan"],
                            "properties": {
                                "plan": { "type": "string" }
                            }
                        },
                        "output_schema": {
                            "type": "object",
                            "properties": {
                                "ok": { "type": "boolean" },
                                "test_id": { "type": "string" },
                                "estimated_renders": { "type": "integer" },
                                "estimated_seconds": { "type": "number" },
                                "outputs": { "type": "array", "items": { "type": "string" } }
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
