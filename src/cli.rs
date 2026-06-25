//! 子命令定义。

use clap::{Parser, Subcommand};

use crate::dns::DnsRecordType;

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
    /// Ping 主机（ICMP，无权限时回退 TCP）
    Ping {
        /// 目标主机名或 IP
        host: String,
        /// 发送包数（默认 4）
        #[arg(short, long, default_value_t = 4)]
        count: u32,
    },
    /// DNS 查询
    Dns {
        /// 目标域名
        domain: String,
        /// 记录类型（默认 A）
        #[arg(short, long, value_enum, default_value_t = DnsRecordType::A)]
        r#type: DnsRecordType,
    },
}
