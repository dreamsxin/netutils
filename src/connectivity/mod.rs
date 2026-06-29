//! 连通性测试模块：TCP 端口连通性 + HTTP 请求测试。

use std::time::{Duration, Instant};

use colored::*;
use serde::Serialize;

use crate::i18n::t;
use crate::output::{print_json, print_json_error, OutputMode};
use crate::table::print_table;

/// 分阶段耗时
#[derive(Serialize, Clone, Default)]
pub struct TimingBreakdown {
    pub dns_ms: f64,
    pub connect_ms: f64,
    pub tls_ms: f64,
    pub ttfb_ms: f64,
    pub total_ms: f64,
}

/// 单次测试结果
#[derive(Serialize, Clone)]
pub struct CheckProbe {
    pub success: bool,
    pub rtt_ms: f64,
    pub status_code: Option<u16>,
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timing: Option<TimingBreakdown>,
}

/// 连通性测试完整输出
#[derive(Serialize)]
pub struct CheckOutput {
    pub target: String,
    pub check_type: String,
    pub probes: Vec<CheckProbe>,
    pub stats: CheckStats,
}

#[derive(Serialize, Clone)]
pub struct CheckStats {
    pub total: usize,
    pub success: usize,
    pub failed: usize,
    pub min_ms: Option<f64>,
    pub max_ms: Option<f64>,
    pub avg_ms: Option<f64>,
}

/// 执行连通性测试
pub async fn run(target: &str, count: u32, timeout: Duration, timing: bool, proxy: Option<String>, no_proxy: bool, mode: OutputMode) {
    if target.starts_with("http://") || target.starts_with("https://") {
        run_http(target, count, timeout, timing, proxy, no_proxy, mode).await;
    } else {
        run_tcp(target, count, timeout, mode).await;
    }
}

/// 解析 host:port（支持 IPv6 如 [::1]:443）
pub(crate) fn parse_host_port(target: &str) -> Option<(String, u16)> {
    if let Ok(addr) = target.parse::<std::net::SocketAddr>() {
        return Some((addr.ip().to_string(), addr.port()));
    }
    if let Some(idx) = target.rfind(':') {
        let host = &target[..idx];
        let port_str = &target[idx + 1..];
        if let Ok(port) = port_str.parse::<u16>() {
            let host = host.trim_start_matches('[').trim_end_matches(']');
            return Some((host.to_string(), port));
        }
    }
    None
}

/// 从 URL 提取 host 和 port
fn parse_url(url: &str) -> Option<(String, u16, bool)> {
    let (scheme, rest) = url.split_once("://")?;
    let is_https = scheme == "https";
    let default_port = if is_https { 443 } else { 80 };
    let host = rest.split('/').next()?;
    let (host, port) = if let Some((h, p)) = host.rsplit_once(':') {
        (h.to_string(), p.parse().unwrap_or(default_port))
    } else {
        (host.to_string(), default_port)
    };
    Some((host, port, is_https))
}

/// TCP 连通性测试
async fn run_tcp(target: &str, count: u32, connect_timeout: Duration, mode: OutputMode) {
    use tokio::net::TcpStream;
    use tokio::time::timeout;

    let (host, port) = match parse_host_port(target) {
        Some(hp) => hp,
        None => {
            if mode == OutputMode::Json {
                print_json_error(&t("check.format_err"));
            } else {
                println!("  {}", t("check.format_err").red());
            }
            return;
        }
    };

    let mut probes = Vec::new();

    for i in 0..count {
        let start = Instant::now();
        let addr = format!("{}:{}", host, port);
        let result = timeout(connect_timeout, TcpStream::connect(&addr)).await;
        let elapsed = start.elapsed();

        match result {
            Ok(Ok(_stream)) => {
                if mode == OutputMode::Table {
                    println!(
                        "  {}",
                        t("check.tcp_ok")
                            .replace("{0}", &(i + 1).to_string())
                            .replace("{1}", &count.to_string())
                            .replace("{2}", &format!("{:.2}", elapsed.as_secs_f64() * 1000.0))
                            .green()
                    );
                }
                probes.push(CheckProbe {
                    success: true,
                    rtt_ms: elapsed.as_secs_f64() * 1000.0,
                    status_code: None,
                    error: None,
                    timing: None,
                });
            }
            Ok(Err(e)) => {
                if mode == OutputMode::Table {
                    println!(
                        "  {}",
                        t("check.tcp_fail")
                            .replace("{0}", &(i + 1).to_string())
                            .replace("{1}", &count.to_string())
                            .replace("{2}", &e.to_string())
                            .red()
                    );
                }
                probes.push(CheckProbe {
                    success: false,
                    rtt_ms: elapsed.as_secs_f64() * 1000.0,
                    status_code: None,
                    error: Some(e.to_string()),
                    timing: None,
                });
            }
            Err(_) => {
                if mode == OutputMode::Table {
                    println!(
                        "  {}",
                        t("check.tcp_timeout")
                            .replace("{0}", &(i + 1).to_string())
                            .replace("{1}", &count.to_string())
                            .replace("{2}", &connect_timeout.as_secs().to_string())
                            .red()
                    );
                }
                probes.push(CheckProbe {
                    success: false,
                    rtt_ms: connect_timeout.as_secs_f64() * 1000.0,
                    status_code: None,
                    error: Some(t("check.req_timeout")),
                    timing: None,
                });
            }
        }

        if i + 1 < count {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    let stats = compute_stats(&probes);
    let output = CheckOutput {
        target: target.to_string(),
        check_type: "tcp".to_string(),
        probes: probes.clone(),
        stats: stats.clone(),
    };

    if mode == OutputMode::Json {
        print_json(&output);
        return;
    }

    print_stats(&stats, false);
}

/// HTTP 连通性测试（自动检测并使用系统代理）
async fn run_http(url: &str, count: u32, connect_timeout: Duration, timing: bool, proxy: Option<String>, no_proxy: bool, mode: OutputMode) {
    // 确定代理：--proxy 优先 > --no-proxy 强制直连 > 系统自动检测
    let proxy_addr = if let Some(ref p) = proxy {
        Some(p.clone())
    } else if no_proxy {
        None
    } else {
        crate::util::get_system_proxy_addr()
    };
    // --timing 仅在直连 HTTPS 时生效，有代理时静默忽略
    let can_breakdown = timing && proxy_addr.is_none() && url.starts_with("https://");

    let mut builder = reqwest::Client::builder().timeout(connect_timeout);

    if let Some(ref proxy_url) = proxy_addr {
        if let Ok(proxy) = reqwest::Proxy::all(proxy_url) {
            builder = builder.proxy(proxy);
        }
    } else {
        builder = builder.no_proxy();
    }

    let client = builder.build().unwrap();

    let mut probes = Vec::new();

    for i in 0..count {
        if can_breakdown {
            // 手动分步计时：DNS → TCP → TLS → TTFB
            let probe = run_http_timing(url, connect_timeout, mode, i, count).await;
            probes.push(probe);
        } else {
            // 降级模式：reqwest 单次计时
            let start = Instant::now();
            let result = client.get(url).send().await;
            let elapsed = start.elapsed();

            match result {
                Ok(resp) => {
                    let status_code = resp.status().as_u16();
                    let is_success = resp.status().is_success();

                    if mode == OutputMode::Table {
                        let symbol = if is_success { "✓".green() } else { "⚠".yellow() };
                        let proxy_tag = if proxy_addr.is_some() {
                            format!(" [{}]", t("diagnose.via_proxy"))
                        } else {
                            String::new()
                        };
                        println!(
                            "  [{}/{}] {} {}  {:.2}ms{}",
                            i + 1,
                            count,
                            symbol,
                            status_code,
                            elapsed.as_secs_f64() * 1000.0,
                            proxy_tag
                        );
                    }

                    probes.push(CheckProbe {
                        success: is_success,
                        rtt_ms: elapsed.as_secs_f64() * 1000.0,
                        status_code: Some(status_code),
                        error: None,
                        timing: None,
                    });
                }
                Err(e) => {
                    let msg = if e.is_connect() {
                        t("check.conn_fail")
                    } else if e.is_timeout() {
                        t("check.req_timeout")
                    } else {
                        e.to_string()
                    };

                    if mode == OutputMode::Table {
                        let proxy_tag = if proxy_addr.is_some() {
                            format!(" [{}]", t("diagnose.via_proxy"))
                        } else {
                            String::new()
                        };
                        println!(
                            "  {}",
                            format!(
                                "[{}/{}] ✗ {}  {:.2}ms{}",
                                i + 1,
                                count,
                                msg,
                                elapsed.as_secs_f64() * 1000.0,
                                proxy_tag
                            ).red()
                        );
                    }

                    probes.push(CheckProbe {
                        success: false,
                        rtt_ms: elapsed.as_secs_f64() * 1000.0,
                        status_code: None,
                        error: Some(msg),
                        timing: None,
                    });
                }
            }
        }

        if i + 1 < count {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    let stats = compute_stats(&probes);
    let output = CheckOutput {
        target: url.to_string(),
        check_type: "http".to_string(),
        probes: probes.clone(),
        stats: stats.clone(),
    };

    if mode == OutputMode::Json {
        print_json(&output);
        return;
    }

    print_stats(&stats, true);
}

/// 手动分步计时 HTTPS 请求（DNS → TCP → TLS → TTFB）
async fn run_http_timing(url: &str, timeout: Duration, mode: OutputMode, i: u32, count: u32) -> CheckProbe {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;
    use tokio_rustls::TlsConnector;

    let (host, port, _) = match parse_url(url) {
        Some(hp) => hp,
        None => {
            return CheckProbe {
                success: false,
                rtt_ms: 0.0,
                status_code: None,
                error: Some("URL parse error".to_string()),
                timing: None,
            };
        }
    };

    let path = url.split("://").nth(1).and_then(|s| s.find('/').map(|idx| &s[idx..])).unwrap_or("/");

    let t0 = Instant::now();

    // ① DNS
    let ip = match crate::util::resolve_host(&host).await {
        Some(ip) => ip,
        None => {
            return CheckProbe {
                success: false,
                rtt_ms: t0.elapsed().as_secs_f64() * 1000.0,
                status_code: None,
                error: Some("DNS resolve failed".to_string()),
                timing: None,
            };
        }
    };
    let dns_ms = t0.elapsed().as_secs_f64() * 1000.0;

    // ② TCP Connect
    let t1 = Instant::now();
    let tcp_result = tokio::time::timeout(timeout, TcpStream::connect((ip, port))).await;
    let tcp_stream = match tcp_result {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => {
            return CheckProbe {
                success: false,
                rtt_ms: t0.elapsed().as_secs_f64() * 1000.0,
                status_code: None,
                error: Some(format!("TCP: {}", e)),
                timing: Some(TimingBreakdown {
                    dns_ms,
                    connect_ms: t1.elapsed().as_secs_f64() * 1000.0,
                    ..Default::default()
                }),
            };
        }
        Err(_) => {
            return CheckProbe {
                success: false,
                rtt_ms: t0.elapsed().as_secs_f64() * 1000.0,
                status_code: None,
                error: Some("TCP: timeout".to_string()),
                timing: Some(TimingBreakdown {
                    dns_ms,
                    connect_ms: t1.elapsed().as_secs_f64() * 1000.0,
                    ..Default::default()
                }),
            };
        }
    };
    let connect_ms = t1.elapsed().as_secs_f64() * 1000.0;

    // ③ TLS Handshake
    let t2 = Instant::now();
    let root_store = rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.iter().cloned().collect(),
    };
    let config = rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(root_store)
        .with_no_client_auth();
    let connector = TlsConnector::from(std::sync::Arc::new(config));

    let server_name = match rustls::pki_types::ServerName::try_from(host.clone()) {
        Ok(n) => n,
        Err(e) => {
            return CheckProbe {
                success: false,
                rtt_ms: t0.elapsed().as_secs_f64() * 1000.0,
                status_code: None,
                error: Some(format!("TLS: {}", e)),
                timing: Some(TimingBreakdown {
                    dns_ms,
                    connect_ms,
                    ..Default::default()
                }),
            };
        }
    };

    let tls_stream = match connector.connect(server_name, tcp_stream).await {
        Ok(s) => s,
        Err(e) => {
            return CheckProbe {
                success: false,
                rtt_ms: t0.elapsed().as_secs_f64() * 1000.0,
                status_code: None,
                error: Some(format!("TLS: {}", e)),
                timing: Some(TimingBreakdown {
                    dns_ms,
                    connect_ms,
                    tls_ms: t2.elapsed().as_secs_f64() * 1000.0,
                    ..Default::default()
                }),
            };
        }
    };
    let tls_ms = t2.elapsed().as_secs_f64() * 1000.0;

    // ④ TTFB: 发送 HTTP GET，读取响应
    let t3 = Instant::now();
    let mut tls_stream = tls_stream;
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: netutils/0.3\r\nConnection: close\r\n\r\n",
        path, host
    );

    if tls_stream.write_all(request.as_bytes()).await.is_err() {
        return CheckProbe {
            success: false,
            rtt_ms: t0.elapsed().as_secs_f64() * 1000.0,
            status_code: None,
            error: Some("HTTP: write failed".to_string()),
            timing: Some(TimingBreakdown {
                dns_ms,
                connect_ms,
                tls_ms,
                ..Default::default()
            }),
        };
    }

    // 读取响应（至少读取状态行）
    let mut buf = [0u8; 4096];
    let n = match tls_stream.read(&mut buf).await {
        Ok(n) => n,
        Err(e) => {
            return CheckProbe {
                success: false,
                rtt_ms: t0.elapsed().as_secs_f64() * 1000.0,
                status_code: None,
                error: Some(format!("HTTP: {}", e)),
                timing: Some(TimingBreakdown {
                    dns_ms,
                    connect_ms,
                    tls_ms,
                    ..Default::default()
                }),
            };
        }
    };
    let ttfb_ms = t3.elapsed().as_secs_f64() * 1000.0;
    let total_ms = t0.elapsed().as_secs_f64() * 1000.0;

    // 解析状态码
    let response = String::from_utf8_lossy(&buf[..n]);
    let status_code = response
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);
    let is_success = (200..300).contains(&status_code);

    let timing_bd = TimingBreakdown {
        dns_ms,
        connect_ms,
        tls_ms,
        ttfb_ms,
        total_ms,
    };

    if mode == OutputMode::Table {
        let symbol = if is_success { "✓".green() } else { "⚠".yellow() };
        println!(
            "  [{}/{}] {} {}  {:.2}ms",
            i + 1,
            count,
            symbol,
            status_code,
            total_ms
        );
        println!("    {:<10} {:.2}ms", t("check.timing_dns"), dns_ms);
        println!("    {:<10} {:.2}ms", t("check.timing_connect"), connect_ms);
        println!("    {:<10} {:.2}ms", t("check.timing_tls"), tls_ms);
        println!("    {:<10} {:.2}ms", t("check.timing_ttfb"), ttfb_ms);
        println!("    {:<10} {:.2}ms", t("check.timing_total"), total_ms);
    }

    CheckProbe {
        success: is_success,
        rtt_ms: total_ms,
        status_code: Some(status_code),
        error: None,
        timing: Some(timing_bd),
    }
}

/// 计算统计
fn compute_stats(probes: &[CheckProbe]) -> CheckStats {
    let total = probes.len();
    let success = probes.iter().filter(|p| p.success).count();
    let rtts: Vec<f64> = probes.iter().filter(|p| p.success).map(|p| p.rtt_ms).collect();
    let stats = crate::util::compute_stats(&rtts);

    CheckStats {
        total,
        success,
        failed: total - success,
        min_ms: stats.min_ms,
        max_ms: stats.max_ms,
        avg_ms: stats.avg_ms,
    }
}

/// 打印统计
fn print_stats(stats: &CheckStats, is_http: bool) {
    println!();
    println!("{}", t("ping.stats").bold());

    let h_metric = t("common.metric");
    let h_value = t("proxy.value");
    let headers = [h_metric.as_str(), h_value.as_str()];
    let mut rows = Vec::new();
    rows.push(vec![t("check.count"), stats.total.to_string()]);

    if is_http {
        rows.push(vec![t("check.ok_2xx"), stats.success.to_string()]);
    } else {
        rows.push(vec![t("check.ok"), stats.success.to_string()]);
    }
    rows.push(vec![t("check.fail_count"), stats.failed.to_string()]);

    if let (Some(min), Some(max), Some(avg)) = (stats.min_ms, stats.max_ms, stats.avg_ms) {
        rows.push(vec![t("ping.min"), format!("{:.2}ms", min)]);
        rows.push(vec![t("ping.max"), format!("{:.2}ms", max)]);
        rows.push(vec![t("ping.avg"), format!("{:.2}ms", avg)]);
    }

    print_table(&headers, &rows);
}
