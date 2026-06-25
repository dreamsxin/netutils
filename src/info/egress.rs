//! 流量出口检测模块。

use std::net::{IpAddr, UdpSocket};

use super::interface::InterfaceInfo;

/// 通过 UDP 探测实际出口 IP（连接公网地址，不实际发送数据）
pub fn detect_egress_ip() -> Option<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    Some(socket.local_addr().ok()?.ip())
}

/// 通过实际出口 IP 匹配对应的接口名
pub fn find_egress_interface(egress_ip: &IpAddr, interfaces: &[InterfaceInfo]) -> Option<String> {
    let target = egress_ip.to_string();
    interfaces
        .iter()
        .find(|i| i.ipv4 == target)
        .map(|i| i.name.clone())
}
