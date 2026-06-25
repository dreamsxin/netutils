//! 输出模式：表格（默认）或 JSON。

/// 输出模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    /// 表格 + 颜色（默认）
    Table,
    /// JSON 序列化
    Json,
}

/// 全局输出设置
#[allow(dead_code)]
pub struct OutputConfig {
    pub mode: OutputMode,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            mode: OutputMode::Table,
        }
    }
}

/// 渲染 JSON 输出
pub fn print_json<T: serde::Serialize>(data: &T) {
    match serde_json::to_string_pretty(data) {
        Ok(s) => println!("{}", s),
        Err(e) => eprintln!("JSON serialization error: {}", e),
    }
}
