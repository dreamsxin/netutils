//! 网络接口信息模块。

use std::process::Command;

/// 网络接口信息
#[derive(Debug, Clone)]
pub struct InterfaceInfo {
    pub name: String,
    pub mac: String,
    pub ipv4: String,
    pub status: String,
    pub description: String,
    pub metric: u32,
}

/// 从 PowerShell 获取所有网络接口信息（含接口跃点）
pub fn get_all_interfaces() -> Vec<InterfaceInfo> {
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
                    metric: parts[6].trim().parse().unwrap_or(0u32),
                });
            }
        }
    }

    interfaces
}

/// 根据描述和名称识别接口类型
pub fn classify_interface(desc: &str, name: &str) -> &'static str {
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

/// 判断接口是否为虚拟网卡
pub fn is_virtual_interface(iftype: &str) -> bool {
    !matches!(iftype, "以太网" | "无线" | "回环" | "其他")
}
