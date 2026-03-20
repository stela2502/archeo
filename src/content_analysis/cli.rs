//cli.rs
use clap::Args;
use std::path::PathBuf;

#[derive(Args, Debug, Default, Clone)]
pub struct ContentCliArgs {
    /// Enable content-based analysis
    #[arg(long)]
    pub content_analysis: bool,

    /// Per-extension mode rule.
    /// Can be provided multiple times:
    ///   --content-mode py=full --content-mode csv=sampled --content-mode bin=skip
    #[arg(long = "content-mode")]
    pub content_modes: Vec<String>,

    /// Per-extension primer rule.
    /// Can be provided multiple times:
    ///   --primer py="Explain the script purpose"
    ///   --primer rs="Summarize module structure"
    #[arg(long = "content-primer")]
    pub content_primers: Vec<String>,

    /// Maximum bytes for fully read files
    #[arg(long, default_value_t = 150_000)]
    pub content_max_full_bytes: usize,

    /// Maximum rows to sample from table-like files
    #[arg(long, default_value_t = 10)]
    pub content_sample_rows: usize,

    /// Maximum columns to sample from table-like files
    #[arg(long, default_value_t = 20)]
    pub content_sample_cols: usize,

    /// Restrict analysis to these extensions (comma-separated)
    #[arg(long)]
    pub content_extensions: Option<String>,

    /// Disable recursive traversal
    #[arg(long)]
    pub no_recursive_content: bool,

}