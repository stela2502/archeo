use clap::Parser;

use archeo::primer::{Primer, PrimerCliArgs, PrimerConfig};
use archeo::report::Report;
use archeo::ollama::Ollama;
use archeo::scanner::scanner_config::{ScanCliArgs, ScanConfig};
use archeo::scanner::scanner::Scanner;
use archeo::content_analysis::{ContentAnalyzer, ContentCliArgs, ContentConfig};

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

    let config = ScanConfig::from_sources(&args.scan);
    println!("{}", config.describe());

    let scanner = Scanner::new(config.clone());
    let files = scanner.scan(&args.path)?;

    println!("Found {} files", files.len());
    for file in &files {
        println!("FILE: {}", file.display());
    }

    let primer_config = PrimerConfig::from_sources(&args.primer, &files);
    let primer = Primer::new(primer_config);

    println!("{}", primer);

    let prompt = primer.build_prompt(&args.path, &files);

    let ollama = Ollama::default();

    let content_config = ContentConfig::from_sources(&args.content, &files);
    let analyzer = ContentAnalyzer::new(content_config.clone());

    let content_reports = if content_config.enabled {
        analyzer.analyze_files( &files, &ollama, &args.model)?
    } else {
        Vec::new()
    };

    let (enriched_prompt, content_summary) = if content_reports.is_empty() {
        ( prompt.clone() ,String::new())
    } else {
        let content_summary = ContentAnalyzer::compress_reports_with_ai(&content_reports, &ollama, &args.model)?;
        ( format!(
            "{}\n\n## File Content Analysis\n{}\n\n## Final Task\nUse the file content analysis above as additional evidence when summarizing the project.",
            prompt,
            content_summary
        ), content_summary )
    };

    let response = ollama.generate(&args.model, &enriched_prompt)?;

    let report = Report::new(
        &args.path,
        &files,
        &config,
        &args.model,
        &response,
        &content_summary,
        &content_reports,
    );

    report.write(&args.output)?;

    println!("✅ Done. Output written to {}\n{}", args.output, report);

    Ok(())
}