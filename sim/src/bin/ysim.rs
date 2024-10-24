use anyhow::{Context, Result};
use binutils::{clap, verbose};
use clap::{error::ErrorKind, CommandFactory, Parser};
use y86_sim::{
    architectures::{arch_names, create_sim},
    assemble,
    framework::{MemData, MEM_SIZE},
    utils, AssembleOption,
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

/// Y86 pipeline simulator written in Rust
#[derive(Parser, Debug)]
#[command(
    author,
    version,
    after_help = after_help(),
    long_about = None,
    styles = binutils::get_styles(),
    arg_required_else_help = true,
)]
struct Args {
    /// Path to the input .ya file
    input: Option<String>,

    /// Get information about the current architecture
    #[arg(short = 'I', long)]
    info: bool,

    /// Specify the pipeline architecture to run
    #[arg(short = 'A', long, default_value = "seq_std")]
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
    let log_level = binutils::verbose_level_to_trace(args.verbose.log_level());
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

    if args.info {
        let empty_sim = create_sim(arch.clone(), MemData::init([0; MEM_SIZE]), false);

        print!("{}", empty_sim);

        utils::render_arch_dependency_graph(&arch, empty_sim.proporder())?;
    } else {
        let a = maybe_a.ok_or(anyhow::anyhow!("no input file"))?;
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

        utils::mem_diff(&a.obj.init_mem(), &mem.read());
        // mem_print(&pipe.mem());
    }
    Ok(())
}
