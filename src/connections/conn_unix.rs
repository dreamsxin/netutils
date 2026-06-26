//! Linux/macOS 网络连接实现。
//!
//! 仅包含平台相关的外部命令调用；解析逻辑（`parse_ss_output` /
//! `parse_lsof_output`）位于 [`super`]，无条件编译，便于在任意平台编写单元测试。

use std::process::Command;

use super::ConnectionInfo;
#[cfg(target_os = "linux")]
use super::{parse_netstat_output, parse_ss_output};
#[cfg(target_os = "macos")]
use super::parse_lsof_output;

/// 获取所有 TCP/UDP 连接
pub fn get_connections() -> Vec<ConnectionInfo> {
    #[cfg(target_os = "linux")]
    {
        get_connections_linux()
    }
    #[cfg(target_os = "macos")]
    {
        get_connections_macos()
    }
}

/// Linux: 优先解析 `ss -tunp`，不可用或无输出时回退到 `netstat -tunp`。
#[cfg(target_os = "linux")]
fn get_connections_linux() -> Vec<ConnectionInfo> {
    // 优先 ss（更现代，输出更易解析）
    if let Ok(output) = Command::new("ss").args(["-tunp"]).output() {
        let text = String::from_utf8_lossy(&output.stdout);
        let conns = parse_ss_output(&text);
        if !conns.is_empty() {
            return conns;
        }
    }

    // 回退 netstat（ss 未安装或无连接时）
    if let Ok(output) = Command::new("netstat").args(["-tunp"]).output() {
        let text = String::from_utf8_lossy(&output.stdout);
        return parse_netstat_output(&text);
    }

    Vec::new()
}

/// macOS: 解析 `lsof -i TCP -i UDP -P -n` 输出
#[cfg(target_os = "macos")]
fn get_connections_macos() -> Vec<ConnectionInfo> {
    let output = match Command::new("lsof")
        .args(["-i", "TCP", "-i", "UDP", "-P", "-n"])
        .output()
    {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    let text = String::from_utf8_lossy(&output.stdout);
    parse_lsof_output(&text)
}
