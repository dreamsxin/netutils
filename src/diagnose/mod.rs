//! 全链路诊断模块：DNS → Ping → TCP → HTTPS → Traceroute，自动定位断点。

use std::net::IpAddr;
use std::time::{Duration, Instant};

use colored::*;
use serde::Serialize;

use crate::i18n::{t, t1};
use crate::output::{print_json, OutputMode};

/// 单步检测结果
#[derive(Serialize, Clone)]
struct DiagStep {
    check: String,
    ok: bool,
    warning: bool,
    message: String,
}

/// 完整诊断报告
#[derive(Serialize)]
struct DiagnoseReport {
    host: String,
    target: String,
    steps: Vec<DiagStep>,
    conclusion: String,
    elapsed_secs: f64,
}

const PING_COUNT: u32 = 2;
const TCP_TIMEOUT: Duration = Duration::from_secs(3);
const HTTPS_TIMEOUT: Duration = Duration::from_secs(5);
const TRACE_MAX_HOPS: u32 = 10;

/// 执行全链路诊断
pub async fn run(host: &str, mode: OutputMode) {
    let start = Instant::now();

    // 步骤 ①②③④ 并行执行
    let (dns, ping, tcp, https) = tokio::join!(
        check_dns(host),
        check_ping(host),
        check_tcp(host),
        check_https(host),
    );

    // 步骤 ⑤ Traceroute 串行（最慢，放最后）
    let target_ip = dns_target_ip(&dns);
    let trace = check_trace(host, target_ip).await;

    let steps = vec![dns, ping, tcp, https, trace];
    let conclusion = derive_conclusion(&steps);

    let elapsed = start.elapsed();
    let target = target_ip.map(|ip| ip.to_string()).unwrap_or_else(|| "N/A".to_string());

    let report = DiagnoseReport {
        host: host.to_string(),
        target: target.clone(),
        steps: steps.clone(),
        conclusion: conclusion.clone(),
        elapsed_secs: elapsed.as_secs_f64(),
    };

    if mode == OutputMode::Json {
        print_json(&report);
        return;
    }

    // 表格输出
    println!();
    println!("{}", t1("diagnose.title", host).bold());
    println!();

    for step in &steps {
        let symbol = if step.ok && !step.warning {
            "✅".green()
        } else if step.warning {
            "⚠️ ".yellow()
        } else {
            "❌".red()
        };
        println!("  {} [{}]", symbol, step.check.dimmed());
        println!("     {}", step.message);
    }

    // 结论
    println!();
    println!("  {}", t1("diagnose.conclusion", &conclusion).bold());

    // 链路状态链
    let chain = build_chain(&steps);
    println!("  {}", t1("diagnose.conclusion_chain", &chain).dimmed());

    println!();
    println!("  {}", t1("diagnose.elapsed", &format!("{:.1}", elapsed.as_secs_f64())));
}

/// 从 DNS 步骤提取解析到的 IP
fn dns_target_ip(dns: &DiagStep) -> Option<IpAddr> {
    if !dns.ok {
        return None;
    }
    // 从 message 中提取 IP（格式: "系统 DNS: host → IP (Xms)" 或 "System DNS: host → IP (Xms)"）
    let msg = &dns.message;
    // 找 "→ " 后面的部分
    let after_arrow = msg.split("→ ").nth(1)?;
    // 取 " (" 之前的部分作为 IP
    let ip_str = after_arrow.split(" (").next()?.trim();
    ip_str.parse().ok()
}

// ═══════════════════════════════════════════════════════════════
//  检测步骤
// ═══════════════════════════════════════════════════════════════

/// ① DNS 解析
async fn check_dns(host: &str) -> DiagStep {
    let check = t("diagnose.step_dns");
    let start = Instant::now();

    match crate::util::resolve_host(host).await {
        Some(ip) => {
            let elapsed = start.elapsed().as_secs_f64() * 1000.0;
            let msg = t("diagnose.dns_ok")
                .replace("{0}", host)
                .replace("{1}", &ip.to_string())
                .replace("{2}", &format!("{:.0}", elapsed));
            DiagStep { check, ok: true, warning: false, message: msg }
        }
        None => {
            let msg = t1("diagnose.dns_fail", host);
            DiagStep { check, ok: false, warning: false, message: msg }
        }
    }
}

/// ② Ping 探测
async fn check_ping(host: &str) -> DiagStep {
    let check = t("diagnose.step_ping");

    let target = match crate::util::resolve_host(host).await {
        Some(ip) => ip,
        None => {
            return DiagStep {
                check,
                ok: false,
                warning: false,
                message: t1("diagnose.dns_fail", host),
            };
        }
    };

    // ICMP 优先，失败回退 TCP
    let probes = match crate::ping::surge_ping_probe(target, PING_COUNT).await {
        Some(r) => r,
        None => crate::ping::tcp_ping_probe(target, PING_COUNT).await,
    };

    let success_count = probes.iter().filter(|p| p.success).count();
    let total = probes.len();
    let loss_rate = if total > 0 {
        ((total - success_count) as f64 / total as f64) * 100.0
    } else {
        100.0
    };

    let rtts: Vec<f64> = probes.iter().filter_map(|p| p.rtt_ms).collect();
    let stats = crate::util::compute_stats(&rtts);
    let avg = stats.avg_ms.unwrap_or(0.0);

    if success_count == 0 {
        let msg = t("diagnose.ping_fail")
            .replace("{0}", &target.to_string());
        DiagStep { check, ok: false, warning: false, message: msg }
    } else if loss_rate > 0.0 {
        let msg = t("diagnose.ping_ok")
            .replace("{0}", &target.to_string())
            .replace("{1}", &format!("{:.0}", avg))
            .replace("{2}", &format!("{:.0}", loss_rate));
        DiagStep { check, ok: true, warning: true, message: msg }
    } else {
        let msg = t("diagnose.ping_ok")
            .replace("{0}", &target.to_string())
            .replace("{1}", &format!("{:.0}", avg))
            .replace("{2}", &format!("{:.0}", loss_rate));
        DiagStep { check, ok: true, warning: false, message: msg }
    }
}

/// ③ TCP 端口 443
async fn check_tcp(host: &str) -> DiagStep {
    let port = 443u16;
    let check = t1("diagnose.step_tcp", &port.to_string());

    let addr = format!("{}:{}", host, port);
    let start = Instant::now();

    let result = tokio::time::timeout(
        TCP_TIMEOUT,
        tokio::net::TcpStream::connect(&addr),
    ).await;

    match result {
        Ok(Ok(_stream)) => {
            let elapsed = start.elapsed().as_secs_f64() * 1000.0;
            let msg = t("diagnose.tcp_ok").replace("{0}", &format!("{:.0}", elapsed));
            DiagStep { check, ok: true, warning: false, message: msg }
        }
        Ok(Err(e)) => {
            let msg = t1("diagnose.tcp_fail", &e.to_string());
            DiagStep { check, ok: false, warning: false, message: msg }
        }
        Err(_) => {
            let msg = t1("diagnose.tcp_fail", &format!("timeout ({}s)", TCP_TIMEOUT.as_secs()));
            DiagStep { check, ok: false, warning: false, message: msg }
        }
    }
}

/// ④ HTTPS 请求
async fn check_https(host: &str) -> DiagStep {
    let check = t("diagnose.step_https");
    let url = format!("https://{}", host);

    // 检测系统代理
    let proxy_addr = crate::util::get_system_proxy_addr();
    let via_proxy = proxy_addr.is_some();
    let proxy_tag = if via_proxy { t("diagnose.via_proxy") } else { t("diagnose.no_proxy") };

    let mut builder = reqwest::Client::builder().timeout(HTTPS_TIMEOUT);
    if let Some(ref proxy_url) = proxy_addr {
        if let Ok(proxy) = reqwest::Proxy::all(proxy_url) {
            builder = builder.proxy(proxy);
        }
    } else {
        builder = builder.no_proxy();
    }
    let client = builder.build().unwrap();

    let start = Instant::now();
    match client.get(&url).send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let elapsed = start.elapsed().as_secs_f64() * 1000.0;
            let is_success = resp.status().is_success();
            let msg = t("diagnose.https_ok")
                .replace("{0}", &url)
                .replace("{1}", &status.to_string())
                .replace("{2}", &format!("{:.0}", elapsed))
                .replace("{3}", &proxy_tag);
            DiagStep {
                check,
                ok: is_success,
                warning: !is_success,
                message: msg,
            }
        }
        Err(e) => {
            let msg = t("diagnose.https_fail")
                .replace("{0}", &e.to_string())
                .replace("{1}", &proxy_tag);
            DiagStep { check, ok: false, warning: false, message: msg }
        }
    }
}

/// ⑤ Traceroute
async fn check_trace(_host: &str, target_ip: Option<IpAddr>) -> DiagStep {
    let check = t1("diagnose.step_trace", &TRACE_MAX_HOPS.to_string());

    let target = match target_ip {
        Some(ip) => ip,
        None => {
            return DiagStep {
                check,
                ok: false,
                warning: true,
                message: t("diagnose.trace_skip"),
            };
        }
    };

    // IPv4 only（traceroute 不支持 IPv6）
    let target_v4 = match target {
        IpAddr::V4(v4) => v4,
        IpAddr::V6(_) => {
            return DiagStep {
                check,
                ok: false,
                warning: true,
                message: t("diagnose.trace_skip"),
            };
        }
    };

    // 尝试创建 raw socket，失败则跳过
    use socket2::{Domain, Protocol, Socket, Type};
    let test_socket = Socket::new(Domain::IPV4, Type::RAW, Some(Protocol::ICMPV4));
    if test_socket.is_err() {
        return DiagStep {
            check,
            ok: false,
            warning: true,
            message: t("diagnose.trace_skip"),
        };
    }
    drop(test_socket);

    // 执行 traceroute（最多 TRACE_MAX_HOPS 跳，每跳 2 次探测）
    let mut hops_reached = 0u32;
    let mut reached = false;

    for ttl in 1..=TRACE_MAX_HOPS {
        let hop = trace_hop_simple(target_v4, ttl).await;
        hops_reached = ttl;
        if hop.reached {
            reached = true;
            break;
        }
    }

    if reached {
        let msg = t("diagnose.trace_reached").replace("{0}", &hops_reached.to_string());
        DiagStep { check, ok: true, warning: false, message: msg }
    } else {
        let msg = t("diagnose.trace_not_reached").replace("{0}", &TRACE_MAX_HOPS.to_string());
        DiagStep { check, ok: false, warning: true, message: msg }
    }
}

/// 简化版单跳探测（复用 traceroute 逻辑）
struct SimpleHop {
    reached: bool,
}

/// 简化版 trace_hop：发 1 个探测包判断是否到达
async fn trace_hop_simple(target: std::net::Ipv4Addr, ttl: u32) -> SimpleHop {
    use socket2::{Domain, Protocol, Socket, Type};
    use std::mem::MaybeUninit;
    use std::net::{IpAddr, SocketAddr};

    let socket = match Socket::new(Domain::IPV4, Type::RAW, Some(Protocol::ICMPV4)) {
        Ok(s) => s,
        Err(_) => return SimpleHop { reached: false },
    };
    let _ = socket.set_ttl_v4(ttl);
    let _ = socket.set_read_timeout(Some(Duration::from_secs(2)));

    let ident = (std::process::id() & 0xFFFF) as u16;
    let seq = (ttl * 10) as u16;
    let packet = build_icmp_echo_request(ident, seq);

    let dest = SocketAddr::new(IpAddr::V4(target), 0);
    if socket.send_to(&packet, &dest.into()).is_err() {
        return SimpleHop { reached: false };
    }

    let mut buf = [MaybeUninit::new(0); 1024];
    loop {
        match socket.recv_from(&mut buf) {
            Ok((len, from)) => {
                let from_ip = match from.as_socket() {
                    Some(s) => s.ip(),
                    None => continue,
                };
                let data: &[u8] =
                    unsafe { std::slice::from_raw_parts(buf.as_ptr() as *const u8, len) };
                if parse_icmp_response(data, ident, seq).is_some() {
                    return SimpleHop { reached: from_ip == IpAddr::V4(target) };
                }
            }
            Err(_) => return SimpleHop { reached: false },
        }
    }
}

/// 构造 ICMP Echo Request 包
fn build_icmp_echo_request(ident: u16, seq: u16) -> Vec<u8> {
    let mut packet = vec![0u8; 8 + 32];
    packet[0] = 8; // Echo Request
    packet[1] = 0;
    packet[4] = (ident >> 8) as u8;
    packet[5] = (ident & 0xFF) as u8;
    packet[6] = (seq >> 8) as u8;
    packet[7] = (seq & 0xFF) as u8;
    for i in 0..32 {
        packet[8 + i] = i as u8;
    }
    let checksum = icmp_checksum(&packet);
    packet[2] = (checksum >> 8) as u8;
    packet[3] = (checksum & 0xFF) as u8;
    packet
}

fn icmp_checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    let mut i = 0;
    while i + 1 < data.len() {
        sum += ((data[i] as u32) << 8) | (data[i + 1] as u32);
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

fn parse_icmp_response(buf: &[u8], ident: u16, seq: u16) -> Option<()> {
    if buf.len() < 20 {
        return None;
    }
    let ihl = ((buf[0] & 0x0F) * 4) as usize;
    if buf.len() < ihl + 8 {
        return None;
    }
    let icmp_type = buf[ihl];
    match icmp_type {
        0 => {
            let recv_ident = u16::from_be_bytes([buf[ihl + 4], buf[ihl + 5]]);
            let recv_seq = u16::from_be_bytes([buf[ihl + 6], buf[ihl + 7]]);
            if recv_ident == ident && recv_seq == seq { Some(()) } else { None }
        }
        11 => {
            let inner_start = ihl + 8;
            if buf.len() < inner_start + 20 + 8 {
                return Some(());
            }
            let inner_ihl = ((buf[inner_start] & 0x0F) * 4) as usize;
            let icmp_offset = inner_start + inner_ihl;
            if buf.len() < icmp_offset + 8 {
                return Some(());
            }
            if buf[icmp_offset] != 8 {
                return None;
            }
            let orig_ident = u16::from_be_bytes([buf[icmp_offset + 4], buf[icmp_offset + 5]]);
            let orig_seq = u16::from_be_bytes([buf[icmp_offset + 6], buf[icmp_offset + 7]]);
            if orig_ident == ident && orig_seq == seq { Some(()) } else { None }
        }
        _ => None,
    }
}

// ═══════════════════════════════════════════════════════════════
//  结论推导
// ═══════════════════════════════════════════════════════════════

/// 根据各步骤状态自动推导结论
fn derive_conclusion(steps: &[DiagStep]) -> String {
    // steps: [dns, ping, tcp, https, trace]
    let dns_ok = steps.get(0).map(|s| s.ok).unwrap_or(false);
    let ping_ok = steps.get(1).map(|s| s.ok).unwrap_or(false);
    let tcp_ok = steps.get(2).map(|s| s.ok).unwrap_or(false);
    let https_ok = steps.get(3).map(|s| s.ok).unwrap_or(false);

    if !dns_ok {
        return t("diagnose.conclusion_dns");
    }
    if !ping_ok {
        return t("diagnose.conclusion_ping");
    }
    if !tcp_ok {
        return t("diagnose.conclusion_tcp");
    }
    if !https_ok {
        return t("diagnose.conclusion_https");
    }
    t("diagnose.conclusion_healthy")
}

/// 构建链路状态链（DNS → Ping → TCP → HTTPS）
fn build_chain(steps: &[DiagStep]) -> String {
    let labels = ["DNS", "Ping", "TCP", "HTTPS"];
    let arrow = " → ";

    steps
        .iter()
        .take(4)
        .enumerate()
        .map(|(i, step)| {
            let label = labels.get(i).unwrap_or(&"?");
            if step.ok {
                format!("✅ {}", label)
            } else if step.warning {
                format!("⚠️ {}", label)
            } else {
                format!("❌ {}", label)
            }
        })
        .collect::<Vec<_>>()
        .join(arrow)
}
