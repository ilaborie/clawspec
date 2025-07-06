#![allow(missing_docs)]
use anyhow::{Context, Result};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tracing::{info, warn};

use axum_example::run;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().pretty().init();

    let AppArgs { host, port } = AppArgs::parse().context("parsing arguments")?;
    let addr = SocketAddr::from((host, port));
    run(addr).await?;

    info!("Bye!");
    Ok(())
}

#[derive(Debug)]
struct AppArgs {
    host: IpAddr,
    port: u16,
}

impl AppArgs {
    fn parse() -> Result<Self> {
        let mut pargs = pico_args::Arguments::from_env();

        let host = pargs
            .opt_value_from_str(["-h", "--host"])
            .context("parsing host argument")?;

        let port = pargs
            .opt_value_from_str(["-p", "--port"])
            .context("parsing port argument")?;

        let result = Self {
            host: host.unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST)),
            port: port.unwrap_or(8080),
        };

        let remaining = pargs.finish();
        if !remaining.is_empty() {
            warn!(?remaining, "Warning: unused arguments left");
        }
        Ok(result)
    }
}
