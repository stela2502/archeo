use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::content_analysis::{ContentConfig, ContentDescriptor, ParseMode};
use crate::ollama::Ollama;

#[derive(Debug, Clone)]
pub struct ContentAnalysisReport {
    pub path: PathBuf,
    pub extension: String,
    pub parse_mode: String,
    pub primer_used: Option<String>,
    pub descriptor: Option<ContentDescriptor>,
    pub ai_response: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ContentAnalyzer {
    pub config: ContentConfig,
}

impl ContentAnalyzer {
    pub fn new(config: ContentConfig) -> Self {
        Self { config }
    }

    pub fn analyze_files(
        &self,
        files: &[PathBuf],
        ollama: &Ollama,
        model: &str,
    ) -> Result<Vec<ContentAnalysisReport>> {
        let mut reports = Vec::with_capacity(files.len());

        for path in files {
            match self.analyze_file(path, ollama, model) {
                Ok(report) => reports.push(report),
                Err(err) => reports.push(ContentAnalysisReport {
                    path: path.clone(),
                    extension: self.config.extension_of(path),
                    parse_mode: "error".to_string(),
                    primer_used: None,
                    descriptor: None,
                    ai_response: None,
                    warnings: vec![format!("analysis failed: {err:#}")],
                }),
            }
        }

        Ok(reports)
    }

    pub fn analyze_file(
        &self,
        path: &Path,
        ollama: &Ollama,
        model: &str,
    ) -> Result<ContentAnalysisReport> {
        if !path.is_file() {
            anyhow::bail!("not a file: {}", path.display());
        }

        if !self.config.allows_path(path) {
            return Ok(ContentAnalysisReport {
                path: path.to_path_buf(),
                extension: self.config.extension_of(path),
                parse_mode: "filtered".to_string(),
                primer_used: None,
                descriptor: None,
                ai_response: None,
                warnings: vec!["extension filtered by content config".to_string()],
            });
        }

        let rule = self.config.rule_for_path(path);
        let primer = self.config.primer_for_path(path);

        if rule.parse_mode == ParseMode::Skip {
            return Ok(ContentAnalysisReport {
                path: path.to_path_buf(),
                extension: self.config.extension_of(path),
                parse_mode: "skip".to_string(),
                primer_used: Some(primer),
                descriptor: None,
                ai_response: None,
                warnings: vec!["skipped by rule".to_string()],
            });
        }

        let descriptor = ContentDescriptor::from_path(path, &self.config, rule.parse_mode)
            .with_context(|| format!("failed to build descriptor for {}", path.display()))?;

        let prompt = self.build_prompt(&primer, &descriptor);
        let ai_response = ollama
            .generate(model, &prompt)
            .with_context(|| format!("ollama failed for {}", path.display()))?;

        Ok(ContentAnalysisReport {
            path: path.to_path_buf(),
            extension: self.config.extension_of(path),
            parse_mode: rule.parse_mode.as_str().to_string(),
            primer_used: Some(primer),
            descriptor: Some(descriptor),
            ai_response: Some(ai_response),
            warnings: Vec::new(),
        })
    }

    fn build_prompt(&self, primer: &str, descriptor: &ContentDescriptor) -> String {
        let mut out = String::new();
        out.push_str(primer);
        out.push_str("\n\n");
        out.push_str(&descriptor.render_for_prompt());
        out
    }

    pub fn render_detailed_summary(reports: &[ContentAnalysisReport]) -> String {
        let mut out = String::new();

        for report in reports {
            out.push_str(&format!("FILE: {}\n", report.path.display()));
            out.push_str(&format!("EXTENSION: {}\n", report.extension));
            out.push_str(&format!("PARSE_MODE: {}\n", report.parse_mode));

            if let Some(descriptor) = &report.descriptor {
                out.push_str(&format!("KIND: {:?}\n", descriptor.kind));
                out.push_str(&format!("TRUNCATED: {}\n", descriptor.is_truncated));
                out.push_str(&format!("SAMPLED: {}\n", descriptor.is_sample));

                if let Some(rows) = descriptor.total_rows {
                    out.push_str(&format!("TOTAL_ROWS: {}\n", rows));
                }
                if let Some(cols) = descriptor.total_cols {
                    out.push_str(&format!("TOTAL_COLS: {}\n", cols));
                }
                if let Some(rows) = descriptor.sampled_rows {
                    out.push_str(&format!("SAMPLED_ROWS: {}\n", rows));
                }
                if let Some(cols) = descriptor.sampled_cols {
                    out.push_str(&format!("SAMPLED_COLS: {}\n", cols));
                }
            }

            if let Some(ai_response) = &report.ai_response {
                out.push_str("INTERPRETATION:\n");
                out.push_str(ai_response.trim());
                out.push('\n');
            }

            if !report.warnings.is_empty() {
                out.push_str("WARNINGS:\n");
                for warning in &report.warnings {
                    out.push_str(&format!("- {}\n", warning));
                }
            }

            out.push_str("\n---\n\n");
        }

        out
    }

    pub fn compress_reports_with_ai(
        reports: &[ContentAnalysisReport],
        ollama: &crate::ollama::Ollama,
        model: &str,
    ) -> anyhow::Result<String> {
        let detailed = Self::render_detailed_summary(reports);

        let prompt = format!(
                "You are converting file-level analysis into a compact index.\n\n\
                 Your task:\n\
                 - produce exactly ONE line per file\n\
                 - DO NOT summarize across files\n\
                 - DO NOT merge files\n\
                 - DO NOT group files\n\
                 - DO NOT write paragraphs\n\
                 - DO NOT explain anything\n\
                 - DO NOT add headers or sections\n\n\
                 Each line must follow exactly:\n\
                 <full file path> -> <short description>\n\n\
                 Rules:\n\
                 - max 15 words per description\n\
                 - describe the file's role in the project\n\
                 - avoid speculation\n\
                 - if unclear, write 'unclear purpose'\n\n\
                 Example:\n\
                 src/main.rs -> CLI entry point coordinating scan and report generation\n\
                 src/ollama.rs -> wrapper for Ollama API calls\n\n\
                 Return ONLY the lines.\n\n\
                 File analyses:\n\n{}",
                detailed
        );

        ollama.generate(model, &prompt)
    }
}