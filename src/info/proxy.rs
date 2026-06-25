//! 代理检测模块。

use std::env;

/// 代理信息条目 (类型名, 值)
pub type ProxyEntry = (String, String);

/// 获取所有代理设置（环境变量 + Windows 系统代理）
pub fn get_proxy_info() -> Vec<ProxyEntry> {
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
