use clap::Parser;

use archeo::primer::{ PrimerCliArgs, PrimerConfig};
use archeo::report::Report;
use archeo::ollama::Ollama;
use archeo::scanner::scanner_config::{ScanCliArgs, ScanConfig};
use archeo::scanner::scanner::Scanner;
use archeo::content_analysis::{ContentAnalyzer, ContentCliArgs, ContentConfig};
use archeo::prompt_defaults::PromptDefaults;

#[derive(Parser, Debug)]
#[command(author, version, about = "Archeo – reconstruct what the hell this project is")]
struct Args {
    /// Path to analyze
    #[arg(short, long)]
    path: String,

    /// Output markdown file
    #[arg(short, long, default_value = "AI_digest.md")]
    output: String,

    /// Model name (ollama)
    #[arg(short, long, default_value = "deepseek-coder")]
    model: String,

    /// A prompts file name
    #[arg(short, long)]
    file_prompts: Option<String>,

    #[command(flatten)]
    scan: ScanCliArgs,

    #[command(flatten)]
    primer: PrimerCliArgs,

    #[command(flatten)]
    content: ContentCliArgs,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    println!("🔍 Archeo analyzing: {}", args.path);

    let scan_config = ScanConfig::from_sources(&args.scan);
    println!("{}", scan_config.describe());

    let scanner = Scanner::new(scan_config.clone());
    let files = scanner.scan(&args.path)?;

    println!("Found {} files", files.len());
    for file in &files {
        println!("FILE: {}", file.display());
    }

    // Load or create the prompt catalog
    let mut prompts = PromptDefaults::new(args.file_prompts.as_deref())?;
    println!("Using prompt catalog: {}", prompts.path.display());

    let changed = prompts.apply_cli_overrides(
        args.primer.primer_task.as_deref(),
        args.primer.primer_extra.as_deref(),
        args.content.content_task.as_deref(),
        args.content.content_extra.as_deref(),
    );

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

    let primer_config = PrimerConfig::from_sources(&args.primer, &files);


    let ollama = Ollama::default();

    // Per-file content analysis
    let content_config = ContentConfig::from_sources(&args.content, &files);
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
            prompts.content_compression_task(args.content.content_task.as_deref());

        PromptDefaults::apply_extra(
            &mut compression_prompt,
            args.content.content_extra.as_deref(),
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