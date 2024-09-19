use binutils::{
    clap::{self, error::ErrorKind, CommandFactory},
    verbose,
};
use clap::Parser;
use y86_sim::architectures::arch_names;

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
    /// Specify the architecture to debug
    #[arg(long, default_value = "seq_std")]
    arch: Option<String>,

    /// Print logs during simulation
    #[command(flatten)]
    verbose: verbose::Verbosity,

    #[arg(short = 'p', long, default_value = "2345")]
    port: Option<u16>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let log_level = match args.verbose.log_level() {
        Some(verbose::Level::Error) => &tracing::Level::INFO,
        Some(verbose::Level::Warn) => &tracing::Level::DEBUG,
        Some(verbose::Level::Info) => &tracing::Level::TRACE,
        Some(verbose::Level::Debug) => &tracing::Level::TRACE,
        Some(verbose::Level::Trace) => &tracing::Level::TRACE,
        None => &tracing::Level::ERROR,
    };
    binutils::logging_setup(log_level, None::<&std::fs::File>);

    let arch = args.arch.unwrap();
    if !arch_names().contains(&arch.as_str()) {
        let mut cmd = Args::command();
        cmd.error(
            ErrorKind::InvalidValue,
            format!("unknown architecture `{}`", arch),
        )
        .exit();
    }

    y86_dbg::start_tcp_listener(args.port.unwrap(), arch)?;

    Ok(())
}
