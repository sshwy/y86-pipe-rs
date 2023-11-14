use anyhow::{Context, Result};
use clap::{error::ErrorKind, CommandFactory, Parser};
use y86_pipe_rs::{assemble, mem_diff, AssembleOption, Pipeline};

// Y86 assembler and pipeline simulator written in rust
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// input file path
    input: String,

    /// output filename (default is input%.yo)
    #[arg(short = 'o', long)]
    output: Option<String>,

    /// run the assembled binary in pipeline simulator
    #[arg(short = 'r', long)]
    run: bool,

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

    if args.run {
        if args.output.is_some() {
            let mut cmd = Args::command();
            cmd.error(
                ErrorKind::ArgumentConflict,
                "Can't both specify output and run",
            )
            .exit();
        }
        let mut pipe: Pipeline = Pipeline::init(obj.obj.binary);
        while !pipe.is_terminate() {
            let _out = pipe.step();
        }

        mem_diff(&obj.obj.binary, &pipe.mem());
        // mem_print(&pipe.mem());
        eprintln!("{}", obj);
    } else {
        let output_path = if let Some(path) = args.output {
            path
        } else {
            let mut path = std::path::PathBuf::from(&args.input);
            path.set_extension("yo");
            path.to_str().unwrap().to_string()
        };
        std::fs::write(&output_path, format!("{}", obj))
            .with_context(|| format!("could not write file `{}`", &output_path))?;
    }
    Ok(())
}
