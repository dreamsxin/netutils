# netutils — 本地网络检测工具集

一个用 Rust 编写的命令行网络检测工具，涵盖网络接口、路由、出口、代理检测，以及 Ping、DNS、Traceroute、端口扫描、连通性测试。

## 子命令一览

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

## 快速开始

```bash
# 编译
cargo build --release

# 运行
./target/release/netutils.exe

# 查看帮助
./target/release/netutils.exe --help
```

## 功能详解

### 📡 网络信息检测

| 子命令 | 说明 |
|--------|------|
| `netutils` / `all` | 全部网络信息（接口 + 出口 + 路由 + 代理） |
| `iface` | 网络接口列表（含物理/虚拟/VPN/TUN 类型识别、接口跃点） |
| `egress` | 流量出口检测 + 选路逻辑说明 |
| `route` | 路由表（默认路由优先） |
| `proxy` | 代理设置（环境变量 + Windows 系统代理注册表） |

### 🏓 Ping

```bash
netutils ping baidu.com              # 默认 4 次
netutils ping 8.8.8.8 --count 10    # 指定次数
```

- ICMP ping（surge-ping），无权限时自动回退 TCP ping
- 自动 DNS 解析主机名
- 统计丢包率、最小/最大/平均延迟

### 🔍 DNS 查询

```bash
netutils dns baidu.com               # 默认 A 记录
netutils dns baidu.com --type mx     # MX 记录
netutils dns baidu.com --type aaaa   # IPv6
```

- 支持 A/AAAA/MX/CNAME/NS/TXT 六种记录类型
- 显示记录值和 TTL

### 🛤️ Traceroute

```bash
netutils trace baidu.com
```

- TTL 递增探测路由路径，最大 30 跳
- 每跳 3 次探测，显示 IP 和延迟
- 使用 raw ICMP socket（socket2）

### 🔎 端口扫描

```bash
netutils scan baidu.com              # 扫描常见端口
netutils scan 192.168.1.1 80,443,8080  # 指定端口
```

- 并发 TCP connect 扫描（100 并发）
- 默认扫描 20 个常见端口（FTP/SSH/HTTP/HTTPS/MySQL 等）
- 超时 1s，只显示开放端口

### 🔌 连通性测试

```bash
netutils check baidu.com:443         # TCP 连通性
netutils check https://baidu.com     # HTTP 连通性
netutils check 8.8.8.8:53 --count 5 # 连续 5 次
```

- `host:port` → TCP 连接延迟
- `http(s)://url` → HTTP 状态码 + 响应时间
- 支持连续测试模式

## 项目结构

```
netutils/
├── Cargo.toml
├── README.md
└── src/
    ├── main.rs           # 入口：解析子命令，分发到各模块
    ├── cli.rs            # 子命令定义（clap）
    ├── table.rs          # 表格渲染（unicode-width 对齐）
    ├── info/
    │   ├── mod.rs        # 信息检测入口 + 全量输出
    │   ├── interface.rs  # 网络接口列表
    │   ├── route.rs      # 路由表
    │   ├── egress.rs     # 出口检测
    │   └── proxy.rs      # 代理检测
    ├── ping/mod.rs       # Ping（ICMP/TCP）
    ├── dns/mod.rs        # DNS 查询
    ├── traceroute/mod.rs # 路由追踪
    ├── portscan/mod.rs   # 端口扫描
    └── connectivity/mod.rs # 连通性测试
```

## 依赖

| 依赖 | 用途 |
|------|------|
| `clap` | 子命令解析 |
| `unicode-width` | 表格中英文混排对齐 |
| `tokio` | 异步运行时 |
| `surge-ping` | ICMP ping |
| `trust-dns-resolver` | DNS 查询 |
| `socket2` | raw socket (traceroute) |
| `reqwest` | HTTP 连通性测试 |
| `winreg` (Windows) | 读取注册表系统代理设置 |

## 系统选路原理

Windows 选择出口接口时按以下顺序决策：

1. **最长前缀匹配**：目标 IP 先匹配最精确的路由条目
2. **有效跃点比较**：`有效跃点 = RouteMetric + InterfaceMetric`，越低越优先
3. **接口优先级**：跃点相同时按接口绑定顺序决定

TUN/VPN 工具（如 Mihomo、Clash）通常在启动时将自己的 `InterfaceMetric` 设为 0，确保流量优先走虚拟网卡。

## 许可

MIT
