//! Windows 网络接口实现（PowerShell CIM）。
use std::time::Duration;

use super::interface::{classify_interface, InterfaceInfo};

const POWERSHELL_TIMEOUT: Duration = Duration::from_secs(5);

/// 从 PowerShell 获取所有网络接口信息（含接口跃点）
pub fn get_all_interfaces() -> Vec<InterfaceInfo> {
    let mut interfaces = Vec::new();

    let ps_script = r#"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$configs = @{}
Get-CimInstance Win32_NetworkAdapterConfiguration -ErrorAction SilentlyContinue | ForEach-Object {
    if ($null -ne $_.InterfaceIndex) {
        $configs[[int]$_.InterfaceIndex] = $_
    }
}
Get-CimInstance Win32_NetworkAdapter -ErrorAction SilentlyContinue | Where-Object {
    $_.NetConnectionID
} | ForEach-Object {
    $adapter = $_
    $cfg = $configs[[int]$adapter.InterfaceIndex]
    $ipv4 = "--"
    if ($cfg -and $cfg.IPAddress) {
        $first = @($cfg.IPAddress | Where-Object { $_ -match '^\d+\.\d+\.\d+\.\d+$' } | Select-Object -First 1)
        if ($first.Count -gt 0) { $ipv4 = $first[0] }
    }
    $status = if ($adapter.NetConnectionStatus -eq 2) { "Up" } else { "Down" }
    $metric = if ($cfg -and $cfg.IPConnectionMetric) { $cfg.IPConnectionMetric } else { 0 }
    "$($adapter.NetConnectionID)|$($adapter.MACAddress)|$ipv4|$status|$($adapter.Description)|$($adapter.InterfaceIndex)|$metric"
}
"#;

    if let Some(output) = crate::util::powershell_output(ps_script, POWERSHELL_TIMEOUT) {
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
