//! 连通性测试模块：TCP 端口连通性 + HTTP 请求测试。

use std::time::{Duration, Instant};

use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::table::print_table;

/// 连接超时
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// 执行连通性测试
///
/// - `target`: 目标地址
///   - `host:port` → TCP 连通性测试
///   - `http(s)://url` → HTTP 请求测试
/// - `count`: 连续测试次数
pub async fn run(target: &str, count: u32) {
    println!();
    println!("🔌 连通性测试: {}", target);

    if target.starts_with("http://") || target.starts_with("https://") {
        run_http(target, count).await;
    } else {
        run_tcp(target, count).await;
    }
}

/// TCP 连通性测试
async fn run_tcp(target: &str, count: u32) {
    // 解析 host:port
    let parts: Vec<&str> = target.rsplitn(2, ':').collect();
    if parts.len() != 2 {
        println!("  ❌ 格式错误，请使用 host:port");
        return;
    }
    let port: u16 = match parts[0].parse() {
        Ok(p) => p,
        Err(_) => {
            println!("  ❌ 端口号无效: {}", parts[0]);
            return;
        }
    };
    let host = parts[1];

    println!("  类型: TCP");
    println!("  目标: {}:{}", host, port);
    println!();

    let mut results = Vec::new();

    for i in 0..count {
        let start = Instant::now();
        let addr = format!("{}:{}", host, port);
        let result = timeout(CONNECT_TIMEOUT, TcpStream::connect(&addr)).await;
        let elapsed = start.elapsed();

        match result {
            Ok(Ok(_stream)) => {
                println!(
                    "  [{}/{}] ✓ 连接成功  {:.2}ms",
                    i + 1,
                    count,
                    elapsed.as_secs_f64() * 1000.0
                );
                results.push((true, elapsed));
            }
            Ok(Err(e)) => {
                println!("  [{}/{}] ✗ 连接失败  {}", i + 1, count, e);
                results.push((false, elapsed));
            }
            Err(_) => {
                println!("  [{}/{}] ✗ 连接超时 ({}s)", i + 1, count, CONNECT_TIMEOUT.as_secs());
                results.push((false, CONNECT_TIMEOUT));
            }
        }

        if i + 1 < count {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    print_tcp_stats(&results);
}

/// HTTP 连通性测试
async fn run_http(url: &str, count: u32) {
    println!("  类型: HTTP");
    println!("  URL:  {}", url);
    println!();

    let client = reqwest::Client::builder()
        .timeout(CONNECT_TIMEOUT)
        .build()
        .unwrap();

    let mut results = Vec::new();

    for i in 0..count {
        let start = Instant::now();
        let result = client.get(url).send().await;
        let elapsed = start.elapsed();

        match result {
            Ok(resp) => {
                let status = resp.status();
                let status_code = status.as_u16();
                let is_success = status.is_success();
                let symbol = if is_success { "✓" } else { "⚠" };

                println!(
                    "  [{}/{}] {} {}  {:.2}ms",
                    i + 1,
                    count,
                    symbol,
                    status_code,
                    elapsed.as_secs_f64() * 1000.0
                );
                results.push((is_success, status_code, elapsed));
            }
            Err(e) => {
                let msg = if e.is_connect() {
                    "连接失败".to_string()
                } else if e.is_timeout() {
                    "请求超时".to_string()
                } else {
                    e.to_string()
                };
                println!("  [{}/{}] ✗ {}  {:.2}ms", i + 1, count, msg, elapsed.as_secs_f64() * 1000.0);
                results.push((false, 0, elapsed));
            }
        }

        if i + 1 < count {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    print_http_stats(&results);
}

/// 打印 TCP 测试统计
fn print_tcp_stats(results: &[(bool, Duration)]) {
    let total = results.len();
    let success = results.iter().filter(|(s, _)| *s).count();
    let rtts: Vec<f64> = results
        .iter()
        .filter(|(s, _)| *s)
        .map(|(_, d)| d.as_secs_f64() * 1000.0)
        .collect();

    println!();
    println!("📊 统计");

    let headers = ["指标", "值"];
    let mut rows = Vec::new();
    rows.push(vec!["测试次数".to_string(), total.to_string()]);
    rows.push(vec!["成功".to_string(), success.to_string()]);
    rows.push(vec!["失败".to_string(), (total - success).to_string()]);

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

/// 打印 HTTP 测试统计
fn print_http_stats(results: &[(bool, u16, Duration)]) {
    let total = results.len();
    let success = results.iter().filter(|(s, _, _)| *s).count();
    let rtts: Vec<f64> = results
        .iter()
        .filter(|(s, _, _)| *s)
        .map(|(_, _, d)| d.as_secs_f64() * 1000.0)
        .collect();

    println!();
    println!("📊 统计");

    let headers = ["指标", "值"];
    let mut rows = Vec::new();
    rows.push(vec!["测试次数".to_string(), total.to_string()]);
    rows.push(vec!["成功 (2xx)".to_string(), success.to_string()]);
    rows.push(vec!["失败/错误".to_string(), (total - success).to_string()]);

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
