mod server;

use std::{
    io::{BufReader, BufWriter},
    net::SocketAddr,
    os::unix::net::UnixListener,
};

use anyhow::Context;

pub fn start_tcp_listener(port: u16) -> anyhow::Result<()> {
    let addrs = [SocketAddr::from(([127, 0, 0, 1], port))];
    let listener = std::net::TcpListener::bind(&addrs[..]).context("failed to bind socket")?;
    tracing::info!("listening on {:?}", listener.local_addr()?);

    ctrlc::set_handler(move || {
        eprintln!("exiting");
        std::process::exit(0);
    })
    .context("failed to set ctrlc handler")?;

    for s in listener.incoming() {
        match s {
            Ok(s) => {
                tracing::trace!("accepted connection");
                std::thread::Builder::new()
                    .stack_size(32 << 20)
                    .spawn(move || {
                        let server = server::DebugServer::new(s.try_clone().unwrap(), s);
                        let r = server.start();
                        tracing::trace!("connection closed, result: {:?}", r);
                    })?;
            }
            Err(e) => {
                tracing::error!("failed to accept connection: {:?}", e);
                break;
            }
        }
    }
    Ok(())
}

pub fn start_unix_listener(socket_name: String) -> anyhow::Result<()> {
    let listener = UnixListener::bind(&socket_name).context("failed to bind socket")?;

    ctrlc::set_handler(move || {
        let _ = std::fs::remove_file(&socket_name).context("failed to remove socket");
        eprintln!("exiting");
        std::process::exit(0);
    })
    .context("failed to set ctrlc handler")?;

    for s in listener.incoming() {
        match s {
            Ok(s) => {
                tracing::trace!("accepted connection");
                std::thread::Builder::new().stack_size(32 << 20).spawn(
                    move || -> anyhow::Result<()> {
                        let server = server::DebugServer::new(s.try_clone()?, s);
                        server.start()
                    },
                )?;
            }
            Err(e) => {
                tracing::error!("failed to accept connection: {:?}", e);
                break;
            }
        }
    }

    Ok(())
}

pub fn start_stdio() -> anyhow::Result<()> {
    let output = BufWriter::new(std::io::stdout());
    let input = BufReader::new(std::io::stdin());
    let server = server::DebugServer::new(input, output);

    server.start()
}
