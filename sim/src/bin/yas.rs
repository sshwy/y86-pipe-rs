use anyhow::{Context, Result};
use binutils::{clap, verbose};
use clap::Parser;
use y86_sim::{assemble, AssembleOption};

/// Y86 assembler written in Rust.
#[derive(Parser, Debug)]
#[command(
    author,
    version,
    long_about = None,
    styles = binutils::get_styles(),
    arg_required_else_help = true,
)]
struct Args {
    /// Path to the input .ya file
    input: String,

    /// Output filename (default is input%.yo)
    ///
    /// Specify this option to write the assembled results to a file. This
    /// option is conflict with `run`.
    #[arg(short = 'o', long)]
    output: Option<String>,

    /// Print logs during simulation
    #[command(flatten)]
    verbose: verbose::Verbosity,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let verbose_asm = args
        .verbose
        .log_level()
        .is_some_and(|lv| lv >= verbose::Level::Trace);
    let log_level = binutils::verbose_level_to_trace(args.verbose.log_level());
    binutils::logging_setup(log_level, None::<&std::fs::File>);

    let content = std::fs::read_to_string(&args.input)
        .with_context(|| format!("could not read file `{}`", args.input))?;
    let a = assemble(&content, AssembleOption::default().set_verbose(verbose_asm))?;

    let output_path = if let Some(path) = args.output {
        path
    } else {
        let mut path = std::path::PathBuf::from(&args.input);
        path.set_extension("yo");
        path.to_string_lossy().to_string()
    };
    std::fs::write(&output_path, format!("{}", a))
        .with_context(|| format!("could not write file `{}`", &output_path))?;
    println!("writing to file `{}`", &output_path);
    Ok(())
}
