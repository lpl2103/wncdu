//! Utility functions for formatting, strings, and graph rendering.

use std::time::{Duration, UNIX_EPOCH};

/// Style for the disk usage graph bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GraphStyle {
    /// Classic ncdu style using hash symbols: `[##########]`
    #[default]
    Hash,
    /// Unicode fractional blocks: `████▌     `
    Blocks,
}

/// Formats a byte size into human-readable string.
///
/// If `si` is false (default for ncdu), units are powers of 1024 (KiB, MiB, GiB, TiB, PiB).
/// If `si` is true, units are powers of 1000 (KB, MB, GB, TB, PB).
#[must_use]
pub fn format_size(bytes: u64, si: bool) -> String {
    let base = if si { 1000.0 } else { 1024.0 };
    let units = if si {
        &["B", "KB", "MB", "GB", "TB", "PB"][..]
    } else {
        &["B", "KiB", "MiB", "GiB", "TiB", "PiB"][..]
    };

    if bytes == 0 {
        return "0 B".to_string();
    }

    let bytes_f = bytes as f64;
    let mut unit_idx = 0;
    let mut val = bytes_f;

    while val >= base && unit_idx + 1 < units.len() {
        val /= base;
        unit_idx += 1;
    }

    if unit_idx == 0 {
        format!("{bytes} B")
    } else {
        format!("{val:.1} {}", units[unit_idx])
    }
}

/// Formats an exact item count with thousands separators (e.g. `1,234,567`).
#[must_use]
pub fn format_count(count: u64) -> String {
    let s = count.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    let rem = s.len() % 3;

    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (i == rem || (i > rem && (i - rem).is_multiple_of(3))) {
            result.push(',');
        }
        result.push(ch);
    }
    result
}

/// Formats a relative usage percentage (e.g. `45.2%`).
#[must_use]
pub fn format_percent(part: u64, total: u64) -> String {
    if total == 0 {
        return "  0.0%".to_string();
    }
    let pct = (part as f64 / total as f64) * 100.0;
    format!("{pct:5.1}%")
}

/// Generates a visual progress/usage bar for a given fraction [0.0, 1.0].
///
/// `width` is the total number of characters for the bar (excluding brackets if any).
#[must_use]
pub fn format_graph(fraction: f64, width: usize, style: GraphStyle) -> String {
    if width == 0 {
        return String::new();
    }
    let clamped = fraction.clamp(0.0, 1.0);

    match style {
        GraphStyle::Hash => {
            let filled = (clamped * width as f64).round() as usize;
            let empty = width.saturating_sub(filled);
            let mut s = String::with_capacity(width + 2);
            s.push('[');
            for _ in 0..filled {
                s.push('#');
            }
            for _ in 0..empty {
                s.push(' ');
            }
            s.push(']');
            s
        }
        GraphStyle::Blocks => {
            // 8 fractional block characters: ▏▎▍▌▋▊▉█
            const BLOCKS: [char; 8] = ['▏', '▎', '▍', '▌', '▋', '▊', '▉', '█'];
            let total_eighths = (clamped * (width * 8) as f64).round() as usize;
            let full_blocks = total_eighths / 8;
            let remainder = total_eighths % 8;

            let mut s = String::with_capacity(width * 3);
            for _ in 0..full_blocks.min(width) {
                s.push('█');
            }
            if full_blocks < width && remainder > 0 {
                s.push(BLOCKS[remainder - 1]);
            }
            let current_len = s.chars().count();
            for _ in current_len..width {
                s.push(' ');
            }
            s
        }
    }
}

/// Formats a UNIX timestamp into a standard human readable date string `YYYY-MM-DD HH:MM`.
#[must_use]
pub fn format_mtime(timestamp: i64) -> String {
    if timestamp <= 0 {
        return "                  ".to_string();
    }
    let duration = Duration::from_secs(timestamp as u64);
    let system_time = UNIX_EPOCH + duration;
    let dt: chrono::DateTime<chrono::Local> = system_time.into();
    dt.format("%Y-%m-%d %H:%M").to_string()
}

/// Truncates a file system path so that it fits within `max_width` characters.
/// If truncated, prepends `...` or keeps the path meaningful.
#[must_use]
pub fn truncate_path(path: &str, max_width: usize) -> String {
    let char_count = path.chars().count();
    if char_count <= max_width {
        return path.to_string();
    }
    if max_width <= 3 {
        return "...".chars().take(max_width).collect();
    }

    let skip = char_count - (max_width - 3);
    let tail: String = path.chars().skip(skip).collect();
    format!("...{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_size_binary() {
        assert_eq!(format_size(0, false), "0 B");
        assert_eq!(format_size(500, false), "500 B");
        assert_eq!(format_size(1024, false), "1.0 KiB");
        assert_eq!(format_size(1536, false), "1.5 KiB");
        assert_eq!(format_size(1024 * 1024, false), "1.0 MiB");
        assert_eq!(format_size(1024 * 1024 * 1024 * 5, false), "5.0 GiB");
    }

    #[test]
    fn test_format_size_si() {
        assert_eq!(format_size(0, true), "0 B");
        assert_eq!(format_size(1000, true), "1.0 KB");
        assert_eq!(format_size(1_000_000, true), "1.0 MB");
        assert_eq!(format_size(1_000_000_000, true), "1.0 GB");
    }

    #[test]
    fn test_format_count() {
        assert_eq!(format_count(0), "0");
        assert_eq!(format_count(999), "999");
        assert_eq!(format_count(1000), "1,000");
        assert_eq!(format_count(1234567), "1,234,567");
    }

    #[test]
    fn test_format_percent() {
        assert_eq!(format_percent(50, 100), " 50.0%");
        assert_eq!(format_percent(0, 100), "  0.0%");
        assert_eq!(format_percent(100, 100), "100.0%");
        assert_eq!(format_percent(1, 1000), "  0.1%");
    }

    #[test]
    fn test_format_graph_hash() {
        assert_eq!(format_graph(0.0, 10, GraphStyle::Hash), "[          ]");
        assert_eq!(format_graph(0.5, 10, GraphStyle::Hash), "[#####     ]");
        assert_eq!(format_graph(1.0, 10, GraphStyle::Hash), "[##########]");
    }

    #[test]
    fn test_format_graph_blocks() {
        assert_eq!(format_graph(0.0, 5, GraphStyle::Blocks), "     ");
        assert_eq!(format_graph(1.0, 5, GraphStyle::Blocks), "█████");
    }

    #[test]
    fn test_truncate_path() {
        let path = "C:\\Users\\Leandro\\Projects\\Rust\\wncdu\\src\\main.rs";
        assert_eq!(truncate_path(path, 100), path);
        let trunc = truncate_path(path, 20);
        assert_eq!(trunc.chars().count(), 20);
        assert!(trunc.starts_with("..."));
    }
}
