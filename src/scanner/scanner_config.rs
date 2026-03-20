//! Scanner configuration types and helpers.
//!
//! This module defines the configuration used by the directory scanner,
//! including defaults, command-line arguments, YAML conversion, and a
//! human-readable summary.

use std::collections::HashMap;

use rust_yaml::Yaml;

/// Configuration controlling how files are discovered during scanning.
///
/// A [`ScanConfig`] can be built from:
/// - [`Default`] values,
/// - an optional YAML file,
/// - and command-line overrides via [`ScanCliArgs`].
///
/// Command-line values always take precedence over YAML values, and YAML
/// values take precedence over defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanConfig {
    /// File extensions that are allowed in the scan result, without leading dots.
    ///
    /// Example: `["rs", "md"]`
    pub allowed_extensions: Vec<String>,

    /// Directory names that should be excluded anywhere in the scanned path.
    ///
    /// Matching is performed on path components, not substrings.
    pub excluded_dirs: Vec<String>,

    /// Maximum file size in bytes for included files.
    pub max_file_size: usize,

    /// Whether hidden files and directories should be included.
    ///
    /// Hidden paths are typically identified by a leading `.` in the file name.
    pub include_hidden: bool,
}

impl Default for ScanConfig {
    /// Returns the default scanning configuration.
    ///
    /// Defaults:
    /// - `allowed_extensions`: `rs`, `py`, `md`, `txt`
    /// - `excluded_dirs`: `.git`, `target`, `node_modules`
    /// - `max_file_size`: `5_000_000`
    /// - `include_hidden`: `false`
    fn default() -> Self {
        Self {
            allowed_extensions: vec![
                "rs".into(),
                "py".into(),
                "md".into(),
                "txt".into(),
            ],
            excluded_dirs: vec![
                ".git".into(),
                "target".into(),
                "node_modules".into(),
            ],
            max_file_size: 5_000_000,
            include_hidden: false,
        }
    }
}


impl ScanConfig {
    /// Builds a [`ScanConfig`] from defaults, optional YAML, and CLI overrides.
    ///
    /// Precedence order:
    /// 1. [`Default`] values
    /// 2. YAML file pointed to by `cli.config` if it can be loaded
    /// 3. Explicit CLI arguments in `cli`
    ///
    /// YAML parsing is intentionally loose: invalid or missing YAML input falls
    /// back silently to defaults.
    pub fn from_sources(
        config_path: Option<&str>,
        ext: &[String],
        exclude_dir: &[String],
        max_file_size: Option<usize>,
        include_hidden: bool,
    ) -> Self {
        let mut cfg = ScanConfig::default();

        if let Some(path) = config_path {
            if let Ok(yaml) = Yaml::load_from_file(path) {
                cfg = ScanConfig::from_yaml_loose(&yaml);
            }
        }

        if !ext.is_empty() {
            cfg.allowed_extensions = ext.to_vec();
        }

        if !exclude_dir.is_empty() {
            cfg.excluded_dirs = exclude_dir.to_vec();
        }

        if let Some(size) = max_file_size {
            cfg.max_file_size = size;
        }

        if include_hidden {
            cfg.include_hidden = true;
        }

        cfg
    }
    /// Parses a [`ScanConfig`] from YAML using forgiving rules.
    ///
    /// Missing keys, wrong YAML shapes, or unparsable values are ignored and
    /// the corresponding default values are retained.
    ///
    /// Expected keys:
    /// - `allowed_extensions`: array of strings
    /// - `excluded_dirs`: array of strings
    /// - `max_file_size`: string-like integer value
    /// - `include_hidden`: `"true"` or any other string
    pub fn from_yaml_loose(y: &Yaml) -> Self {
        let mut cfg = ScanConfig::default();

        let map = match y {
            Yaml::Hash(m) => m,
            _ => return cfg,
        };

        if let Some(Yaml::Array(arr)) = map.get("allowed_extensions") {
            let vals: Vec<String> = arr.iter().filter_map(as_string).collect();
            if !vals.is_empty() {
                cfg.allowed_extensions = vals;
            }
        }

        if let Some(Yaml::Array(arr)) = map.get("excluded_dirs") {
            let vals: Vec<String> = arr.iter().filter_map(as_string).collect();
            if !vals.is_empty() {
                cfg.excluded_dirs = vals;
            }
        }

        if let Some(Yaml::Value(s)) = map.get("max_file_size") {
            if let Ok(v) = s.parse::<usize>() {
                cfg.max_file_size = v;
            }
        }

        if let Some(Yaml::Value(s)) = map.get("include_hidden") {
            cfg.include_hidden = s == "true";
        }

        cfg
    }

    /// Converts this configuration to YAML.
    ///
    /// This is primarily useful for debugging, logging, or writing a config
    /// back to disk in the same loose schema accepted by [`Self::from_yaml_loose`].
    pub fn to_yaml(&self) -> Yaml {
        let mut map = HashMap::new();

        map.insert(
            "allowed_extensions".into(),
            Yaml::Array(
                self.allowed_extensions
                    .iter()
                    .map(|s| Yaml::Value(s.clone()))
                    .collect(),
            ),
        );

        map.insert(
            "excluded_dirs".into(),
            Yaml::Array(
                self.excluded_dirs
                    .iter()
                    .map(|s| Yaml::Value(s.clone()))
                    .collect(),
            ),
        );

        map.insert(
            "max_file_size".into(),
            Yaml::Value(self.max_file_size.to_string()),
        );

        map.insert(
            "include_hidden".into(),
            Yaml::Value(self.include_hidden.to_string()),
        );

        Yaml::Hash(map)
    }

    /// Returns a human-readable multi-line summary of the configuration.
    pub fn describe(&self) -> String {
        format!(
            r#"Scan configuration:
  allowed_extensions: {}
  excluded_dirs: {}
  max_file_size: {} bytes
  include_hidden: {}"#,
            self.allowed_extensions.join(", "),
            self.excluded_dirs.join(", "),
            self.max_file_size,
            self.include_hidden,
        )
    }
}

/// Extracts a string from a YAML value.
///
/// Returns `Some(String)` only for [`Yaml::Value`].
fn as_string(y: &Yaml) -> Option<String> {
    match y {
        Yaml::Value(s) => Some(s.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_cli() -> ScanCliArgs {
        ScanCliArgs {
            config: None,
            ext: Vec::new(),
            exclude_dir: Vec::new(),
            max_file_size: None,
            include_hidden: false,
        }
    }

    #[test]
    fn default_config_matches_expected_values() {
        let cfg = ScanConfig::default();

        assert_eq!(cfg.allowed_extensions, vec!["rs", "py", "md", "txt"]);
        assert_eq!(cfg.excluded_dirs, vec![".git", "target", "node_modules"]);
        assert_eq!(cfg.max_file_size, 5_000_000);
        assert!(!cfg.include_hidden);
    }

    #[test]
    fn from_yaml_loose_reads_all_supported_fields() {
        let mut map = HashMap::new();
        map.insert(
            "allowed_extensions".into(),
            Yaml::Array(vec![Yaml::Value("rs".into()), Yaml::Value("toml".into())]),
        );
        map.insert(
            "excluded_dirs".into(),
            Yaml::Array(vec![Yaml::Value("target".into()), Yaml::Value("dist".into())]),
        );
        map.insert("max_file_size".into(), Yaml::Value("1234".into()));
        map.insert("include_hidden".into(), Yaml::Value("true".into()));

        let cfg = ScanConfig::from_yaml_loose(&Yaml::Hash(map));

        assert_eq!(cfg.allowed_extensions, vec!["rs", "toml"]);
        assert_eq!(cfg.excluded_dirs, vec!["target", "dist"]);
        assert_eq!(cfg.max_file_size, 1234);
        assert!(cfg.include_hidden);
    }

    #[test]
    fn from_yaml_loose_falls_back_to_defaults_for_invalid_shapes() {
        let mut map = HashMap::new();
        map.insert("allowed_extensions".into(), Yaml::Value("rs".into()));
        map.insert("excluded_dirs".into(), Yaml::Value("target".into()));
        map.insert("max_file_size".into(), Yaml::Value("not_a_number".into()));
        map.insert("include_hidden".into(), Yaml::Value("yes".into()));

        let cfg = ScanConfig::from_yaml_loose(&Yaml::Hash(map));
        let default_cfg = ScanConfig::default();

        assert_eq!(cfg.allowed_extensions, default_cfg.allowed_extensions);
        assert_eq!(cfg.excluded_dirs, default_cfg.excluded_dirs);
        assert_eq!(cfg.max_file_size, default_cfg.max_file_size);
        assert!(!cfg.include_hidden);
    }

    #[test]
    fn from_yaml_loose_ignores_non_string_array_entries() {
        let mut map = HashMap::new();
        map.insert(
            "allowed_extensions".into(),
            Yaml::Array(vec![Yaml::Value("rs".into()), Yaml::Hash(HashMap::new())]),
        );

        let cfg = ScanConfig::from_yaml_loose(&Yaml::Hash(map));

        assert_eq!(cfg.allowed_extensions, vec!["rs"]);
    }

    #[test]
    fn from_sources_uses_defaults_when_no_overrides_are_present() {
        let cli = empty_cli();
        let cfg = ScanConfig::from_sources(&cli);

        assert_eq!(cfg, ScanConfig::default());
    }

    #[test]
    fn from_sources_applies_cli_overrides() {
        let cli = ScanCliArgs {
            config: None,
            ext: vec!["toml".into(), "yaml".into()],
            exclude_dir: vec!["build".into()],
            max_file_size: Some(42),
            include_hidden: true,
        };

        let cfg = ScanConfig::from_sources(&cli);

        assert_eq!(cfg.allowed_extensions, vec!["toml", "yaml"]);
        assert_eq!(cfg.excluded_dirs, vec!["build"]);
        assert_eq!(cfg.max_file_size, 42);
        assert!(cfg.include_hidden);
    }

    #[test]
    fn to_yaml_round_trips_through_from_yaml_loose() {
        let cfg = ScanConfig {
            allowed_extensions: vec!["rs".into(), "md".into()],
            excluded_dirs: vec!["target".into(), ".git".into()],
            max_file_size: 1024,
            include_hidden: true,
        };

        let yaml = cfg.to_yaml();
        let decoded = ScanConfig::from_yaml_loose(&yaml);

        assert_eq!(decoded, cfg);
    }

    #[test]
    fn describe_contains_all_key_information() {
        let cfg = ScanConfig {
            allowed_extensions: vec!["rs".into(), "md".into()],
            excluded_dirs: vec!["target".into()],
            max_file_size: 2048,
            include_hidden: true,
        };

        let text = cfg.describe();

        assert!(text.contains("allowed_extensions: rs, md"));
        assert!(text.contains("excluded_dirs: target"));
        assert!(text.contains("max_file_size: 2048 bytes"));
        assert!(text.contains("include_hidden: true"));
    }

    #[test]
    fn as_string_extracts_plain_yaml_values_only() {
        assert_eq!(as_string(&Yaml::Value("abc".into())), Some("abc".into()));
        assert_eq!(as_string(&Yaml::Array(vec![])), None);
    }
}