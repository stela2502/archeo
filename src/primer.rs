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
use clap::Args;

/// CLI arguments controlling the primer stage.
#[derive(Args, Debug, Default, Clone)]
pub struct PrimerCliArgs {
    /// Comma-separated languages (override auto-detection)
    #[arg(long)]
    pub languages: Option<String>,

    /// Comma-separated domains (override auto-detection)
    #[arg(long)]
    pub domains: Option<String>,

    /// Disable README suggestions
    #[arg(long)]
    pub no_readme_advice: bool,

    /// Disable technical debt analysis
    #[arg(long)]
    pub no_technical_debt: bool,

    /// Short user task/question for the project-level AI analysis
    #[arg(long)]
    pub primer_task: Option<String>,

    /// Additional instructions appended to the default primer prompt
    #[arg(long)]
    pub primer_extra: Option<String>,
}

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
    pub fn from_sources(cli: &PrimerCliArgs, files: &[PathBuf]) -> Self {
        // 1. inferred defaults
        let mut cfg = Self::infer_from_files(files);

        // 2. CLI overrides
        if let Some(langs) = &cli.languages {
            cfg.languages = langs
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }

        if let Some(domains) = &cli.domains {
            cfg.domains = domains
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }

        if cli.no_readme_advice {
            cfg.include_readme_advice = false;
        }

        if cli.no_technical_debt {
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

    #[test]
    fn from_sources_overrides_languages_and_domains() {
        let files = vec![pb("main.rs")];

        let cli = PrimerCliArgs {
            languages: Some("Go, Rust".to_string()),
            domains: Some("bio, infra".to_string()),
            ..Default::default()
        };

        let cfg = PrimerConfig::from_sources(&cli, &files);

        assert_eq!(cfg.languages, vec!["Go".to_string(), "Rust".to_string()]);
        assert_eq!(cfg.domains, vec!["bio".to_string(), "infra".to_string()]);
    }

    #[test]
    fn from_sources_respects_boolean_flags() {
        let files = vec![pb("main.rs")];

        let cli = PrimerCliArgs {
            no_readme_advice: true,
            no_technical_debt: true,
            ..Default::default()
        };

        let cfg = PrimerConfig::from_sources(&cli, &files);

        assert!(!cfg.include_readme_advice);
        assert!(!cfg.include_technical_debt);
    }

    #[test]
    fn from_sources_does_not_overwrite_languages_with_empty_cli_values() {
        let mut expected = PrimerConfig::default();
        expected.languages = vec!["Rust".to_string()];

        let cli = PrimerCliArgs {
            languages: Some("Rust, , ".to_string()),
            ..Default::default()
        };

        let mut base = PrimerConfig::default();
        base.languages = vec!["Rust".to_string()];

        let inferred = PrimerConfig::infer_from_files(&[PathBuf::from("main.rs")]);
        let cfg = PrimerConfig::from_sources(&cli, &[PathBuf::from("main.rs")]);

        assert_eq!(cfg.languages, inferred.languages);
    }

}
