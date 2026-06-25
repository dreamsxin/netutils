//! 网络接口信息模块。

use serde::Serialize;
use std::process::Command;

/// 网络接口信息
#[derive(Debug, Clone, Serialize)]
pub struct InterfaceInfo {
    pub name: String,
    pub mac: String,
    pub ipv4: String,
    pub status: String,
    pub description: String,
    pub metric: u32,
    /// 接口类型（英文标识，供 JSON 使用）
    pub iftype: String,
    /// 是否为虚拟网卡
    pub is_virtual: bool,
    /// 是否为出口
    pub is_egress: bool,
    /// 是否为备用默认路由
    pub is_backup: bool,
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
                let iftype = classify_interface(parts[4], parts[0]);
                interfaces.push(InterfaceInfo {
                    name: parts[0].trim().to_string(),
                    mac: parts[1].trim().to_string(),
                    ipv4: parts[2].trim().to_string(),
                    status: parts[3].trim().to_string(),
                    description: parts[4].trim().to_string(),
                    metric: parts[6].trim().parse().unwrap_or(0u32),
                    iftype: iftype.to_id(),
                    is_virtual: iftype.is_virtual(),
                    is_egress: false,
                    is_backup: false,
                });
            }
        }
    }

    interfaces
}

/// 接口类型分类
pub enum IfaceType {
    Loopback,
    Ethernet,
    Wireless,
    MihomoTun,
    ClashTun,
    Wireguard,
    Openvpn,
    Virtualbox,
    Vmware,
    Hyperv,
    Docker,
    TunTap,
    Other,
}

impl IfaceType {
    /// 英文标识（用于 JSON）
    pub fn to_id(&self) -> String {
        match self {
            IfaceType::Loopback => "loopback",
            IfaceType::Ethernet => "ethernet",
            IfaceType::Wireless => "wireless",
            IfaceType::MihomoTun => "mihomo-tun",
            IfaceType::ClashTun => "clash-tun",
            IfaceType::Wireguard => "wireguard",
            IfaceType::Openvpn => "openvpn",
            IfaceType::Virtualbox => "virtualbox",
            IfaceType::Vmware => "vmware",
            IfaceType::Hyperv => "hyperv",
            IfaceType::Docker => "docker",
            IfaceType::TunTap => "tun-tap",
            IfaceType::Other => "other",
        }
        .to_string()
    }

    /// 显示名称（根据语言）
    pub fn to_label(&self) -> String {
        use crate::i18n::{t, Lang};
        let lang = crate::i18n::current();
        let key = match self {
            IfaceType::Loopback => "iface.type_loopback",
            IfaceType::Ethernet => "iface.type_ethernet",
            IfaceType::Wireless => "iface.type_wireless",
            IfaceType::MihomoTun => "iface.type_mihomo",
            IfaceType::ClashTun => "iface.type_clash",
            IfaceType::Wireguard => "iface.type_wireguard",
            IfaceType::Openvpn => "iface.type_openvpn",
            IfaceType::Virtualbox => "iface.type_virtualbox",
            IfaceType::Vmware => "iface.type_vmware",
            IfaceType::Hyperv => "iface.type_hyperv",
            IfaceType::Docker => "iface.type_docker",
            IfaceType::TunTap => "iface.type_tuntap",
            IfaceType::Other => "iface.type_other",
        };
        match lang {
            Lang::Zh => match self {
                IfaceType::Loopback => "回环",
                IfaceType::Ethernet => "以太网",
                IfaceType::Wireless => "无线",
                IfaceType::MihomoTun => "Mihomo/TUN",
                IfaceType::ClashTun => "Clash/TUN",
                IfaceType::Wireguard => "WireGuard",
                IfaceType::Openvpn => "OpenVPN",
                IfaceType::Virtualbox => "VirtualBox",
                IfaceType::Vmware => "VMware",
                IfaceType::Hyperv => "Hyper-V",
                IfaceType::Docker => "Docker",
                IfaceType::TunTap => "TUN/TAP",
                IfaceType::Other => "其他",
            }
            .to_string(),
            Lang::En => t(key),
        }
    }

    pub fn is_virtual(&self) -> bool {
        !matches!(self, IfaceType::Ethernet | IfaceType::Wireless | IfaceType::Loopback | IfaceType::Other)
    }
}

/// 根据描述和名称识别接口类型
pub fn classify_interface(desc: &str, name: &str) -> IfaceType {
    let desc_lower = desc.to_lowercase();
    let name_lower = name.to_lowercase();

    if desc_lower.contains("loopback") || name_lower == "lo" {
        IfaceType::Loopback
    } else if desc_lower.contains("mihomo") || name_lower.contains("mihomo") {
        IfaceType::MihomoTun
    } else if desc_lower.contains("clash") || name_lower.contains("clash") {
        IfaceType::ClashTun
    } else if desc_lower.contains("wireguard") || name_lower.contains("wg") {
        IfaceType::Wireguard
    } else if desc_lower.contains("openvpn") {
        IfaceType::Openvpn
    } else if desc_lower.contains("virtualbox") || desc_lower.contains("vbox") {
        IfaceType::Virtualbox
    } else if desc_lower.contains("vmware") {
        IfaceType::Vmware
    } else if desc_lower.contains("hyper-v") || desc_lower.contains("vethernet") {
        IfaceType::Hyperv
    } else if desc_lower.contains("docker") {
        IfaceType::Docker
    } else if desc_lower.contains("tun") || desc_lower.contains("tap") {
        IfaceType::TunTap
    } else if desc_lower.contains("wireless")
        || desc_lower.contains("wi-fi")
        || desc_lower.contains("wlan")
    {
        IfaceType::Wireless
    } else if desc_lower.contains("ethernet")
        || desc_lower.contains("以太网")
        || desc_lower.contains("pcie")
    {
        IfaceType::Ethernet
    } else {
        IfaceType::Other
    }
}
