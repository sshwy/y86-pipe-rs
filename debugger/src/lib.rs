mod server;

use std::net::SocketAddr;

use anyhow::Context;

#[derive(Debug, Clone)]
pub struct SimOption {
    pub arch: String,
    pub max_cpu_cycle: u64,
}

pub fn start_tcp_listener(port: u16, option: SimOption) -> anyhow::Result<()> {
    let addrs = [SocketAddr::from(([127, 0, 0, 1], port))];
    let listener = std::net::TcpListener::bind(&addrs[..]).context("failed to bind socket")?;
    eprintln!(
        "Debug server listening on {:?}, Press Ctrl+C to quit",
        listener.local_addr()?
    );

    ctrlc::set_handler(move || {
        eprintln!("exiting");
        std::process::exit(0);
    })
    .context("failed to set ctrlc handler")?;

    for s in listener.incoming() {
        let option = option.clone();
        match s {
            Ok(s) => {
                tracing::trace!("accepted connection");
                std::thread::Builder::new()
                    .stack_size(32 << 20)
                    .spawn(move || {
                        let server = server::DebugServer::new(s.try_clone().unwrap(), s, option);
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
