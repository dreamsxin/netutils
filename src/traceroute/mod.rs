//! Traceroute 模块：TTL 递增探测路由路径。

use std::mem::MaybeUninit;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};

use colored::*;
use serde::Serialize;

use crate::i18n::{t, t1, t2};
use crate::output::{print_json, print_json_error, OutputMode};
use crate::table::print_table;

use socket2::{Domain, Protocol, Socket, Type};

const PROBES_PER_HOP: u32 = 3;
const TIMEOUT: Duration = Duration::from_secs(2);

/// 单次探测结果
#[derive(Serialize, Clone)]
pub struct Probe {
    pub ip: Option<String>,
    pub rtt_ms: Option<f64>,
}

/// 单跳结果
#[derive(Serialize, Clone)]
pub struct Hop {
    pub ttl: u32,
    pub probes: Vec<Probe>,
    pub reached: bool,
}

/// Traceroute 完整输出
#[derive(Serialize)]
pub struct TraceOutput {
    pub host: String,
    pub target: String,
    pub hops: Vec<Hop>,
}

/// 执行 traceroute 并输出结果
pub async fn run(host: &str, max_hops: u32, mode: OutputMode) {
    // 解析主机
    let target = match crate::util::resolve_host(host).await {
        Some(ip) => ip,
        None => {
            let msg = t1("trace.resolve_fail", host);
            if mode == OutputMode::Json {
                print_json_error(&msg);
            } else {
                println!("  {}", msg.red());
            }
            return;
        }
    };

    let mut hops = Vec::new();
    let mut reached_dest = false;

    for ttl in 1..=max_hops {
        let hop = trace_hop(target, ttl).await;
        let is_reached = hop.reached;
        hops.push(hop);

        if is_reached {
            reached_dest = true;
            break;
        }
    }

    let output = TraceOutput {
        host: host.to_string(),
        target: target.to_string(),
        hops: hops.clone(),
    };

    if mode == OutputMode::Json {
        print_json(&output);
        return;
    }

    // 表格输出
    println!();
    println!("{}", t1("trace.title", host).bold());
    println!("  {}", t2("trace.target", host, &target.to_string()));
    println!("  {}", t1("trace.max_hops", &max_hops.to_string()));
    println!();

    let h_hop = t("trace.hop");
    let h_ip = t("trace.ip");
    let h_p1 = t1("trace.probe", "1");
    let h_p2 = t1("trace.probe", "2");
    let h_p3 = t1("trace.probe", "3");
    let headers = [h_hop.as_str(), h_ip.as_str(), h_p1.as_str(), h_p2.as_str(), h_p3.as_str()];

    let rows: Vec<Vec<String>> = hops
        .iter()
        .map(|hop| {
            let mut row = vec![hop.ttl.to_string()];

            let ip_str = hop
                .probes
                .iter()
                .find_map(|p| p.ip.as_ref().map(|ip| ip.clone()))
                .unwrap_or_else(|| "*".to_string());
            row.push(ip_str);

            for i in 0..PROBES_PER_HOP as usize {
                if let Some(Some(rtt)) = hop.probes.get(i).map(|p| p.rtt_ms) {
                    row.push(format!("{:.2}ms", rtt));
                } else {
                    row.push("*".to_string());
                }
            }

            row
        })
        .collect();

    print_table(&headers, &rows);

    if !reached_dest {
        println!();
        println!("  {}", t1("trace.not_reached", &max_hops.to_string()).yellow());
    }
}

/// 探测单跳
async fn trace_hop(target: IpAddr, ttl: u32) -> Hop {
    let mut probes = Vec::new();
    let mut reached = false;

    for probe_seq in 0..PROBES_PER_HOP {
        match send_probe(target, ttl, probe_seq).await {
            Some((ip, rtt)) => {
                if ip == target {
                    reached = true;
                }
                probes.push(Probe {
                    ip: Some(ip.to_string()),
                    rtt_ms: Some(rtt.as_secs_f64() * 1000.0),
                });
            }
            None => probes.push(Probe {
                ip: None,
                rtt_ms: None,
            }),
        }
    }

    Hop {
        ttl,
        probes,
        reached,
    }
}

/// 发送单个 ICMP 探测包并等待响应
async fn send_probe(target: IpAddr, ttl: u32, probe_seq: u32) -> Option<(IpAddr, Duration)> {
    match target {
        IpAddr::V4(addr) => send_probe_v4(addr, ttl, probe_seq).await,
        IpAddr::V6(_) => None,
    }
}

/// IPv4 ICMP 探测
async fn send_probe_v4(target: Ipv4Addr, ttl: u32, probe_seq: u32) -> Option<(IpAddr, Duration)> {
    let socket = Socket::new(Domain::IPV4, Type::RAW, Some(Protocol::ICMPV4)).ok()?;
    socket.set_ttl_v4(ttl).ok()?;
    socket.set_read_timeout(Some(TIMEOUT)).ok()?;

    let ident = (std::process::id() & 0xFFFF) as u16;
    let seq = (probe_seq + ttl * 10) as u16;
    let packet = crate::icmp::build_icmp_echo_request(ident, seq);

    let start = Instant::now();
    let dest = SocketAddr::new(IpAddr::V4(target), 0);
    socket.send_to(&packet, &dest.into()).ok()?;

    let mut buf = [MaybeUninit::new(0); 1024];
    loop {
        match socket.recv_from(&mut buf) {
            Ok((len, from)) => {
                let from_ip = from.as_socket().map(|s| s.ip())?;
                let data: &[u8] =
                    unsafe { std::slice::from_raw_parts(buf.as_ptr() as *const u8, len) };
                if crate::icmp::parse_icmp_response(data, ident, seq).is_some() {
                    return Some((from_ip, start.elapsed()));
                }
            }
            Err(_) => return None,
        }
    }
}
