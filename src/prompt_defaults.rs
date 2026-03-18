//! Prompt catalog management for Archeo.
//!
//! This module is responsible for:
//! - loading a prompt catalog from disk,
//! - creating a default catalog when none exists,
//! - resolving prompt text for a given file descriptor,
//! - applying CLI-level prompt overrides,
//! - writing either the active or default catalog back to disk.
//!
//! The central type is [`PromptDefaults`], which combines the loaded
//! [`PromptCatalog`] with metadata about where it came from.

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::content_analysis::descriptor::ContentDescriptor;

/// Serializable prompt catalog.
///
/// This structure is stored as YAML and contains:
/// - high-level prompts used for folder or project analysis,
/// - file-level prompts keyed by extension,
/// - file-level prompts keyed by semantic kind.
///
/// The `Option<String>` fields allow a user-supplied catalog to omit values
/// and fall back to built-in defaults.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PromptCatalog {
    /// System prompt used for the project-level "primer" stage.
    pub primer_system: Option<String>,
    /// Task prompt used for the project-level "primer" stage.
    pub primer_task: Option<String>,
    /// Optional extra instructions for the primer stage.
    pub primer_extra: Option<String>,
    /// Task prompt used to compress file-level analyses into a compact index.
    pub content_compression_task: Option<String>,
    /// Optional extra instructions for file-level analysis.
    pub content_extra: Option<String>,
    /// Fallback prompt used when no extension- or kind-specific prompt exists.
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
/// - the file path it was loaded from,
/// - whether the file had to be created from built-in defaults.
#[derive(Debug, Clone)]
pub struct PromptDefaults {
    /// Path to the catalog file on disk.
    pub path: PathBuf,
    /// The active prompt catalog.
    pub catalog: PromptCatalog,
    /// Whether the catalog file was created during loading.
    pub was_created: bool,
}

impl PromptDefaults {
    /// Load a prompt catalog from disk, creating a default one if necessary.
    ///
    /// Path resolution rules:
    /// - if `path` is `None`, the global default prompt file is used;
    /// - if `path` is absolute, it is used as-is;
    /// - if `path` contains multiple path components, it is used as-is;
    /// - otherwise it is treated as relative to the global config directory.
    ///
    /// Returns an initialized [`PromptDefaults`] containing the parsed catalog.
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

        Ok(Self {
            path,
            catalog,
            was_created,
        })
    }

    /// Apply non-empty CLI overrides into the loaded catalog.
    ///
    /// Returns `true` if at least one catalog value changed.
    pub fn apply_cli_overrides(
        &mut self,
        primer_task: Option<&str>,
        primer_extra: Option<&str>,
        content_task: Option<&str>,
        content_extra: Option<&str>,
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

        if let Some(task) = content_task.map(str::trim).filter(|s| !s.is_empty()) {
            if self.catalog.content_compression_task.as_deref() != Some(task) {
                self.catalog.content_compression_task = Some(task.to_string());
                changed = true;
            }
        }

        if let Some(extra) = content_extra.map(str::trim).filter(|s| !s.is_empty()) {
            if self.catalog.content_extra.as_deref() != Some(extra) {
                self.catalog.content_extra = Some(extra.to_string());
                changed = true;
            }
        }

        changed
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
            "Explain the file's role in the Rust project. Identify key structs, enums, traits, impl blocks, \
functions, and modules. Summarize what the central items do and how they likely interact with \
other parts of the codebase, so that file-level reports can later be merged into an understanding \
of the project's internal logic and architecture. Stay evidence-based and avoid speculation."
                .to_string(),
        );

        by_extension.insert(
            "py".to_string(),
            "Explain whether this Python file is a script, library module, notebook-export, \
pipeline helper, or utility. Identify inputs, outputs, dependencies, and its likely role \
in the project."
                .to_string(),
        );

        by_extension.insert(
            "r".to_string(),
            "Explain the statistical or analytical purpose of this R file. Identify likely \
inputs, outputs, packages, and whether it appears exploratory or reusable."
                .to_string(),
        );

        by_extension.insert(
            "ipynb".to_string(),
            "Summarize this notebook as a paper-quality Methods section. Identify the likely data inputs, \
processing steps, transformations, and generated outputs (plots/tables). Describe the analysis \
workflow clearly and in order, and indicate whether the notebook appears exploratory, \
demonstrative, or report-like. Be precise and avoid speculation."
                .to_string(),
        );

        by_extension.insert(
            "md".to_string(),
            "Determine whether this markdown file is documentation, notes, report output, \
analysis narrative, changelog, or planning material. Summarize its role in the project."
                .to_string(),
        );

        by_extension.insert(
            "txt".to_string(),
            "First classify the text file if possible: notes, report, log, config-like text, \
data dictionary, or unknown. Then summarize cautiously."
                .to_string(),
        );

        by_extension.insert(
            "csv".to_string(),
            "Infer the likely role of this table. Mention likely identifier columns, measured \
values, and whether it looks like raw input, metadata, or derived results."
                .to_string(),
        );

        by_extension.insert(
            "tsv".to_string(),
            "Infer the likely role of this table. Mention likely identifier columns, measured \
values, and whether it looks like raw input, metadata, or derived results."
                .to_string(),
        );

        by_extension.insert(
            "yaml".to_string(),
            "Explain the likely role of this YAML file in the project: configuration, pipeline \
definition, metadata, schema, or other structured settings."
                .to_string(),
        );

        by_extension.insert(
            "yml".to_string(),
            "Explain the likely role of this YAML file in the project: configuration, pipeline \
definition, metadata, schema, or other structured settings."
                .to_string(),
        );

        by_extension.insert(
            "toml".to_string(),
            "Explain the likely role of this TOML file in the project: package metadata, \
configuration, build settings, or application settings."
                .to_string(),
        );

        by_extension.insert(
            "json".to_string(),
            "Explain the likely role of this JSON file in the project: configuration, metadata, \
schema, cached output, or structured results."
                .to_string(),
        );

        by_extension.insert(
            "sh".to_string(),
            "Explain whether this shell script is an entrypoint, setup helper, pipeline wrapper, \
maintenance script, or deployment utility. Mention inputs, outputs, and side effects if visible."
                .to_string(),
        );

        by_extension.insert(
            "bash".to_string(),
            "Explain whether this shell script is an entrypoint, setup helper, pipeline wrapper, \
maintenance script, or deployment utility. Mention inputs, outputs, and side effects if visible."
                .to_string(),
        );

        by_extension.insert(
            "zsh".to_string(),
            "Explain whether this shell script is an entrypoint, setup helper, pipeline wrapper, \
maintenance script, or deployment utility. Mention inputs, outputs, and side effects if visible."
                .to_string(),
        );

        let mut by_kind = BTreeMap::new();

        by_kind.insert(
            "text".to_string(),
            "Classify this text file by likely role first, then summarize its project function \
cautiously and concretely."
                .to_string(),
        );

        by_kind.insert(
            "table".to_string(),
            "Summarize this structured table by likely semantic role, probable key columns, \
and whether it looks like raw input, metadata, intermediate output, or final results."
                .to_string(),
        );

        by_kind.insert(
            "notebook".to_string(),
            "Summarize this notebook as an analysis workflow, including likely inputs, steps, \
outputs, and whether it appears exploratory or presentation-oriented."
                .to_string(),
        );

        PromptCatalog {
            primer_system: Some(
                "You are Archeo, an AI system for reconstructing the purpose, structure, and \
history of old research and software folders.\n\n\
Your job is not to praise the folder. Your job is to infer what was built, what likely \
mattered, what is incomplete, and what should be documented.\n\n\
You must base your analysis ONLY on the provided evidence.\n\
- Do NOT infer meaning from project names alone.\n\
- Do NOT assume standard patterns unless clearly supported by evidence.\n\
- Do NOT invent functionality.\n\
- If something is unclear, say so explicitly.\n\
- Do NOT ask the user for clarification.\n\n\
Reasoning rules:\n\
- Prefer concrete observations over speculation.\n\
- Separate direct evidence from interpretation.\n\
- Use cautious language.\n\
- Avoid confident statements without supporting evidence.\n"
                    .to_string(),
            ),
            primer_task: Some(
                "Reconstruct what this folder is about, identify likely main components, infer \
the likely workflow or development goal, and highlight the files that appear central."
                    .to_string(),
            ),
            content_extra: None,
            primer_extra: None,
            content_compression_task: Some(
                "You are converting file-level analysis into a compact index.\n\n\
Your task:\n\
- produce exactly ONE line per file\n\
- DO NOT summarize across files\n\
- DO NOT merge files\n\
- DO NOT group files\n\
- DO NOT write paragraphs\n\
- DO NOT explain anything\n\
- DO NOT add headers or sections\n\n\
Each line must follow exactly:\n\
<full file path> -> <short description>\n\n\
Rules:\n\
- max 15 words per description\n\
- describe the file's role in the project\n\
- avoid speculation\n\
- if unclear, write 'unclear purpose'\n\n\
Return ONLY the lines.\n
When identifying problems or gaps, prefer documentation issues, testing gaps, unfinished work,
and structural inconsistencies. Do not raise security concerns unless there is direct evidence
in the provided files or analysis.\n\n"
                    .to_string(),
            ),
            content_fallback: Some(
                "Explain the file's likely role in the project based only on the provided \
content and metadata. Be concrete, brief, and avoid speculation."
                    .to_string(),
            ),
            by_extension,
            by_kind,
        }
    }

    /// Return the primer system prompt, honoring an explicit override first.
    pub fn primer_system(&self, override_value: Option<&str>) -> String {
        override_value
            .map(|s| s.to_string())
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
            .map(|s| s.to_string())
            .or_else(|| self.catalog.primer_task.clone())
            .unwrap_or_else(|| {
                Self::default_catalog()
                    .primer_task
                    .expect("default primer_task must exist")
            })
    }

    /// Return the content compression task, honoring an explicit override first.
    pub fn content_compression_task(&self, override_value: Option<&str>) -> String {
        override_value
            .map(|s| s.to_string())
            .or_else(|| self.catalog.content_compression_task.clone())
            .unwrap_or_else(|| {
                Self::default_catalog()
                    .content_compression_task
                    .expect("default content_compression_task must exist")
            })
    }

    /// Resolve the best file-specific content prompt for a descriptor.
    ///
    /// Resolution order:
    /// 1. active catalog by file extension,
    /// 2. active catalog by content kind,
    /// 3. active fallback prompt,
    /// 4. built-in defaults by file extension,
    /// 5. built-in defaults by content kind,
    /// 6. built-in fallback prompt.
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

    /// Render a complete analysis prompt for a content descriptor.
    ///
    /// This combines generic instructions, file metadata, file-type-specific
    /// instructions, and the sampled file content into a single prompt string.
    pub fn render_descriptor_prompt(&self, desc: &ContentDescriptor) -> String {
        let mut out = String::new();

        out.push_str("Analyze this file and summarize its likely role in the project.\n\n");
        out.push_str("Rules:\n");
        out.push_str("- Base your answer only on the provided metadata and content\n");
        out.push_str("- Do not invent behavior or hidden dependencies\n");
        out.push_str("- Be explicit when uncertain\n");
        out.push_str("- Focus on role, purpose, and likely relation to the project\n\n");

        out.push_str("File metadata:\n");
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

        out.push('\n');
        out.push_str("File-type instructions:\n");
        out.push_str(&self.content_prompt_for(desc));
        out.push_str("\n\nContent:\n");
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
    use crate::content_analysis::descriptor::{ContentDescriptor, ContentKind};
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
    fn apply_cli_overrides_updates_only_non_empty_values() {
        let mut defaults = PromptDefaults {
            path: PathBuf::from("prompts.yml"),
            catalog: PromptDefaults::default_catalog(),
            was_created: false,
        };

        let changed = defaults.apply_cli_overrides(
            Some("new primer task"),
            Some("   "),
            Some("new content task"),
            None,
        );

        assert!(changed);
        assert_eq!(defaults.catalog.primer_task.as_deref(), Some("new primer task"));
        assert_eq!(
            defaults.catalog.content_compression_task.as_deref(),
            Some("new content task")
        );
        assert_eq!(defaults.catalog.primer_extra, None);
    }

    #[test]
    fn apply_cli_overrides_reports_unchanged_when_nothing_changes() {
        let mut defaults = PromptDefaults {
            path: PathBuf::from("prompts.yml"),
            catalog: PromptDefaults::default_catalog(),
            was_created: false,
        };

        let changed = defaults.apply_cli_overrides(None, Some("   "), None, Some(""));

        assert!(!changed);
    }

    #[test]
    fn content_prompt_for_prefers_extension_prompt() {
        let defaults = PromptDefaults {
            path: PathBuf::from("prompts.yml"),
            catalog: PromptDefaults::default_catalog(),
            was_created: false,
        };

        let desc = sample_descriptor("rs", ContentKind::Code);
        let prompt = defaults.content_prompt_for(&desc);

        assert!(prompt.contains("Rust project"));
    }

    #[test]
    fn content_prompt_for_falls_back_to_kind_prompt() {
        let defaults = PromptDefaults {
            path: PathBuf::from("prompts.yml"),
            catalog: PromptCatalog {
                by_extension: BTreeMap::new(),
                by_kind: BTreeMap::from([(
                    "text".to_string(),
                    "kind-level text prompt".to_string(),
                )]),
                content_fallback: Some("fallback prompt".to_string()),
                ..PromptCatalog::default()
            },
            was_created: false,
        };

        let desc = sample_descriptor("unknown", ContentKind::Code);
        let prompt = defaults.content_prompt_for(&desc);

        assert_eq!(prompt, "fallback prompt");
    }

    #[test]
    fn content_prompt_for_falls_back_to_catalog_fallback() {
        let defaults = PromptDefaults {
            path: PathBuf::from("prompts.yml"),
            catalog: PromptCatalog {
                by_extension: BTreeMap::new(),
                by_kind: BTreeMap::new(),
                content_fallback: Some("fallback prompt".to_string()),
                ..PromptCatalog::default()
            },
            was_created: false,
        };

        let desc = sample_descriptor("unknown", ContentKind::Code);
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
    fn render_descriptor_prompt_includes_metadata_and_content() {
        let defaults = PromptDefaults {
            path: PathBuf::from("prompts.yml"),
            catalog: PromptDefaults::default_catalog(),
            was_created: false,
        };

        let desc = sample_descriptor("rs", ContentKind::Code);
        let rendered = defaults.render_descriptor_prompt(&desc);

        assert!(rendered.contains("File metadata:"));
        assert!(rendered.contains("- extension: rs"));
        assert!(rendered.contains("- kind: code"));
        assert!(rendered.contains("Content:"));
        assert!(rendered.contains("fn main() {}"));
    }
}
