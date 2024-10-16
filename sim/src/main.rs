use anyhow::{Context, Result};
use binutils::{clap, verbose};
use clap::{error::ErrorKind, CommandFactory, Parser};
use y86_sim::{
    architectures::{arch_names, create_sim},
    assemble,
    framework::{MemData, MEM_SIZE},
    mem_diff, AssembleOption,
};

/// Print architecture information after help message
fn after_help() -> String {
    let extras = y86_sim::architectures::EXTRA_ARCH_NAMES;
    use binutils::clap::builder::styling::*;
    let t = Style::new()
        .bold()
        .fg_color(Some(Color::Ansi(AnsiColor::Green)));
    let es = Style::new()
        .bold()
        .fg_color(Some(Color::Ansi(AnsiColor::Magenta)));
    let bs = Style::new()
        .bold()
        .fg_color(Some(Color::Ansi(AnsiColor::Cyan)));
    format!(
        "{t}Architectures{t:#}: {}",
        arch_names()
            .into_iter()
            .map(|s| if extras.contains(&s) {
                format!("{es}{}{es:#}", s)
            } else {
                format!("{bs}{}{bs:#}", s)
            })
            .collect::<Vec<_>>()
            .join(", ")
    )
}

#[derive(clap::Args, Debug)]
#[group(multiple = false)]
struct Action {
    /// Execute the assembled binary in simulator
    #[arg(short = 'R', long)]
    run: bool,

    /// Get information about the current architecture
    #[arg(short = 'I', long)]
    info: bool,
}

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    after_help = after_help(),
    about,
    long_about = None,
    styles = binutils::get_styles(),
    arg_required_else_help = true,
)]
struct Args {
    /// Path to the input .ya file
    input: Option<String>,

    /// Output filename (default is input%.yo)
    ///
    /// Specify this option to write the assembled results to a file. This
    /// option is conflict with `run`.
    #[arg(short = 'o', long)]
    output: Option<String>,

    #[clap(flatten)]
    act: Action,

    /// Specify the pipeline architecture to run
    #[arg(long, default_value = "seq_std")]
    arch: Option<String>,

    /// Limit the maximum number of CPU cycles to prevent infinite loop
    #[arg(long, default_value = "100000")]
    max_cpu_cycle: Option<u64>,

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
    let log_level = match args.verbose.log_level() {
        Some(verbose::Level::Error) => &tracing::Level::WARN,
        Some(verbose::Level::Warn) => &tracing::Level::INFO,
        Some(verbose::Level::Info) => &tracing::Level::DEBUG,
        Some(verbose::Level::Debug) => &tracing::Level::TRACE,
        Some(verbose::Level::Trace) => &tracing::Level::TRACE,
        None => &tracing::Level::ERROR,
    };
    binutils::logging_setup(log_level, None::<&std::fs::File>);

    let maybe_a = if let Some(input) = &args.input {
        let content = std::fs::read_to_string(input)
            .with_context(|| format!("could not read file `{}`", input))?;
        let obj = assemble(&content, AssembleOption::default().set_verbose(verbose_asm))?;
        Some(obj)
    } else {
        None
    };

    let arch = args.arch.unwrap();
    if !arch_names().contains(&arch.as_str()) {
        let mut cmd = Args::command();
        cmd.error(
            ErrorKind::InvalidValue,
            format!("unknown architecture `{}`", arch),
        )
        .exit();
    }

    if args.act.run {
        let a = maybe_a.ok_or(anyhow::anyhow!("no input file"))?;
        if args.output.is_some() {
            let mut cmd = Args::command();
            cmd.error(
                ErrorKind::ArgumentConflict,
                "Can't both specify output and run",
            )
            .exit();
        }
        let mem = MemData::init(a.obj.init_mem());
        let mut pipe = create_sim(arch, mem.clone(), true);

        let max_cpu_cycle = args.max_cpu_cycle.unwrap();
        while !pipe.is_terminate() {
            pipe.step();
            if pipe.cycle_count() > max_cpu_cycle {
                anyhow::bail!(
                    "exceed maximum CPU cycle limit (use --max-cpu-cycle to change the limit)"
                );
            }
        }

        mem_diff(&a.obj.init_mem(), &mem.read());
        // mem_print(&pipe.mem());
    } else if args.act.info {
        let empty_sim = create_sim(arch.clone(), MemData::init([0; MEM_SIZE]), false);

        println!("{}", empty_sim);

        y86_sim::render_arch_dependency_graph(&arch, empty_sim.proporder())?;
    } else {
        let a = maybe_a.ok_or(anyhow::anyhow!("no input file"))?;
        let output_path = if let Some(path) = args.output {
            path
        } else {
            let mut path = std::path::PathBuf::from(&args.input.unwrap());
            path.set_extension("yo");
            path.to_string_lossy().to_string()
        };
        std::fs::write(&output_path, format!("{}", a))
            .with_context(|| format!("could not write file `{}`", &output_path))?;
        println!("writing to file `{}`", &output_path);
    }
    Ok(())
}
