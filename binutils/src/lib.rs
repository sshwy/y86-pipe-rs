/// re-export clap
pub use clap;
use clap::builder::styling::{AnsiColor, Color, Style};

pub mod verbose {
    pub use clap_verbosity_flag::{Level, Verbosity};
}

/// Cargo-like terminal color style.
///
/// # Example
///
/// ```
/// # use clap::{Parser, command, arg};
/// #[derive(Parser)]
/// #[command(
///     name = "cli",
///     disable_version_flag = true,
///     styles = binutils::get_styles(),
/// )]
/// struct Cli {
///     /// display version information
///     #[arg(short = 'V', long)]
///     version: bool,
/// }
/// ```
pub fn get_styles() -> clap::builder::Styles {
    clap::builder::Styles::styled()
        .usage(
            Style::new()
                .bold()
                .fg_color(Some(Color::Ansi(AnsiColor::BrightGreen))),
        )
        .header(
            Style::new()
                .bold()
                .fg_color(Some(Color::Ansi(AnsiColor::BrightGreen))),
        )
        .literal(
            Style::new()
                .bold()
                .fg_color(Some(Color::Ansi(AnsiColor::Cyan))),
        )
        .invalid(
            Style::new()
                .bold()
                .fg_color(Some(Color::Ansi(AnsiColor::Red))),
        )
        .error(
            Style::new()
                .bold()
                .fg_color(Some(Color::Ansi(AnsiColor::Red))),
        )
        .valid(
            Style::new()
                .bold()
                .underline()
                .fg_color(Some(Color::Ansi(AnsiColor::Green))),
        )
        .placeholder(Style::new().fg_color(Some(Color::Ansi(AnsiColor::Cyan))))
}

/// logging configuration for development
pub fn logging_setup(
    max_level: tracing::Level,
    log_file: Option<impl std::io::Write + Clone + Send + 'static>,
) {
    use tracing_subscriber::{filter, prelude::*};

    let filter = filter::filter_fn(move |meta| {
        *meta.level() <= max_level // && !from_actix_session
    });

    let terminal_log = tracing_subscriber::fmt::layer()
        .compact()
        .with_file(false)
        .with_target(false)
        .without_time()
        .with_thread_names(false)
        // .with_writer(std::io::stderr)
        .with_filter(filter.clone());

    let file_log = log_file.map(|file| {
        let file = std::sync::Mutex::new(file);
        tracing_subscriber::fmt::layer()
            .json()
            .with_thread_names(true)
            .with_writer(move || file.lock().unwrap().clone())
            .with_filter(filter)
    });

    tracing_subscriber::registry()
        .with(file_log)
        .with(terminal_log)
        .init();
}

pub fn verbose_level_to_trace(level: Option<verbose::Level>) -> tracing::Level {
    match level {
        Some(verbose::Level::Error) => tracing::Level::WARN,
        Some(verbose::Level::Warn) => tracing::Level::INFO,
        Some(verbose::Level::Info) => tracing::Level::DEBUG,
        Some(verbose::Level::Debug) => tracing::Level::TRACE,
        Some(verbose::Level::Trace) => tracing::Level::TRACE,
        None => tracing::Level::ERROR,
    }
}
