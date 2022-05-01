use std::path::PathBuf;
use std::sync::Arc;

use bundle::layout::MacOsTarget;
use bundle::KbdgenBundle;
use clap::{Args, Parser, Subcommand};

use crate::build::macos::MacOsBuild;
use crate::build::svg::SvgBuild;
use crate::build::windows::WindowsBuild;
use crate::build::BuildSteps;
use crate::bundle::{read_kbdgen_bundle, Error};

mod build;
mod bundle;
mod util;

async fn macos_target(
    bundle: Arc<KbdgenBundle>,
    output_path: PathBuf,
    target: &TargetMacOs,
) -> anyhow::Result<()> {
    match target.command {
        TargetMacOsCommand::Build(_) => {
            let mut build = MacOsBuild {
                bundle,
                output_path,
                steps: vec![],
            };

            build.populate_steps(); // This shouldn't be a thing
            build.build_full().await;
        }
        TargetMacOsCommand::Generate(_) => {
            // These are currently equivalent to Build because packaging isn't done yet
            let mut build = MacOsBuild {
                bundle,
                output_path,
                steps: vec![],
            };

            build.populate_steps(); // This shouldn't be a thing
            build.build_full().await;
        }
        TargetMacOsCommand::Package(_) => todo!(),
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match &cli.command {
        Command::Target(target_command_struct) => {
            let bundle_path = &target_command_struct.bundle_path;
            let bundle = Arc::new(read_kbdgen_bundle(&bundle_path)?);
            let output_path = target_command_struct.output_path.to_path_buf();

            tracing::info!("Output Path: {:?}", &output_path);
            std::fs::create_dir_all(&output_path)?;

            match &target_command_struct.target_command {
                TargetCommand::Windows(_windows_command) => {
                    let mut build = WindowsBuild {
                        bundle,
                        output_path: target_command_struct.output_path.clone(),
                        steps: vec![],
                    };

                    build.populate_steps(); // This shouldn't be a thing
                    build.build_full().await;
                }
                TargetCommand::MacOs(target) => {
                    macos_target(bundle, output_path, target).await?;
                }
                TargetCommand::Svg(_svg_command) => {
                    let mut build = SvgBuild {
                        bundle,
                        output_path: target_command_struct.output_path.clone(),
                        steps: vec![],
                    };

                    build.populate_steps(); // This shouldn't be a thing
                    build.build_full().await;
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
    #[clap(about = "Functionality relating to specific targets")]
    Target(TargetCommandStruct),
}

#[derive(Subcommand)]
enum TargetCommand {
    #[clap(about = "Windows functionality")]
    Windows(TargetWindowsCommand),
    #[clap(name = "macos", about = "macOS functionality")]
    MacOs(TargetMacOs),
    #[clap(name = "macos", about = "SVG functionality")]
    Svg(TargetSvgCommand),
}

#[derive(Args)]
struct TargetCommandStruct {
    #[clap(subcommand)]
    target_command: TargetCommand,

    #[clap(short, long, parse(from_os_str))]
    /// Path to a .kbdgen bundle to process
    bundle_path: PathBuf,

    #[clap(short, long, parse(from_os_str))]
    /// The directory to place generated output
    output_path: PathBuf,
}

#[derive(Parser)]
struct TargetWindowsCommand {}

#[derive(Parser)]
struct TargetMacOs {
    #[clap(subcommand)]
    command: TargetMacOsCommand,
}

#[derive(Subcommand)]
enum TargetMacOsCommand {
    /// Run all build steps (recommended option)
    Build(TargetMacOsBuildCommand),

    /// Run generation step (advanced)
    Generate(TargetMacOsGenerateCommand),

    /// Run packaging step (advanced)
    Package(TargetMacOsPackageCommand),
}

#[derive(Parser)]
struct TargetMacOsBuildCommand {}

#[derive(Parser)]
struct TargetMacOsGenerateCommand {}

#[derive(Parser)]
struct TargetMacOsPackageCommand {}

#[derive(Parser)]
struct TargetSvgCommand {}
