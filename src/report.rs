use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use crate::scanner::scanner_config::ScanConfig;
use crate::content_analysis::ContentAnalysisReport;

#[derive(Debug, Clone)]
pub struct Report {
    root: PathBuf,
    files: Vec<PathBuf>,
    config: ScanConfig,
    model: String,
    ai_summary: String,
    content_summary: String,
    ai_single_files: Vec<ContentAnalysisReport>,
}

impl Report {
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

    fn relative_or_full(&self, path: &Path) -> String {
        match path.strip_prefix(&self.root) {
            Ok(rel) => rel.display().to_string(),
            Err(_) => path.display().to_string(),
        }
    }
}

impl fmt::Display for Report {
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
            writeln!(f,"{}",  format!("### {}\n\n", report.path.display()))?;
            // writeln!(f, "{}", format!("- Extension: `{}`\n", report.extension))?;
            // writeln!(f, "{}", format!("- Parse mode: `{}`\n", report.parse_mode))?;

            /*if let Some(descriptor) = &report.descriptor {
                writeln!(f, "{}", &format!("- Truncated: `{}`\n", descriptor.is_truncated))?;
                writeln!(f, "{}", &format!("- Sampled: `{}`\n", descriptor.is_sample))?;
            }*/

            if !report.warnings.is_empty() {
                writeln!(f, "- Warnings:\n")?;
                for w in &report.warnings {
                    writeln!(f, "{}", &format!("  - {}\n", w))?;
                }
            }

            if let Some(response) = &report.ai_response {
                //writeln!(f, "{}", "\n#### AI Interpretation\n\n");
                writeln!(f, "{}", response)?;
                writeln!(f, "{}", "\n")?;
            } else {
                writeln!(f, "{}", "\n_No AI interpretation._\n\n")?;
            }
        }

        Ok(())
    }
}