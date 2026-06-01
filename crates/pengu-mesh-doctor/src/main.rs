use anyhow::Result;
use clap::Parser;
use pengu_mesh_doctor::{build_report, build_setup_wizard, render_setup_wizard};
use pengu_mesh_shared::OperationOutcome;

#[derive(Parser)]
struct Args {
    #[arg(long)]
    json: bool,

    #[arg(long)]
    setup_wizard: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    if args.setup_wizard {
        let wizard = build_setup_wizard()?;
        if args.json {
            let payload = OperationOutcome::success("setup wizard", wizard);
            println!("{}", serde_json::to_string_pretty(&payload)?);
        } else {
            print!("{}", render_setup_wizard(&wizard));
        }
        return Ok(());
    }

    let payload = OperationOutcome::success("doctor report", build_report()?);
    if args.json {
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!("doctor report generated at {}", payload.timestamp);
    }
    Ok(())
}
