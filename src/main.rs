use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use kbdgen::build::android::clone_giellakbd::CloneGiellaKbd;
use kbdgen::build::android::generate_android::GenerateAndroid;
use kbdgen::build::macos::{GenerateInstaller, GenerateMacOs};
use kbdgen::build::BuildStep;
use kbdgen::bundle::KbdgenBundle;

use kbdgen::build::android::AndroidBuild;
use kbdgen::build::chromeos::ChromeOsBuild;
use kbdgen::build::ios::{IosBuild, IosProjectExt};
use kbdgen::build::macos::MacOsBuild;
use kbdgen::build::svg::SvgBuild;
use kbdgen::build::windows::WindowsBuild;
use kbdgen::build::BuildSteps;
use kbdgen::bundle::read_kbdgen_bundle;

async fn android_target(
    bundle: KbdgenBundle,
    output_path: PathBuf,
    target: &TargetAndroid,
) -> anyhow::Result<()> {
    match target.command {
        TargetAndroidCommand::Build(_) => {
            let build = AndroidBuild::new(bundle, output_path);
            build.build_full().await?;
        }
        TargetAndroidCommand::Clone(_) => CloneGiellaKbd.build(&bundle, &output_path).await?,
        TargetAndroidCommand::Generate(_) => GenerateAndroid.build(&bundle, &output_path).await?,
    }

    Ok(())
}

async fn macos_target(
    bundle: KbdgenBundle,
    output_path: PathBuf,
    target: &TargetMacOs,
) -> anyhow::Result<()> {
    match target.command {
        TargetMacOsCommand::Build(_) => {
            let build = MacOsBuild::new(bundle, output_path);
            build.build_full().await?
        }
        TargetMacOsCommand::Generate(_) => GenerateMacOs.build(&bundle, &output_path).await?,
        TargetMacOsCommand::Installer(_) => GenerateInstaller.build(&bundle, &output_path).await?,
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match &cli.command {
        Command::Fetch(options) => {
            let bundle_path = &options.bundle_path;
            let bundle = read_kbdgen_bundle(&bundle_path)?;

            kbdgen::bundle::fetch(&bundle.path, &bundle.project).await?;
        }
        Command::Target(target_command_struct) => {
            let bundle_path = &target_command_struct.bundle_path;
            let bundle = read_kbdgen_bundle(&bundle_path)?;
            let output_path = &target_command_struct.output_path;

            tracing::debug!("Output Path: {:?}", &output_path);
            std::fs::create_dir_all(&output_path)?;

            let output_path = dunce::canonicalize(output_path).unwrap();

            match &target_command_struct.target_command {
                TargetCommand::Windows(_windows_command) => {
                    let build = WindowsBuild::new(bundle, output_path.clone());

                    build.build_full().await?;
                }
                TargetCommand::ChromeOs(_chromeos_command) => {
                    let build = ChromeOsBuild::new(bundle, output_path.clone());

                    build.build_full().await?;
                }
                TargetCommand::MacOs(target) => {
                    macos_target(bundle, output_path, target).await?;
                }
                TargetCommand::Svg(_svg_command) => {
                    let build = SvgBuild::new(bundle, output_path.clone());

                    build.build_full().await?;
                }
                TargetCommand::Android(target) => {
                    android_target(bundle, output_path, target).await?;
                }
                TargetCommand::Ios(options) => match options.command {
                    TargetIosCommand::Build(_) => {
                        let build = IosBuild::new(bundle, output_path.clone());

                        build.build_full().await?;
                    }
                    TargetIosCommand::PrintPkgIds(_) => {
                        use IosProjectExt;

                        for id in bundle.all_pkg_ids() {
                            println!("{}", id);
                        }
                    }
                },
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

    #[clap(about = "Fetch dependencies for provided project")]
    Fetch(FetchCommand),
}

#[derive(Args)]
struct FetchCommand {
    #[clap(short, long, parse(from_os_str))]
    /// Path to a .kbdgen bundle to process
    bundle_path: PathBuf,
}

#[derive(Subcommand)]
enum TargetCommand {
    #[clap(about = "Windows functionality")]
    Windows(TargetWindowsCommand),
    #[clap(name = "chromeos", about = "ChromeOS functionality")]
    ChromeOs(TargetChromeOsCommand),
    #[clap(name = "macos", about = "macOS functionality")]
    MacOs(TargetMacOs),
    #[clap(name = "svg", about = "SVG functionality")]
    Svg(TargetSvgCommand),
    #[clap(about = "Android functionality")]
    Android(TargetAndroid),
    #[clap(about = "iOS functionality")]
    Ios(TargetIos),
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

// Android

#[derive(Parser)]
struct TargetAndroid {
    #[clap(subcommand)]
    command: TargetAndroidCommand,
}

#[derive(Subcommand)]
enum TargetAndroidCommand {
    /// Run all build steps (recommended option)
    Build(TargetAndroidBuildCommand),

    /// Only clone the GiellaKbd repository (advanced)
    Clone(TargetAndroidCloneGiellaKbdCommand),

    /// Only generate the changes to the Android repository (advanced)
    Generate(TargetAndroidGenerateCommand),
}

#[derive(Parser)]
struct TargetAndroidBuildCommand {}

#[derive(Parser)]
struct TargetAndroidCloneGiellaKbdCommand {}

#[derive(Parser)]
struct TargetAndroidGenerateCommand {}

// iOS

#[derive(Parser)]
struct TargetIos {
    #[clap(subcommand)]
    command: TargetIosCommand,
}

#[derive(Subcommand)]
enum TargetIosCommand {
    /// Run all build steps (recommended option)
    Build(TargetIosBuildCommand),

    /// Print all package identifiers
    PrintPkgIds(TargetIosPrintPkgIds),
}

#[derive(Parser)]
struct TargetIosBuildCommand {}

#[derive(Parser)]
struct TargetIosPrintPkgIds {}

// Windows

#[derive(Parser)]
struct TargetWindowsCommand {}

// ChromeOS

#[derive(Parser)]
struct TargetChromeOsCommand {}

// MacOS

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

// SVG

#[derive(Parser)]
struct TargetSvgCommand {}
