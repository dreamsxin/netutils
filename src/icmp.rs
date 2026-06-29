//! ICMP 协议工具模块（traceroute 和 diagnose 共享）。

/// 构造 ICMP Echo Request 包
pub fn build_icmp_echo_request(ident: u16, seq: u16) -> Vec<u8> {
    let mut packet = vec![0u8; 8 + 32];
    packet[0] = 8; // Type = Echo Request
    packet[1] = 0; // Code = 0
    packet[4] = (ident >> 8) as u8;
    packet[5] = (ident & 0xFF) as u8;
    packet[6] = (seq >> 8) as u8;
    packet[7] = (seq & 0xFF) as u8;
    for i in 0..32 {
        packet[8 + i] = i as u8;
    }
    let checksum = icmp_checksum(&packet);
    packet[2] = (checksum >> 8) as u8;
    packet[3] = (checksum & 0xFF) as u8;
    packet
}

/// 计算 ICMP 校验和
pub fn icmp_checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    let mut i = 0;
    while i + 1 < data.len() {
        sum += ((data[i] as u32) << 8) | (data[i + 1] as u32);
        i += 2;
    }
    if i < data.len() {
        sum += (data[i] as u32) << 8;
    }
    while (sum >> 16) != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    !(sum as u16)
}

/// 解析 ICMP 响应，校验 ident 和 seq
///
/// 返回 Some(()) 如果是我们请求的响应（Echo Reply 或 Time Exceeded 包含我们的原始包）
pub fn parse_icmp_response(buf: &[u8], ident: u16, seq: u16) -> Option<()> {
    if buf.len() < 20 {
        return None;
    }
    let ihl = ((buf[0] & 0x0F) * 4) as usize;
    if buf.len() < ihl + 8 {
        return None;
    }
    let icmp_type = buf[ihl];
    match icmp_type {
        // Echo Reply (type 0) — 验证 ident 和 seq 匹配
        0 => {
            let recv_ident = u16::from_be_bytes([buf[ihl + 4], buf[ihl + 5]]);
            let recv_seq = u16::from_be_bytes([buf[ihl + 6], buf[ihl + 7]]);
            if recv_ident == ident && recv_seq == seq {
                Some(())
            } else {
                None
            }
        }
        // Time Exceeded (type 11) — 包含原始 IP+ICMP 头，验证原始 ident/seq
        11 => {
            let inner_start = ihl + 8;
            if buf.len() < inner_start + 20 + 8 {
                return Some(()); // 无法解析，仍然接受
            }
            let inner_ihl = ((buf[inner_start] & 0x0F) * 4) as usize;
            let icmp_offset = inner_start + inner_ihl;
            if buf.len() < icmp_offset + 8 {
                return Some(());
            }
            if buf[icmp_offset] != 8 {
                return None; // 不是 Echo Request
            }
            let orig_ident = u16::from_be_bytes([buf[icmp_offset + 4], buf[icmp_offset + 5]]);
            let orig_seq = u16::from_be_bytes([buf[icmp_offset + 6], buf[icmp_offset + 7]]);
            if orig_ident == ident && orig_seq == seq {
                Some(())
            } else {
                None
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_icmp_echo_request() {
        let packet = build_icmp_echo_request(0x1234, 0x0001);
        assert_eq!(packet[0], 8); // Type
        assert_eq!(packet[4], 0x12); // Ident high
        assert_eq!(packet[5], 0x34); // Ident low
        assert_eq!(packet[6], 0x00); // Seq high
        assert_eq!(packet[7], 0x01); // Seq low
    }

    #[test]
    fn test_icmp_checksum() {
        // 校验和应该让整个包的 16 位字反码和为 0
        let packet = build_icmp_echo_request(0x1234, 0x0001);
        let mut sum: u32 = 0;
        let mut i = 0;
        while i + 1 < packet.len() {
            sum += ((packet[i] as u32) << 8) | (packet[i + 1] as u32);
            i += 2;
        }
        if i < packet.len() {
            sum += (packet[i] as u32) << 8;
        }
        while (sum >> 16) != 0 {
            sum = (sum & 0xFFFF) + (sum >> 16);
        }
        assert_eq!(sum, 0xFFFF); // 反码和应为 0xFFFF（即 !0）
    }
}
