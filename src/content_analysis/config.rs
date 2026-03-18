//config.rs
use crate::content_analysis::{ContentCliArgs, ExtensionRule, ParseMode};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Configuration controlling how file content is analyzed.
///
/// The configuration stores:
/// - whether content analysis is enabled,
/// - whether it should recurse,
/// - sampling limits for large tabular files,
/// - per-extension parsing rules,
/// - and an optional allow-list of extensions.
#[derive(Debug, Clone)]
pub struct ContentConfig {
    /// Enables or disables content analysis globally.
    pub enabled: bool,

    /// Whether content analysis should recurse into nested files or folders.
    pub recursive: bool,

    /// Default primer used when no extension-specific primer is configured.
    pub default_primer: String,

    /// Maximum number of bytes to include when reading files in full mode.
    pub max_full_bytes: usize,

    /// Number of rows to sample for sampled parsing modes.
    pub sample_rows: usize,

    /// Number of columns to sample for sampled parsing modes.
    pub sample_cols: usize,

    /// Per-extension parsing and primer rules.
    pub rules: BTreeMap<String, ExtensionRule>,

    /// Optional explicit allow-list of extensions.
    ///
    /// When `None`, all non-empty extensions are allowed.
    /// When `Some`, only the listed extensions are allowed.
    pub allowed_extensions: Option<BTreeSet<String>>,
}

impl Default for ContentConfig {
    /// Creates the default content-analysis configuration.
    ///
    /// Defaults favor full parsing for text/code formats and sampled parsing
    /// for common tabular formats such as CSV and TSV.
    fn default() -> Self {
        let mut rules = BTreeMap::new();
        rules.insert("py".into(), ExtensionRule::new(ParseMode::Full, None));
        rules.insert("rs".into(), ExtensionRule::new(ParseMode::Full, None));
        rules.insert("r".into(), ExtensionRule::new(ParseMode::Full, None));
        rules.insert("R".into(), ExtensionRule::new(ParseMode::Full, None));
        rules.insert("ipynb".into(), ExtensionRule::new(ParseMode::Full, None));
        rules.insert("md".into(), ExtensionRule::new(ParseMode::Full, None));
        rules.insert("txt".into(), ExtensionRule::new(ParseMode::Full, None));
        rules.insert("csv".into(), ExtensionRule::new(ParseMode::Sampled, None));
        rules.insert("tsv".into(), ExtensionRule::new(ParseMode::Sampled, None));

        Self {
            enabled: false,
            recursive: true,
            default_primer: "Analyze this file content. Explain its likely purpose, important signals, probable role in the project, and important domain clues.".to_string(),
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
    /// 3. default primer from inline text or file
    /// 4. extension allow-list from CLI
    /// 5. per-extension mode and primer overrides from CLI
    ///
    /// The `_files` parameter is currently unused but retained for API
    /// compatibility and future expansion.
    pub fn from_sources(cli: &ContentCliArgs, _files: &[PathBuf]) -> Self {
        let mut cfg = Self {
            enabled: cli.content_analysis,
            recursive: !cli.no_recursive_content,
            max_full_bytes: cli.content_max_full_bytes,
            sample_rows: cli.content_sample_rows,
            sample_cols: cli.content_sample_cols,
            ..Self::default()
        };

        if let Some(text) = cfg.resolve_text_source(
            cli.content_default_primer.as_ref(),
            cli.content_default_primer_file.as_ref(),
        ) {
            cfg.default_primer = text;
        }

        if let Some(exts) = &cli.content_extensions {
            let parsed = cfg.parse_csv_set(exts);
            if !parsed.is_empty() {
                cfg.allowed_extensions = Some(parsed);
            }
        }

        cfg.apply_mode_rules(&cli.content_modes);
        cfg.apply_inline_primers(&cli.content_primers);
        cfg.apply_file_primers(&cli.content_primer_files);

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

    /// Returns the effective rule for a path.
    ///
    /// If no explicit rule exists for the extension, a default full-parse rule
    /// with no primer is returned.
    pub fn rule_for_path(&self, path: &Path) -> ExtensionRule {
        let ext = self.extension_of(path);
        self.rules
            .get(&ext)
            .cloned()
            .unwrap_or_else(|| ExtensionRule::new(ParseMode::Full, None))
    }

    /// Returns the effective primer for a path.
    ///
    /// Extension-specific primers take precedence over the global default primer.
    pub fn primer_for_path(&self, path: &Path) -> String {
        let ext = self.extension_of(path);
        self.rules
            .get(&ext)
            .and_then(|r| r.primer.clone())
            .unwrap_or_else(|| self.default_primer.clone())
    }

    /// Applies CLI extension-to-mode rules such as `"rs=full"` or `"csv=sampled"`.
    fn apply_mode_rules(&mut self, rules: &[String]) {
        for item in rules {
            if let Some((ext, raw_mode)) = self.parse_rule(item) {
                if let Some(mode) = ParseMode::from_cli_value(&raw_mode) {
                    self.rules
                        .entry(ext)
                        .and_modify(|r| r.parse_mode = mode)
                        .or_insert_with(|| ExtensionRule::new(mode, None));
                }
            }
        }
    }

    /// Applies inline per-extension primers such as `"rs=Explain Rust ownership here"`.
    fn apply_inline_primers(&mut self, primers: &[String]) {
        for item in primers {
            if let Some((ext, primer)) = self.parse_rule(item) {
                self.rules
                    .entry(ext)
                    .and_modify(|r| r.primer = Some(primer.clone()))
                    .or_insert_with(|| ExtensionRule::new(ParseMode::Full, Some(primer)));
            }
        }
    }

    /// Applies per-extension primers loaded from files.
    ///
    /// Each input is expected to have the form `"ext=/path/to/file.txt"`.
    /// Missing or unreadable files are ignored.
    fn apply_file_primers(&mut self, primer_files: &[String]) {
        for item in primer_files {
            if let Some((ext, path_str)) = self.parse_rule(item) {
                let path = PathBuf::from(path_str);
                if let Ok(primer) = fs::read_to_string(&path) {
                    self.rules
                        .entry(ext)
                        .and_modify(|r| r.primer = Some(primer.clone()))
                        .or_insert_with(|| ExtensionRule::new(ParseMode::Full, Some(primer)));
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

    /// Resolves text from either an inline string or a file path.
    ///
    /// Inline text takes precedence over file-based text. Missing or unreadable
    /// files return `None`.
    fn resolve_text_source(
        &self,
        inline: Option<&String>,
        file: Option<&PathBuf>,
    ) -> Option<String> {
        if let Some(text) = inline {
            return Some(text.clone());
        }

        if let Some(path) = file {
            if let Ok(text) = fs::read_to_string(path) {
                return Some(text);
            }
        }

        None
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
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_file_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();

        std::env::temp_dir().join(format!("archeo_content_config_{nanos}_{name}"))
    }

    #[test]
    fn default_contains_expected_extension_rules() {
        let cfg = ContentConfig::default();

        assert_eq!(cfg.rule_for_path(Path::new("main.rs")).parse_mode.as_str(), "full");
        assert_eq!(cfg.rule_for_path(Path::new("notes.md")).parse_mode.as_str(), "full");
        assert_eq!(cfg.rule_for_path(Path::new("table.csv")).parse_mode.as_str(), "sampled");
        assert_eq!(cfg.rule_for_path(Path::new("table.tsv")).parse_mode.as_str(), "sampled");
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
        assert_eq!(rule.parse_mode.as_str(), "full");
        assert!(rule.primer.is_none());
    }

    #[test]
    fn primer_for_path_prefers_extension_specific_primer() {
        let mut cfg = ContentConfig::default();
        cfg.default_primer = "default primer".to_string();
        cfg.rules.insert(
            "rs".to_string(),
            ExtensionRule::new(ParseMode::Full, Some("rust primer".to_string())),
        );

        assert_eq!(cfg.primer_for_path(Path::new("main.rs")), "rust primer");
        assert_eq!(cfg.primer_for_path(Path::new("notes.md")), "default primer");
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

        assert_eq!(cfg.rule_for_path(Path::new("table.csv")).parse_mode.as_str(), "full");
        assert_eq!(cfg.rule_for_path(Path::new("Cargo.toml")).parse_mode.as_str(), "sampled");
        assert_eq!(cfg.rule_for_path(Path::new("file.badmode")).parse_mode.as_str(), "full");
    }

    #[test]
    fn apply_inline_primers_updates_and_inserts_primers() {
        let mut cfg = ContentConfig::default();

        cfg.apply_inline_primers(&[
            "rs=Explain Rust code carefully".to_string(),
            "toml=Configuration file".to_string(),
        ]);

        assert_eq!(cfg.primer_for_path(Path::new("main.rs")), "Explain Rust code carefully");
        assert_eq!(cfg.primer_for_path(Path::new("Cargo.toml")), "Configuration file");
    }

    #[test]
    fn apply_file_primers_reads_text_from_files() {
        let mut cfg = ContentConfig::default();

        let primer_path = temp_file_path("primer.txt");
        fs::write(&primer_path, "primer from file").expect("should write temp primer file");

        cfg.apply_file_primers(&[format!("rs={}", primer_path.display())]);

        assert_eq!(cfg.primer_for_path(Path::new("main.rs")), "primer from file");

        let _ = fs::remove_file(primer_path);
    }

    #[test]
    fn apply_file_primers_ignores_missing_files() {
        let mut cfg = ContentConfig::default();
        cfg.default_primer = "default".to_string();

        let missing = temp_file_path("missing_primer.txt");
        cfg.apply_file_primers(&[format!("rs={}", missing.display())]);

        assert_eq!(cfg.primer_for_path(Path::new("main.rs")), "default");
    }

    #[test]
    fn resolve_text_source_prefers_inline_text_over_file() {
        let cfg = ContentConfig::default();

        let file_path = temp_file_path("default_primer.txt");
        fs::write(&file_path, "from file").expect("should write temp file");

        let inline = "from inline".to_string();
        let resolved = cfg.resolve_text_source(Some(&inline), Some(&file_path));

        assert_eq!(resolved.as_deref(), Some("from inline"));

        let _ = fs::remove_file(file_path);
    }

    #[test]
    fn resolve_text_source_reads_from_file_when_inline_missing() {
        let cfg = ContentConfig::default();

        let file_path = temp_file_path("only_file_primer.txt");
        fs::write(&file_path, "from file").expect("should write temp file");

        let resolved = cfg.resolve_text_source(None, Some(&file_path));

        assert_eq!(resolved.as_deref(), Some("from file"));

        let _ = fs::remove_file(file_path);
    }

    #[test]
    fn resolve_text_source_returns_none_when_nothing_is_available() {
        let cfg = ContentConfig::default();

        let missing = temp_file_path("definitely_missing.txt");
        let resolved = cfg.resolve_text_source(None, Some(&missing));

        assert!(resolved.is_none());
    }
}
