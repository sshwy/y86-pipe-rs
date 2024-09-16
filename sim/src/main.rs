use anyhow::{Context, Result};
use binutils::{clap, verbose};
use clap::{error::ErrorKind, CommandFactory, Parser};
use y86_sim::{
    assemble,
    framework::{CpuSim, PipeSim},
    mem_diff, AssembleOption,
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

    // / Print logs during simulation
    #[command(flatten)]
    verbose: verbose::Verbosity,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let content = std::fs::read_to_string(&args.input)
        .with_context(|| format!("could not read file `{}`", &args.input))?;

    let verbose_asm = args
        .verbose
        .log_level()
        .is_some_and(|lv| lv >= verbose::Level::Trace);

    let obj = assemble(&content, AssembleOption::default().set_verbose(verbose_asm))?;

    let log_level = match args.verbose.log_level() {
        Some(verbose::Level::Error) => &tracing::Level::WARN,
        Some(verbose::Level::Warn) => &tracing::Level::INFO,
        Some(verbose::Level::Info) => &tracing::Level::DEBUG,
        Some(verbose::Level::Debug) => &tracing::Level::TRACE,
        Some(verbose::Level::Trace) => &tracing::Level::TRACE,
        None => &tracing::Level::ERROR,
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
        let mut pipe = PipeSim::new(obj.obj.init_mem(), true);

        while !pipe.is_terminate() {
            let _out = pipe.step();
        }

        mem_diff(&obj.obj.init_mem(), &pipe.mem());
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
