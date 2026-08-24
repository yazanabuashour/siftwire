use std::io::{self, Write as _};

use anyhow::{Context, Result};

/// Renders one aligned text table using visible character widths.
pub(super) fn print_table(header: &[&str], rows: &[Vec<String>]) {
    let widths: Vec<usize> = header
        .iter()
        .enumerate()
        .map(|(column, cell)| {
            let widest = rows
                .iter()
                .filter_map(|row| row.get(column))
                .map(|cell| cell.chars().count())
                .max()
                .unwrap_or(0);
            cell.chars().count().max(widest)
        })
        .collect();
    let mut rendered = String::new();
    append_row(&mut rendered, header.iter().copied(), &widths);
    for row in rows {
        append_row(&mut rendered, row.iter().map(String::as_str), &widths);
    }
    print!("{rendered}");
}

/// Shortens long text to the visible character budget with an ellipsis marker.
pub(super) fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_owned();
    }
    let mut shortened: String = value.chars().take(max_chars.saturating_sub(1)).collect();
    shortened.push('…');
    shortened
}

pub(super) fn write_json<T: serde::Serialize>(value: &T) -> Result<()> {
    let stdout = io::stdout();
    let mut writer = stdout.lock();
    serde_json::to_writer_pretty(&mut writer, value)
        .and_then(|()| writeln!(writer).map_err(serde_json::Error::io))
        .context("encode json output")
}

fn append_row<'a>(rendered: &mut String, cells: impl Iterator<Item = &'a str>, widths: &[usize]) {
    for (column, cell) in cells.enumerate() {
        rendered.push_str(&pad(cell, widths.get(column).copied().unwrap_or(0)));
        rendered.push_str("  ");
    }
    rendered.push('\n');
}

fn pad(value: &str, width: usize) -> String {
    let visible = value.chars().count();
    if visible >= width {
        return value.to_owned();
    }
    let mut padded = String::from(value);
    padded.extend(std::iter::repeat_n(' ', width.saturating_sub(visible)));
    padded
}
