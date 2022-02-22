use clap::{Parser, Subcommand};

fn main() {
    let cli = Cli::parse();

    

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
    hmm: bool,
}
