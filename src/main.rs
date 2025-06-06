#![cfg(feature = "cli")]

use std::{
    fs,
    io::{stdout, Write},
    path::PathBuf,
};

use bitcut::{apply_patch, make_diff, Op};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "bitcut")]
#[command(about = "Create and apply binary patches", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create binary patch from two files and write to stdout
    Diff { old: PathBuf, new: PathBuf },
    /// Apply binary patch to a file and write result to stdout
    Patch { old: PathBuf, patch: PathBuf },
    /// Print patch opcodes
    Debug { patch: PathBuf },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Diff { old, new } => {
            let old = fs::read(old)?;
            let new = fs::read(new)?;
            let patch = make_diff(&old, &new);
            stdout().write_all(&patch)?;
        }
        Commands::Patch { old, patch } => {
            let old = fs::read(old)?;
            let patch = fs::read(patch)?;
            let new = apply_patch(&old, &patch).map_err(|e| anyhow::anyhow!("{e}"))?;
            stdout().write_all(&new)?;
        }
        Commands::Debug { patch } => {
            let patch = fs::read(patch)?;
            println!(
                "{:#?}",
                Op::deserialize_all(&patch).map_err(|e| anyhow::anyhow!("{e}"))?
            );
        }
    }

    Ok(())
}
