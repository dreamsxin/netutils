//! 网络信息检测模块：接口、路由、出口、代理。

pub mod egress;
pub mod interface;
pub mod proxy;
pub mod route;

use crate::table::print_table;
use egress::{detect_egress_ip, find_egress_interface};
use interface::{classify_interface, get_all_interfaces, is_virtual_interface};
use route::{get_default_routes, get_route_table};

/// 打印 banner 标题框
pub fn print_banner(title: &str) {
    let title_w = crate::table::display_width(title);
    let inner_w = title_w + 4;
    let border: String = "─".repeat(inner_w);
    println!();
    println!("┌{}┐", border);
    println!("│  {}  │", title);
    println!("└{}┘", border);
}

/// 打印网络接口列表
pub fn print_interfaces() {
    let interfaces = get_all_interfaces();
    let egress_ip = detect_egress_ip();
    let egress_iface = egress_ip.and_then(|ip| find_egress_interface(&ip, &interfaces));

    let default_routes = get_default_routes();
    let default_route_ifaces: Vec<&str> = default_routes
        .iter()
        .map(|(_, iface)| iface.as_str())
        .collect();

    println!();
    println!("📡 网络接口列表");

    let headers = ["名称", "MAC 地址", "IPv4", "状态", "类型", "跃点", "出口"];
    let rows: Vec<Vec<String>> = interfaces
        .iter()
        .map(|iface| {
            let iftype = classify_interface(&iface.description, &iface.name);
            let status = if iface.status == "Up" {
                "Up".to_string()
            } else {
                "Down".to_string()
            };
            let metric_str = if iface.metric == 0 {
                "0 *".to_string()
            } else {
                iface.metric.to_string()
            };
            let is_egress = if Some(&iface.name) == egress_iface.as_ref() {
                "✓ 出口".to_string()
            } else if default_route_ifaces.contains(&iface.name.as_str()) {
                "~ 备用".to_string()
            } else {
                "".to_string()
            };

            vec![
                iface.name.clone(),
                iface.mac.clone(),
                iface.ipv4.clone(),
                status,
                iftype.to_string(),
                metric_str,
                is_egress,
            ]
        })
        .collect();

    print_table(&headers, &rows);

    let virtual_count = interfaces
        .iter()
        .filter(|i| is_virtual_interface(classify_interface(&i.description, &i.name)))
        .count();
    println!(
        "  共 {} 个接口，其中 {} 个虚拟网卡",
        interfaces.len(),
        virtual_count
    );
}

/// 打印流量出口信息
pub fn print_egress() {
    let interfaces = get_all_interfaces();
    let egress_ip = detect_egress_ip();
    let egress_iface = egress_ip.and_then(|ip| find_egress_interface(&ip, &interfaces));

    println!();
    println!("🚪 流量出口");
    match egress_iface {
        Some(ref name) => {
            if let Some(iface) = interfaces.iter().find(|i| &i.name == name) {
                let iftype = classify_interface(&iface.description, &iface.name);
                println!("  接口:  {}", iface.name);
                println!("  IP:    {}", iface.ipv4);
                println!("  类型:  {}", iftype);
                println!("  跃点:  {} (接口跃点，越小优先级越高)", iface.metric);
                println!();
                println!("  ┌─ 选路逻辑");
                println!("  │  系统为出站流量选择出口时，比较每个候选路由的 有效跃点：");
                println!("  │    有效跃点 = 路由跃点(RouteMetric) + 接口跃点(InterfaceMetric)");
                println!("  │  有效跃点越低，接口越优先。");
                println!("  │");
                println!(
                    "  │  {} 的有效跃点 = 路由跃点({}) + 接口跃点({}) = {}，选中",
                    iface.name, 0, iface.metric, iface.metric
                );
                println!("  └─");
            } else {
                println!("  IP:   {}", egress_ip.unwrap());
            }
        }
        None => {
            println!("  无法检测（可能无网络连接）");
        }
    }
}

/// 打印路由表
pub fn print_routes() {
    println!();
    println!("🗺️  路由表 (默认路由优先)");
    let routes = get_route_table();
    let headers = ["目标", "网关", "接口", "跃点"];
    let rows: Vec<Vec<String>> = routes
        .iter()
        .map(|r| {
            vec![
                r.destination.clone(),
                r.gateway.clone(),
                r.interface.clone(),
                r.metric.clone(),
            ]
        })
        .collect();
    print_table(&headers, &rows);
}

/// 打印代理设置
pub fn print_proxy() {
    println!();
    println!("🔒 代理设置");
    let proxies = proxy::get_proxy_info();
    let headers = ["类型", "值"];
    let rows: Vec<Vec<String>> = proxies
        .iter()
        .map(|(k, v)| vec![k.clone(), v.clone()])
        .collect();
    print_table(&headers, &rows);
}

/// 打印全部网络信息
pub fn print_all() {
    print_banner("本地网络检测报告");
    print_interfaces();
    print_egress();
    print_routes();
    print_proxy();
    println!();
}
