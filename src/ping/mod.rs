//! Ping 模块：ICMP ping，无权限时回退 TCP ping。

use std::time::{Duration, Instant};

use crate::table::print_table;

/// 单次 ping 结果
struct PingResult {
    seq: u32,
    success: bool,
    rtt: Option<Duration>,
    error: Option<String>,
}

/// 执行 ping 并输出结果
///
/// - `host`: 主机名或 IP
/// - `count`: 发送包数
pub async fn run(host: &str, count: u32) {
    println!();
    println!("🏓 Ping {}", host);

    // 解析主机
    let target = match resolve_host(host).await {
        Some(ip) => ip,
        None => {
            println!("  ❌ 无法解析主机: {}", host);
            return;
        }
    };

    println!("  目标: {} ({})", host, target);
    println!();

    // 先尝试 ICMP ping
    let results = match surge_ping(host, target, count).await {
        Some(r) => r,
        None => {
            println!("  ⚠ ICMP 不可用，回退到 TCP ping (端口 80)");
            tcp_ping(host, target, count).await
        }
    };

    print_ping_results(&results);
}

/// 解析主机名为 IP 地址
async fn resolve_host(host: &str) -> Option<std::net::IpAddr> {
    // 如果已经是 IP 地址，直接返回
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        return Some(ip);
    }

    // DNS 解析
    use trust_dns_resolver::TokioAsyncResolver;
    use trust_dns_resolver::config::*;

    let resolver = TokioAsyncResolver::tokio(ResolverConfig::default(), ResolverOpts::default());
    match resolver.lookup_ip(host).await {
        Ok(ips) => ips.iter().next(),
        Err(_) => None,
    }
}

/// ICMP ping（需要权限）
async fn surge_ping(
    host: &str,
    target: std::net::IpAddr,
    count: u32,
) -> Option<Vec<PingResult>> {
    use surge_ping::{Client, ConfigBuilder, PingIdentifier};

    let client = match Client::new(&ConfigBuilder::default().build()) {
        Ok(c) => c,
        Err(e) => {
            println!("  ICMP client 创建失败: {}", e);
            return None;
        }
    };

    let mut results = Vec::new();
    let identifier = PingIdentifier(std::process::id() as u16);

    for seq in 0..count {
        let result = ping_once(&client, target, identifier, seq).await;
        results.push(result);
        // 打印每次结果
        print_ping_line(host, &results.last().unwrap());
        if seq + 1 < count {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    Some(results)
}

/// 单次 ICMP ping
async fn ping_once(
    client: &surge_ping::Client,
    target: std::net::IpAddr,
    identifier: surge_ping::PingIdentifier,
    seq: u32,
) -> PingResult {
    use surge_ping::PingSequence;

    let payload = [0u8; 32];

    let mut pinger = client.pinger(target, identifier).await;
    let result = pinger.ping(PingSequence(seq as u16), &payload).await;

    match result {
        Ok((packet, rtt)) => {
            let _ = packet;
            PingResult {
                seq,
                success: true,
                rtt: Some(rtt),
                error: None,
            }
        }
        Err(e) => PingResult {
            seq,
            success: false,
            rtt: None,
            error: Some(format!("{}", e)),
        },
    }
}

/// TCP ping 回退方案（连接 80 端口测延迟）
async fn tcp_ping(host: &str, target: std::net::IpAddr, count: u32) -> Vec<PingResult> {
    use tokio::net::TcpStream;

    let mut results = Vec::new();

    for seq in 0..count {
        let start = Instant::now();
        let addr = format!("{}:80", target);
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            TcpStream::connect(&addr),
        )
        .await;

        match result {
            Ok(Ok(_stream)) => {
                let rtt = start.elapsed();
                let pr = PingResult {
                    seq,
                    success: true,
                    rtt: Some(rtt),
                    error: None,
                };
                print_ping_line(host, &pr);
                results.push(pr);
            }
            Ok(Err(e)) => {
                let pr = PingResult {
                    seq,
                    success: false,
                    rtt: None,
                    error: Some(format!("TCP: {}", e)),
                };
                print_ping_line(host, &pr);
                results.push(pr);
            }
            Err(_) => {
                let pr = PingResult {
                    seq,
                    success: false,
                    rtt: None,
                    error: Some("TCP: 超时".to_string()),
                };
                print_ping_line(host, &pr);
                results.push(pr);
            }
        }

        if seq + 1 < count {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    results
}

/// 打印单行 ping 结果
fn print_ping_line(host: &str, result: &PingResult) {
    match result.success {
        true => match result.rtt {
            Some(rtt) => println!(
                "  seq={} 来自 {} 时间={:.2}ms",
                result.seq,
                host,
                rtt.as_secs_f64() * 1000.0
            ),
            None => println!("  seq={} 来自 {} (无延迟数据)", result.seq, host),
        },
        false => {
            let err = result.error.as_deref().unwrap_or("未知错误");
            println!("  seq={} 失败: {}", result.seq, err);
        }
    }
}

/// 打印 ping 统计结果
fn print_ping_results(results: &[PingResult]) {
    println!();
    println!("📊 统计");

    let total = results.len();
    let success = results.iter().filter(|r| r.success).count();
    let lost = total - success;
    let loss_rate = if total > 0 {
        (lost as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    let rtts: Vec<f64> = results
        .iter()
        .filter_map(|r| r.rtt.map(|d| d.as_secs_f64() * 1000.0))
        .collect();

    let headers = ["指标", "值"];
    let mut rows = Vec::new();

    rows.push(vec!["发送".to_string(), total.to_string()]);
    rows.push(vec!["接收".to_string(), success.to_string()]);
    rows.push(vec!["丢失".to_string(), lost.to_string()]);
    rows.push(vec![
        "丢包率".to_string(),
        format!("{:.1}%", loss_rate),
    ]);

    if !rtts.is_empty() {
        let min = rtts.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = rtts.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let avg = rtts.iter().sum::<f64>() / rtts.len() as f64;
        rows.push(vec!["最小延迟".to_string(), format!("{:.2}ms", min)]);
        rows.push(vec!["最大延迟".to_string(), format!("{:.2}ms", max)]);
        rows.push(vec!["平均延迟".to_string(), format!("{:.2}ms", avg)]);
    }

    print_table(&headers, &rows);
}
