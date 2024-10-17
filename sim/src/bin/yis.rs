use anyhow::{Context, Result};
use binutils::{clap, verbose};
use clap::Parser;
use y86_sim::utils;

/// Y86 ISA simulator written in Rust.
#[derive(Parser, Debug)]
#[command(
    author,
    version,
    long_about = None,
    styles = binutils::get_styles(),
    arg_required_else_help = true,
)]
struct Args {
    /// Path to the input .yo file
    input: String,

    /// Print logs during simulation
    #[command(flatten)]
    verbose: verbose::Verbosity,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let log_level = binutils::verbose_level_to_trace(args.verbose.log_level());
    binutils::logging_setup(log_level, None::<&std::fs::File>);

    let content = std::fs::read_to_string(&args.input)
        .with_context(|| format!("could not read file `{}`", args.input))?;

    let mut bin = [0; y86_sim::BIN_SIZE];

    for line in content.lines() {
        if let Some(index) = line.find('|') {
            if let Some(sep) = line[..index].find(':') {
                let addr = utils::parse_literal(line[..sep].trim()).ok_or(anyhow::anyhow!(
                    "invalid address literal: {}",
                    line[..sep].trim()
                ))? as usize;
                let value = line[sep + 1..index].trim();
                if !value.is_empty() {
                    let bytes = utils::decode_hex(value)?;
                    bin[addr..addr + bytes.len()].copy_from_slice(&bytes);
                }
            }
        }
    }

    y86_sim::isa::simulate(bin, true)?;

    Ok(())
}
