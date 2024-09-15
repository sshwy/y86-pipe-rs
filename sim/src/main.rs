use anyhow::{Context, Result};
use binutils::clap;
use clap::{error::ErrorKind, CommandFactory, Parser};
use y86_pipe_rs::{
    assemble, mem_diff,
    pipeline::{CpuArch, Simulator},
    Arch, AssembleOption,
};

// Y86 assembler and pipeline simulator written in rust
#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about,
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

    /// Run the assembled binary in pipeline simulator
    #[arg(long)]
    run: bool,

    /// Print logs during simulation
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

    let log_level = if args.verbose {
        &tracing::Level::DEBUG
    } else {
        &tracing::Level::INFO
    };
    binutils::logging_setup(log_level, None::<&std::fs::File>);

    if args.run {
        if args.output.is_some() {
            let mut cmd = Args::command();
            cmd.error(
                ErrorKind::ArgumentConflict,
                "Can't both specify output and run",
            )
            .exit();
        }
        let mut pipe = Simulator::new(<Arch as CpuArch>::Units::init(obj.obj.binary), true);

        while !pipe.is_terminate() {
            let _out = pipe.step();
        }

        mem_diff(&obj.obj.binary, &pipe.mem());
        // mem_print(&pipe.mem());
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
