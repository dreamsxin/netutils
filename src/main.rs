mod cli;
mod info;
mod table;

use clap::Parser;
use cli::{Cli, Commands};

fn main() {
    let cli = Cli::parse();

    match cli.command {
        None | Some(Commands::All) => info::print_all(),
        Some(Commands::Iface) => info::print_interfaces(),
        Some(Commands::Egress) => info::print_egress(),
        Some(Commands::Route) => info::print_routes(),
        Some(Commands::Proxy) => info::print_proxy(),
    }
}
