use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::bundle::{read_kbdgen_bundle, Error};

mod bundle;

fn main() -> Result<(), Error> {
    let cli = Cli::parse();

    match &cli.command {
        Command::Target(target_command_struct) => {
            let bundle_path = target_command_struct.bundle_path.clone();
            let bundle = read_kbdgen_bundle(&bundle_path)?;

            println!("Printing bundle for kicks: {:?}", bundle);

            match &target_command_struct.target_command {
                TargetCommand::Svg(svg_command) => {
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
}

#[derive(Args)]
struct TargetCommandStruct {
    #[clap(subcommand)]
    target_command: TargetCommand,

    #[clap(parse(from_os_str))]
    bundle_path: PathBuf,
}

#[derive(Parser)]
struct TargetSvgCommand {
    #[clap(short, long)]
    aaa: Option<bool>,
}
