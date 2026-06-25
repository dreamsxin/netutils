//! 代理检测模块。

use serde::Serialize;
use std::env;

/// 代理信息条目
#[derive(Debug, Clone, Serialize)]
pub struct ProxyEntry {
    pub ptype: String,
    pub value: String,
}

/// 获取所有代理设置（环境变量 + Windows 系统代理）
pub fn get_proxy_info() -> Vec<ProxyEntry> {
    use crate::i18n::t;
    let mut proxies = Vec::new();

    let proxy_vars = [
        ("HTTP_PROXY", "proxy.http"),
        ("HTTPS_PROXY", "proxy.https"),
        ("ALL_PROXY", "proxy.all"),
        ("NO_PROXY", "proxy.no"),
    ];

    for (var, label_key) in &proxy_vars {
        let value = env::var(var)
            .or_else(|_| env::var(var.to_lowercase()))
            .unwrap_or_default();
        if !value.is_empty() {
            proxies.push(ProxyEntry {
                ptype: t(label_key),
                value,
            });
        }
    }

    if proxies.is_empty() {
        proxies.push(ProxyEntry {
            ptype: t("proxy.env"),
            value: t("common.not_set"),
        });
    }

    #[cfg(target_os = "windows")]
    {
        match get_windows_system_proxy() {
            Some(proxy) => proxies.push(ProxyEntry {
                ptype: t("proxy.system"),
                value: proxy,
            }),
            None => proxies.push(ProxyEntry {
                ptype: t("proxy.system"),
                value: t("proxy.disabled"),
            }),
        }
    }

    proxies
}

#[cfg(target_os = "windows")]
pub fn get_windows_system_proxy() -> Option<String> {
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
