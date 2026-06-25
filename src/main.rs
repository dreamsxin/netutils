mod cli;
mod connectivity;
mod dns;
mod info;
mod ping;
mod portscan;
mod table;
mod traceroute;

use clap::Parser;
use cli::{Cli, Commands};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        None | Some(Commands::All) => info::print_all(),
        Some(Commands::Iface) => info::print_interfaces(),
        Some(Commands::Egress) => info::print_egress(),
        Some(Commands::Route) => info::print_routes(),
        Some(Commands::Proxy) => info::print_proxy(),
        Some(Commands::Ping { host, count }) => ping::run(&host, count).await,
        Some(Commands::Dns { domain, r#type }) => dns::run(&domain, r#type).await,
        Some(Commands::Trace { host }) => traceroute::run(&host).await,
        Some(Commands::Scan { host, ports }) => {
            let port_list = ports.as_ref().map(|s| {
                s.split(',')
                    .filter_map(|p| p.trim().parse::<u16>().ok())
                    .collect::<Vec<u16>>()
            });
            // 如果解析后为空但用户提供了参数，视为 None
            let port_ref = port_list
                .as_ref()
                .filter(|v| !v.is_empty())
                .map(|v| v.as_slice());
            portscan::run(&host, port_ref).await;
        }
        Some(Commands::Check { target, count }) => connectivity::run(&target, count).await,
    }
}
