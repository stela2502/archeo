//parse_mode.rs
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