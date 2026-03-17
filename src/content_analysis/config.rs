//config.rs

use crate::content_analysis::{ContentCliArgs, ExtensionRule, ParseMode};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ContentConfig {
    pub enabled: bool,
    pub recursive: bool,
    pub default_primer: String,
    pub max_full_bytes: usize,
    pub sample_rows: usize,
    pub sample_cols: usize,
    pub rules: BTreeMap<String, ExtensionRule>,
    pub allowed_extensions: Option<BTreeSet<String>>,
}

impl Default for ContentConfig {
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

    pub fn extension_of(&self, path: &Path) -> String {
        path.extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .trim()
            .trim_start_matches('.')
            .to_string()
    }

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

    pub fn rule_for_path(&self, path: &Path) -> ExtensionRule {
        let ext = self.extension_of(path);
        self.rules
            .get(&ext)
            .cloned()
            .unwrap_or_else(|| ExtensionRule::new(ParseMode::Full, None))
    }

    pub fn primer_for_path(&self, path: &Path) -> String {
        let ext = self.extension_of(path);
        self.rules
            .get(&ext)
            .and_then(|r| r.primer.clone())
            .unwrap_or_else(|| self.default_primer.clone())
    }

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

    fn parse_rule(&self, input: &str) -> Option<(String, String)> {
        let (left, right) = input.split_once('=')?;
        let ext = left.trim().trim_start_matches('.').to_string();
        let value = right.trim().to_string();

        if ext.is_empty() || value.is_empty() {
            return None;
        }

        Some((ext, value))
    }

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

    fn parse_csv_set(&self, input: &str) -> BTreeSet<String> {
        input
            .split(',')
            .map(|s| s.trim().trim_start_matches('.'))
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect()
    }
}