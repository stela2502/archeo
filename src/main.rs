use clap::Parser;

use archeo::primer::{ PrimerConfig};
use archeo::report::Report;
use archeo::ollama::Ollama;
use archeo::scanner::scanner_config::{ ScanConfig};
use archeo::scanner::scanner::Scanner;
use archeo::content_analysis::{ContentAnalyzer,  ContentConfig};
use archeo::prompt_defaults::PromptDefaults;

#[derive(Parser, Debug)]
#[command(author, version, about = "Archeo – use a local AI system to make sense of your files")]
struct Args {
    // -------------------------------------------------
    // Core
    // -------------------------------------------------

    /// Root path to analyze.
    ///
    /// Example:
    ///   --path .
    ///   --path /data/project
    #[arg(short, long, value_name = "PATH")]
    path: String,

    /// Markdown file to write the final report to.
    ///
    /// Default:
    ///   AI_digest.md
    ///
    /// Example:
    ///   --output report.md
    #[arg(short, long, default_value = "AI_digest.md", value_name = "FILE")]
    output: String,

    /// Ollama model name used for all AI requests.
    ///
    /// Example:
    ///   --model llama3.2
    ///   --model deepseek-coder
    #[arg(short, long, default_value = "deepseek-coder", value_name = "MODEL")]
    model: String,

    /// YAML prompt catalog file to load instead of built-in defaults.
    ///
    /// This file can override primer, file-analysis, compression,
    /// extension-specific, and kind-specific prompts.
    ///
    /// Example:
    ///   --prompts-file prompts.yml
    #[arg(long, value_name = "FILE")]
    prompts_file: Option<String>,

    // -------------------------------------------------
    // Global prompt overrides
    // -------------------------------------------------

    /// Override the global project-primer task prompt.
    ///
    /// This changes the high-level instruction used to summarize what the
    /// whole folder or project is about.
    ///
    /// Example:
    ///   --primer-task "Explain the project purpose and main components"
    #[arg(long, value_name = "TEXT")]
    primer_task: Option<String>,

    /// Extra instructions appended to the project-primer prompt.
    ///
    /// Use this to bias the project summary without replacing the base task.
    ///
    /// Example:
    ///   --primer-extra "Focus on architecture, tests, and missing documentation"
    #[arg(long, value_name = "TEXT")]
    primer_extra: Option<String>,

    /// Override the global file-analysis task prompt.
    ///
    /// This changes the instruction used when Archeo analyzes an individual file.
    ///
    /// Example:
    ///   --file-analysis-task "Explain what this file does and how it fits the project"
    #[arg(long, value_name = "TEXT")]
    file_analysis_task: Option<String>,

    /// Extra instructions appended to the file-analysis prompt.
    ///
    /// Example:
    ///   --file-analysis-extra "Mention important structs, functions, and likely role"
    #[arg(long, value_name = "TEXT")]
    file_analysis_extra: Option<String>,

    /// Override the task used to compress file analyses into a shorter summary.
    ///
    /// Example:
    ///   --content-compression-task "Compress each file analysis into 2 concise sentences"
    #[arg(long, value_name = "TEXT")]
    content_compression_task: Option<String>,

    /// Fallback content prompt used when no extension-specific or kind-specific
    /// prompt matches a file.
    ///
    /// Example:
    ///   --content-fallback "Explain the likely purpose of this file from its contents"
    #[arg(long, value_name = "TEXT")]
    content_fallback: Option<String>,

    // -------------------------------------------------
    // Content analysis
    // -------------------------------------------------

    /// Enable content-based file analysis.
    ///
    /// Without this flag, Archeo mainly relies on filenames, paths, and project
    /// structure. With this flag, Archeo may read and analyze file contents.
    #[arg(long)]
    content_analysis: bool,

    /// Per-extension content reading mode rule in the form EXT=MODE.
    ///
    /// This option is repeatable. Use it once per rule.
    ///
    /// Allowed modes depend on your parser, for example:
    ///   full      read the file content fully
    ///   sampled   read only a sampled subset
    ///   skip      do not analyze file contents
    ///
    /// Examples:
    ///   --content-mode rs=full
    ///   --content-mode py=full
    ///   --content-mode csv=sampled
    ///   --content-mode bin=skip
    #[arg(long = "content-mode", value_name = "EXT=MODE")]
    content_modes: Vec<String>,

    /// Per-extension content prompt rule in the form EXT=TEXT.
    ///
    /// This option is repeatable. Use it once per rule.
    /// These prompts override the generic file-analysis prompt for matching
    /// extensions.
    ///
    /// Examples:
    ///   --content-primer rs="Explain the Rust module purpose"
    ///   --content-primer py="Explain the Python script purpose"
    ///   --content-primer md="Summarize the document intent"
    #[arg(long = "content-primer", value_name = "EXT=TEXT")]
    content_primers: Vec<String>,

    /// Per-kind content prompt rule in the form KIND=TEXT.
    ///
    /// This option is repeatable. Use it once per rule.
    /// Use this when you want one prompt for all files of a detected content kind.
    ///
    /// Typical kinds include:
    ///   text, table, notebook, code, config, data, UNKNOWN
    ///
    /// Examples:
    ///   --kind-primer code="Explain structure, responsibilities, and key entry points"
    ///   --kind-primer table="Describe columns, rows, and likely meaning"
    ///   --kind-primer config="Explain what this configuration controls"
    #[arg(long = "kind-primer", value_name = "KIND=TEXT")]
    kind_primers: Vec<String>,

    /// Maximum number of bytes to read when a file is analyzed in full mode.
    ///
    /// Larger files may be truncated to this limit before being sent to the model.
    ///
    /// Example:
    ///   --content-max-full-bytes 50000
    #[arg(long, default_value_t = 150_000, value_name = "N")]
    content_max_full_bytes: usize,

    /// Maximum number of rows to sample from table-like files.
    ///
    /// Used for CSV/TSV and similar structured text inputs when sampling is enabled.
    ///
    /// Example:
    ///   --content-sample-rows 20
    #[arg(long, default_value_t = 10, value_name = "N")]
    content_sample_rows: usize,

    /// Maximum number of columns to sample from table-like files.
    ///
    /// Example:
    ///   --content-sample-cols 30
    #[arg(long, default_value_t = 20, value_name = "N")]
    content_sample_cols: usize,

    /// Restrict content analysis to a comma-separated list of extensions.
    ///
    /// Only matching file extensions will be considered for content-based analysis.
    ///
    /// Example:
    ///   --content-extensions rs,py,md
    #[arg(long, value_name = "EXT1,EXT2,...")]
    content_extensions: Option<String>,

    /// Disable recursive content analysis.
    ///
    /// When set, Archeo analyzes only the top-level files in the selected path
    /// for content-based processing.
    #[arg(long)]
    no_recursive_content: bool,

    // -------------------------------------------------
    // Scan
    // -------------------------------------------------

    /// YAML configuration file for scan settings.
    ///
    /// This may define extensions, excluded directories, size limits, and other
    /// scanning defaults.
    ///
    /// Example:
    ///   --config archeo.scan.yml
    #[arg(long, value_name = "FILE")]
    config: Option<String>,

    /// Allowed file extension to include in the scan.
    ///
    /// This option is repeatable. Use it once per extension.
    /// Extensions should usually be given without a leading dot.
    ///
    /// Examples:
    ///   --ext rs
    ///   --ext py
    ///   --ext md
    #[arg(long,short='x', value_name = "EXT")]
    ext: Vec<String>,

    /// Directory name to exclude from scanning.
    ///
    /// This option is repeatable. Use it once per excluded directory name.
    /// Matching is typically done by directory name, not full path.
    ///
    /// Examples:
    ///   --exclude-dir .git
    ///   --exclude-dir target
    ///   --exclude-dir node_modules
    #[arg(long, value_name = "DIR")]
    exclude_dir: Vec<String>,

    /// Maximum file size in bytes allowed during scanning.
    ///
    /// Files larger than this limit are skipped.
    ///
    /// Example:
    ///   --max-file-size 5000000
    #[arg(long, value_name = "BYTES")]
    max_file_size: Option<usize>,

    /// Include hidden files and hidden directories.
    ///
    /// By default, hidden paths may be skipped.
    #[arg(long)]
    include_hidden: bool,
}




fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    println!("🔍 Archeo analyzing: {}", args.path);

    let scan_config = ScanConfig::from_sources(
        args.config.as_deref(),
        &args.ext,
        &args.exclude_dir,
        args.max_file_size,
        args.include_hidden,
    );
    println!("{}", scan_config.describe());

    let scanner = Scanner::new(scan_config.clone());
    let files = scanner.scan(&args.path)?;

    println!("Found {} files", files.len());
    for file in &files {
        println!("FILE: {}", file.display());
    }

    // Load or create the prompt catalog
    let mut prompts = PromptDefaults::new(args.prompts_file.as_deref())?;
    println!("Using prompt catalog: {}", prompts.path.display());

    let mut changed = prompts.apply_cli_overrides(
        args.primer_task.as_deref(),
        args.primer_extra.as_deref(),
        args.file_analysis_task.as_deref(),
        args.file_analysis_extra.as_deref(),
        args.content_compression_task.as_deref(),
    );
    changed |= prompts.apply_content_primer_rules(&args.content_primers);
    changed |= prompts.apply_kind_primer_rules(&args.kind_primers);


    if changed {
        let prompt_snapshot_path = std::path::Path::new(&args.output)
            .with_extension("prompts.yml");
        prompts.write_used_catalog(&prompt_snapshot_path)?;

        println!(
            "Saved effective prompt config for this run to {}",
            prompt_snapshot_path.display()
        );
        println!(
            "To reuse these prompts as defaults, replace {} with that file.",
            prompts.path.display()
        );
    }

    let primer_config = PrimerConfig::from_sources(
        &files,
        None,
        None,
        true,
        true,
    );


    let ollama = Ollama::default();

    // Per-file content analysis
    let content_config = ContentConfig::from_sources(
        args.content_analysis,
        args.no_recursive_content,
        args.content_max_full_bytes,
        args.content_sample_rows,
        args.content_sample_cols,
        args.content_extensions.as_deref(),
        &args.content_modes,
    );
    let analyzer = ContentAnalyzer::new(content_config.clone());


    let content_reports = if content_config.enabled {
        analyzer.analyze_files(&files, &ollama, &args.model, &prompts)?
    } else {
        Vec::new()
    };

    let content_summary = if content_reports.is_empty() {
        String::new()
    } else {
        let mut compression_prompt =
            prompts.content_compression_task(None);

        PromptDefaults::apply_extra(
            &mut compression_prompt, None,
        );

        ContentAnalyzer::compress_reports_with_ai(
            &content_reports,
            &ollama,
            &args.model,
            &compression_prompt,
        )?
    };

    // Final project-level prompt
    let final_prompt = build_project_prompt(
        &args.path,
        &files,
        &primer_config,
        &prompts,
        &content_summary,
    )?;

    let response = ollama.generate(&args.model, &final_prompt)?;

    let report = Report::new(
        &args.path,
        &files,
        &scan_config,
        &args.model,
        &response,
        &content_summary,
        &content_reports,
    );

    report.write(&args.output)?;

    let prompt_snapshot_path = std::path::Path::new(&args.output)
        .with_extension("prompts.yml");

    println!(
        "Saved effective prompts for this run to {}",
        prompt_snapshot_path.display()
    );
    println!(
        "To reuse these prompts as defaults, copy that file to {}",
        prompts.path.display()
    );

    println!("✅ Done. Output written to {}", args.output);
    println!("\n===== SUMMARY =====\n{}\n", response);

    Ok(())
}

fn build_project_prompt<P: AsRef<std::path::Path>>(
    root: P,
    files: &[std::path::PathBuf],
    primer_config: &PrimerConfig,
    prompts: &PromptDefaults,
    content_summary: &str,
) -> anyhow::Result<String> {
    let root = root.as_ref();

    let mut out = String::new();

    out.push_str(&prompts.primer_system(None));
    out.push('\n');

    out.push_str("Context\n");
    out.push_str("=======\n");
    out.push_str(&format!("Root folder: {}\n", root.display()));

    if !primer_config.languages.is_empty() {
        out.push_str("Primary languages:\n");
        for lang in &primer_config.languages {
            out.push_str(&format!("- {}\n", lang));
        }
    }

    if !primer_config.domains.is_empty() {
        out.push_str("Domain hints:\n");
        for domain in &primer_config.domains {
            out.push_str(&format!("- {}\n", domain));
        }
    }

    if !primer_config.project_hints.is_empty() {
        out.push_str("Additional project hints:\n");
        for hint in &primer_config.project_hints {
            out.push_str(&format!("- {}\n", hint));
        }
    }

    out.push('\n');
    out.push_str("Task\n");
    out.push_str("====\n");

    let task_block = prompts.primer_task(None);
    //PromptDefaults::apply_extra(&mut task_block, prompts.primer_extra(None));

    out.push_str(&task_block);
    out.push('\n');

    if primer_config.include_technical_debt {
        out.push_str("- Point out probable technical debt, oddities, or missing pieces\n");
    }

    if primer_config.include_readme_advice {
        out.push_str("- Suggest what a useful README should contain\n");
    }

    out.push_str("\nRequired output format\n");
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

    if primer_config.include_technical_debt {
        out.push_str("## Problems / Gaps\n");
        out.push_str("List likely missing documentation, unfinished work, or technical debt.\n\n");
    }

    if primer_config.include_readme_advice {
        out.push_str("## README Suggestions\n");
        out.push_str("Propose a concise README structure for this folder.\n\n");
    }

    out.push_str("Do not wrap the whole answer in a code fence. Write normal markdown.\n\n");

    out.push_str("File inventory\n");
    out.push_str("==============\n");
    for file in files {
        let display = match file.strip_prefix(root) {
            Ok(rel) => rel.display().to_string(),
            Err(_) => file.display().to_string(),
        };
        out.push_str(&format!("- {}\n", display));
    }

    if !content_summary.trim().is_empty() {
        out.push_str("\nAdditional evidence from file content analysis\n");
        out.push_str("=============================================\n");
        out.push_str(content_summary);
        out.push_str("\n\nUse the file content analysis above as additional evidence when summarizing the project.\n");
    }

    Ok(out)
}