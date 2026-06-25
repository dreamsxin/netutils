mod cli;
mod dns;
mod info;
mod ping;
mod table;

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
    }
}
