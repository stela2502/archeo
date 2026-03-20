//! Prompt catalog management for Archeo.
//!
//! This module is the single source of truth for all prompt text used by
//! content analysis. It is responsible for:
//!
//! - loading a prompt catalog from disk,
//! - creating a default catalog when none exists,
//! - resolving project-level primer prompts,
//! - resolving file-analysis prompts,
//! - resolving file-type-specific prompts by extension or kind,
//! - validating that descriptor kinds are covered,
//! - applying CLI overrides,
//! - and writing either the active or built-in catalog back to disk.
//!
//! Architectural intent:
//!
//! - `ContentConfig` decides **how a file is read**
//! - `ContentDescriptor` decides **what kind of file it is**
//! - `PromptDefaults` decides **what prompt text applies**
//!
//! This keeps prompt ownership centralized and avoids duplicated or drifting
//! prompt logic in other parts of the content-analysis subsystem.

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::content_analysis::descriptor::{ContentDescriptor, ContentKind};

/// Serializable prompt catalog stored as YAML.
///
/// All fields are optional so that partially specified user catalogs can fall
/// back to built-in defaults.
///
/// The catalog is split into three prompt layers:
///
/// - **project primer**: used for folder/project reconstruction,
/// - **file analysis**: used when analyzing an individual file,
/// - **content compression**: used when compressing file-level analyses into a
///   compact index or summary representation.
///
/// File-specific prompting is resolved through:
///
/// - `by_extension` first,
/// - then `by_kind`,
/// - then `content_fallback`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PromptCatalog {
    /// System prompt for the project-level primer stage.
    pub primer_system: Option<String>,

    /// Task prompt for the project-level primer stage.
    pub primer_task: Option<String>,

    /// Optional extra instructions appended to the project-level primer stage.
    pub primer_extra: Option<String>,

    /// System prompt for individual file analysis.
    pub file_analysis_system: Option<String>,

    /// Task prompt for individual file analysis.
    pub file_analysis_task: Option<String>,

    /// Optional extra instructions appended to individual file analysis.
    pub file_analysis_extra: Option<String>,

    /// Task prompt used to compress file-level analyses into a compact index.
    pub content_compression_task: Option<String>,

    /// Fallback file-type-specific prompt used when no extension- or
    /// kind-specific prompt exists.
    pub content_fallback: Option<String>,

    /// Prompts keyed by lowercase file extension, e.g. `"rs"` or `"py"`.
    #[serde(default)]
    pub by_extension: BTreeMap<String, String>,

    /// Prompts keyed by content kind, e.g. `"text"`, `"table"`, `"notebook"`.
    #[serde(default)]
    pub by_kind: BTreeMap<String, String>,
}

/// Loaded prompt defaults together with source metadata.
///
/// This wraps a [`PromptCatalog`] and tracks:
///
/// - the path from which the catalog was loaded,
/// - whether that file had to be created from built-in defaults.
#[derive(Debug, Clone)]
pub struct PromptDefaults {
    /// Path to the active catalog file on disk.
    pub path: PathBuf,

    /// Active prompt catalog.
    pub catalog: PromptCatalog,

    /// Whether the catalog file was created during loading.
    pub was_created: bool,
}

impl PromptDefaults {
    /// Load a prompt catalog from disk, creating a default one if necessary.
    ///
    /// Path resolution rules:
    ///
    /// - if `path` is `None`, the default global prompt file is used;
    /// - if `path` is absolute, it is used as-is;
    /// - if `path` contains multiple path components, it is used as-is;
    /// - otherwise it is treated as relative to the global Archeo config
    ///   directory.
    pub fn new(path: Option<&str>) -> Result<Self> {
        let path = Self::resolve_catalog_path(path)?;

        let was_created = if !path.exists() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).with_context(|| {
                    format!("Failed to create prompt directory {}", parent.display())
                })?;
            }

            let yaml = serde_yaml::to_string(&Self::default_catalog())
                .context("Failed to serialize default prompt catalog")?;

            fs::write(&path, yaml).with_context(|| {
                format!("Failed to write default prompt catalog to {}", path.display())
            })?;

            true
        } else {
            false
        };

        let text = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read prompt catalog {}", path.display()))?;

        let catalog: PromptCatalog = serde_yaml::from_str(&text)
            .with_context(|| format!("Failed to parse prompt catalog {}", path.display()))?;

        let loaded = Self {
            path,
            catalog,
            was_created,
        };

        loaded
            .validate_internal_coverage()
            .context("prompt catalog failed internal coverage validation")?;

        Ok(loaded)
    }

    /// Apply non-empty CLI overrides into the loaded catalog.
    ///
    /// Returns `true` if at least one catalog value changed.
    pub fn apply_cli_overrides(
        &mut self,
        primer_task: Option<&str>,
        primer_extra: Option<&str>,
        file_analysis_task: Option<&str>,
        file_analysis_extra: Option<&str>,
        content_compression_task: Option<&str>,
    ) -> bool {
        let mut changed = false;

        if let Some(task) = primer_task.map(str::trim).filter(|s| !s.is_empty()) {
            if self.catalog.primer_task.as_deref() != Some(task) {
                self.catalog.primer_task = Some(task.to_string());
                changed = true;
            }
        }

        if let Some(extra) = primer_extra.map(str::trim).filter(|s| !s.is_empty()) {
            if self.catalog.primer_extra.as_deref() != Some(extra) {
                self.catalog.primer_extra = Some(extra.to_string());
                changed = true;
            }
        }

        if let Some(task) = file_analysis_task.map(str::trim).filter(|s| !s.is_empty()) {
            if self.catalog.file_analysis_task.as_deref() != Some(task) {
                self.catalog.file_analysis_task = Some(task.to_string());
                changed = true;
            }
        }

        if let Some(extra) = file_analysis_extra.map(str::trim).filter(|s| !s.is_empty()) {
            if self.catalog.file_analysis_extra.as_deref() != Some(extra) {
                self.catalog.file_analysis_extra = Some(extra.to_string());
                changed = true;
            }
        }

        if let Some(task) = content_compression_task
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            if self.catalog.content_compression_task.as_deref() != Some(task) {
                self.catalog.content_compression_task = Some(task.to_string());
                changed = true;
            }
        }

        changed
    }

    /// parse command line option "content_primers(s)" as defined in main
    pub fn apply_content_primer_rules(&mut self, rules: &[String]) -> bool {
        let changed = rules.len() > 0 ;

        for rule in rules {
            if let Some((ext, text)) = Self::parse_prompt_rule(rule) {
                self.catalog.by_extension.insert(ext, text);
            }
        }
        changed
    }

    /// parse command line option "kind_primer(s)" as defined in main
    pub fn apply_kind_primer_rules(&mut self, rules: &[String]) -> bool {
        let changed = rules.len() > 0 ;
        for rule in rules {
            if let Some((kind, text)) = Self::parse_prompt_rule(rule) {
                self.catalog.by_kind.insert(kind, text);
            }
        }
        changed
    }


    fn parse_prompt_rule(input: &str) -> Option<(String, String)> {
        let (left, right) = input.split_once('=')?;
        let key = left.trim().trim_start_matches('.').to_string();
        let value = right.trim().to_string();

        if key.is_empty() || value.is_empty() {
            return None;
        }

        Some((key, value))
    }

    /// Resolve the catalog file path from an optional user-provided value.
    fn resolve_catalog_path(path: Option<&str>) -> Result<PathBuf> {
        match path {
            Some(raw) => {
                let candidate = PathBuf::from(raw);

                if candidate.is_absolute() || candidate.components().count() > 1 {
                    Ok(candidate)
                } else {
                    Ok(Self::global_config_dir()?
                        .unwrap_or_else(|| PathBuf::from("."))
                        .join(raw))
                }
            }
            None => Self::default_global_prompt_file(),
        }
    }

    /// Return the built-in default prompt catalog.
    pub fn default_catalog() -> PromptCatalog {
        let mut by_extension = BTreeMap::new();

        by_extension.insert(
            "rs".to_string(),
            "Explain the file's role in the Rust project. Identify key structs, enums, traits, impl blocks, functions, and modules. Summarize what the central items do and how they likely interact with other parts of the codebase, so that file-level reports can later be merged into an understanding of the project's internal logic and architecture. Stay evidence-based and avoid speculation."
                .to_string(),
        );

        by_extension.insert(
            "py".to_string(),
            "Explain whether this Python file is a script, library module, notebook-export, pipeline helper, or utility. Identify inputs, outputs, dependencies, and its likely role in the project."
                .to_string(),
        );

        by_extension.insert(
            "r".to_string(),
            "Explain the statistical or analytical purpose of this R file. Identify likely inputs, outputs, packages, and whether it appears exploratory or reusable."
                .to_string(),
        );

        by_extension.insert(
            "ipynb".to_string(),
            "Summarize this notebook as a paper-quality Methods section. Identify the likely data inputs, processing steps, transformations, and generated outputs (plots/tables). Describe the analysis workflow clearly and in order, and indicate whether the notebook appears exploratory, demonstrative, or report-like. Be precise and avoid speculation."
                .to_string(),
        );

        by_extension.insert(
            "md".to_string(),
            "Determine whether this markdown file is documentation, notes, report output, analysis narrative, changelog, or planning material. Summarize its role in the project."
                .to_string(),
        );

        by_extension.insert(
            "txt".to_string(),
            "First classify the text file if possible: notes, report, log, config-like text, data dictionary, or unknown. Then summarize cautiously."
                .to_string(),
        );

        by_extension.insert(
            "csv".to_string(),
            "Infer the likely role of this table. Mention likely identifier columns, measured values, and whether it looks like raw input, metadata, or derived results."
                .to_string(),
        );

        by_extension.insert(
            "tsv".to_string(),
            "Infer the likely role of this table. Mention likely identifier columns, measured values, and whether it looks like raw input, metadata, or derived results."
                .to_string(),
        );

        by_extension.insert(
            "yaml".to_string(),
            "Explain the likely role of this YAML file in the project: configuration, pipeline definition, metadata, schema, or other structured settings."
                .to_string(),
        );

        by_extension.insert(
            "yml".to_string(),
            "Explain the likely role of this YAML file in the project: configuration, pipeline definition, metadata, schema, or other structured settings."
                .to_string(),
        );

        by_extension.insert(
            "toml".to_string(),
            "Explain the likely role of this TOML file in the project: package metadata, configuration, build settings, or application settings."
                .to_string(),
        );

        by_extension.insert(
            "json".to_string(),
            "Explain the likely role of this JSON file in the project: configuration, metadata, schema, cached output, or structured results."
                .to_string(),
        );

        by_extension.insert(
            "sh".to_string(),
            "Explain whether this shell script is an entrypoint, setup helper, pipeline wrapper, maintenance script, or deployment utility. Mention inputs, outputs, and side effects if visible."
                .to_string(),
        );

        by_extension.insert(
            "bash".to_string(),
            "Explain whether this shell script is an entrypoint, setup helper, pipeline wrapper, maintenance script, or deployment utility. Mention inputs, outputs, and side effects if visible."
                .to_string(),
        );

        by_extension.insert(
            "zsh".to_string(),
            "Explain whether this shell script is an entrypoint, setup helper, pipeline wrapper, maintenance script, or deployment utility. Mention inputs, outputs, and side effects if visible."
                .to_string(),
        );

        let mut by_kind = BTreeMap::new();

        by_kind.insert(
            "text".to_string(),
            "Classify this text file by likely role first, then summarize its project function cautiously and concretely."
                .to_string(),
        );

        by_kind.insert(
            "table".to_string(),
            "Summarize this structured table by likely semantic role, probable key columns, and whether it looks like raw input, metadata, intermediate output, or final results."
                .to_string(),
        );

        by_kind.insert(
            "notebook".to_string(),
            "Summarize this notebook as an analysis workflow, including likely inputs, steps, outputs, and whether it appears exploratory or presentation-oriented."
                .to_string(),
        );

        by_kind.insert(
            "code".to_string(),
            "Explain the file's likely role in the software project, identify major definitions, and summarize how it appears to participate in the codebase."
                .to_string(),
        );

        by_kind.insert(
            "config".to_string(),
            "Explain the likely role of this configuration file and what part of the project it seems to control."
                .to_string(),
        );

        by_kind.insert(
            "data".to_string(),
            "Explain what kind of data this appears to contain and whether it looks like input, metadata, intermediate state, or output."
                .to_string(),
        );

        PromptCatalog {
            primer_system: Some(
                "You are Archeo, an AI system for reconstructing the purpose, structure, and history of old research and software folders.\n\nYour job is not to praise the folder. Your job is to infer what was built, what likely mattered, what is incomplete, and what should be documented.\n\nYou must base your analysis ONLY on the provided evidence.\n- Do NOT infer meaning from project names alone.\n- Do NOT assume standard patterns unless clearly supported by evidence.\n- Do NOT invent functionality.\n- If something is unclear, say so explicitly.\n- Do NOT ask the user for clarification.\n\nReasoning rules:\n- Prefer concrete observations over speculation.\n- Separate direct evidence from interpretation.\n- Use cautious language.\n- Avoid confident statements without supporting evidence.\n"
                    .to_string(),
            ),
            primer_task: Some(
                "Reconstruct what this folder is about, identify likely main components, infer the likely workflow or development goal, and highlight the files that appear central."
                    .to_string(),
            ),
            primer_extra: None,
            file_analysis_system: Some(
                "You analyze one file at a time and describe its likely role based only on the provided metadata and content. Do not invent hidden behavior, unseen dependencies, or undocumented intent. Be explicit when uncertain."
                    .to_string(),
            ),
            file_analysis_task: Some(
                "Analyze this file and summarize its likely role in the project. Focus on purpose, structure, and likely relation to surrounding project components."
                    .to_string(),
            ),
            file_analysis_extra: None,
            content_compression_task: Some(
                "You are converting file-level analysis into a compact index.\n\nYour task:\n- produce exactly ONE line per file\n- DO NOT summarize across files\n- DO NOT merge files\n- DO NOT group files\n- DO NOT write paragraphs\n- DO NOT explain anything\n- DO NOT add headers or sections\n\nEach line must follow exactly:\n<full file path> -> <short description>\n\nRules:\n- max 15 words per description\n- describe the file's role in the project\n- avoid speculation\n- if unclear, write 'unclear purpose'\n\nReturn ONLY the lines.\nWhen identifying problems or gaps, prefer documentation issues, testing gaps, unfinished work, and structural inconsistencies. Do not raise security concerns unless there is direct evidence in the provided files or analysis.\n"
                    .to_string(),
            ),
            content_fallback: Some(
                "Explain the file's likely role in the project based only on the provided content and metadata. Be concrete, brief, and avoid speculation."
                    .to_string(),
            ),
            by_extension,
            by_kind,
        }
    }

    /// Return all currently defined content kinds.
    ///
    /// This must stay aligned with the `ContentKind` enum.
    fn all_content_kinds() -> &'static [ContentKind] {
        const ALL: &[ContentKind] = &[
            ContentKind::Text,
            ContentKind::Table,
            ContentKind::Notebook,
            ContentKind::Code,
            ContentKind::Config,
            ContentKind::Data,
            ContentKind::Unknown,
        ];
        ALL
    }

    /// Validate that the prompt layer covers the current descriptor model.
    ///
    /// This verifies:
    ///
    /// - required global prompts are resolvable and non-empty,
    /// - every known `ContentKind` is covered either by an explicit `by_kind`
    ///   entry or by a resolvable non-empty fallback.
    ///
    /// Extensions are intentionally not required to be fully covered.
    pub fn validate_internal_coverage(&self) -> Result<()> {
        let mut problems = Vec::new();
        let defaults = Self::default_catalog();

        if self.primer_system(None).trim().is_empty() {
            problems.push("primer_system resolves to an empty string".to_string());
        }

        if self.primer_task(None).trim().is_empty() {
            problems.push("primer_task resolves to an empty string".to_string());
        }

        if self.file_analysis_system(None).trim().is_empty() {
            problems.push("file_analysis_system resolves to an empty string".to_string());
        }

        if self.file_analysis_task(None).trim().is_empty() {
            problems.push("file_analysis_task resolves to an empty string".to_string());
        }

        if self.content_compression_task(None).trim().is_empty() {
            problems.push("content_compression_task resolves to an empty string".to_string());
        }

        let fallback_exists = self
            .catalog
            .content_fallback
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .is_some()
            || defaults
                .content_fallback
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .is_some();

        for kind in Self::all_content_kinds() {
            let key = kind.as_str();

            let covered_in_active = self
                .catalog
                .by_kind
                .get(key)
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false);

            let covered_in_defaults = defaults
                .by_kind
                .get(key)
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false);

            if !(covered_in_active || covered_in_defaults || fallback_exists) {
                problems.push(format!(
                    "descriptor kind '{key}' is not covered by by_kind or fallback"
                ));
            }
        }

        if problems.is_empty() {
            Ok(())
        } else {
            anyhow::bail!(problems.join("; "));
        }
    }

    /// Return the primer system prompt, honoring an explicit override first.
    pub fn primer_system(&self, override_value: Option<&str>) -> String {
        override_value
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| self.catalog.primer_system.clone())
            .unwrap_or_else(|| {
                Self::default_catalog()
                    .primer_system
                    .expect("default primer_system must exist")
            })
    }

    /// Return the primer task prompt, honoring an explicit override first.
    pub fn primer_task(&self, override_value: Option<&str>) -> String {
        override_value
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| self.catalog.primer_task.clone())
            .unwrap_or_else(|| {
                Self::default_catalog()
                    .primer_task
                    .expect("default primer_task must exist")
            })
    }

    /// Return the file-analysis system prompt, honoring an explicit override first.
    pub fn file_analysis_system(&self, override_value: Option<&str>) -> String {
        override_value
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| self.catalog.file_analysis_system.clone())
            .unwrap_or_else(|| {
                Self::default_catalog()
                    .file_analysis_system
                    .expect("default file_analysis_system must exist")
            })
    }

    /// Return the file-analysis task prompt, honoring an explicit override first.
    pub fn file_analysis_task(&self, override_value: Option<&str>) -> String {
        override_value
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| self.catalog.file_analysis_task.clone())
            .unwrap_or_else(|| {
                Self::default_catalog()
                    .file_analysis_task
                    .expect("default file_analysis_task must exist")
            })
    }

    /// Return the content-compression task, honoring an explicit override first.
    pub fn content_compression_task(&self, override_value: Option<&str>) -> String {
        override_value
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| self.catalog.content_compression_task.clone())
            .unwrap_or_else(|| {
                Self::default_catalog()
                    .content_compression_task
                    .expect("default content_compression_task must exist")
            })
    }

    /// Resolve the best file-type-specific prompt for a descriptor.
    ///
    /// Resolution order:
    ///
    /// 1. active catalog by file extension,
    /// 2. active catalog by content kind,
    /// 3. active catalog fallback,
    /// 4. built-in defaults by file extension,
    /// 5. built-in defaults by content kind,
    /// 6. built-in fallback.
    pub fn content_prompt_for(&self, desc: &ContentDescriptor) -> String {
        let extension_key = desc.extension.to_lowercase();

        if let Some(p) = self.catalog.by_extension.get(&extension_key) {
            return p.clone();
        }

        let kind_key = desc.kind.as_str();
        if let Some(p) = self.catalog.by_kind.get(kind_key) {
            return p.clone();
        }

        if let Some(p) = &self.catalog.content_fallback {
            return p.clone();
        }

        let defaults = Self::default_catalog();

        if let Some(p) = defaults.by_extension.get(&extension_key) {
            return p.clone();
        }

        if let Some(p) = defaults.by_kind.get(kind_key) {
            return p.clone();
        }

        defaults
            .content_fallback
            .expect("default content_fallback must exist")
    }

    /// Render a complete file-analysis prompt for a content descriptor.
    ///
    /// This combines:
    ///
    /// - the global file-analysis system prompt,
    /// - the global file-analysis task,
    /// - the resolved file-type-specific prompt,
    /// - optional extra file-analysis instructions,
    /// - file metadata,
    /// - and the sampled or truncated content itself.
    pub fn render_descriptor_prompt(
        &self,
        desc: &ContentDescriptor,
        file_analysis_system_override: Option<&str>,
        file_analysis_task_override: Option<&str>,
        file_analysis_extra_override: Option<&str>,
    ) -> String {
        let mut out = String::new();

        out.push_str("System:\n");
        out.push_str(&self.file_analysis_system(file_analysis_system_override));
        out.push_str("\n\nTask:\n");
        out.push_str(&self.file_analysis_task(file_analysis_task_override));
        out.push_str("\n\nFile-type instructions:\n");
        out.push_str(&self.content_prompt_for(desc));

        let effective_extra = file_analysis_extra_override
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| self.catalog.file_analysis_extra.clone());

        Self::apply_extra(&mut out, effective_extra.as_deref());

        out.push_str("\nFile metadata:\n");
        out.push_str(&format!("- path: {}\n", desc.path.display()));
        out.push_str(&format!("- extension: {}\n", desc.extension));
        out.push_str(&format!("- kind: {}\n", desc.kind.as_str()));
        out.push_str(&format!("- parse_mode: {}\n", desc.parse_mode.as_str()));
        out.push_str(&format!("- file_size: {}\n", desc.file_size));
        out.push_str(&format!("- is_truncated: {}\n", desc.is_truncated));
        out.push_str(&format!("- is_sample: {}\n", desc.is_sample));

        if let Some(v) = desc.total_rows {
            out.push_str(&format!("- total_rows: {}\n", v));
        }
        if let Some(v) = desc.total_cols {
            out.push_str(&format!("- total_cols: {}\n", v));
        }
        if let Some(v) = desc.sampled_rows {
            out.push_str(&format!("- sampled_rows: {}\n", v));
        }
        if let Some(v) = desc.sampled_cols {
            out.push_str(&format!("- sampled_cols: {}\n", v));
        }

        out.push_str("\nContent:\n");
        out.push_str(&desc.content);

        out
    }

    /// Write the currently active catalog to `path` as YAML.
    pub fn write_used_catalog<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create parent directory for {}", path.display())
            })?;
        }

        let yaml = serde_yaml::to_string(&self.catalog)
            .context("Failed to serialize active prompt catalog")?;

        fs::write(path, yaml)
            .with_context(|| format!("Failed to write active prompt catalog to {}", path.display()))
    }

    /// Write the built-in default catalog to `path` as YAML.
    pub fn write_default_catalog<P: AsRef<Path>>(path: P) -> Result<()> {
        let path = path.as_ref();

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create parent directory for {}", path.display())
            })?;
        }

        let yaml = serde_yaml::to_string(&Self::default_catalog())
            .context("Failed to serialize default prompt catalog")?;

        fs::write(path, yaml)
            .with_context(|| format!("Failed to write default prompt catalog to {}", path.display()))
    }

    /// Append additional free-form instructions to an existing prompt string.
    ///
    /// Empty or whitespace-only input is ignored.
    pub fn apply_extra(base: &mut String, extra: Option<&str>) {
        if let Some(extra) = extra {
            let extra = extra.trim();
            if !extra.is_empty() {
                base.push_str("\n\nAdditional instructions:\n");
                base.push_str(extra);
                base.push('\n');
            }
        }
    }

    /// Return the default global prompt catalog file path.
    pub fn default_global_prompt_file() -> Result<PathBuf> {
        Ok(Self::global_config_dir()?
            .unwrap_or_else(|| PathBuf::from("."))
            .join("prompts.yml"))
    }

    /// Return the global Archeo config directory, if it can be determined.
    fn global_config_dir() -> Result<Option<PathBuf>> {
        let proj = ProjectDirs::from("org", "archeo", "archeo");
        Ok(proj.map(|p| p.config_dir().to_path_buf()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content_analysis::ParseMode;
    use std::path::PathBuf;

    fn sample_descriptor(extension: &str, kind: ContentKind) -> ContentDescriptor {
        ContentDescriptor {
            path: PathBuf::from(format!("example.{extension}")),
            extension: extension.to_string(),
            kind,
            parse_mode: ParseMode::Full,
            file_size: 123,
            is_truncated: false,
            is_sample: false,
            total_rows: Some(10),
            total_cols: Some(3),
            sampled_rows: Some(10),
            sampled_cols: Some(3),
            content: "fn main() {}".to_string(),
        }
    }

    fn defaults_with_catalog(catalog: PromptCatalog) -> PromptDefaults {
        PromptDefaults {
            path: PathBuf::from("prompts.yml"),
            catalog,
            was_created: false,
        }
    }

    #[test]
    fn default_catalog_contains_expected_global_file_analysis_prompts() {
        let catalog = PromptDefaults::default_catalog();

        assert!(catalog.file_analysis_system.is_some());
        assert!(catalog.file_analysis_task.is_some());
        assert!(catalog.content_compression_task.is_some());
    }

    #[test]
    fn default_catalog_contains_expected_rust_prompt() {
        let catalog = PromptDefaults::default_catalog();
        let prompt = catalog
            .by_extension
            .get("rs")
            .expect("default catalog should contain an rs prompt");

        assert!(prompt.contains("Rust project"));
    }

    #[test]
    fn validate_internal_coverage_accepts_default_catalog() {
        let defaults = defaults_with_catalog(PromptDefaults::default_catalog());
        assert!(defaults.validate_internal_coverage().is_ok());
    }

    #[test]
    fn validate_internal_coverage_accepts_missing_kind_entries_when_fallback_exists() {
        let defaults = defaults_with_catalog(PromptCatalog {
            by_extension: BTreeMap::new(),
            by_kind: BTreeMap::new(),
            content_fallback: Some("fallback prompt".to_string()),
            ..PromptCatalog::default()
        });

        assert!(defaults.validate_internal_coverage().is_ok());
    }

    #[test]
    fn validate_internal_coverage_rejects_empty_required_global_prompts() {
        let defaults = defaults_with_catalog(PromptCatalog {
            primer_system: Some("   ".to_string()),
            primer_task: Some("".to_string()),
            file_analysis_system: Some("".to_string()),
            file_analysis_task: Some("".to_string()),
            content_compression_task: Some("".to_string()),
            content_fallback: Some("fallback prompt".to_string()),
            by_extension: BTreeMap::new(),
            by_kind: BTreeMap::new(),
            ..PromptCatalog::default()
        });

        let err = defaults
            .validate_internal_coverage()
            .expect_err("coverage validation should fail")
            .to_string();

        assert!(err.contains("primer_system"));
        assert!(err.contains("primer_task"));
        assert!(err.contains("file_analysis_system"));
        assert!(err.contains("file_analysis_task"));
        assert!(err.contains("content_compression_task"));
    }

    /*
    //useless AI test
    #[test]
    fn validate_internal_coverage_rejects_missing_kind_coverage_without_fallback() {
        let defaults = defaults_with_catalog(PromptCatalog {
            by_extension: BTreeMap::new(),
            by_kind: BTreeMap::new(),
            content_fallback: Some("".to_string()),
            primer_system: Some("primer system".to_string()),
            primer_task: Some("primer task".to_string()),
            file_analysis_system: Some("file analysis system".to_string()),
            file_analysis_task: Some("file analysis task".to_string()),
            content_compression_task: Some("compression task".to_string()),
            ..PromptCatalog::default()
        });

        let err = defaults
            .validate_internal_coverage()
            .expect_err("coverage validation should fail")
            .to_string();

        assert!(err.contains("descriptor kind 'text'"));
        assert!(err.contains("descriptor kind 'table'"));
        assert!(err.contains("descriptor kind 'notebook'"));
        assert!(err.contains("descriptor kind 'code'"));
        assert!(err.contains("descriptor kind 'config'"));
        assert!(err.contains("descriptor kind 'data'"));
        assert!(err.contains("descriptor kind 'UNKNOWN'"));
    }*/

    #[test]
    fn apply_cli_overrides_updates_only_non_empty_values() {
        let mut defaults = defaults_with_catalog(PromptDefaults::default_catalog());

        let changed = defaults.apply_cli_overrides(
            Some("new primer task"),
            Some("   "),
            Some("new file analysis task"),
            None,
            Some("new compression task"),
        );

        assert!(changed);
        assert_eq!(defaults.catalog.primer_task.as_deref(), Some("new primer task"));
        assert_eq!(
            defaults.catalog.file_analysis_task.as_deref(),
            Some("new file analysis task")
        );
        assert_eq!(
            defaults.catalog.content_compression_task.as_deref(),
            Some("new compression task")
        );
        assert_eq!(defaults.catalog.primer_extra, None);
    }

    #[test]
    fn apply_cli_overrides_reports_unchanged_when_nothing_changes() {
        let mut defaults = defaults_with_catalog(PromptDefaults::default_catalog());

        let changed = defaults.apply_cli_overrides(None, Some("   "), None, Some(""), None);

        assert!(!changed);
    }

    #[test]
    fn file_analysis_getters_prefer_non_empty_override() {
        let defaults = defaults_with_catalog(PromptDefaults::default_catalog());

        assert_eq!(
            defaults.file_analysis_system(Some("override system")),
            "override system"
        );
        assert_eq!(
            defaults.file_analysis_task(Some("override task")),
            "override task"
        );
        assert_eq!(
            defaults.content_compression_task(Some("override compression")),
            "override compression"
        );
    }

    #[test]
    fn content_prompt_for_prefers_extension_prompt_over_kind_prompt() {
        let defaults = defaults_with_catalog(PromptCatalog {
            by_extension: BTreeMap::from([(
                "rs".to_string(),
                "extension-level rust prompt".to_string(),
            )]),
            by_kind: BTreeMap::from([("code".to_string(), "kind-level code prompt".to_string())]),
            content_fallback: Some("fallback prompt".to_string()),
            ..PromptCatalog::default()
        });

        let desc = sample_descriptor("rs", ContentKind::Code);
        let prompt = defaults.content_prompt_for(&desc);

        assert_eq!(prompt, "extension-level rust prompt");
    }

    #[test]
    fn content_prompt_for_uses_kind_prompt_when_extension_missing() {
        let defaults = defaults_with_catalog(PromptCatalog {
            by_extension: BTreeMap::new(),
            by_kind: BTreeMap::from([("code".to_string(), "kind-level code prompt".to_string())]),
            content_fallback: Some("fallback prompt".to_string()),
            ..PromptCatalog::default()
        });

        let desc = sample_descriptor("unknown", ContentKind::Code);
        let prompt = defaults.content_prompt_for(&desc);

        assert_eq!(prompt, "kind-level code prompt");
    }

    #[test]
    fn content_prompt_for_uses_catalog_fallback_when_extension_and_kind_are_missing() {
        let defaults = defaults_with_catalog(PromptCatalog {
            by_extension: BTreeMap::new(),
            by_kind: BTreeMap::new(),
            content_fallback: Some("fallback prompt".to_string()),
            ..PromptCatalog::default()
        });

        let desc = sample_descriptor("unknown", ContentKind::Unknown);
        let prompt = defaults.content_prompt_for(&desc);

        assert_eq!(prompt, "fallback prompt");
    }

    #[test]
    fn apply_extra_appends_section_when_non_empty() {
        let mut base = "base".to_string();
        PromptDefaults::apply_extra(&mut base, Some("extra instructions"));

        assert!(base.contains("Additional instructions:"));
        assert!(base.contains("extra instructions"));
    }

    #[test]
    fn apply_extra_ignores_empty_input() {
        let mut base = "base".to_string();
        PromptDefaults::apply_extra(&mut base, Some("   "));

        assert_eq!(base, "base");
    }

    #[test]
    fn render_descriptor_prompt_includes_global_task_specific_prompt_metadata_and_content() {
        let defaults = defaults_with_catalog(PromptCatalog {
            file_analysis_system: Some("GLOBAL SYSTEM".to_string()),
            file_analysis_task: Some("GLOBAL TASK".to_string()),
            file_analysis_extra: Some("GLOBAL EXTRA".to_string()),
            by_extension: BTreeMap::from([(
                "rs".to_string(),
                "RUST SPECIFIC INSTRUCTION".to_string(),
            )]),
            content_fallback: Some("FALLBACK".to_string()),
            ..PromptCatalog::default()
        });

        let desc = sample_descriptor("rs", ContentKind::Code);
        let rendered = defaults.render_descriptor_prompt(&desc, None, None, None);

        assert!(rendered.contains("System:\nGLOBAL SYSTEM"));
        assert!(rendered.contains("Task:\nGLOBAL TASK"));
        assert!(rendered.contains("File-type instructions:\nRUST SPECIFIC INSTRUCTION"));
        assert!(rendered.contains("Additional instructions:\nGLOBAL EXTRA"));
        assert!(rendered.contains("File metadata:"));
        assert!(rendered.contains("- extension: rs"));
        assert!(rendered.contains("- kind: code"));
        assert!(rendered.contains("Content:\nfn main() {}"));
    }

    #[test]
    fn render_descriptor_prompt_prefers_per_call_extra_override() {
        let defaults = defaults_with_catalog(PromptCatalog {
            file_analysis_system: Some("GLOBAL SYSTEM".to_string()),
            file_analysis_task: Some("GLOBAL TASK".to_string()),
            file_analysis_extra: Some("GLOBAL EXTRA".to_string()),
            content_fallback: Some("FALLBACK".to_string()),
            ..PromptCatalog::default()
        });

        let desc = sample_descriptor("unknown", ContentKind::Unknown);
        let rendered = defaults.render_descriptor_prompt(
            &desc,
            Some("OVERRIDE SYSTEM"),
            Some("OVERRIDE TASK"),
            Some("OVERRIDE EXTRA"),
        );

        assert!(rendered.contains("System:\nOVERRIDE SYSTEM"));
        assert!(rendered.contains("Task:\nOVERRIDE TASK"));
        assert!(rendered.contains("Additional instructions:\nOVERRIDE EXTRA"));
        assert!(!rendered.contains("Additional instructions:\nGLOBAL EXTRA"));
    }
}