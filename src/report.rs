//! Report generation for Archeo.
//!
//! This module defines the [`Report`] type, which represents the final
//! human-readable output of a scan and analysis run.
//!
//! A report includes:
//! - scan metadata (target path, model, configuration),
//! - list of included files,
//! - high-level AI summary,
//! - compressed content summary,
//! - per-file detailed AI analysis.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use crate::scanner::scanner_config::ScanConfig;
use crate::content_analysis::ContentAnalysisReport;

/// Final report structure produced by Archeo.
///
/// This struct holds all information required to render a full report,
/// including both global summaries and per-file analysis results.
#[derive(Debug, Clone)]
pub struct Report {
    /// Root directory that was analyzed.
    root: PathBuf,
    /// List of files included in the analysis.
    files: Vec<PathBuf>,
    /// Scan configuration used to collect files.
    config: ScanConfig,
    /// Model identifier used for AI generation.
    model: String,
    /// High-level AI-generated project summary.
    ai_summary: String,
    /// Compressed summary of all file-level analyses.
    content_summary: String,
    /// Detailed per-file AI analysis results.
    ai_single_files: Vec<ContentAnalysisReport>,
}

impl Report {
    /// Create a new [`Report`] instance from all collected analysis data.
    pub fn new<P: AsRef<Path>>(
        root: P,
        files: &[PathBuf],
        config: &ScanConfig,
        model: &str,
        ai_summary: &str,
        content_summary: &str,
        ai_single_files: &[ContentAnalysisReport],
    ) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
            files: files.to_vec(),
            config: config.clone(),
            model: model.to_string(),
            ai_summary: ai_summary.to_string(),
            content_summary: content_summary.to_string(),
            ai_single_files: ai_single_files.to_vec(),
        }
    }

    /// Write the rendered report to a file.
    ///
    /// Ensures that the parent directory exists before writing.
    pub fn write<P: AsRef<Path>>(&self, path: P) -> anyhow::Result<()> {
        let path = path.as_ref();

        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }

        fs::write(path, self.to_string())?;
        Ok(())
    }

    /// Convert a path to a relative path if it is under the report root.
    ///
    /// Falls back to the full path if it cannot be relativized.
    fn relative_or_full(&self, path: &Path) -> String {
        match path.strip_prefix(&self.root) {
            Ok(rel) => rel.display().to_string(),
            Err(_) => path.display().to_string(),
        }
    }
}

impl fmt::Display for Report {
    /// Render the report as Markdown.
    ///
    /// The output includes:
    /// - metadata sections,
    /// - file list,
    /// - global AI summaries,
    /// - per-file detailed analysis.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "# Archeo Report")?;
        writeln!(f)?;

        writeln!(f, "## Target")?;
        writeln!(f, "{}", self.root.display())?;
        writeln!(f)?;

        writeln!(f, "## Model")?;
        writeln!(f, "{}", self.model)?;
        writeln!(f)?;

        writeln!(f, "## Scan Configuration")?;
        writeln!(f, "```")?;
        writeln!(f, "{}", self.config.describe())?;
        writeln!(f, "```")?;
        writeln!(f)?;

        writeln!(f, "## Included Files")?;
        for file in &self.files {
            writeln!(f, "- {}", self.relative_or_full(file))?;
        }
        writeln!(f)?;

        writeln!(f, "## AI Analysis")?;
        writeln!(f)?;
        writeln!(f, "{}", self.ai_summary.trim())?;
        writeln!(f)?;

        writeln!(f, "## Content Analysis Summary")?;
        writeln!(f, "{}\n\n", self.content_summary)?;

        writeln!(f, "## Content Analysis Detailed Per File")?;

        for report in &self.ai_single_files {
            writeln!(f, "{}", format!("### {}\n\n", report.path.display()))?;

            if !report.warnings.is_empty() {
                writeln!(f, "- Warnings:\n")?;
                for w in &report.warnings {
                    writeln!(f, "{}", format!("  - {}\n", w))?;
                }
            }

            if let Some(response) = &report.ai_response {
                writeln!(f, "{}", response)?;
                writeln!(f, "{}", "\n")?;
            } else {
                writeln!(f, "{}", "\n_No AI interpretation._\n\n")?;
            }
        }

        Ok(())
    }
}
