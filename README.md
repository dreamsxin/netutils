# netutils — 本地网络检测工具集

一个用 Rust 编写的命令行网络检测工具，支持网络接口、路由、出口、代理检测，后续将扩展 Ping、DNS、Traceroute、端口扫描等功能。

## 功能

### 已实现（阶段 1）

| 子命令 | 说明 |
|--------|------|
| `netutils` | 显示全部网络信息（默认） |
| `netutils iface` | 网络接口列表（含物理/虚拟/VPN/TUN 类型识别、接口跃点） |
| `netutils egress` | 流量出口检测 + 选路逻辑说明 |
| `netutils route` | 路由表（默认路由优先） |
| `netutils proxy` | 代理设置（环境变量 + Windows 系统代理注册表） |

### 规划中（阶段 2-4）

| 子命令 | 说明 |
|--------|------|
| `netutils ping <host>` | Ping 主机（ICMP/TCP） |
| `netutils dns <domain>` | DNS 查询（A/AAAA/MX/CNAME/NS/TXT） |
| `netutils trace <host>` | 路由追踪（TTL 递增） |
| `netutils scan <host> [ports]` | 端口扫描（并发 TCP connect） |
| `netutils check <target>` | 连通性测试（TCP/HTTP 延迟） |

## 快速开始

```bash
# 编译
cargo build --release

# 运行（显示全部）
cargo run --release

# 或直接运行编译产物
./target/release/netutils.exe

# 使用子命令
./target/release/netutils.exe iface
./target/release/netutils.exe egress
./target/release/netutils.exe proxy
```

## 输出示例

```
┌────────────────────┐
│  本地网络检测报告  │
└────────────────────┘

📡 网络接口列表
+------+-------------------+-----------------+------+------------+------+--------+
| 名称 | MAC 地址          | IPv4            | 状态 | 类型       | 跃点 | 出口   |
+------+-------------------+-----------------+------+------------+------+--------+
| Mihomo                    |              | 198.18.0.1   | Up | Mihomo/TUN | 0 * | ✓ 出口 |
| 以太网                    | 74-56-3C-... | 192.168.50.4 | Up | 以太网     | 25  | ~ 备用  |
+------+-------------------+-----------------+------+------------+------+--------+

🚪 流量出口
  接口:  Mihomo
  IP:    198.18.0.1
  类型:  Mihomo/TUN
  跃点:  0 (接口跃点，越小优先级越高)

  ┌─ 选路逻辑
  │  有效跃点 = 路由跃点(RouteMetric) + 接口跃点(InterfaceMetric)
  │  Mihomo 的有效跃点 = 0 + 0 = 0，选中
  └─
```

## 出口标记说明

| 标记 | 含义 |
|------|------|
| `✓ 出口` | UDP 探测到的**实际出口接口** |
| `~ 备用` | 路由表中有默认路由，但不是实际出口 |
| `0 *` | 接口跃点为 0（最高优先级） |

## 项目结构

```
netutils/
├── Cargo.toml
├── README.md
└── src/
    ├── main.rs          # 入口：解析子命令，分发到各模块
    ├── cli.rs           # 子命令定义（clap）
    ├── table.rs         # 表格渲染（unicode-width 对齐）
    └── info/
        ├── mod.rs       # 信息检测入口 + 全量输出
        ├── interface.rs # 网络接口列表
        ├── route.rs     # 路由表 + 默认路由
        ├── egress.rs    # 出口检测（UDP 探测）
        └── proxy.rs     # 代理检测
```

## 系统选路原理

Windows 选择出口接口时按以下顺序决策：

1. **最长前缀匹配**：目标 IP 先匹配最精确的路由条目
2. **有效跃点比较**：`有效跃点 = RouteMetric + InterfaceMetric`，越低越优先
3. **接口优先级**：跃点相同时按接口绑定顺序决定

TUN/VPN 工具（如 Mihomo、Clash）通常在启动时将自己的 `InterfaceMetric` 设为 0，确保流量优先走虚拟网卡。

## 依赖

| 依赖 | 用途 |
|------|------|
| `clap` | 子命令解析 |
| `unicode-width` | 表格中英文混排对齐 |
| `winreg` (Windows) | 读取注册表系统代理设置 |

## 许可

MIT
