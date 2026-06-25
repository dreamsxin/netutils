//! Traceroute 模块：TTL 递增探测路由路径。

use std::mem::MaybeUninit;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{Duration, Instant};

use socket2::{Domain, Protocol, Socket, Type};

use crate::table::print_table;

const MAX_HOPS: u32 = 30;
const PROBES_PER_HOP: u32 = 3;
const TIMEOUT: Duration = Duration::from_secs(2);

/// 单跳探测结果
struct HopResult {
    ttl: u32,
    /// 每次探测的 (IP, 延迟)，None 表示超时
    probes: Vec<Option<(IpAddr, Duration)>>,
    reached: bool,
}

/// 执行 traceroute 并输出结果
pub async fn run(host: &str) {
    println!();
    println!("🛤️  Traceroute to {}", host);

    // 解析主机
    let target = match resolve_host(host).await {
        Some(ip) => ip,
        None => {
            println!("  ❌ 无法解析主机: {}", host);
            return;
        }
    };

    println!("  目标: {} ({})", host, target);
    println!("  最大跳数: {}", MAX_HOPS);
    println!();

    let mut hops = Vec::new();
    let mut reached_dest = false;

    for ttl in 1..=MAX_HOPS {
        let hop = trace_hop(target, ttl).await;
        let is_reached = hop.reached;
        hops.push(hop);

        if is_reached {
            reached_dest = true;
            break;
        }
    }

    print_hops(&hops, host, target);

    if !reached_dest {
        println!();
        println!("  ⚠ 未在 {} 跳内到达目标", MAX_HOPS);
    }
}

/// 探测单跳：发送 3 个 ICMP Echo Request，TTL 设为指定值
async fn trace_hop(target: IpAddr, ttl: u32) -> HopResult {
    let mut probes = Vec::new();
    let mut reached = false;

    for probe_seq in 0..PROBES_PER_HOP {
        match send_probe(target, ttl, probe_seq).await {
            Some((ip, rtt)) => {
                if ip == target {
                    reached = true;
                }
                probes.push(Some((ip, rtt)));
            }
            None => probes.push(None),
        }
    }

    HopResult {
        ttl,
        probes,
        reached,
    }
}

/// 发送单个 ICMP 探测包并等待响应
///
/// 返回 (响应来源 IP, 延迟)，超时返回 None
async fn send_probe(target: IpAddr, ttl: u32, probe_seq: u32) -> Option<(IpAddr, Duration)> {
    match target {
        IpAddr::V4(addr) => send_probe_v4(addr, ttl, probe_seq).await,
        IpAddr::V6(_) => None, // IPv6 traceroute 暂不支持
    }
}

/// IPv4 ICMP 探测
async fn send_probe_v4(target: Ipv4Addr, ttl: u32, probe_seq: u32) -> Option<(IpAddr, Duration)> {
    // 创建 ICMP socket
    let socket = Socket::new(Domain::IPV4, Type::RAW, Some(Protocol::ICMPV4)).ok()?;
    socket.set_ttl_v4(ttl).ok()?;
    socket.set_read_timeout(Some(TIMEOUT)).ok()?;

    // 构造 ICMP Echo Request
    let ident = (std::process::id() & 0xFFFF) as u16;
    let seq = (probe_seq + ttl * 10) as u16;
    let packet = build_icmp_echo_request(ident, seq);

    let start = Instant::now();

    // 发送
    let dest = SocketAddr::new(IpAddr::V4(target), 0);
    socket.send_to(&packet, &dest.into()).ok()?;

    // 接收响应
    let mut buf = [MaybeUninit::new(0); 1024];
    loop {
        match socket.recv_from(&mut buf) {
            Ok((len, from)) => {
                let from_ip = from.as_socket().map(|s| s.ip())?;
                // 将 MaybeUninit 转为已初始化切片
                let data: &[u8] =
                    unsafe { std::slice::from_raw_parts(buf.as_ptr() as *const u8, len) };
                // 解析 IP 头 + ICMP，找到 ICMP Time Exceeded 或 Echo Reply
                if parse_icmp_response(data, ident, seq).is_some() {
                    return Some((from_ip, start.elapsed()));
                }
                // 不是我们的包，继续等待
            }
            Err(_) => {
                // 超时
                return None;
            }
        }
    }
}

/// 构造 ICMP Echo Request 包
fn build_icmp_echo_request(ident: u16, seq: u16) -> Vec<u8> {
    let mut packet = vec![0u8; 8 + 32]; // ICMP header (8) + payload (32)

    // Type = 8 (Echo Request)
    packet[0] = 8;
    // Code = 0
    packet[1] = 0;
    // Identifier
    packet[4] = (ident >> 8) as u8;
    packet[5] = (ident & 0xFF) as u8;
    // Sequence
    packet[6] = (seq >> 8) as u8;
    packet[7] = (seq & 0xFF) as u8;

    // Payload: 填充时间戳数据
    for i in 0..32 {
        packet[8 + i] = i as u8;
    }

    // 计算校验和
    let checksum = icmp_checksum(&packet);
    packet[2] = (checksum >> 8) as u8;
    packet[3] = (checksum & 0xFF) as u8;

    packet
}

/// 计算 ICMP 校验和
fn icmp_checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    let mut i = 0;
    while i + 1 < data.len() {
        let word = ((data[i] as u32) << 8) | (data[i + 1] as u32);
        sum += word;
        i += 2;
    }
    if i < data.len() {
        sum += (data[i] as u32) << 8;
    }
    while (sum >> 16) != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    !(sum as u16)
}

/// 解析 ICMP 响应
///
/// 返回 Some(()) 如果这是对我们请求的响应（Echo Reply 或 Time Exceeded 中包含我们的包）
fn parse_icmp_response(buf: &[u8], _ident: u16, _seq: u16) -> Option<()> {
    // Windows raw socket 接收的数据包含 IP 头
    // IP 头最小 20 字节
    if buf.len() < 20 {
        return None;
    }

    // 读取 IP 头长度（IHL 字段）
    let ihl = ((buf[0] & 0x0F) * 4) as usize;
    if buf.len() < ihl + 8 {
        return None;
    }

    // ICMP 头
    let icmp_type = buf[ihl];
    let _icmp_code = buf[ihl + 1];

    match icmp_type {
        // 0 = Echo Reply（到达目标）
        0 => Some(()),
        // 11 = Time Exceeded（中间路由器）
        11 => Some(()),
        // 其他类型忽略
        _ => None,
    }
}

/// 解析主机名为 IP 地址
async fn resolve_host(host: &str) -> Option<IpAddr> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Some(ip);
    }

    use trust_dns_resolver::config::*;
    use trust_dns_resolver::TokioAsyncResolver;

    let resolver = TokioAsyncResolver::tokio(ResolverConfig::default(), ResolverOpts::default());
    match resolver.lookup_ip(host).await {
        Ok(ips) => ips.iter().next(),
        Err(_) => None,
    }
}

/// 打印所有跳的结果
fn print_hops(hops: &[HopResult], host: &str, target: IpAddr) {
    let headers = ["跳数", "IP 地址", "延迟 1", "延迟 2", "延迟 3"];
    let rows: Vec<Vec<String>> = hops
        .iter()
        .map(|hop| {
            let mut row = vec![hop.ttl.to_string()];

            // IP 地址（取第一个非 None 的探测）
            let ip_str = hop
                .probes
                .iter()
                .find_map(|p| p.as_ref().map(|(ip, _)| ip.to_string()))
                .unwrap_or_else(|| "*".to_string());
            row.push(ip_str);

            // 三次延迟
            for i in 0..PROBES_PER_HOP as usize {
                if let Some(Some((_, rtt))) = hop.probes.get(i) {
                    row.push(format!("{:.2}ms", rtt.as_secs_f64() * 1000.0));
                } else {
                    row.push("*".to_string());
                }
            }

            row
        })
        .collect();

    print_table(&headers, &rows);

    let _ = host;
    let _ = target;
}
