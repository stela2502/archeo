//! Content analysis orchestration.
//!
//! This module contains the high-level analyzer that turns files into
//! [`ContentAnalysisReport`] values by:
//! - checking whether a path should be analyzed,
//! - building a [`ContentDescriptor`],
//! - rendering an AI prompt,
//! - and calling Ollama for interpretation.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::content_analysis::{ContentConfig, ContentDescriptor, ParseMode};
use crate::ollama::Ollama;
use crate::prompt_defaults::PromptDefaults;

/// Result of analyzing a single file.
///
/// A report contains both metadata about how the file was handled and the
/// optional AI-generated interpretation.
#[derive(Debug, Clone)]
pub struct ContentAnalysisReport {
    /// Path of the analyzed file.
    pub path: PathBuf,

    /// File extension determined from the path or descriptor.
    pub extension: String,

    /// Effective parse mode used for this file.
    ///
    /// Typical values are `"skip"`, `"filtered"`, `"error"`, or the string form
    /// of a [`ParseMode`].
    pub parse_mode: String,

    /// Combined file-specific prompt block used when constructing the AI prompt.
    pub primer_used: Option<String>,

    /// Structured descriptor extracted from the file, if analysis reached that stage.
    pub descriptor: Option<ContentDescriptor>,

    /// AI interpretation returned by Ollama, if available.
    pub ai_response: Option<String>,

    /// Non-fatal warnings collected while analyzing the file.
    pub warnings: Vec<String>,
}

/// High-level content analyzer.
///
/// The analyzer delegates the actual content extraction rules to
/// [`ContentConfig`] and [`ContentDescriptor`], and uses [`Ollama`] to
/// generate a final interpretation.
#[derive(Debug, Clone)]
pub struct ContentAnalyzer {
    /// Configuration controlling which files are analyzed and how prompts are built.
    pub config: ContentConfig,
}

impl ContentAnalyzer {
    /// Creates a new analyzer from a [`ContentConfig`].
    pub fn new(config: ContentConfig) -> Self {
        Self { config }
    }

    /// Analyzes a list of files and returns one report per input path.
    ///
    /// This method is best-effort: if analysis of one file fails, an `"error"`
    /// report is produced for that file and processing continues for the rest.
    pub fn analyze_files(
        &self,
        files: &[PathBuf],
        ollama: &Ollama,
        model: &str,
        prompts: &PromptDefaults,
    ) -> Result<Vec<ContentAnalysisReport>> {
        let mut reports = Vec::with_capacity(files.len());

        for path in files {
            match self.analyze_file(path, ollama, model, prompts) {
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

    /// Analyzes a single file and returns a structured report.
    ///
    /// The flow is:
    /// 1. Verify the path is a file.
    /// 2. Check whether the content configuration allows the path.
    /// 3. Resolve the matching rule and parse mode.
    /// 4. Build a [`ContentDescriptor`].
    /// 5. Build the combined file primer block.
    /// 6. Render the final AI prompt and call Ollama.
    pub fn analyze_file(
        &self,
        path: &Path,
        ollama: &Ollama,
        model: &str,
        prompts: &PromptDefaults,
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

        let parse_mode = self.config.rule_for_path(path);
        if parse_mode == ParseMode::Skip {
            return Ok(ContentAnalysisReport {
                path: path.to_path_buf(),
                extension: self.config.extension_of(path),
                parse_mode: "skip".to_string(),
                primer_used: None,
                descriptor: None,
                ai_response: None,
                warnings: vec!["skipped by rule".to_string()],
            });
        }

        let descriptor = ContentDescriptor::from_path(path, &self.config, parse_mode)
            .with_context(|| format!("failed to build descriptor for {}", path.display()))?;

        prompts
            .validate_internal_coverage()
            .context("prompt defaults failed internal coverage validation")?;

        let primer = self.combined_file_primer(&descriptor, prompts);
        println!(
            "\n=== FILE PRIMER [{}] ===\n{}\n=======================\n",
            descriptor.path.display(),
            primer
        );    
        let prompt = self.build_prompt(&descriptor, prompts);

        let ai_response = ollama
            .generate(model, &prompt)
            .with_context(|| format!("ollama failed for {}", path.display()))?;

        Ok(ContentAnalysisReport {
            path: path.to_path_buf(),
            extension: descriptor.extension.clone(),
            parse_mode: parse_mode.as_str().to_string(),
            primer_used: Some(primer),
            descriptor: Some(descriptor),
            ai_response: Some(ai_response),
            warnings: Vec::new(),
        })
    }

    /// Return the combined file primer block for one descriptor.
    ///
    /// This combines:
    /// - the global file-analysis task,
    /// - the file-type-specific prompt resolved from [`PromptDefaults`],
    /// - and optional global file-analysis extra instructions.
    ///
    /// This is the exact logical prompt block used to steer per-file analysis.
    pub fn combined_file_primer(
        &self,
        descriptor: &ContentDescriptor,
        prompts: &PromptDefaults,
    ) -> String {
        let mut out = String::new();

        out.push_str(prompts.file_analysis_task(None).trim());
        out.push_str("\n\nFile-type instructions:\n");
        out.push_str(prompts.content_prompt_for(descriptor).trim());

        if let Some(extra) = prompts.catalog.file_analysis_extra.as_deref() {
            let extra = extra.trim();
            if !extra.is_empty() {
                out.push_str("\n\nAdditional instructions:\n");
                out.push_str(extra);
            }
        }

        out
    }

    /// Builds the final prompt sent to Ollama for a single descriptor.
    ///
    /// The prompt is fully driven by [`PromptDefaults`].
    fn build_prompt(
        &self,
        descriptor: &ContentDescriptor,
        prompts: &PromptDefaults,
    ) -> String {
        prompts.render_descriptor_prompt(descriptor, None, None, None)
    }

    /// Renders a human-readable multi-file summary.
    ///
    /// This format is intended both for diagnostics and as input to a final
    /// summarization step.
    pub fn render_detailed_summary(reports: &[ContentAnalysisReport]) -> String {
        let mut out = String::new();

        for report in reports {
            out.push_str(&format!("FILE: {}\n", report.path.display()));
            out.push_str(&format!("EXTENSION: {}\n", report.extension));
            out.push_str(&format!("PARSE_MODE: {}\n", report.parse_mode));

            if let Some(primer) = &report.primer_used {
                out.push_str("PRIMER_USED:\n");
                out.push_str(primer.trim());
                out.push('\n');
            }

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

    /// Produces a final compressed summary over all reports using Ollama.
    ///
    /// The provided `prompt` is prepended to the rendered detailed report
    /// summary and the combined prompt is sent to Ollama.
    pub fn compress_reports_with_ai(
        reports: &[ContentAnalysisReport],
        ollama: &crate::ollama::Ollama,
        model: &str,
        prompt: &str,
    ) -> anyhow::Result<String> {
        let detailed = Self::render_detailed_summary(reports);
        let final_prompt = format!("{}\n\nFile analyses:\n\n{}", prompt, detailed);
        ollama.generate(model, &final_prompt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_report() -> ContentAnalysisReport {
        ContentAnalysisReport {
            path: PathBuf::from("src/example.rs"),
            extension: "rs".to_string(),
            parse_mode: "full".to_string(),
            primer_used: None,
            descriptor: None,
            ai_response: None,
            warnings: Vec::new(),
        }
    }

    #[test]
    fn render_detailed_summary_includes_core_report_fields() {
        let report = base_report();

        let text = ContentAnalyzer::render_detailed_summary(&[report]);

        assert!(text.contains("FILE: src/example.rs"));
        assert!(text.contains("EXTENSION: rs"));
        assert!(text.contains("PARSE_MODE: full"));
        assert!(text.contains("\n---\n"));
    }

    #[test]
    fn render_detailed_summary_includes_primer_and_interpretation() {
        let mut report = base_report();
        report.primer_used = Some("  custom primer  ".to_string());
        report.ai_response = Some("  looks like Rust source  ".to_string());

        let text = ContentAnalyzer::render_detailed_summary(&[report]);

        assert!(text.contains("PRIMER_USED:\ncustom primer\n"));
        assert!(text.contains("INTERPRETATION:\nlooks like Rust source\n"));
    }

    #[test]
    fn render_detailed_summary_includes_warnings_as_bullets() {
        let mut report = base_report();
        report.warnings = vec![
            "first warning".to_string(),
            "second warning".to_string(),
        ];

        let text = ContentAnalyzer::render_detailed_summary(&[report]);

        assert!(text.contains("WARNINGS:\n- first warning\n- second warning\n"));
    }

    #[test]
    fn render_detailed_summary_handles_multiple_reports() {
        let first = base_report();

        let mut second = base_report();
        second.path = PathBuf::from("README.md");
        second.extension = "md".to_string();
        second.parse_mode = "sample".to_string();

        let text = ContentAnalyzer::render_detailed_summary(&[first, second]);

        assert!(text.contains("FILE: src/example.rs"));
        assert!(text.contains("FILE: README.md"));
        assert!(text.contains("EXTENSION: md"));
        assert!(text.contains("PARSE_MODE: sample"));

        let separator_count = text.matches("\n---\n\n").count();
        assert_eq!(separator_count, 2);
    }
}