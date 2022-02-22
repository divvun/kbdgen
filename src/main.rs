use clap::{Parser, Subcommand};

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Command::Target(target_command) => {



            match target_command {
                TargetCommand::Svg(svg_command) => {



                    println!("whee");
                }
            }
        }
    };

    println!("Hello, world!");
}

#[derive(Parser)]
#[clap(version)]
struct Cli {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[clap(subcommand)]
    Target(TargetCommand)
}

#[derive(Subcommand)]
enum TargetCommand {
    Svg(TargetSvgCommand)
}

#[derive(Parser)]
struct TargetSvgCommand {
    #[clap(short, long)]
    aaa: Option<bool>,
}
