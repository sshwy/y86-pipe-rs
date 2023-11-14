use anyhow::{Context, Result};
use clap::Parser;
use y86_pipe_rs::{assemble, AssembleOption};

/// Y86 assembler written in rust
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// input file path
    input: String,
    /// output filename (default is input%.yo)
    #[arg(short = 'o', long)]
    output: Option<String>,
    #[arg(short = 'v', long)]
    verbose: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let content = std::fs::read_to_string(&args.input)
        .with_context(|| format!("could not read file `{}`", &args.input))?;
    let obj = assemble(
        &content,
        AssembleOption::default().set_verbose(args.verbose),
    )?;
    let output_path = if let Some(path) = args.output {
        path
    } else {
        let mut path = std::path::PathBuf::from(&args.input);
        path.set_extension("yo");
        path.to_str().unwrap().to_string()
    };
    std::fs::write(&output_path, format!("{}", obj))
        .with_context(|| format!("could not write file `{}`", &output_path))?;
    Ok(())
}
