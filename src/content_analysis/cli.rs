//cli.rs
use clap::Args;
use std::path::PathBuf;

#[derive(Args, Debug, Default, Clone)]
pub struct ContentCliArgs {
    /// Enable content-based analysis
    #[arg(long)]
    pub content_analysis: bool,

    /// Per-extension mode rule, e.g. "py=full", "csv=sampled", "bin=skip"
    #[arg(long = "content-mode")]
    pub content_modes: Vec<String>,

    /// Per-extension primer, e.g. "py=Explain the script purpose"
    #[arg(long = "content-primer")]
    pub content_primers: Vec<String>,

    /// Per-extension primer loaded from file, e.g. "py=primers/python.txt"
    #[arg(long = "content-primer-file")]
    pub content_primer_files: Vec<String>,

    /// Generic fallback primer for extensions without a dedicated primer
    #[arg(long)]
    pub content_default_primer: Option<String>,

    /// Generic fallback primer loaded from file
    #[arg(long)]
    pub content_default_primer_file: Option<PathBuf>,

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