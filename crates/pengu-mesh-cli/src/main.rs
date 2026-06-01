use anyhow::Result;
use clap::{Parser, Subcommand};
use pengu_mesh_core::StageOneRuntime;
use pengu_mesh_mcp::{ToolCallRequest, core_tools, execute_tool};
use pengu_mesh_shared::{IdKind, StableId};
use serde_json::Value;

#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    command: CommandSet,
}

#[derive(Subcommand)]
enum CommandSet {
    Contract,
    SampleIds,
    Health,
    Call {
        #[arg(long)]
        tool: String,
        #[arg(long, default_value = "{}")]
        args: String,
    },
}

fn main() -> Result<()> {
    let args = Args::parse();
    let runtime = StageOneRuntime::new_with_entrypoint("pengu-mesh-cli")?;
    match args.command {
        CommandSet::Contract => {
            for tool in core_tools() {
                println!("{}\t{}", tool.name, tool.summary);
            }
            Ok(())
        }
        CommandSet::SampleIds => {
            for (kind, seed) in [
                (IdKind::Profile, "default"),
                (IdKind::Instance, "chrome-dev"),
                (IdKind::Tab, "landing-page"),
                (IdKind::Run, "bootstrap"),
                (IdKind::Lease, "writer"),
                (IdKind::Event, "capture-start"),
                (IdKind::Artifact, "screenshot"),
            ] {
                println!("{}", StableId::new(kind, seed).as_str());
            }
            Ok(())
        }
        CommandSet::Health => {
            println!(
                "{}",
                serde_json::to_string_pretty(&runtime.browser_health()?)?
            );
            Ok(())
        }
        CommandSet::Call { tool, args } => {
            let args: Value = serde_json::from_str(&args)?;
            let payload = execute_tool(&runtime, ToolCallRequest { tool, args })?;
            println!("{}", serde_json::to_string_pretty(&payload)?);
            Ok(())
        }
    }
}
