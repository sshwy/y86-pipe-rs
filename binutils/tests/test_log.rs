#[test]

fn test_terminal_log() {
    let named_file = tempfile::NamedTempFile::new().unwrap();
    let log_file = std::sync::Arc::new(named_file.reopen().unwrap());
    binutils::logging_setup(tracing::Level::TRACE, Some(log_file));

    tracing::info!("This is a test info message");
    tracing::warn!("This is a test warn message");
    tracing::error!("This is a test error message");
    tracing::debug!("This is a test debug message");
    tracing::trace!("This is a test trace message");

    let file2 = named_file.reopen().unwrap();
    let content = std::io::read_to_string(file2).unwrap();
    eprintln!("{}", content);
}
