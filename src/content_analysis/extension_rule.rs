//extension_rule.rs 
use crate::content_analysis::ParseMode;

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