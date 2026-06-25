//! 端口扫描模块：并发 TCP connect 扫描。

use std::net::IpAddr;
use std::time::Duration;

use tokio::net::TcpStream;
use tokio::sync::Semaphore;
use tokio::time::timeout;

use crate::table::print_table;

/// 单次连接超时
const CONNECT_TIMEOUT: Duration = Duration::from_secs(1);
/// 最大并发数
const MAX_CONCURRENT: usize = 100;

/// 常见端口与服务名映射
const COMMON_PORTS: &[(u16, &str)] = &[
    (21, "FTP"),
    (22, "SSH"),
    (23, "Telnet"),
    (25, "SMTP"),
    (53, "DNS"),
    (80, "HTTP"),
    (110, "POP3"),
    (143, "IMAP"),
    (443, "HTTPS"),
    (445, "SMB"),
    (993, "IMAPS"),
    (995, "POP3S"),
    (1433, "SQL Server"),
    (3306, "MySQL"),
    (3389, "RDP"),
    (5432, "PostgreSQL"),
    (6379, "Redis"),
    (8080, "HTTP Alt"),
    (8443, "HTTPS Alt"),
    (9090, "Prometheus"),
];

/// 执行端口扫描并输出结果
///
/// - `host`: 目标主机
/// - `ports`: 端口列表，None 则扫描常见端口
pub async fn run(host: &str, ports: Option<&[u16]>) {
    println!();
    println!("🔎 端口扫描: {}", host);

    // 解析主机
    let target = match resolve_host(host).await {
        Some(ip) => ip,
        None => {
            println!("  ❌ 无法解析主机: {}", host);
            return;
        }
    };

    let port_list: Vec<u16> = match ports {
        Some(p) => p.to_vec(),
        None => COMMON_PORTS.iter().map(|(p, _)| *p).collect(),
    };

    println!("  目标: {} ({})", host, target);
    println!("  扫描 {} 个端口，并发 {}", port_list.len(), MAX_CONCURRENT);
    println!();

    let semaphore = std::sync::Arc::new(Semaphore::new(MAX_CONCURRENT));
    let mut handles = Vec::new();

    for port in port_list {
        let permit = semaphore.clone();
        let target = target;
        handles.push(tokio::spawn(async move {
            let _permit = permit.acquire_owned().await.unwrap();
            scan_port(target, port).await
        }));
    }

    let mut results = Vec::new();
    for handle in handles {
        if let Ok(result) = handle.await {
            results.push(result);
        }
    }

    // 按端口排序
    results.sort_by_key(|r| r.port);

    // 只显示开放的端口
    let open: Vec<&ScanResult> = results.iter().filter(|r| r.open).collect();

    if open.is_empty() {
        println!("  未发现开放端口");
    } else {
        let headers = ["端口", "状态", "服务"];
        let rows: Vec<Vec<String>> = open
            .iter()
            .map(|r| {
                vec![
                    r.port.to_string(),
                    "open".to_string(),
                    r.service.to_string(),
                ]
            })
            .collect();
        print_table(&headers, &rows);
    }

    println!();
    println!(
        "  扫描完成: {}/{} 开放",
        open.len(),
        results.len()
    );
}

/// 扫描结果
struct ScanResult {
    port: u16,
    open: bool,
    service: &'static str,
}

/// 扫描单个端口
async fn scan_port(target: IpAddr, port: u16) -> ScanResult {
    let addr = format!("{}:{}", target, port);
    let result = timeout(CONNECT_TIMEOUT, TcpStream::connect(&addr)).await;

    let open = result.map(|r| r.is_ok()).unwrap_or(false);
    let service = COMMON_PORTS
        .iter()
        .find(|(p, _)| *p == port)
        .map(|(_, s)| *s)
        .unwrap_or("unknown");

    ScanResult {
        port,
        open,
        service,
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
