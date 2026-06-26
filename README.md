# netutils — 本地网络检测工具集

[English](README_EN.md) | 中文

---

一个用 Rust 编写的跨平台命令行网络诊断工具。涵盖网络接口、路由、出口检测、代理检测、Ping、DNS、Traceroute、端口扫描、连通性测试、连接列表、一键诊断和全链路诊断。

### 功能

| 子命令 | 说明 | 示例 |
|--------|------|------|
| `(无)` | 显示全部网络信息 | `netutils` |
| `iface` | 网络接口列表 | `netutils iface` |
| `egress` | 流量出口 + 选路逻辑 | `netutils egress` |
| `route` | 路由表 | `netutils route` |
| `proxy` | 代理设置 | `netutils proxy` |
| `ping` | Ping 主机 (ICMP/TCP) | `netutils ping baidu.com --count 4` |
| `dns` | DNS 查询 | `netutils dns baidu.com --type mx` |
| `trace` | 路由追踪 | `netutils trace baidu.com` |
| `scan` | 端口扫描 | `netutils scan 192.168.1.1 80,443` |
| `check` | 连通性测试 | `netutils check https://baidu.com` |
| `connections` | 网络连接列表 (TCP/UDP) | `netutils connections --state LISTEN` |
| `diag` | 一键诊断 | `netutils diag` |
| `diagnose` | 全链路诊断 (DNS→Ping→TCP→HTTPS→Trace) | `netutils diagnose baidu.com` |

### 安装

```bash
# 从 crates.io 安装（推荐）
cargo install netutils-cli

# 安装后直接使用
netutils --help
```

### 快速开始

```bash
# 从源码编译
git clone https://github.com/dreamsxin/netutils-cli.git
cd netutils-cli
cargo build --release

# 运行
./target/release/netutils

# 查看帮助
./target/release/netutils --help
```

### 一键诊断

```bash
$ netutils diag

🔍 网络诊断报告  2026-06-25 14:30:00

  ✅ [出口] 网络连接正常 (出口: 以太网 192.168.50.4)
  ✅ [国内 DNS] DNS 解析正常 (baidu.com → 111.63.65.247, 45ms)
  ✅ [国际 DNS] DNS 解析正常 (google.com → 142.250.69.174, 180ms)
  ✅ [网关] 默认网关可达 (192.168.50.1, 0.5ms)
  ⚠️  [代理] 系统代理已启用 (127.0.0.1:7897)
  ✅ [国内连通] HTTPS 连通正常 (baidu.com → 200, 54ms) [经代理]
  ✅ [国际连通] HTTPS 连通正常 (google.com → 200, 1096ms) [经代理]
  ❌ [IPv6] IPv6 不可用

  诊断耗时: 8.2s
```

### 全链路诊断

对指定目标自动执行完整链路检测（DNS → Ping → TCP → HTTPS → Traceroute），并自动定位断点给出结论：

```bash
$ netutils diagnose google.com

🔍 全链路诊断: google.com

  ✅ [① DNS 解析]
     系统 DNS: google.com → 142.251.188.138 (199ms)
  ❌ [② Ping 探测]
     173.194.43.139 不可达 (100% 丢包)
  ❌ [③ TCP 端口 443]
     连接失败: timeout (3s)
  ✅ [④ HTTPS 请求]
     https://google.com → 200 (807ms) [经代理]
  ⚠️  [⑤ Traceroute (最多 10 跳)]
     未到达目标 (10 跳内)

  📍 诊断结论: 主机不可达，IP 无法 ping 通
  链路: ✅ DNS → ❌ Ping → ❌ TCP → ✅ HTTPS

  耗时: 20.2s
```

自动结论定位：DNS 失败 → "DNS 解析失败" / Ping 失败 → "主机不可达" / TCP 失败 → "端口不通" / HTTPS 失败 → "HTTPS 异常" / 全部正常 → "链路正常"

### 核心特性

- **国际化**: 自动检测系统语言（中英文），`--lang zh|en` 可覆盖
- **JSON 输出**: `--json` 全局参数，所有子命令支持，便于脚本处理
- **颜色高亮**: 出口绿色、错误红色、虚拟网卡黄色
- **命令别名**: `i`/`e`/`r`/`p`/`pg`/`d`/`t`/`s`/`c`/`co`/`dx`/`dg`
- **跨平台**: Windows (PowerShell)、Linux (`ip`)、macOS (`ifconfig`)
- **系统代理感知**: HTTP 检测自动走系统代理，标注 `[经代理]`/`[直连]`
- **出口检测**: UDP 探测识别实际流量出口 + 解释选路逻辑
- **端口范围语法**: `netutils scan host 80-100,443,8080-8090`

### 系统选路原理

Windows 选择出口接口时按以下顺序决策：

1. **最长前缀匹配**：目标 IP 先匹配最精确的路由条目
2. **有效跃点比较**：`有效跃点 = RouteMetric + InterfaceMetric`，越低越优先
3. **接口优先级**：跃点相同时按接口绑定顺序决定

TUN/VPN 工具（如 Mihomo、Clash）通常在启动时将自己的 `InterfaceMetric` 设为 0，确保流量优先走虚拟网卡。

### 许可

MIT
