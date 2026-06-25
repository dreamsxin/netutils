//! 连通性测试模块：TCP 端口连通性 + HTTP 请求测试。

use std::time::{Duration, Instant};

use colored::*;
use serde::Serialize;

use crate::i18n::t;
use crate::output::{print_json, OutputMode};
use crate::table::print_table;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// 单次测试结果
#[derive(Serialize, Clone)]
pub struct CheckProbe {
    pub success: bool,
    pub rtt_ms: f64,
    pub status_code: Option<u16>,
    pub error: Option<String>,
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
pub async fn run(target: &str, count: u32, mode: OutputMode) {
    if target.starts_with("http://") || target.starts_with("https://") {
        run_http(target, count, mode).await;
    } else {
        run_tcp(target, count, mode).await;
    }
}

/// TCP 连通性测试
async fn run_tcp(target: &str, count: u32, mode: OutputMode) {
    use tokio::net::TcpStream;
    use tokio::time::timeout;

    let parts: Vec<&str> = target.rsplitn(2, ':').collect();
    if parts.len() != 2 {
        if mode == OutputMode::Json {
            println!("{{\"error\": \"{}\"}}", t("check.format_err"));
        } else {
            println!("  {}", t("check.format_err").red());
        }
        return;
    }
    let port: u16 = match parts[0].parse() {
        Ok(p) => p,
        Err(_) => {
            if mode == OutputMode::Json {
                println!("{{\"error\": \"{}\"}}", t1("check.port_err", parts[0]));
            } else {
                println!("  {}", t1("check.port_err", parts[0]).red());
            }
            return;
        }
    };
    let host = parts[1];

    let mut probes = Vec::new();

    for i in 0..count {
        let start = Instant::now();
        let addr = format!("{}:{}", host, port);
        let result = timeout(CONNECT_TIMEOUT, TcpStream::connect(&addr)).await;
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
                });
            }
            Err(_) => {
                if mode == OutputMode::Table {
                    println!(
                        "  {}",
                        t("check.tcp_timeout")
                            .replace("{0}", &(i + 1).to_string())
                            .replace("{1}", &count.to_string())
                            .replace("{2}", &CONNECT_TIMEOUT.as_secs().to_string())
                            .red()
                    );
                }
                probes.push(CheckProbe {
                    success: false,
                    rtt_ms: CONNECT_TIMEOUT.as_secs_f64() * 1000.0,
                    status_code: None,
                    error: Some(t("check.req_timeout")),
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

/// HTTP 连通性测试
async fn run_http(url: &str, count: u32, mode: OutputMode) {
    let client = reqwest::Client::builder()
        .timeout(CONNECT_TIMEOUT)
        .build()
        .unwrap();

    let mut probes = Vec::new();

    for i in 0..count {
        let start = Instant::now();
        let result = client.get(url).send().await;
        let elapsed = start.elapsed();

        match result {
            Ok(resp) => {
                let status_code = resp.status().as_u16();
                let is_success = resp.status().is_success();

                if mode == OutputMode::Table {
                    let symbol = if is_success { "✓".green() } else { "⚠".yellow() };
                    println!(
                        "  [{}/{}] {} {}  {:.2}ms",
                        i + 1,
                        count,
                        symbol,
                        status_code,
                        elapsed.as_secs_f64() * 1000.0
                    );
                }

                probes.push(CheckProbe {
                    success: is_success,
                    rtt_ms: elapsed.as_secs_f64() * 1000.0,
                    status_code: Some(status_code),
                    error: None,
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
                    println!(
                        "  {}",
                        t("check.http_fail")
                            .replace("{0}", &(i + 1).to_string())
                            .replace("{1}", &count.to_string())
                            .replace("{2}", &msg)
                            .replace("{3}", &format!("{:.2}", elapsed.as_secs_f64() * 1000.0))
                            .red()
                    );
                }

                probes.push(CheckProbe {
                    success: false,
                    rtt_ms: elapsed.as_secs_f64() * 1000.0,
                    status_code: None,
                    error: Some(msg),
                });
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

/// 计算统计
fn compute_stats(probes: &[CheckProbe]) -> CheckStats {
    let total = probes.len();
    let success = probes.iter().filter(|p| p.success).count();
    let rtts: Vec<f64> = probes.iter().filter(|p| p.success).map(|p| p.rtt_ms).collect();

    CheckStats {
        total,
        success,
        failed: total - success,
        min_ms: rtts.iter().cloned().fold(f64::INFINITY, f64::min).into(),
        max_ms: rtts.iter().cloned().fold(f64::NEG_INFINITY, f64::max).into(),
        avg_ms: if rtts.is_empty() {
            None
        } else {
            Some(rtts.iter().sum::<f64>() / rtts.len() as f64)
        },
    }
}

/// 打印统计
fn print_stats(stats: &CheckStats, is_http: bool) {
    println!();
    println!("{}", t("ping.stats").bold());

    let headers = ["指标", "值"];
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

use crate::i18n::t1;
