//! 表格渲染模块，支持中英文混排对齐。

use unicode_width::UnicodeWidthChar;

/// 计算字符串的显示宽度（使用 unicode-width 精确计算）
pub fn display_width(s: &str) -> usize {
    let mut width = 0;
    for ch in s.chars() {
        width += ch.width().unwrap_or(0);
    }
    width
}

/// 生成一行表格（数据行，统一用显示宽度对齐）
fn format_row(cells: &[String], widths: &[usize]) -> String {
    let parts: Vec<String> = cells
        .iter()
        .zip(widths.iter())
        .map(|(cell, w)| {
            let visible = display_width(cell);
            let padding = if visible < *w { *w - visible } else { 0 };
            format!(" {}{} ", cell, " ".repeat(padding))
        })
        .collect();
    format!("|{}|", parts.join("|"))
}

/// 生成一行表格（表头，&str）
fn format_header_row(headers: &[&str], widths: &[usize]) -> String {
    let parts: Vec<String> = headers
        .iter()
        .zip(widths.iter())
        .map(|(h, w)| {
            let visible = display_width(h);
            let padding = if visible < *w { *w - visible } else { 0 };
            format!(" {}{} ", h, " ".repeat(padding))
        })
        .collect();
    format!("|{}|", parts.join("|"))
}

/// 打印表格（自动计算列宽，支持中英文混排）
pub fn print_table(headers: &[&str], rows: &[Vec<String>]) {
    if rows.is_empty() {
        return;
    }

    // 计算每列显示宽度
    let mut widths: Vec<usize> = headers.iter().map(|h| display_width(h)).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() {
                widths[i] = widths[i].max(display_width(cell));
            }
        }
    }

    // 分隔线
    let separator: String = widths
        .iter()
        .map(|w| "-".repeat(w + 2))
        .collect::<Vec<_>>()
        .join("+");
    println!("+{}+", separator);

    // 表头
    println!("{}", format_header_row(headers, &widths));
    println!("+{}+", separator);

    // 数据行
    for row in rows {
        println!("{}", format_row(row, &widths));
    }
    println!("+{}+", separator);
}
