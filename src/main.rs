use std::path::PathBuf;
use std::sync::Arc;

use clap::{Args, Parser, Subcommand};

use crate::build::windows::WindowsBuild;
use crate::build::BuildSteps;
use crate::bundle::{read_kbdgen_bundle, Error, KbdgenBundle};

mod build;
mod bundle;

fn main() -> Result<(), Error> {
    let cli = Cli::parse();

    match &cli.command {
        Command::Target(target_command_struct) => {
            let bundle_path = target_command_struct.bundle_path.clone();
            let bundle = read_kbdgen_bundle(&bundle_path)?;

            println!("Output Path: {:?}", &target_command_struct.output_path);
            std::fs::create_dir_all(&target_command_struct.output_path)?;

            //println!("Printing bundle for kicks: {:?}", bundle);

            match &target_command_struct.target_command {
                TargetCommand::Windows(_windows_command) => {
                    let mut build = WindowsBuild {
                        bundle: Arc::new(bundle),
                        output_path: target_command_struct.output_path.clone(),
                        steps: vec![],
                    };

                    build.populate_steps(); // This shouldn't be a thing
                    build.build_full();
                }
                TargetCommand::Svg(_svg_command) => {
                    println!("whee");
                }
            }
        }
    };

    Ok(())
}

#[derive(Parser)]
#[clap(version)]
struct Cli {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Target(TargetCommandStruct),
}

#[derive(Subcommand)]
enum TargetCommand {
    Svg(TargetSvgCommand),
    Windows(TargetWindowsCommand),
}

#[derive(Args)]
struct TargetCommandStruct {
    #[clap(subcommand)]
    target_command: TargetCommand,

    #[clap(parse(from_os_str))]
    bundle_path: PathBuf,

    #[clap(parse(from_os_str))]
    output_path: PathBuf,
}

#[derive(Parser)]
struct TargetSvgCommand {
    #[clap(short, long)]
    aaa: Option<bool>,
}

#[derive(Parser)]
struct TargetWindowsCommand {
    #[clap(short, long)]
    boop: Option<String>,
}
