//! The `kbdgen ldml` subcommands' arguments and their dispatch.

use std::path::PathBuf;

use clap::{Args, Subcommand};
use kbd_model::Host;

use super::LdmlError;
use super::compile::{Selection, compile};
use super::export::export;
use super::import::{destinations, group, report, write};
use super::layouts::{compiled_layouts, host_documents, layout_files};

// [spec:kbdgen:def:ldml.cli.commands]
/// `kbdgen ldml <command>`.
#[derive(Debug, Subcommand)]
pub enum LdmlCommand {
    /// Write <OUT>/<tag>.<host>.xml for each v4 layout and host
    Export(OutputArgs),
    /// Read keyboard3 XML files into v4 layouts/<tag>.yaml
    Import(ImportArgs),
    /// Write <OUT>/<tag>.<host>.dvkb engine models for each v4 layout and host
    Compile(OutputArgs),
}

/// The arguments of `export` and `compile`.
#[derive(Debug, Args)]
pub struct OutputArgs {
    /// Path to a .kbdgen bundle
    #[arg(short = 'b', long = "bundle-path", value_name = "BUNDLE")]
    pub bundle: PathBuf,
    /// The directory to write, created with its parents
    #[arg(short = 'o', long = "output-path", value_name = "OUT")]
    pub output: PathBuf,
    /// Restrict the run to these layout tags; repeatable
    #[arg(long = "layout", value_name = "TAG")]
    pub layouts: Vec<String>,
    /// Restrict the run to these hosts; repeatable
    #[arg(long = "host", value_name = "HOST", value_parser = parse_host)]
    pub hosts: Vec<Host>,
}

#[derive(Debug, Args)]
pub struct ImportArgs {
    /// Path to a .kbdgen bundle
    #[arg(short = 'b', long = "bundle-path", value_name = "BUNDLE")]
    pub bundle: PathBuf,
    /// Replace existing layouts/<tag>.yaml files
    #[arg(long)]
    pub force: bool,
    /// keyboard3 XML files
    #[arg(value_name = "XML", required = true)]
    pub files: Vec<PathBuf>,
}

/// A `--host` value; the error lists the host names of `ldml.yaml.hosts`.
fn parse_host(name: &str) -> Result<Host, String> {
    Host::from_name(name).ok_or_else(|| {
        let names: Vec<&str> = Host::ALL.iter().map(|h| h.name()).collect();
        format!("unknown host {name}; one of {}", names.join(", "))
    })
}

impl OutputArgs {
    fn selection(&self) -> Selection {
        Selection {
            layouts: self.layouts.clone(),
            hosts: self.hosts.clone(),
        }
    }
}

/// Runs a command, printing each written path or reported line. Loading
/// the bundle, and every check, finishes before the first file is
/// written, so a failure writes nothing.
pub fn run(command: &LdmlCommand) -> Result<(), LdmlError> {
    match command {
        LdmlCommand::Export(args) => {
            let documents = host_documents(&layout_files(&args.bundle)?, &args.layouts)?;
            for path in export(&documents, &args.selection(), &args.output)? {
                println!("{}", path.display());
            }
        }
        LdmlCommand::Compile(args) => {
            let documents = host_documents(&layout_files(&args.bundle)?, &args.layouts)?;
            let layouts = compiled_layouts(&documents)?;
            for path in compile(&layouts, &args.selection(), &args.output)? {
                println!("{}", path.display());
            }
        }
        LdmlCommand::Import(args) => {
            let groups = group(&args.files)?;
            let paths = destinations(&args.bundle, &groups, args.force)?;
            let warnings = write(&groups, &paths)?;
            for line in report(&groups) {
                println!("{line}");
            }
            for line in warnings {
                println!("{line}");
            }
            for path in paths {
                println!("{}", path.display());
            }
        }
    }
    Ok(())
}
