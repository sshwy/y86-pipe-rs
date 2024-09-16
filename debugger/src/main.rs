fn main() -> anyhow::Result<()> {
    binutils::logging_setup(&tracing::Level::TRACE, None::<&std::fs::File>);

    y86_dbg::start_tcp_listener(2345)?;

    Ok(())
}
