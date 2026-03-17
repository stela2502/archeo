use std::path::{Path, PathBuf};
use std::fmt;
use clap::Args;

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
}

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

impl PrimerConfig{
    /// parse the CLI elements from the main input
    pub fn from_sources(cli: &PrimerCliArgs, files: &[PathBuf]) -> Self {
        // --- 1. start with inferred defaults ---
        let mut cfg = Self::infer_from_files(files);

        // --- 2. CLI overrides ---
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
    
    /// Infer a `PrimerConfig` from the provided file list by detecting
    /// programming languages (via file extensions) and domain hints
    /// (via filename patterns). Produces a best-effort, deduplicated
    /// configuration used as defaults before CLI overrides are applied.
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

                if lower.contains("scrna") || lower.contains("singlecell") || lower.contains("single_cell") {
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

#[derive(Debug, Clone)]
pub struct Primer {
    config: PrimerConfig,
}


impl fmt::Display for Primer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Primer configuration:")?;

        if self.config.languages.is_empty() {
            writeln!(f, "  languages: (none)")?;
        } else {
            writeln!(f, "  languages: {}", self.config.languages.join(", "))?;
        }

        if self.config.domains.is_empty() {
            writeln!(f, "  domains: (none)")?;
        } else {
            writeln!(f, "  domains: {}", self.config.domains.join(", "))?;
        }

        if self.config.project_hints.is_empty() {
            writeln!(f, "  hints: (none)")?;
        } else {
            writeln!(f, "  hints: {}", self.config.project_hints.join(", "))?;
        }

        writeln!(
            f,
            "  include_readme_advice: {}",
            self.config.include_readme_advice
        )?;

        writeln!(
            f,
            "  include_technical_debt: {}",
            self.config.include_technical_debt
        )?;

        Ok(())
    }
}

impl Primer {
    pub fn new(config: PrimerConfig) -> Self {
        Self { config }
    }

    pub fn build_prompt<P: AsRef<Path>>(&self, root: P, files: &[PathBuf]) -> String {
        let root = root.as_ref();

        let mut out = String::new();

        self.push_role_section(&mut out);
        self.push_context_section(&mut out, root);
        self.push_task_section(&mut out);
        self.push_output_contract(&mut out);
        self.push_files_section(&mut out, root, files);

        out
    }

    fn push_role_section(&self, out: &mut String) {
            out.push_str(
                "You are Archeo, an AI system for reconstructing the purpose, structure, and history \
                 of old research and software folders.\n\n",
            );

            out.push_str(
                "Your job is not to praise the folder. Your job is to infer what was built, \
                 what likely mattered, what is incomplete, and what should be documented.\n\n",
            );

            out.push_str(
                "You must base your analysis ONLY on the provided evidence.\n\
                 - Do NOT infer meaning from project names alone.\n\
                 - Do NOT assume standard patterns (e.g. tokenization, pipelines, analyzers) \
                   unless clearly supported by evidence.\n\
                 - Do NOT invent functionality.\n\
                 - If something is unclear, say so explicitly.\n\
                 - Do NOT ask the user for clarification.\n\n",
            );

            out.push_str(
                "Reasoning rules:\n\
                 - Prefer concrete observations over speculation.\n\
                 - Separate direct evidence from interpretation.\n\
                 - Use cautious language (e.g. 'likely', 'appears to').\n\
                 - Avoid confident statements without supporting evidence.\n\n",
            );
    }

    fn push_context_section(&self, out: &mut String, root: &Path) {
        out.push_str("Context\n");
        out.push_str("=======\n");
        out.push_str(&format!("Root folder: {}\n", root.display()));

        if !self.config.languages.is_empty() {
            out.push_str("Primary languages:\n");
            for lang in &self.config.languages {
                out.push_str(&format!("- {}\n", lang));
            }
        }

        if !self.config.domains.is_empty() {
            out.push_str("Domain hints:\n");
            for domain in &self.config.domains {
                out.push_str(&format!("- {}\n", domain));
            }
        }

        if !self.config.project_hints.is_empty() {
            out.push_str("Additional project hints:\n");
            for hint in &self.config.project_hints {
                out.push_str(&format!("- {}\n", hint));
            }
        }

        out.push('\n');
    }

    fn push_task_section(&self, out: &mut String) {
        out.push_str("Task\n");
        out.push_str("====\n");
        out.push_str("- Reconstruct what this folder is about\n");
        out.push_str("- Identify the likely main components\n");
        out.push_str("- Infer the likely workflow or development goal\n");
        out.push_str("- Highlight the files that appear central\n");

        if self.config.include_technical_debt {
            out.push_str("- Point out probable technical debt, oddities, or missing pieces\n");
        }

        if self.config.include_readme_advice {
            out.push_str("- Suggest what a useful README should contain\n");
        }

        out.push_str(
            "- Be explicit when uncertain; do not invent facts that are not supported by the file inventory\n\n",
        );
    }

    fn push_output_contract(&self, out: &mut String) {
        out.push_str("Required output format\n");
        out.push_str("======================\n");
        out.push_str("Return markdown with exactly these sections:\n\n");
        out.push_str("## Short Summary\n");
        out.push_str("A compact explanation of what this folder most likely is.\n\n");
        out.push_str("## Main Components\n");
        out.push_str("Bullet list of important subareas, files, or tool groups.\n\n");
        out.push_str("## Likely Workflow\n");
        out.push_str("Describe how the pieces probably fit together.\n\n");
        out.push_str("## Important Files\n");
        out.push_str("List the files that look most informative and explain why.\n\n");

        if self.config.include_technical_debt {
            out.push_str("## Problems / Gaps\n");
            out.push_str("List likely missing documentation, unfinished work, or technical debt.\n\n");
        }

        if self.config.include_readme_advice {
            out.push_str("## README Suggestions\n");
            out.push_str("Propose a concise README structure for this folder.\n\n");
        }

        out.push_str(
            "Do not wrap the whole answer in a code fence. Write normal markdown.\n\n",
        );
    }

    fn push_files_section(&self, out: &mut String, root: &Path, files: &[PathBuf]) {
        out.push_str("File inventory\n");
        out.push_str("==============\n");

        if files.is_empty() {
            out.push_str("No files were provided.\n");
            return;
        }

        for file in files {
            let display = match file.strip_prefix(root) {
                Ok(rel) => rel.display().to_string(),
                Err(_) => file.display().to_string(),
            };
            out.push_str(&format!("- {}\n", display));
        }

        out.push('\n');
    }
}

pub fn build_prompt<P: AsRef<Path>>(root: P, files: &[PathBuf]) -> String {
    Primer::new(PrimerConfig::default()).build_prompt(root, files)
}