// descriptor.rs
use anyhow::{Context, Result};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

use crate::content_analysis::{ContentConfig, ParseMode};

#[derive(Debug, Clone)]
pub enum ContentKind {
    Text,
    Table,
    Notebook,
    Code,
    Config,
    Data,
    Unknown,
}

impl ContentKind{
    pub fn as_str(&self) -> &'static str {
        match self {
            ContentKind::Text => "text",
            ContentKind::Table => "table",
            ContentKind::Notebook => "notebook",
            ContentKind::Code => "code",
            ContentKind::Config => "config", 
            ContentKind::Data => "data",
            ContentKind::Unknown => "UNKNOWN",
        }
    }

}



#[derive(Debug, Clone)]
pub struct ContentDescriptor {
    pub path: PathBuf,
    pub extension: String,
    pub kind: ContentKind,
    pub parse_mode: ParseMode,
    pub file_size: usize,
    pub is_truncated: bool,
    pub is_sample: bool,
    pub total_rows: Option<usize>,
    pub total_cols: Option<usize>,
    pub sampled_rows: Option<usize>,
    pub sampled_cols: Option<usize>,
    pub content: String,
}

impl ContentDescriptor {
    pub fn from_path(path: &Path, config: &ContentConfig, parse_mode: ParseMode) -> Result<Self> {
        let ext = config.extension_of(path);

        match ext.as_str() {
            "ipynb" => Self::from_notebook(path, config, parse_mode),
            "csv" => Self::from_delimited(path, config, parse_mode, ','),
            "tsv" => Self::from_delimited(path, config, parse_mode, '\t'),
            _ => Self::from_text(path, config, parse_mode),
        }
    }

    fn from_text(path: &Path, config: &ContentConfig, parse_mode: ParseMode) -> Result<Self> {
        let file_size = fs::metadata(path)?.len() as usize;
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read text file {}", path.display()))?;

        let (content, is_truncated, is_sample) = match parse_mode {
            ParseMode::Full => {
                if file_size <= config.max_full_bytes {
                    (raw, false, false)
                } else {
                    (
                        Self::truncate_text_preserving_context(&raw, config.max_full_bytes),
                        true,
                        false,
                    )
                }
            }
            ParseMode::Sampled => (Self::sample_text_lines(&raw, 50, 30, 50), true, true),
            ParseMode::Skip => anyhow::bail!("skip mode should be handled before descriptor creation"),
        };

        Ok(Self {
            path: path.to_path_buf(),
            extension: config.extension_of(path),
            kind: ContentKind::Text,
            parse_mode,
            file_size,
            is_truncated,
            is_sample,
            total_rows: None,
            total_cols: None,
            sampled_rows: None,
            sampled_cols: None,
            content,
        })
    }

    fn from_notebook(path: &Path, config: &ContentConfig, parse_mode: ParseMode) -> Result<Self> {
        let file_size = fs::metadata(path)?.len() as usize;
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read notebook {}", path.display()))?;
        let json: Value = serde_json::from_str(&raw)
            .with_context(|| format!("invalid notebook json {}", path.display()))?;

        let mut rendered = String::new();

        if let Some(cells) = json.get("cells").and_then(|v| v.as_array()) {
            for (idx, cell) in cells.iter().enumerate() {
                let cell_type = cell.get("cell_type").and_then(|v| v.as_str()).unwrap_or("unknown");
                let source = Self::render_ipynb_source(cell.get("source"));

                rendered.push_str(&format!("## Cell {}\n", idx + 1));
                rendered.push_str(&format!("Type: {}\n\n", cell_type));
                rendered.push_str(&source);
                rendered.push_str("\n\n");
            }
        }

        let (content, is_truncated, is_sample) = match parse_mode {
            ParseMode::Full => {
                if rendered.len() <= config.max_full_bytes {
                    (rendered, false, false)
                } else {
                    (
                        Self::truncate_text_preserving_context(&rendered, config.max_full_bytes),
                        true,
                        false,
                    )
                }
            }
            ParseMode::Sampled => (Self::sample_text_lines(&rendered, 80, 40, 80), true, true),
            ParseMode::Skip => anyhow::bail!("skip mode should be handled before descriptor creation"),
        };

        Ok(Self {
            path: path.to_path_buf(),
            extension: config.extension_of(path),
            kind: ContentKind::Notebook,
            parse_mode,
            file_size,
            is_truncated,
            is_sample,
            total_rows: None,
            total_cols: None,
            sampled_rows: None,
            sampled_cols: None,
            content,
        })
    }

    fn from_delimited(
        path: &Path,
        config: &ContentConfig,
        parse_mode: ParseMode,
        delimiter: char,
    ) -> Result<Self> {
        let file_size = fs::metadata(path)?.len() as usize;
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read table {}", path.display()))?;
        let lines: Vec<&str> = raw.lines().collect();

        if lines.is_empty() {
            return Ok(Self {
                path: path.to_path_buf(),
                extension: config.extension_of(path),
                kind: ContentKind::Table,
                parse_mode,
                file_size,
                is_truncated: false,
                is_sample: false,
                total_rows: Some(0),
                total_cols: Some(0),
                sampled_rows: Some(0),
                sampled_cols: Some(0),
                content: String::new(),
            });
        }

        let header: Vec<String> = lines[0]
            .split(delimiter)
            .map(|s| s.trim().to_string())
            .collect();

        let total_cols = header.len();
        let total_rows = lines.len().saturating_sub(1);

        let sampled_cols = match parse_mode {
            ParseMode::Full => total_cols,
            ParseMode::Sampled => total_cols.min(config.sample_cols),
            ParseMode::Skip => anyhow::bail!("skip mode should be handled before descriptor creation"),
        };

        let sampled_rows = match parse_mode {
            ParseMode::Full => total_rows,
            ParseMode::Sampled => total_rows.min(config.sample_rows),
            ParseMode::Skip => anyhow::bail!("skip mode should be handled before descriptor creation"),
        };

        let mut content = String::new();
        content.push_str("Headers:\n");
        content.push_str(
            &header
                .iter()
                .take(sampled_cols)
                .cloned()
                .collect::<Vec<_>>()
                .join(", "),
        );
        content.push_str("\n\nPreview:\n");

        for row in lines.iter().skip(1).take(sampled_rows) {
            let cols = row
                .split(delimiter)
                .map(|s| s.trim().to_string())
                .take(sampled_cols)
                .collect::<Vec<_>>();
            content.push_str(&cols.join(", "));
            content.push('\n');
        }

        Ok(Self {
            path: path.to_path_buf(),
            extension: config.extension_of(path),
            kind: ContentKind::Table,
            parse_mode,
            file_size,
            is_truncated: sampled_rows < total_rows || sampled_cols < total_cols,
            is_sample: parse_mode == ParseMode::Sampled,
            total_rows: Some(total_rows),
            total_cols: Some(total_cols),
            sampled_rows: Some(sampled_rows),
            sampled_cols: Some(sampled_cols),
            content,
        })
    }

    pub fn render_for_prompt(&self) -> String {
        let mut out = String::new();

        out.push_str(&format!("File: {}\n", self.path.display()));
        out.push_str(&format!("Extension: {}\n", self.extension));
        out.push_str(&format!("Kind: {:?}\n", self.kind));
        out.push_str(&format!("Parse mode: {}\n", self.parse_mode.as_str()));
        out.push_str(&format!("File size: {}\n", self.file_size));
        out.push_str(&format!("Truncated: {}\n", self.is_truncated));
        out.push_str(&format!("Sampled: {}\n", self.is_sample));

        if let Some(v) = self.total_rows {
            out.push_str(&format!("Total rows: {}\n", v));
        }
        if let Some(v) = self.total_cols {
            out.push_str(&format!("Total cols: {}\n", v));
        }
        if let Some(v) = self.sampled_rows {
            out.push_str(&format!("Sampled rows: {}\n", v));
        }
        if let Some(v) = self.sampled_cols {
            out.push_str(&format!("Sampled cols: {}\n", v));
        }

        out.push_str("\nContent:\n");
        out.push_str(&self.content);

        out
    }

    fn render_ipynb_source(source: Option<&Value>) -> String {
        match source {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Array(arr)) => {
                let mut out = String::new();
                for item in arr {
                    if let Some(s) = item.as_str() {
                        out.push_str(s);
                    }
                }
                out
            }
            _ => String::new(),
        }
    }

    fn truncate_text_preserving_context(text: &str, max_bytes: usize) -> String {
        if text.len() <= max_bytes {
            return text.to_string();
        }

        let half = max_bytes / 2;
        let head = Self::safe_byte_slice(text, 0, half);
        let tail = Self::safe_byte_slice(text, text.len().saturating_sub(half), text.len());

        format!(
            "{}\n\n[... truncated ...]\n\n{}",
            head.trim_end(),
            tail.trim_start()
        )
    }

    fn safe_byte_slice(text: &str, start: usize, end: usize) -> String {
        let mut real_start = start.min(text.len());
        let mut real_end = end.min(text.len());

        while real_start < text.len() && !text.is_char_boundary(real_start) {
            real_start += 1;
        }
        while real_end > 0 && !text.is_char_boundary(real_end) {
            real_end -= 1;
        }

        if real_start >= real_end {
            String::new()
        } else {
            text[real_start..real_end].to_string()
        }
    }

    fn sample_text_lines(text: &str, head: usize, middle: usize, tail: usize) -> String {
        let lines: Vec<&str> = text.lines().collect();

        if lines.len() <= head + middle + tail {
            return text.to_string();
        }

        let head_part = &lines[..head.min(lines.len())];
        let tail_start = lines.len().saturating_sub(tail);
        let tail_part = &lines[tail_start..];

        let mid_start = lines.len() / 2;
        let mid_half = middle / 2;
        let mid_lo = mid_start.saturating_sub(mid_half);
        let mid_hi = (mid_lo + middle).min(lines.len());
        let middle_part = &lines[mid_lo..mid_hi];

        let mut out = String::new();
        out.push_str(&head_part.join("\n"));
        out.push_str("\n\n[... middle sample ...]\n\n");
        out.push_str(&middle_part.join("\n"));
        out.push_str("\n\n[... tail sample ...]\n\n");
        out.push_str(&tail_part.join("\n"));
        out
    }
}