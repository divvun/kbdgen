use std::path::PathBuf;

use build::macos::{GenerateInstaller, GenerateMacOs};
use build::BuildStep;
use bundle::KbdgenBundle;
use clap::{Args, Parser, Subcommand};

use crate::build::chromeos::ChromeOsBuild;
use crate::build::macos::MacOsBuild;
use crate::build::svg::SvgBuild;
use crate::build::windows::WindowsBuild;
use crate::build::BuildSteps;
use crate::bundle::read_kbdgen_bundle;

mod build;
mod bundle;
mod util;

async fn macos_target(
    bundle: KbdgenBundle,
    output_path: PathBuf,
    target: &TargetMacOs,
) -> anyhow::Result<()> {
    match target.command {
        TargetMacOsCommand::Build(_) => {
            let build = MacOsBuild::new(bundle, output_path);
            build.build_full().await;
        }
        TargetMacOsCommand::Generate(_) => GenerateMacOs.build(&bundle, &output_path).await,
        TargetMacOsCommand::Installer(_) => GenerateInstaller.build(&bundle, &output_path).await,
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
            let bundle = read_kbdgen_bundle(&bundle_path)?;
            let output_path = target_command_struct.output_path.to_path_buf();

            tracing::info!("Output Path: {:?}", &output_path);
            std::fs::create_dir_all(&output_path)?;

            match &target_command_struct.target_command {
                TargetCommand::Windows(_windows_command) => {
                    let build =
                        WindowsBuild::new(bundle, target_command_struct.output_path.clone());

                    build.build_full().await;
                }
                TargetCommand::ChromeOs(_chromeos_command) => {
                    let build =
                        ChromeOsBuild::new(bundle, target_command_struct.output_path.clone());

                    build.build_full().await;
                }
                TargetCommand::MacOs(target) => {
                    macos_target(bundle, output_path, target).await?;
                }
                TargetCommand::Svg(_svg_command) => {
                    let build =
                        SvgBuild::new(bundle, target_command_struct.output_path.clone());

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
    #[clap(name = "chromeos", about = "ChromeOS functionality")]
    ChromeOs(TargetChromeOsCommand),
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
struct TargetChromeOsCommand {}

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

    /// Run installer generation step (advanced)
    Installer(TargetMacOsInstallerCommand),
}

#[derive(Parser)]
struct TargetMacOsBuildCommand {}

#[derive(Parser)]
struct TargetMacOsGenerateCommand {}

#[derive(Parser)]
struct TargetMacOsInstallerCommand {}

#[derive(Parser)]
struct TargetSvgCommand {}
