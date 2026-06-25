//! DNS 查询模块：支持 A/AAAA/MX/CNAME/NS/TXT 记录。

use crate::table::print_table;
use trust_dns_resolver::config::*;
use trust_dns_resolver::lookup::Lookup;
use trust_dns_resolver::proto::rr::{RecordType, RData};
use trust_dns_resolver::TokioAsyncResolver;

/// DNS 记录类型
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum DnsRecordType {
    /// IPv4 地址
    A,
    /// IPv6 地址
    Aaaa,
    /// 邮件交换
    Mx,
    /// 别名记录
    Cname,
    /// 域名服务器
    Ns,
    /// 文本记录
    Txt,
}

impl DnsRecordType {
    fn to_record_type(self) -> RecordType {
        match self {
            DnsRecordType::A => RecordType::A,
            DnsRecordType::Aaaa => RecordType::AAAA,
            DnsRecordType::Mx => RecordType::MX,
            DnsRecordType::Cname => RecordType::CNAME,
            DnsRecordType::Ns => RecordType::NS,
            DnsRecordType::Txt => RecordType::TXT,
        }
    }
}

/// 执行 DNS 查询并输出结果
pub async fn run(domain: &str, record_type: DnsRecordType) {
    println!();
    println!("🔍 DNS 查询: {} ({:?})", domain, record_type);

    let resolver = TokioAsyncResolver::tokio(ResolverConfig::default(), ResolverOpts::default());

    let type_str = match record_type {
        DnsRecordType::A => "A",
        DnsRecordType::Aaaa => "AAAA",
        DnsRecordType::Mx => "MX",
        DnsRecordType::Cname => "CNAME",
        DnsRecordType::Ns => "NS",
        DnsRecordType::Txt => "TXT",
    };

    let start = std::time::Instant::now();
    let result = query_record(&resolver, domain, record_type).await;
    let elapsed = start.elapsed();

    match result {
        Ok(lookup) => {
            let records: Vec<(&RData, u32)> = lookup
                .record_iter()
                .filter_map(|r| r.data().map(|d| (d, r.ttl())))
                .collect();

            if records.is_empty() {
                println!("  未找到 {} 记录", type_str);
            } else {
                let headers = ["序号", "记录值", "TTL"];
                let rows: Vec<Vec<String>> = records
                    .iter()
                    .enumerate()
                    .map(|(i, (rdata, ttl))| {
                        vec![
                            (i + 1).to_string(),
                            format_record(rdata),
                            format!("{}s", ttl),
                        ]
                    })
                    .collect();
                print_table(&headers, &rows);
            }
        }
        Err(e) => {
            println!("  ❌ 查询失败: {}", e);
        }
    }

    println!();
    println!("  查询耗时: {:.2}ms", elapsed.as_secs_f64() * 1000.0);
}

/// 查询指定类型的 DNS 记录
async fn query_record(
    resolver: &TokioAsyncResolver,
    domain: &str,
    record_type: DnsRecordType,
) -> Result<Lookup, String> {
    let rt = record_type.to_record_type();
    resolver
        .lookup(domain, rt)
        .await
        .map_err(|e| e.to_string())
}

/// 格式化 DNS 记录为字符串
fn format_record(rdata: &RData) -> String {
    match rdata {
        RData::A(addr) => addr.0.to_string(),
        RData::AAAA(addr) => addr.0.to_string(),
        RData::MX(mx) => format!("{} {}", mx.preference(), mx.exchange()),
        RData::CNAME(cname) => cname.0.to_string(),
        RData::NS(ns) => ns.0.to_string(),
        RData::TXT(txt) => {
            let data: Vec<String> = txt
                .txt_data()
                .iter()
                .map(|d| String::from_utf8_lossy(d).to_string())
                .collect();
            data.join(" ")
        }
        other => format!("{:?}", other),
    }
}
