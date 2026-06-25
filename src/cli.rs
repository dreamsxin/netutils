//! 子命令定义。

use clap::{Parser, Subcommand};

/// 本地网络检测工具集
#[derive(Parser, Debug)]
#[command(name = "netutils", version, about = "本地网络检测工具集", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// 显示全部网络信息（默认）
    All,
    /// 仅显示网络接口列表
    Iface,
    /// 仅显示流量出口
    Egress,
    /// 仅显示路由表
    Route,
    /// 仅显示代理设置
    Proxy,
}
