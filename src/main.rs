use std::env;
use std::net::IpAddr;
use std::process::Command;

/// 从 PowerShell 获取所有网络接口信息（含接口跃点）
fn get_all_interfaces() -> Vec<InterfaceInfo> {
    let mut interfaces = Vec::new();

    let ps_script = r#"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Get-NetAdapter | ForEach-Object {
    $adapter = $_
    $ip = Get-NetIPAddress -InterfaceIndex $adapter.ifIndex -AddressFamily IPv4 -ErrorAction SilentlyContinue | Select-Object -First 1
    $metric = Get-NetIPInterface -InterfaceIndex $adapter.ifIndex -AddressFamily IPv4 -ErrorAction SilentlyContinue | Select-Object -First 1
    [PSCustomObject]@{
        Name = $adapter.InterfaceAlias
        MAC = $adapter.MacAddress
        IPv4 = if ($ip) { $ip.IPAddress } else { "--" }
        Status = $adapter.Status
        Type = $adapter.InterfaceDescription
        Index = $adapter.ifIndex
        Metric = if ($metric) { $metric.InterfaceMetric } else { 0 }
    }
} | ForEach-Object { "$($_.Name)|$($_.MAC)|$($_.IPv4)|$($_.Status)|$($_.Type)|$($_.Index)|$($_.Metric)" }
"#;

    if let Ok(output) = Command::new("powershell").args(["-Command", ps_script]).output() {
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            let parts: Vec<&str> = line.splitn(7, '|').collect();
            if parts.len() >= 7 {
                interfaces.push(InterfaceInfo {
                    name: parts[0].trim().to_string(),
                    mac: parts[1].trim().to_string(),
                    ipv4: parts[2].trim().to_string(),
                    status: parts[3].trim().to_string(),
                    description: parts[4].trim().to_string(),
                    _index: parts[5].trim().parse().unwrap_or(0),
                    metric: parts[6].trim().parse().unwrap_or(0u32),
                });
            }
        }
    }

    interfaces
}

#[derive(Debug, Clone)]
struct InterfaceInfo {
    name: String,
    mac: String,
    ipv4: String,
    status: String,
    description: String,
    _index: u32,
    metric: u32,
}

fn classify_interface(desc: &str, name: &str) -> &'static str {
    let desc_lower = desc.to_lowercase();
    let name_lower = name.to_lowercase();

    if desc_lower.contains("loopback") || name_lower == "lo" {
        "回环"
    } else if desc_lower.contains("mihomo") || name_lower.contains("mihomo") {
        "Mihomo/TUN"
    } else if desc_lower.contains("clash") || name_lower.contains("clash") {
        "Clash/TUN"
    } else if desc_lower.contains("wireguard") || name_lower.contains("wg") {
        "WireGuard"
    } else if desc_lower.contains("openvpn") {
        "OpenVPN"
    } else if desc_lower.contains("radmin") {
        "Radmin VPN"
    } else if desc_lower.contains("zerotier") {
        "ZeroTier"
    } else if desc_lower.contains("tailscale") {
        "Tailscale"
    } else if desc_lower.contains("virtualbox") || desc_lower.contains("vbox") {
        "VirtualBox"
    } else if desc_lower.contains("vmware") {
        "VMware"
    } else if desc_lower.contains("hyper-v") || desc_lower.contains("vethernet") {
        "Hyper-V"
    } else if desc_lower.contains("docker") {
        "Docker"
    } else if desc_lower.contains("tun") || desc_lower.contains("tap") {
        "TUN/TAP"
    } else if desc_lower.contains("wireless")
        || desc_lower.contains("wi-fi")
        || desc_lower.contains("wlan")
    {
        "无线"
    } else if desc_lower.contains("ethernet")
        || desc_lower.contains("以太网")
        || desc_lower.contains("pcie")
    {
        "以太网"
    } else {
        "其他"
    }
}

/// 检测实际出口 IP
fn detect_egress_ip() -> Option<IpAddr> {
    use std::net::UdpSocket;
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    Some(socket.local_addr().ok()?.ip())
}

/// 通过实际出口 IP 匹配对应的接口名
fn find_egress_interface(egress_ip: &IpAddr, interfaces: &[InterfaceInfo]) -> Option<String> {
    let target = egress_ip.to_string();
    interfaces
        .iter()
        .find(|i| i.ipv4 == target)
        .map(|i| i.name.clone())
}

/// 从路由表获取所有默认路由
fn get_default_routes() -> Vec<(String, String)> {
    let mut routes = Vec::new();

    let ps_script = r#"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Get-NetRoute -DestinationPrefix "0.0.0.0/0" -ErrorAction SilentlyContinue | Sort-Object RouteMetric | ForEach-Object { "$($_.NextHop)|$($_.InterfaceAlias)" }
"#;

    if let Ok(output) = Command::new("powershell").args(["-Command", ps_script]).output() {
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            if let Some((gw, iface)) = line.split_once('|') {
                routes.push((gw.trim().to_string(), iface.trim().to_string()));
            }
        }
    }

    routes
}

/// 获取路由表
fn get_route_table() -> Vec<RouteEntry> {
    let mut routes = Vec::new();

    let ps_script = r#"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
Get-NetRoute -ErrorAction SilentlyContinue | Where-Object {
    $_.DestinationPrefix -notlike 'ff*' -and $_.DestinationPrefix -notlike '224*' -and $_.DestinationPrefix -notlike '255*'
} | Sort-Object RouteMetric | Select-Object -First 20 | ForEach-Object {
    "$($_.DestinationPrefix)|$($_.NextHop)|$($_.InterfaceAlias)|$($_.RouteMetric)"
}
"#;

    if let Ok(output) = Command::new("powershell").args(["-Command", ps_script]).output() {
        let text = String::from_utf8_lossy(&output.stdout);
        for line in text.lines() {
            let parts: Vec<&str> = line.splitn(4, '|').collect();
            if parts.len() >= 4 {
                routes.push(RouteEntry {
                    destination: parts[0].trim().to_string(),
                    gateway: if parts[1] == "0.0.0.0" || parts[1] == "::" {
                        "--".to_string()
                    } else {
                        parts[1].trim().to_string()
                    },
                    interface: parts[2].trim().to_string(),
                    metric: parts[3].trim().to_string(),
                });
            }
        }
    }

    routes
}

struct RouteEntry {
    destination: String,
    gateway: String,
    interface: String,
    metric: String,
}

fn get_proxy_info() -> Vec<(String, String)> {
    let mut proxies = Vec::new();

    let proxy_vars = [
        ("HTTP_PROXY", "HTTP 代理"),
        ("HTTPS_PROXY", "HTTPS 代理"),
        ("ALL_PROXY", "全局代理"),
        ("NO_PROXY", "排除列表"),
    ];

    for (var, label) in &proxy_vars {
        let value = env::var(var)
            .or_else(|_| env::var(var.to_lowercase()))
            .unwrap_or_default();
        if !value.is_empty() {
            proxies.push((label.to_string(), value));
        }
    }

    if proxies.is_empty() {
        proxies.push(("环境变量".to_string(), "未设置".to_string()));
    }

    #[cfg(target_os = "windows")]
    {
        match get_windows_system_proxy() {
            Some(proxy) => proxies.push(("系统代理".to_string(), proxy)),
            None => proxies.push(("系统代理".to_string(), "未启用".to_string())),
        }
    }

    proxies
}

#[cfg(target_os = "windows")]
fn get_windows_system_proxy() -> Option<String> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let internet_settings = hkcu
        .open_subkey_with_flags(
            "Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings",
            KEY_READ,
        )
        .ok()?;

    let proxy_enable: u32 = internet_settings.get_value("ProxyEnable").ok()?;
    if proxy_enable == 1 {
        let proxy_server: String = internet_settings.get_value("ProxyServer").ok()?;
        if proxy_server.is_empty() {
            None
        } else {
            Some(proxy_server)
        }
    } else {
        None
    }
}

/// 计算字符串的显示宽度（使用 unicode-width 精确计算）
fn display_width(s: &str) -> usize {
    use unicode_width::UnicodeWidthChar;
    let mut width = 0;
    for ch in s.chars() {
        width += ch.width().unwrap_or(0);
    }
    width
}

/// 生成一行表格（统一用显示宽度对齐）
fn format_row(cells: &[String], widths: &[usize]) -> String {
    let parts: Vec<String> = cells
        .iter()
        .zip(widths.iter())
        .map(|(cell, w)| {
            let visible = display_width(cell);
            let padding = if visible < *w { *w - visible } else { 0 };
            format!(" {}{} ", cell, " ".repeat(padding))
        })
        .collect();
    format!("|{}|", parts.join("|"))
}

/// 生成一行表格（表头，&str）
fn format_header_row(headers: &[&str], widths: &[usize]) -> String {
    let parts: Vec<String> = headers
        .iter()
        .zip(widths.iter())
        .map(|(h, w)| {
            let visible = display_width(h);
            let padding = if visible < *w { *w - visible } else { 0 };
            format!(" {}{} ", h, " ".repeat(padding))
        })
        .collect();
    format!("|{}|", parts.join("|"))
}

/// 打印表格
fn print_table(headers: &[&str], rows: &[Vec<String>]) {
    if rows.is_empty() {
        return;
    }

    // 计算每列显示宽度
    let mut widths: Vec<usize> = headers.iter().map(|h| display_width(h)).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() {
                widths[i] = widths[i].max(display_width(cell));
            }
        }
    }

    // 分隔线
    let separator: String = widths
        .iter()
        .map(|w| "-".repeat(w + 2))
        .collect::<Vec<_>>()
        .join("+");
    println!("+{}+", separator);

    // 表头
    println!("{}", format_header_row(headers, &widths));
    println!("+{}+", separator);

    // 数据行
    for row in rows {
        println!("{}", format_row(row, &widths));
    }
    println!("+{}+", separator);
}

fn main() {
    // 生成对齐的 banner 框（用 ASCII 边框避免宽字符计算问题）
    let banner_title = "本地网络检测报告";
    let title_w = display_width(banner_title);
    let inner_w = title_w + 4; // 左右各 2 padding
    let border: String = "─".repeat(inner_w);
    println!();
    println!("┌{}┐", border);
    println!("│  {}  │", banner_title);
    println!("└{}┘", border);

    // 获取所有接口
    let interfaces = get_all_interfaces();

    // 检测出口
    let egress_ip = detect_egress_ip();
    let egress_iface = egress_ip.and_then(|ip| find_egress_interface(&ip, &interfaces));

    // 获取默认路由
    let default_routes = get_default_routes();
    let default_route_ifaces: Vec<&str> = default_routes
        .iter()
        .map(|(_, iface)| iface.as_str())
        .collect();

    // 网络接口列表
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

            // 跃点
            // 跃点：0 显示为特殊标记（最低优先级）
            let metric_str = if iface.metric == 0 {
                "0 *".to_string()
            } else {
                iface.metric.to_string()
            };

            // 出口标记
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

    // 统计
    let virtual_count = interfaces
        .iter()
        .filter(|i| {
            let t = classify_interface(&i.description, &i.name);
            t != "以太网" && t != "无线" && t != "回环" && t != "其他"
        })
        .count();
    println!(
        "  共 {} 个接口，其中 {} 个虚拟网卡",
        interfaces.len(),
        virtual_count
    );

    // 流量出口
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
                // 解释选路
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

    // 路由表
    println!();
    println!("🗺️  路由表 (默认路由优先)");
    let routes = get_route_table();
    let route_headers = ["目标", "网关", "接口", "跃点"];
    let route_rows: Vec<Vec<String>> = routes
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
    print_table(&route_headers, &route_rows);

    // 代理设置
    println!();
    println!("🔒 代理设置");
    let proxies = get_proxy_info();
    let proxy_headers = ["类型", "值"];
    let proxy_rows: Vec<Vec<String>> = proxies
        .iter()
        .map(|(k, v)| vec![k.clone(), v.clone()])
        .collect();
    print_table(&proxy_headers, &proxy_rows);

    println!();
}
