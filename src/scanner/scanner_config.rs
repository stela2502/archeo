use std::collections::HashMap;

use clap::Args;
use rust_yaml::Yaml;

#[derive(Debug, Clone)]
pub struct ScanConfig {
    pub allowed_extensions: Vec<String>,
    pub excluded_dirs: Vec<String>,
    pub max_file_size: usize,
    pub include_hidden: bool,
}

impl Default for ScanConfig {
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

#[derive(Args, Debug, Clone)]
pub struct ScanCliArgs {
    /// YAML config file
    #[arg(long)]
    pub config: Option<String>,

    /// Allowed extensions (override)
    #[arg(long)]
    pub ext: Vec<String>,

    /// Exclude directories (override)
    #[arg(long)]
    pub exclude_dir: Vec<String>,

    /// Max file size in bytes
    #[arg(long)]
    pub max_file_size: Option<usize>,

    /// Include hidden files
    #[arg(long)]
    pub include_hidden: bool,
}

impl ScanConfig {
    /// Build config from CLI + optional YAML (loose parsing)
    pub fn from_sources(cli: &ScanCliArgs) -> Self {
        // 1. defaults
        let mut cfg = ScanConfig::default();

        // 2. YAML (optional)
        if let Some(path) = &cli.config {
            if let Ok(yaml) = Yaml::load_from_file(path) {
                cfg = ScanConfig::from_yaml_loose(&yaml);
            }
        }

        // 3. CLI overrides (highest priority)

        if !cli.ext.is_empty() {
            cfg.allowed_extensions = cli.ext.clone();
        }

        if !cli.exclude_dir.is_empty() {
            cfg.excluded_dirs = cli.exclude_dir.clone();
        }

        if let Some(size) = cli.max_file_size {
            cfg.max_file_size = size;
        }

        if cli.include_hidden {
            cfg.include_hidden = true;
        }

        cfg
    }

    /// Loose YAML parsing (fallback to defaults silently)
    pub fn from_yaml_loose(y: &Yaml) -> Self {
        let mut cfg = ScanConfig::default();

        let map = match y {
            Yaml::Hash(m) => m,
            _ => return cfg,
        };

        // allowed_extensions
        if let Some(Yaml::Array(arr)) = map.get("allowed_extensions") {
            let vals: Vec<String> = arr.iter().filter_map(as_string).collect();
            if !vals.is_empty() {
                cfg.allowed_extensions = vals;
            }
        }

        // excluded_dirs
        if let Some(Yaml::Array(arr)) = map.get("excluded_dirs") {
            let vals: Vec<String> = arr.iter().filter_map(as_string).collect();
            if !vals.is_empty() {
                cfg.excluded_dirs = vals;
            }
        }

        // max_file_size
        if let Some(Yaml::Value(s)) = map.get("max_file_size") {
            if let Ok(v) = s.parse::<usize>() {
                cfg.max_file_size = v;
            }
        }

        // include_hidden
        if let Some(Yaml::Value(s)) = map.get("include_hidden") {
            cfg.include_hidden = s == "true";
        }

        cfg
    }

    /// Convert back to YAML (for saving/debugging)
    pub fn to_yaml(&self) -> Yaml {
        let mut map = HashMap::new();

        map.insert(
            "allowed_extensions".into(),
            Yaml::Array(self.allowed_extensions.iter().map(|s| Yaml::Value(s.clone())).collect()),
        );

        map.insert(
            "excluded_dirs".into(),
            Yaml::Array(self.excluded_dirs.iter().map(|s| Yaml::Value(s.clone())).collect()),
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

    /// Human-readable summary
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

/// helper
fn as_string(y: &Yaml) -> Option<String> {
    match y {
        Yaml::Value(s) => Some(s.clone()),
        _ => None,
    }
}
