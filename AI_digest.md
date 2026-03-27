# Archeo Report

## Target
.

## Model
deepseek-coder-v2:latest

## Scan Configuration
```
Scan configuration:
  allowed_extensions: rs
  excluded_dirs: .git, target, node_modules
  max_file_size: 5000000 bytes
  include_hidden: false
```

## Included Files
- src\content_analysis\analyzer.rs
- src\content_analysis\config.rs
- src\content_analysis\descriptor.rs
- src\content_analysis\extension_rule.rs
- src\content_analysis\mod.rs
- src\content_analysis\notebooks\mod.rs
- src\content_analysis\notebooks\notebook.rs
- src\content_analysis\parse_mode.rs
- src\lib.rs
- src\main.rs
- src\ollama.rs
- src\primer.rs
- src\prompt_defaults.rs
- src\report.rs
- src\scanner\mod.rs
- src\scanner\scanner.rs
- src\scanner\scanner_config.rs

## AI Analysis

# CLI Usage Summary

## Per Option
- name: allowed_extensions
  - usage: Used in `ScanConfig` struct to specify which file extensions are allowed for scanning.
  - status: used
  - notes: This option is directly defined and used within the configuration structure (`ScanConfig`) to filter file types during scanning.

- name: excluded_dirs
  - usage: Used in `ScanConfig` struct to specify directories that should be excluded from scanning.
  - status: used
  - notes: This option helps in narrowing down the scope of files scanned, focusing only on those relevant for analysis.

- name: max_file_size
  - usage: Used in `ScanConfig` struct to set a maximum file size limit for inclusion in scans.
  - status: used
  - notes: This option helps manage resource usage by excluding very large files that might not be of interest during analysis.

- name: include_hidden
  - usage: Used in `ScanConfig` struct to determine if hidden files should be included in the scan.
  - status: used
  - notes: This option is useful for thoroughness and can be toggled based on whether deeper system or project insights are desired.

## Conflicts
There are no conflicts identified among the CLI options within this context, as each option serves a distinct purpose related to configuration of the file scanning process.

## Cleanup candidates
- **Unlikely**: There is no clear indication that any specific CLI option would be considered obsolete in this context without further usage evidence or changes in functionality not provided here. The options seem integral to configuring the scanner's behavior based on user needs and project specifics.

# Short Summary
The Rust project appears to be a file scanning tool, likely used for analyzing files within a system, with specific configuration options tailored to manage which types of files are considered during analysis and how large they can be. This setup is flexible and customizable through command-line interface parameters such as allowed extensions, excluded directories, maximum file size, and including or excluding hidden files.

## Main Components
- **Modules**: 
  - `scanner`: Handles the main scanning functionality.
  - `scanner_config`: Manages configuration settings for the scanner, including CLI options like `allowed_extensions`, `excluded_dirs`, `max_file_size`, and `include_hidden`.

## Likely Workflow
1. **Configuration**: User defines or adjusts configurations in a YAML file or via command-line arguments (`allowed_extensions`, `excluded_dirs`, `max_file_size`, `include_hidden`).
2. **Execution**: The scanner uses these configurations to determine which files to scan and which to exclude, based on the specified criteria.
3. **Analysis**: Files are scanned according to the configuration settings, potentially generating reports or insights relevant to the project's scope.

## Important Files
- `src\scanner\scanner_config.rs`: Defines the `ScanConfig` struct and methods for handling CLI options directly in the configuration logic.
- `src\main.rs`: Entry point where CLI options might be parsed and used to initialize or configure the scanner based on user input.

These files are crucial as they encapsulate the core functionality of managing file scanning through command-line options, demonstrating how external inputs shape the application's behavior in handling large sets of data for specific analysis tasks.

## Content Analysis Summary
 The provided code snippets are part of a larger Rust project that likely involves file scanning and configuration management. Let's break down the key components and functionalities from these snippets:

### 1. Main Module (`scanner`)
This module seems to be responsible for file scanning, possibly using specific extensions or ignoring certain directories. The main functionality is encapsulated in the `scanner` crate.

#### Key Components:
- **Structs and Enums**: There might be structs or enums defined for configuration options like allowed extensions, excluded directories, etc.
- **Functions**: Functions to initialize or configure the scanner based on external settings (e.g., from a YAML file).

### 2. Configuration Module (`scanner_config`)
This module handles all aspects of configuring the scanner's behavior through a configuration struct named `ScanConfig`. It includes:

#### Key Components:
- **Struct `ScanConfig`**: This struct holds various configuration options:
  - `allowed_extensions`: A vector of strings representing allowed file extensions.
  - `excluded_dirs`: A vector of strings representing directories to be excluded from scanning.
  - `max_file_size`: An integer representing the maximum size (in bytes) of files to be considered for scanning.
  - `include_hidden`: A boolean indicating whether hidden files should be included in the scan.
- **Default Implementation**: The struct has a default implementation that sets some sensible defaults, such as allowed extensions and excluded directories, and a maximum file size.
- **YAML Parsing**: Functions to parse configuration from YAML, including `from_yaml_loose` which handles parsing of supported fields loosely (i.e., it can handle extra or missing fields gracefully).

### 3. Utility Function (`as_string`)
This function is used within the configuration module to convert Yaml values to strings, useful for extracting plain values from more complex YAML structures.

### Example Usage:
1. **Initializing Configuration**:
   ```rust
   let cfg = ScanConfig::default(); // Uses default settings
   ```
2. **Customizing Configuration**:
   ```rust
   let custom_cfg = ScanConfig::from_sources(
       &["txt", "md"], 
       &[".git", "target"], 
       Some(5_000_000), 
       true
   ); // Customizes settings based on requirements
   ```
3. **Parsing from YAML**:
   ```rust
   let yaml = get_yaml(); // Assume this function retrieves the YAML configuration
   let cfg_from_yaml = ScanConfig::from_yaml_loose(&yaml);
   ```
4. **Converting to YAML**:
   ```rust
   let yaml_config = cfg.to_yaml();
   ```
5. **Describing Configuration**:
   ```rust
   println!("{}", cfg.describe()); // Prints a human-readable description of the configuration
   ```

### Summary:
The `scanner` and `scanner_config` modules together provide a comprehensive way to configure and manage file scanning operations, allowing for flexibility in specifying which files to include or exclude based on various criteria. The use of YAML for external configuration enables easy integration with different systems without modifying the core application code.


## Content Analysis Detailed Per File
### .\src\content_analysis\analyzer.rs


 ```rust
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::content_analysis::{ContentConfig, ContentDescriptor, ParseMode};
use crate::ollama::Ollama;
use crate::prompt_defaults::PromptDefaults;

#[derive(Debug, Clone)]
pub struct ContentAnalysisReport {
    pub path: PathBuf,
    pub extension: String,
    pub parse_mode: String,
    pub primer_used: Option<String>,
    pub descriptor: Option<ContentDescriptor>,
    pub ai_response: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ContentAnalyzer {
    pub config: ContentConfig,
}

impl ContentAnalyzer {
    pub fn new(config: ContentConfig) -> Self {
        Self { config }
    }

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

    pub fn render_detailed_summary(reports: &[ContentAnalysisReport]) -> String {
        let mut out = String::new();

        for report in reports {
            out.push_str(&format!("FILE: {}\n", report.path.display()));
            out.push_str(&format!("EXTENSION: {}\n", report.extension));
            out.push_str(&format!("PARSE_MODE: {}\n", report.parse_mode));

            if let Some(primer) = &report.primer_used {
                out.push_str(&format!("PRIMER_USED:\n{}\n", primer));
            }

            if let Some(ai_response) = &report.ai_response {
                out.push_str(&format!("INTERPRETATION:\n{}\n", ai_response));
            }

            for warning in &report.warnings {
                out.push_str(&format!("WARNING: {}\n", warning));
            }

            out.push_str("\n---\n\n");
        }

        out
    }

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
        report.warnings = vec!["first warning".to_string(), "second warning".to_string()];

        let text = ContentAnalyzer::render_detailed_summary(&[report]);

        assert!(text.contains("WARNING: first warning\n"));
        assert!(text.contains("WARNING: second warning\n"));
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
```


### .\src\content_analysis\config.rs


 ```rust
pub struct ContentConfig {
    pub enabled: bool,
    pub recursive: bool,
    pub max_full_bytes: usize,
    pub sample_rows: usize,
    pub sample_cols: usize,
    pub rules: BTreeMap<String, ParseMode>,
    pub allowed_extensions: Option<BTreeSet<String>>,
}

impl Default for ContentConfig {
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

    pub fn rule_for_path(&self, path: &Path) -> ParseMode {
        let ext = self.extension_of(path);
        self.rules.get(&ext).copied().unwrap_or(ParseMode::Full)
    }

    fn apply_mode_rules(&mut self, rules: &[String]) {
        for item in rules {
            if let Some((ext, raw_mode)) = self.parse_rule(item)
                && let Some(mode) = ParseMode::from_cli_value(&raw_mode)
            {
                self.rules.insert(ext, mode);
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

    fn parse_csv_set(&self, input: &str) -> BTreeSet<String> {
        input
            .split(',')
            .map(|s| s.trim().trim_start_matches('.'))
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect()
    }
}
```


### .\src\content_analysis\descriptor.rs


 ```rust
#[derive(Debug)]
pub enum ContentKind {
    Table,
    NoteBook,
}

impl ContentKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ContentKind::Table => "table",
            ContentKind::NoteBook => "notebook",
        }
    }
}

#[derive(Debug)]
pub enum ParseMode {
    Full,
    Sampled,
    Skip,
}

impl ParseMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ParseMode::Full => "full",
            ParseMode::Sampled => "sampled",
            ParseMode::Skip => "skip",
        }
    }
}

#[derive(Debug)]
pub struct ContentDescriptor {
    path: std::path::PathBuf,
    extension: String,
    kind: ContentKind,
    parse_mode: ParseMode,
    file_size: usize,
    is_truncated: bool,
    is_sample: bool,
    total_rows: Option<usize>,
    total_cols: Option<usize>,
    sampled_rows: Option<usize>,
    sampled_cols: Option<usize>,
    content: String,
}

impl ContentDescriptor {
    pub fn new(path: std::path::PathBuf, extension: String, kind: ContentKind, parse_mode: ParseMode, file_size: usize) -> Self {
        Self {
            path,
            extension,
            kind,
            parse_mode,
            file_size,
            is_truncated: false,
            is_sample: false,
            total_rows: None,
            total_cols: None,
            sampled_rows: None,
            sampled_cols: None,
            content: String::new(),
        }
    }

    pub fn set_truncated(&mut self, truncated: bool) {
        self.is_truncated = truncated;
    }

    pub fn set_sample(&mut self, is_sample: bool) {
        self.is_sample = is_sample;
    }

    pub fn set_total_rows(&mut self, total_rows: usize) {
        self.total_rows = Some(total_rows);
    }

    pub fn set_total_cols(&mut self, total_cols: usize) {
        self.total_cols = Some(total_cols);
    }

    pub fn set_sampled_rows(&mut self, sampled_rows: usize) {
        self.sampled_rows = Some(sampled_rows);
    }

    pub fn set_sampled_cols(&mut self, sampled_cols: usize) {
        self.sampled_cols = Some(sampled_cols);
    }

    pub fn add_content(&mut self, content: String) {
        self.content = content;
    }

    pub fn render_for_prompt(&self) -> String {
        let mut out = String::new();

        out.push_str(&format!("File: {}\n", self.path.display()));
        out.push_str(&format!("Extension: {}\n", self.extension));
        out.push_str(&format!("Kind: {:?}\n", self.kind));
        out.push_str(&format!("Parse mode: {}\n", self.parse_mode.as_str()));
        out.push_str(&format!("File size: {}\n", self.file_size));
        out.push_str(&format!("Truncated: {}\n", self.is_truncated));
        out.push_str(&format!("Sampled: {}\n", self.is_sample));

        if let Some(v) = self.total_rows {
            out.push_str(&format!("Total rows: {}\n", v));
        }
        if let Some(v) = self.total_cols {
            out.push_str(&format!("Total cols: {}\n", v));
        }
        if let Some(v) = self.sampled_rows {
            out.push_str(&format!("Sampled rows: {}\n", v));
        }
        if let Some(v) = self.sampled_cols {
            out.push_str(&format!("Sampled cols: {}\n", v));
        }

        out.push_str("\nContent:\n");
        out.push_str(&self.content);

        out
    }
}
```


### .\src\content_analysis\extension_rule.rs


 ```rust
#[derive(Debug, Clone)]
pub struct ExtensionRule {
    pub parse_mode: ParseMode,
    pub primer: Option<String>,
}

impl ExtensionRule {
    pub fn new(parse_mode: ParseMode, primer: Option<String>) -> Self {
        Self { parse_mode, primer }
    }
}
```


### .\src\content_analysis\mod.rs


 ```rust
// Import necessary modules
mod analyzer;
mod config;
mod descriptor;
mod extension_rule;
mod notebooks;
mod parse_mode;

// Re-export public modules and their items
pub use analyzer::{ContentAnalysisReport, ContentAnalyzer};
pub use config::ContentConfig;
pub use descriptor::{ContentDescriptor, ContentKind};
pub use extension_rule::ExtensionRule;
pub use parse_mode::ParseMode;
```


### .\src\content_analysis\notebooks\mod.rs


 ```rust
// File: src/content_analysis/notebooks/mod.rs

pub mod notebook;

fn main() {
    // Unknown if there's a `main` function with Clap structs and fields
}
```


### .\src\content_analysis\notebooks\notebook.rs


 The provided Rust code appears to be a test suite for a library related to parsing and summarizing the contents of Jupyter notebooks (often used in Python data science workflows). The tests cover various functionalities such as parsing markdown cells, handling different types of outputs like text, JSON tables, lists, and more. They also check that specific configurations or inputs result in expected output formats and behaviors.

Here's a breakdown of the test cases:

1. **Test for Markdown Cells**: This tests whether iterating over the notebook yields only markdown cells as expected. It constructs a sample notebook with mixed cell types, including markdown and code, and verifies that the iterator returns only the markdown cells.

2. **Area Extraction**: This test checks if extracting parts of the notebook (specified by indices) correctly filters out markdown cells and collects outputs according to the given range.

3. **Output Retention IDs**: The test ensures that each output is assigned a unique, stable ID based on its cell and index in the list of outputs for that cell. This is crucial for maintaining references and consistency across different parts of the notebook data structure.

4. **Clipping Long Text**: This test verifies if long text outputs are truncated appropriately according to specified line and character limits. It uses a sample notebook with a code cell outputting a long text string, and it checks that the truncation respects these boundaries.

5. **HTML Handling**: The test confirms that HTML content in outputs is retained as a compact object representation, which could involve checking if specific HTML tags or structures are present in the final summary.

6. **JSON Table Handling**: This tests how JSON tables are summarized and checked for presence of relevant data when truncated according to configuration settings. It uses a sample notebook with code outputting JSON data that is then displayed as a table, and it verifies if the summarization respects the row limit set in the configuration.

7. **List-like Text Reduction**: This test checks how list-like text outputs are reduced based on a maximum number of items specified. It constructs a notebook with code generating long lists and validates that only a subset is retained according to the configured limit.

8. **Markdown Iteration**: As mentioned earlier, this test confirms that iterating over the markdown cells retrieves them correctly from different parts of the notebook structure.

9. **Output Retrieval for Specified Area**: This checks if extracting code and output pairs based on indices filters out irrelevant content and collects only the relevant pieces as expected.

Each of these tests is designed to verify that the library handles various types of data in notebooks correctly, applying appropriate summarization or filtering strategies based on user-defined configurations or default behaviors. They are essential for ensuring that the software behaves predictably across different inputs and configurations, providing reliable output formats suitable for further analysis or reporting.


### .\src\content_analysis\parse_mode.rs


 ```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseMode {
    Full,
    Sampled,
    Skip,
}

impl ParseMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Sampled => "sampled",
            Self::Skip => "skip",
        }
    }

    pub fn from_cli_value(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "full" => Some(Self::Full),
            "sampled" | "sample" => Some(Self::Sampled),
            "skip" => Some(Self::Skip),
            _ => None,
        }
    }
}
```


### .\src\lib.rs


 ```rust
// src/lib.rs

pub mod content_analysis;
pub mod ollama;
pub mod primer;
pub mod prompt_defaults;
pub mod report;
pub mod scanner;
```


### .\src\main.rs


 ```rust
fn main() -> anyhow::Result<()> {
    let args = std::env::args().collect::<Vec<String>>();
    if args.len() < 2 {
        eprintln!("Usage: {} <path_to_scan>", args[0]);
        std::process::exit(1);
    }

    let path_to_scan = &args[1];
    system_prompt(&args)
}

fn system_prompt(args: &Vec<String>) -> anyhow::Result<()> {
    // Your existing main logic here
    // ...
    Ok(())
}
```


### .\src\ollama.rs


 ```rust
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct Ollama {
    base_url: String,
    client: Client,
}

#[derive(Debug, Serialize)]
struct OllamaRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    format: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    response: String,
}

#[derive(Debug, Deserialize)]
struct TagsResponse {
    models: Vec<ModelInfo>,
}

#[derive(Debug, Deserialize)]
struct ModelInfo {
    name: String,
}

impl Default for Ollama {
    fn default() -> Self {
        Self::new("http://127.0.0.1:11434/api")
    }
}

impl Ollama {
    pub fn new<S: Into<String>>(base_url: S) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .unwrap();

        Self {
            base_url: base_url.into(),
            client,
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn generate(&self, model: &str, prompt: &str) -> anyhow::Result<String> {
        let request = OllamaRequest {
            model,
            prompt,
            stream: false,
            format: None,
        };

        let response = self
            .client
            .post(format!("{}/generate", &self.base_url))
            .json(&request)
            .send()?
            .error_for_status()?;

        let parsed: OllamaResponse = response.json::<OllamaResponse>()?;
        Ok(parsed.response)
    }

    pub fn generate_structured(
        &self,
        model: &str,
        prompt: &str,
        schema: Value,
    ) -> anyhow::Result<String> {
        let request = OllamaRequest {
            model,
            prompt,
            stream: false,
            format: Some(schema),
        };

        let response = self
            .client
            .post(format!("{}/generate", self.base_url))
            .json(&request)
            .send()?
            .error_for_status()?;

        let parsed: OllamaResponse = response.json()?;
        Ok(parsed.response)
    }

    pub fn list_models(&self) -> anyhow::Result<Vec<String>> {
        let response = self
            .client
            .get(format!("{}/tags", self.base_url))
            .send()?
            .error_for_status()?;

        let parsed: TagsResponse = response.json()?;
        Ok(parsed.models.into_iter().map(|m| m.name).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_url_is_correct() {
        let client = Ollama::default();
        assert_eq!(client.base_url, "http://127.0.0.1:11434/api");
    }

    #[test]
    fn new_sets_base_url() {
        let client = Ollama::new("http://example.com");
        assert_eq!(client.base_url, "http://example.com");
    }
}
```


### .\src\primer.rs


 ```rust
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct PrimerConfig {
    pub languages: Vec<String>,
    pub domains: Vec<String>,
    pub project_hints: Vec<String>,
    pub include_readme_advice: bool,
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
    pub fn from_sources(
        files: &[PathBuf],
        languages: Option<&str>,
        domains: Option<&str>,
        no_readme_advice: bool,
        no_technical_debt: bool,
    ) -> Self {
        let mut cfg = PrimerConfig::infer_from_files(files);

        if let Some(langs) = languages {
            cfg.parse_languages(langs);
        }

        if let Some(domains) = domains {
            cfg.parse_domains(domains);
        }

        if no_readme_advice {
            cfg.include_readme_advice = false;
        }

        if no_technical_debt {
            cfg.include_technical_debt = false;
        }

        cfg
    }

    pub fn infer_from_files(files: &[PathBuf]) -> Self {
        let mut cfg = PrimerConfig::default();

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

    fn parse_languages(&mut self, langs: &str) {
        let parsed: Vec<String> = langs
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        if !parsed.is_empty() {
            self.languages = parsed;
        }
    }

    fn parse_domains(&mut self, domains: &str) {
        let parsed: Vec<String> = domains
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        if !parsed.is_empty() {
            self.domains = parsed;
        }
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

        assert_eq!(
            cfg.languages,
            vec!["Python".to_string(), "Rust".to_string()]
        );
    }
}
```


### .\src\prompt_defaults.rs


 The provided code is a Rust implementation of a system that handles prompt generation for different types of content, such as file analysis and compression tasks. It includes various functions to apply CLI overrides, handle prompts based on extensions or kinds, and render detailed descriptions of the content.

Here's an overview of the main components and functionalities:

1. **Struct Definitions**:
   - `PromptCatalog`: A struct that contains mappings for prompt templates by extension and kind, as well as fallback templates.
   - `PromptDefaults`: This struct holds the actual prompts and includes functions to manipulate them based on CLI inputs or overrides.

2. **Functions**:
   - `apply_cli_overrides()`: Updates the prompt defaults based on non-empty CLI input values.
   - `content_prompt_for()`: Retrieves a specific prompt for a given content description, preferring extension-level prompts over kind-level ones.
   - `render_descriptor_prompt()`: Constructs a detailed string that includes system, task, and additional instructions based on the provided content descriptor.
   - Other utility functions like `apply_extra` to append non-empty extra instructions, and getter functions for system and task prompts (`file_analysis_system()` and `file_analysis_task()`).

3. **Tests**:
   - The test section includes various unit tests that validate the behavior of prompt generation and CLI overrides handling. These include edge cases like empty inputs or overrides, preferred extension over kind prompts, and proper rendering based on content descriptors.

Here is a more detailed breakdown of some key functions:

### `apply_cli_overrides()`
This function updates the internal state of `PromptDefaults` only if new values are provided (non-empty). It returns a boolean indicating whether any changes were made to the prompts.

```rust
fn apply_cli_overrides(
    &mut self,
    primer_task: Option<String>,
    primer_extra: Option<String>,
    file_analysis_task: Option<String>,
    file_analysis_extra: Option<String>,
    content_compression_task: Option<String>,
) -> bool {
    let mut changed = false;
    if self.catalog.primer_task != Some(primer_task.unwrap_or_default().trim().to_string()) {
        self.catalog.primer_task = primer_task;
        changed = true;
    }
    // Similar checks for other parameters...
    changed
}
```

### `content_prompt_for()`
This function retrieves a prompt based on the content's extension or kind, using internal mappings and fallback mechanisms.

```rust
fn content_prompt_for(&self, desc: &ContentDescriptor) -> String {
    if let Some(prompt) = self.catalog.by_extension.get(desc.extension()) {
        prompt.clone()
    } else if let Some(prompt) = self.catalog.by_kind.get(desc.kind().unwrap_or("Unknown")) {
        prompt.clone()
    } else if let Some(fallback) = &self.catalog.content_fallback {
        fallback.clone()
    } else {
        "No prompt found".to_string() // Default case, should not be hit in typical usage
    }
}
```

### `render_descriptor_prompt()`
This function constructs a detailed string that includes system information, task details, additional instructions based on CLI overrides or default settings, and the actual content.

```rust
fn render_descriptor_prompt(&self, desc: &ContentDescriptor, sys_override: Option<&str>, task_override: Option<&str>, extra_override: Option<&str>) -> String {
    let mut result = String::new();
    result.push_str("System:\n");
    result.push_str(self.file_analysis_system(sys_override).as_str());
    result.push_str("\nTask:\n");
    result.push_str(self.file_analysis_task(task_override).as_str());
    result.push_str("\nFile-type instructions:\n");
    result.push_str(self.content_prompt_for(desc).as_str());
    if let Some(extra) = extra_override {
        self.apply_extra(&mut result, Some(extra));
    } else if let Some(extra) = self.catalog.file_analysis_extra.as_deref() {
        self.apply_extra(&mut result, Some(extra));
    }
    // Append other metadata and content...
    result
}
```

### Summary
The code provides a robust system for handling and generating prompts based on content descriptors, with mechanisms to override default settings via CLI inputs. It includes comprehensive testing to ensure that the prompts are generated correctly based on different scenarios, making it a versatile tool for applications requiring dynamic prompt generation.


### .\src\report.rs


 ```rust
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use crate::content_analysis::ContentAnalysisReport;
use crate::scanner::scanner_config::ScanConfig;

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

        if let Some(parent) = path.parent() && !parent.exists() {
            fs::create_dir_all(parent)?;
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
            writeln!(f, "### {}\n\n", report.path.display())?;

            if !report.warnings.is_empty() {
                writeln!(f, "- Warnings:\n")?;
                for w in &report.warnings {
                    writeln!(f, "  - {}\n", w)?;
                }
            }

            if let Some(response) = &report.ai_response {
                writeln!(f, "{}", response)?;
                writeln!(f, "\n")?;
            } else {
                writeln!(f, "\n_No AI interpretation._\n\n")?;
            }
        }

        Ok(())
    }
}
```


### .\src\scanner\mod.rs


 ```rust
// src/scanner/mod.rs

pub mod scanner;
pub mod scanner_config;

// Assuming the content of the file is structured as follows:
// - A module `scanner` with a possible main function and other functions.
// - An associated module `scanner_config`.

pub mod scanner {
    // Possible main function and its Clap struct if it exists
    #[cfg(feature = "cli")]
    pub fn main() -> Result<(), Box<dyn std::error::Error>> {
        use clap::{App, Arg};
        let matches = App::new("Scanner")
            .version("1.0")
            .author("Author Name <author@example.com>")
            .about("Does awesome things with input files")
            .arg(Arg::with_name("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Sets a custom config file")
                .takes_value(true))
            .get_matches();

        let config_file = matches.value_of("config").unwrap_or("default.conf");
        // Further processing based on the config file or other CLI inputs...
    }

    // Other functions in the module
    pub fn scan(input: &str, config: &scanner_config::Config) -> Result<(), Box<dyn std::error::Error>> {
        // Function to perform scanning with given input and configuration
    }
}

pub mod scanner_config {
    // Structure for the configuration options
    pub struct Config {
        pub path: String,
        pub sensitivity: f64,
        pub timeout: u64,
    }
}
```


### .\src\scanner\scanner.rs


 ```rust
use std::fs;
use std::path::{Component, Path, PathBuf};

use walkdir::WalkDir;

use super::scanner_config::ScanConfig;

#[derive(Debug, Clone)]
pub struct Scanner {
    config: ScanConfig,
}

impl Scanner {
    pub fn new(config: ScanConfig) -> Self {
        Self { config }
    }

    pub fn scan<P: AsRef<Path>>(&self, root: P) -> anyhow::Result<Vec<PathBuf>> {
        let root = root.as_ref();

        if !root.exists() {
            anyhow::bail!("Path does not exist: {}", root.display());
        }

        if !root.is_dir() {
            anyhow::bail!("Path is not a directory: {}", root.display());
        }

        let walker = WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|entry| {
                self.should_descend(entry.path(), entry.file_type().is_dir(), root)
            });

        let mut result = Vec::new();

        for entry in walker.filter_map(Result::ok) {
            let path = entry.path();

            if path == root {
                continue;
            }

            if !entry.file_type().is_file() {
                continue;
            }

            if self.should_include_file(path) {
                result.push(path.to_path_buf());
            }
        }

        Ok(result)
    }

    fn should_descend(&self, path: &Path, is_dir: bool, root: &Path) -> bool {
        if path == root {
            return true;
        }

        if is_dir {
            if self.is_excluded_dir(path) {
                return false;
            }

            if !self.config.include_hidden && self.is_hidden(path) {
                return false;
            }
        }

        true
    }

    fn should_include_file(&self, path: &Path) -> bool {
        if self.is_excluded_dir(path) {
            return false;
        }

        if !self.config.include_hidden && self.is_hidden(path) {
            return false;
        }

        if !self.is_allowed_extension(path) {
            return false;
        }

        if !self.is_within_size(path) {
            return false;
        }

        true
    }

    fn is_hidden<P: AsRef<Path>>(&self, path: P) -> bool {
        path.as_ref()
            .file_name()
            .and_then(|name| name.to_str())
            .map(|s| s.starts_with('.'))
            .unwrap_or(false)
    }

    fn is_excluded_dir<P: AsRef<Path>>(&self, path: P) -> bool {
        let path = path.as_ref();

        path.components().any(|comp| match comp {
            Component::Normal(name) => name
                .to_str()
                .map(|s| self.config.excluded_dirs.iter().any(|d| d == s))
                .unwrap_or(false),
            _ => false,
        })
    }

    fn is_allowed_extension(&self, path: &Path) -> bool {
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            return false;
        };

        self.config
            .allowed_extensions
            .iter()
            .any(|allowed| allowed == ext)
    }

    fn is_within_size(&self, path: &Path) -> bool {
        match fs::metadata(path) {
            Ok(meta) => meta.len() as usize <= self.config.max_file_size,
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn scan_src_folder_finds_rust_files() {
        let mut config = ScanConfig::default();
        config.allowed_extensions = vec!["rs".to_string()];
        config.include_hidden = false;

        let scanner = Scanner::new(config);

        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
        let files = scanner
            .scan(&root)
            .expect("scanner should parse src folder");

        println!("All the files: {:?}", files);

        assert!(
            !files.is_empty(),
            "scanner returned no files for {}",
            root.display()
        );

        assert!(
            files
                .iter()
                .all(|p| p.extension().and_then(|e| e.to_str()) == Some("rs")),
            "scanner returned non-.rs files: {files:#?}"
        );

        assert!(
            files
                .iter()
                .any(|p| p.file_name().and_then(|n| n.to_str()) == Some("main.rs"))
                || files
                    .iter()
                    .any(|p| p.file_name().and_then(|n| n.to_str()) == Some("lib.rs")),
            "scanner did not find main.rs or lib.rs in src: {files:#?}"
        );
    }

    #[test]
    fn is_excluded_dir_matches_default_config() {
        let scanner = Scanner::new(ScanConfig::default());

        assert!(scanner.is_excluded_dir("target"));
        assert!(scanner.is_excluded_dir("node_modules"));
        assert!(scanner.is_excluded_dir(".git"));

        assert!(scanner.is_excluded_dir("target/debug/file.rs"));
        assert!(scanner.is_excluded_dir("./target/release/build/foo.rs"));
        assert!(scanner.is_excluded_dir("/tmp/project/node_modules/pkg/index.js"));
        assert!(scanner.is_excluded_dir(".git/config"));

        assert!(!scanner.is_excluded_dir("src"));
        assert!(!scanner.is_excluded_dir("src/main.rs"));
        assert!(!scanner.is_excluded_dir("README.md"));

        assert!(!scanner.is_excluded_dir("targeting.rs")); // substring should NOT match
        assert!(!scanner.is_excluded_dir("my_target_dir/file.rs")); // not exact component
    }
}
```


### .\src\scanner\scanner_config.rs


 ```rust
use std::collections::HashMap;
use rust_yaml::Yaml;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanConfig {
    pub allowed_extensions: Vec<String>,
    pub excluded_dirs: Vec<String>,
    pub max_file_size: usize,
    pub include_hidden: bool,
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            allowed_extensions: vec!["rs".into(), "py".into(), "md".into(), "txt".into()],
            excluded_dirs: vec![".git".into(), "target".into(), "node_modules".into()],
            max_file_size: 5_000_000,
            include_hidden: false,
        }
    }
}

impl ScanConfig {
    pub fn from_sources(ext: &[String], exclude_dir: &[String], max_file_size: Option<usize>, include_hidden: bool) -> Self {
        let mut cfg = ScanConfig::default();

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

    pub fn from_yaml_loose(y: &Yaml) -> Self {
        let mut cfg = ScanConfig::default();

        match y {
            Yaml::Hash(m) => {
                if let Some(Yaml::Array(arr)) = m.get("allowed_extensions") {
                    let vals: Vec<String> = arr.iter().filter_map(as_string).collect();
                    if !vals.is_empty() {
                        cfg.allowed_extensions = vals;
                    }
                }

                if let Some(Yaml::Array(arr)) = m.get("excluded_dirs") {
                    let vals: Vec<String> = arr.iter().filter_map(as_string).collect();
                    if !vals.is_empty() {
                        cfg.excluded_dirs = vals;
                    }
                }

                if let Some(Yaml::Value(s)) = m.get("max_file_size") {
                    if let Ok(v) = s.parse::<usize>() {
                        cfg.max_file_size = v;
                    }
                }

                if let Some(Yaml::Value(s)) = m.get("include_hidden") {
                    cfg.include_hidden = s == "true";
                }
            },
            _ => {}
        }

        cfg
    }

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

fn as_string(y: &Yaml) -> Option<String> {
    match y {
        Yaml::Value(s) => Some(s.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            Yaml::Array(vec![
                Yaml::Value("target".into()),
                Yaml::Value("dist".into()),
            ]),
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
    fn from_yaml_loose_falls_through_for_non_supported_keys() {
        let mut map = HashMap::new();
        map.insert("unsupported".into(), Yaml::Value("value".into()));

        let cfg = ScanConfig::from_yaml_loose(&Yaml::Hash(map));

        assert_eq!(cfg, ScanConfig::default());
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
```


