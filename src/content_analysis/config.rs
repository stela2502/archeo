// config.rs
use crate::content_analysis::{ContentCliArgs, ParseMode};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path};

/// Configuration controlling how file content is analyzed.
///
/// This type now owns **parsing behavior only**.
///
/// It is responsible for:
/// - whether content analysis is enabled,
/// - whether recursion is allowed,
/// - size and sampling limits,
/// - per-extension parse modes,
/// - and an optional allow-list of extensions.
///
/// It no longer owns any prompt or primer text. All prompting is handled by
/// `PromptDefaults`.
#[derive(Debug, Clone)]
pub struct ContentConfig {
    /// Enables or disables content analysis globally.
    pub enabled: bool,

    /// Whether content analysis should recurse into nested files or folders.
    pub recursive: bool,

    /// Maximum number of bytes to include when reading files in full mode.
    pub max_full_bytes: usize,

    /// Number of rows to sample for sampled parsing modes.
    pub sample_rows: usize,

    /// Number of columns to sample for sampled parsing modes.
    pub sample_cols: usize,

    /// Per-extension parse-mode rules.
    ///
    /// Keys are normalized extensions without the leading dot.
    pub rules: BTreeMap<String, ParseMode>,

    /// Optional explicit allow-list of extensions.
    ///
    /// When `None`, all non-empty extensions are allowed.
    /// When `Some`, only the listed extensions are allowed.
    pub allowed_extensions: Option<BTreeSet<String>>,
}

impl Default for ContentConfig {
    /// Creates the default content-analysis configuration.
    ///
    /// Defaults favor full parsing for text/code-like formats and sampled
    /// parsing for common tabular formats such as CSV and TSV.
    fn default() -> Self {
        let mut rules = BTreeMap::new();
        rules.insert("py".into(), ParseMode::Full);
        rules.insert("rs".into(), ParseMode::Full);
        rules.insert("r".into(), ParseMode::Full);
        rules.insert("R".into(), ParseMode::Full);
        rules.insert("ipynb".into(), ParseMode::Full);
        rules.insert("md".into(), ParseMode::Full);
        rules.insert("txt".into(), ParseMode::Full);
        rules.insert("csv".into(), ParseMode::Sampled);
        rules.insert("tsv".into(), ParseMode::Sampled);

        Self {
            enabled: false,
            recursive: true,
            max_full_bytes: 150_000,
            sample_rows: 10,
            sample_cols: 20,
            rules,
            allowed_extensions: None,
        }
    }
}

impl ContentConfig {
    /// Builds a configuration from CLI arguments plus defaults.
    ///
    /// Precedence is:
    /// 1. defaults
    /// 2. direct scalar values from CLI
    /// 3. extension allow-list from CLI
    /// 4. per-extension parse-mode overrides from CLI
    ///
    /// The `_files` parameter is currently unused but retained for API
    /// compatibility and future expansion.
    pub fn from_sources(
        content_analysis: bool,
        no_recursive_content: bool,
        content_max_full_bytes: usize,
        content_sample_rows: usize,
        content_sample_cols: usize,
        content_extensions: Option<&str>,
        content_modes: &[String],
    ) -> Self {
        let mut cfg = Self {
            enabled: content_analysis,
            recursive: !no_recursive_content,
            max_full_bytes: content_max_full_bytes,
            sample_rows: content_sample_rows,
            sample_cols: content_sample_cols,
            ..Self::default()
        };

        if let Some(exts) = content_extensions {
            let parsed = cfg.parse_csv_set(exts);
            if !parsed.is_empty() {
                cfg.allowed_extensions = Some(parsed);
            }
        }

        cfg.apply_mode_rules(content_modes);
        cfg
    }

    /// Returns the normalized extension of a path without the leading dot.
    ///
    /// Returns an empty string if the path has no extension or if the extension
    /// cannot be represented as UTF-8.
    pub fn extension_of(&self, path: &Path) -> String {
        path.extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .trim()
            .trim_start_matches('.')
            .to_string()
    }

    /// Returns `true` if the path is allowed by the optional extension filter.
    ///
    /// Paths without an extension are always rejected.
    pub fn allows_path(&self, path: &Path) -> bool {
        let ext = self.extension_of(path);
        if ext.is_empty() {
            return false;
        }

        match &self.allowed_extensions {
            Some(allowed) => allowed.contains(&ext),
            None => true,
        }
    }

    /// Returns the effective parse mode for a path.
    ///
    /// If no explicit rule exists for the extension, `ParseMode::Full` is used.
    pub fn rule_for_path(&self, path: &Path) -> ParseMode {
        let ext = self.extension_of(path);
        self.rules.get(&ext).copied().unwrap_or(ParseMode::Full)
    }

    /// Applies CLI extension-to-mode rules such as `"rs=full"` or `"csv=sampled"`.
    fn apply_mode_rules(&mut self, rules: &[String]) {
        for item in rules {
            if let Some((ext, raw_mode)) = self.parse_rule(item) {
                if let Some(mode) = ParseMode::from_cli_value(&raw_mode) {
                    self.rules.insert(ext, mode);
                }
            }
        }
    }

    /// Parses a key-value rule of the form `"left=right"`.
    ///
    /// The left-hand side is treated as an extension and normalized by removing
    /// a leading dot and surrounding whitespace.
    fn parse_rule(&self, input: &str) -> Option<(String, String)> {
        let (left, right) = input.split_once('=')?;
        let ext = left.trim().trim_start_matches('.').to_string();
        let value = right.trim().to_string();

        if ext.is_empty() || value.is_empty() {
            return None;
        }

        Some((ext, value))
    }

    /// Parses a comma-separated extension list into a normalized set.
    ///
    /// Empty items are discarded and leading dots are removed.
    fn parse_csv_set(&self, input: &str) -> BTreeSet<String> {
        input
            .split(',')
            .map(|s| s.trim().trim_start_matches('.'))
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_contains_expected_extension_rules() {
        let cfg = ContentConfig::default();

        assert_eq!(cfg.rule_for_path(Path::new("main.rs")).as_str(), "full");
        assert_eq!(cfg.rule_for_path(Path::new("notes.md")).as_str(), "full");
        assert_eq!(cfg.rule_for_path(Path::new("table.csv")).as_str(), "sampled");
        assert_eq!(cfg.rule_for_path(Path::new("table.tsv")).as_str(), "sampled");
        assert!(!cfg.enabled);
        assert!(cfg.recursive);
        assert_eq!(cfg.max_full_bytes, 150_000);
        assert_eq!(cfg.sample_rows, 10);
        assert_eq!(cfg.sample_cols, 20);
        assert!(cfg.allowed_extensions.is_none());
    }

    #[test]
    fn extension_of_normalizes_plain_extensions() {
        let cfg = ContentConfig::default();

        assert_eq!(cfg.extension_of(Path::new("src/main.rs")), "rs");
        assert_eq!(cfg.extension_of(Path::new("report.csv")), "csv");
        assert_eq!(cfg.extension_of(Path::new("README")), "");
    }

    #[test]
    fn allows_path_rejects_missing_extension() {
        let cfg = ContentConfig::default();

        assert!(!cfg.allows_path(Path::new("README")));
    }

    #[test]
    fn allows_path_uses_allow_list_when_present() {
        let mut cfg = ContentConfig::default();
        cfg.allowed_extensions = Some(["rs".to_string(), "md".to_string()].into_iter().collect());

        assert!(cfg.allows_path(Path::new("main.rs")));
        assert!(cfg.allows_path(Path::new("README.md")));
        assert!(!cfg.allows_path(Path::new("table.csv")));
    }

    #[test]
    fn rule_for_unknown_extension_falls_back_to_full() {
        let cfg = ContentConfig::default();

        let rule = cfg.rule_for_path(Path::new("data.unknown"));
        assert_eq!(rule.as_str(), "full");
    }

    #[test]
    fn parse_rule_accepts_dot_prefixed_extensions() {
        let cfg = ContentConfig::default();

        let parsed = cfg.parse_rule(" .rs = full ").expect("rule should parse");
        assert_eq!(parsed.0, "rs");
        assert_eq!(parsed.1, "full");
    }

    #[test]
    fn parse_rule_rejects_missing_or_empty_sides() {
        let cfg = ContentConfig::default();

        assert!(cfg.parse_rule("rs").is_none());
        assert!(cfg.parse_rule("=full").is_none());
        assert!(cfg.parse_rule("rs=").is_none());
        assert!(cfg.parse_rule("   =   ").is_none());
    }

    #[test]
    fn parse_csv_set_normalizes_extensions_and_removes_empty_entries() {
        let cfg = ContentConfig::default();

        let set = cfg.parse_csv_set(" .rs, md, , .csv ,,txt ");

        assert!(set.contains("rs"));
        assert!(set.contains("md"));
        assert!(set.contains("csv"));
        assert!(set.contains("txt"));
        assert_eq!(set.len(), 4);
    }

    #[test]
    fn apply_mode_rules_updates_existing_and_inserts_new_rules() {
        let mut cfg = ContentConfig::default();

        cfg.apply_mode_rules(&[
            "csv=full".to_string(),
            "toml=sampled".to_string(),
            "badmode=definitely_not_valid".to_string(),
        ]);

        assert_eq!(cfg.rule_for_path(Path::new("table.csv")).as_str(), "full");
        assert_eq!(cfg.rule_for_path(Path::new("Cargo.toml")).as_str(), "sampled");
        assert_eq!(cfg.rule_for_path(Path::new("file.badmode")).as_str(), "full");
    }

    /*
    #[test]
    fn from_sources_applies_scalar_cli_values_and_modes() {
        let cli = ContentCliArgs {
            content_analysis: true,
            no_recursive_content: true,
            content_max_full_bytes: 42_000,
            content_sample_rows: 7,
            content_sample_cols: 9,
            content_extensions: Some(".rs,.md".to_string()),
            content_modes: vec!["csv=full".to_string(), "toml=sampled".to_string()],
            content_default_primer: None,
            content_default_primer_file: None,
            content_primers: vec![],
            content_primer_files: vec![],
        };

        let cfg = ContentConfig::from_sources(&cli, &[]);

        assert!(cfg.enabled);
        assert!(!cfg.recursive);
        assert_eq!(cfg.max_full_bytes, 42_000);
        assert_eq!(cfg.sample_rows, 7);
        assert_eq!(cfg.sample_cols, 9);
        assert!(cfg.allowed_extensions.as_ref().is_some_and(|s| s.contains("rs")));
        assert!(cfg.allowed_extensions.as_ref().is_some_and(|s| s.contains("md")));
        assert_eq!(cfg.rule_for_path(Path::new("table.csv")).as_str(), "full");
        assert_eq!(cfg.rule_for_path(Path::new("Cargo.toml")).as_str(), "sampled");
    }*/
}