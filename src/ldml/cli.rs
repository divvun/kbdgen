//! The `kbdgen ldml` subcommands' arguments and their dispatch.

use std::path::PathBuf;

use clap::{Args, Subcommand};
use kbd_model::Host;

use super::LdmlError;
use super::compile::{Selection, compile};
use super::conformance::bundle::run_bundle;
use super::export::export;
use super::import::{destinations, group, report, write};
use super::layouts::{compiled_layouts, host_documents, layout_files};
use super::migrate::migrate_bundle;

// [spec:kbdgen:def:ldml.cli.commands]
/// `kbdgen ldml <command>`.
#[derive(Debug, Subcommand)]
pub enum LdmlCommand {
    /// Write <OUT>/<tag>.<host>.xml for each v4 layout and host
    Export(OutputArgs),
    /// Read keyboard3 XML files into v4 layouts/<tag>.yaml
    Import(ImportArgs),
    /// Convert the bundle's v3 layouts to v4 in place, reporting data defects
    Migrate(MigrateArgs),
    /// Write <OUT>/<tag>.<host>.dvkb engine models for each v4 layout and host
    Compile(OutputArgs),
    /// Run the bundle's tests/*.yaml vectors, and with --cldr CLDR's
    Test(TestArgs),
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

#[derive(Debug, Args)]
pub struct MigrateArgs {
    /// Path to a .kbdgen bundle
    #[arg(short = 'b', long = "bundle-path", value_name = "BUNDLE")]
    pub bundle: PathBuf,
    /// Report only; write no layout
    #[arg(long)]
    pub dry_run: bool,
    /// Write the report to FILE as YAML instead of printing it
    #[arg(long, value_name = "FILE")]
    pub report: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct TestArgs {
    /// Path to a .kbdgen bundle
    #[arg(short = 'b', long = "bundle-path", value_name = "BUNDLE")]
    pub bundle: PathBuf,
    /// Also run the vendored CLDR keyboardTest3 vectors
    #[arg(long)]
    pub cldr: bool,
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
        // [spec:kbdgen:def:ldml.migrate.report]
        // [spec:kbdgen:req:ldml.migrate.survey]
        LdmlCommand::Migrate(args) => {
            let report = migrate_bundle(&args.bundle, args.dry_run)?;
            match &args.report {
                Some(path) => {
                    std::fs::write(path, report.yaml()).map_err(|source| LdmlError::Io {
                        path: path.clone(),
                        source,
                    })?;
                    for line in report.summary() {
                        println!("{line}");
                    }
                }
                None => print!("{}", report.text()),
            }
            let blocked: Vec<String> = report.blocked().iter().map(|l| l.file.clone()).collect();
            if !blocked.is_empty() {
                return Err(LdmlError::MigrationBlocked { layouts: blocked });
            }
        }
        // [spec:kbdgen:req:ldml.cli.test]
        LdmlCommand::Test(args) => {
            let report = run_bundle(&args.bundle, args.cldr)?;
            for failure in &report.failures {
                println!("{failure}");
            }
            for note in &report.notes {
                println!("note: {note}");
            }
            println!("{}", report.summary());
            if !report.failures.is_empty() {
                return Err(LdmlError::TestsFailed {
                    failed: report.failures.len(),
                });
            }
        }
    }
    Ok(())
}
