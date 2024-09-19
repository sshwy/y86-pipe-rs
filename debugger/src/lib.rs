mod server;

use std::net::SocketAddr;

use anyhow::Context;

pub fn start_tcp_listener(port: u16, arch: String) -> anyhow::Result<()> {
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
                let arch = arch.clone();
                tracing::trace!("accepted connection");
                std::thread::Builder::new()
                    .stack_size(32 << 20)
                    .spawn(move || {
                        let server = server::DebugServer::new(s.try_clone().unwrap(), s, arch);
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
