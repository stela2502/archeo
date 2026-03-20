//! CLI and inferred configuration for the project-level "primer" analysis.
//!
//! This module defines:
//! - [`PrimerCliArgs`]: command-line inputs parsed via `clap`,
//! - [`PrimerConfig`]: a normalized configuration derived from CLI and file-based inference.
//!
//! The typical flow is:
//! 1. infer defaults from the provided file list,
//! 2. apply CLI overrides,
//! 3. pass the resulting config to the prompt builder.

use std::path::PathBuf;


/// Normalized configuration used by the primer stage.
#[derive(Debug, Clone)]
pub struct PrimerConfig {
    /// Detected or user-specified programming languages.
    pub languages: Vec<String>,
    /// Detected or user-specified project domains.
    pub domains: Vec<String>,
    /// Additional hints derived from file names (reserved for future use).
    pub project_hints: Vec<String>,
    /// Whether README advice should be included.
    pub include_readme_advice: bool,
    /// Whether technical debt analysis should be included.
    pub include_technical_debt: bool,
}

impl Default for PrimerConfig {
    fn default() -> Self {
        Self {
            languages: Vec::new(),
            domains: Vec::new(),
            project_hints: Vec::new(),
            include_readme_advice: true,
            include_technical_debt: true,
        }
    }
}

impl PrimerConfig {
    /// Build a [`PrimerConfig`] from CLI arguments and a list of files.
    ///
    /// Order of operations:
    /// 1. infer defaults from files,
    /// 2. override with CLI values if provided.
    pub fn from_sources(
        files: &[PathBuf],
        languages: Option<&str>,
        domains: Option<&str>,
        no_readme_advice: bool,
        no_technical_debt: bool,
    ) -> Self {
        let mut cfg = Self::infer_from_files(files);

        if let Some(langs) = languages {
            let parsed: Vec<String> = langs
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            if !parsed.is_empty() {
                cfg.languages = parsed;
            }
        }

        if let Some(domains) = domains {
            let parsed: Vec<String> = domains
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            if !parsed.is_empty() {
                cfg.domains = parsed;
            }
        }

        if no_readme_advice {
            cfg.include_readme_advice = false;
        }

        if no_technical_debt {
            cfg.include_technical_debt = false;
        }

        cfg
    }

    /// Infer configuration from file extensions and file names.
    ///
    /// - Languages are derived from file extensions.
    /// - Domains are inferred from file name patterns.
    /// - Results are deduplicated and sorted.
    pub fn infer_from_files(files: &[PathBuf]) -> Self {
        let mut cfg = Self::default();

        for f in files {
            if let Some(ext) = f.extension().and_then(|e| e.to_str()) {
                match ext {
                    "rs" => cfg.languages.push("Rust".to_string()),
                    "py" => cfg.languages.push("Python".to_string()),
                    "r" => cfg.languages.push("R".to_string()),
                    "ipynb" => cfg.languages.push("Jupyter".to_string()),
                    "sh" | "bash" | "zsh" => cfg.languages.push("Shell".to_string()),
                    _ => {}
                }
            }

            if let Some(name) = f.file_name().and_then(|n| n.to_str()) {
                let lower = name.to_lowercase();

                if lower.contains("scrna")
                    || lower.contains("singlecell")
                    || lower.contains("single_cell")
                {
                    cfg.domains.push("single-cell RNA".to_string());
                }
                if lower.contains("snp") || lower.contains("variant") {
                    cfg.domains.push("variant analysis".to_string());
                }
                if lower.contains("vdj") || lower.contains("tcr") || lower.contains("bcr") {
                    cfg.domains.push("immune repertoire".to_string());
                }
                if lower.contains("pipeline") || lower.contains("nextflow") {
                    cfg.domains.push("data pipeline".to_string());
                }
            }
        }

        cfg.languages.sort();
        cfg.languages.dedup();
        cfg.domains.sort();
        cfg.domains.dedup();

        cfg
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pb(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn infer_from_files_detects_languages_and_domains() {
        let files = vec![
            pb("main.rs"),
            pb("analysis.py"),
            pb("script.sh"),
            pb("vdj_pipeline.nf"),
            pb("scrna_counts.tsv"),
        ];

        let cfg = PrimerConfig::infer_from_files(&files);

        assert!(cfg.languages.contains(&"Rust".to_string()));
        assert!(cfg.languages.contains(&"Python".to_string()));
        assert!(cfg.languages.contains(&"Shell".to_string()));

        assert!(cfg.domains.contains(&"immune repertoire".to_string()));
        assert!(cfg.domains.contains(&"data pipeline".to_string()));
        assert!(cfg.domains.contains(&"single-cell RNA".to_string()));
    }

    #[test]
    fn infer_from_files_deduplicates_and_sorts() {
        let files = vec![pb("a.rs"), pb("b.rs"), pb("c.py"), pb("d.py")];

        let cfg = PrimerConfig::infer_from_files(&files);

        assert_eq!(cfg.languages, vec!["Python".to_string(), "Rust".to_string()]);
    }


}
