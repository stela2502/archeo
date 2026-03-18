//! Scanner module
//!
//! Provides functionality to recursively scan directories and return files
//! matching a configurable set of filters such as:
//! - allowed extensions
//! - excluded directories
//! - file size limits
//! - hidden file handling

use std::fs;
use std::path::{Component, Path, PathBuf};

use walkdir::WalkDir;

use super::scanner_config::ScanConfig;

/// Scans a directory tree and returns files matching [`ScanConfig`].
///
/// # Example
/// ```
/// use archeo::scanner::scanner::Scanner;
/// use archeo::scanner::scanner_config::ScanConfig;
///
/// let mut config = ScanConfig::default();
/// config.allowed_extensions = vec!["rs".into()];
///
/// let scanner = Scanner::new(config);
/// let files = scanner.scan("src").unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct Scanner {
    config: ScanConfig,
}

impl Scanner {
    /// Create a new [`Scanner`] with the given configuration.
    pub fn new(config: ScanConfig) -> Self {
        Self { config }
    }

    /// Recursively scan a directory and return matching files.
    ///
    /// # Errors
    /// - If `root` does not exist
    /// - If `root` is not a directory
    pub fn scan<P: AsRef<Path>>(&self, root: P) -> anyhow::Result<Vec<PathBuf>> {
        let root = root.as_ref();

        if !root.exists() {
            anyhow::bail!("Path does not exist: {}", root.display());
        }

        if !root.is_dir() {
            anyhow::bail!("Path is not a directory: {}", root.display());
        }

        let walker = WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|entry| self.should_descend(entry.path(), entry.file_type().is_dir(), root));

        let mut result = Vec::new();

        for entry in walker.filter_map(Result::ok) {
            let path = entry.path();

            if path == root {
                continue;
            }

            if !entry.file_type().is_file() {
                continue;
            }

            if self.should_include_file(path) {
                result.push(path.to_path_buf());
            }
        }

        Ok(result)
    }

    /// Determines if traversal should continue into a directory.
    fn should_descend(&self, path: &Path, is_dir: bool, root: &Path) -> bool {
        if path == root {
            return true;
        }

        if is_dir {
            if self.is_excluded_dir(path) {
                return false;
            }

            if !self.config.include_hidden && self.is_hidden(path) {
                return false;
            }
        }

        true
    }

    /// Determines if a file should be included in the result set.
    fn should_include_file(&self, path: &Path) -> bool {
        // safety guard
        if self.is_excluded_dir(path) {
            return false;
        }

        if !self.config.include_hidden && self.is_hidden(path) {
            return false;
        }

        if !self.is_allowed_extension(path) {
            return false;
        }

        if !self.is_within_size(path) {
            return false;
        }

        true
    }

    /// Returns true if a file or directory is hidden (starts with `.`).
    fn is_hidden<P: AsRef<Path>>(&self, path: P) -> bool {
        path.as_ref()
            .file_name()
            .and_then(|name| name.to_str())
            .map(|s| s.starts_with('.'))
            .unwrap_or(false)
    }

    /// Returns true if any component of the path matches an excluded directory.
    ///
    /// Matching is done on full path components, not substrings.
    fn is_excluded_dir<P: AsRef<Path>>(&self, path: P) -> bool {
        let path = path.as_ref();

        path.components().any(|comp| match comp {
            Component::Normal(name) => {
                name.to_str()
                    .map(|s| self.config.excluded_dirs.iter().any(|d| d == s))
                    .unwrap_or(false)
            }
            _ => false,
        })
    }

    /// Returns true if the file extension is allowed.
    fn is_allowed_extension(&self, path: &Path) -> bool {
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            return false;
        };

        self.config
            .allowed_extensions
            .iter()
            .any(|allowed| allowed == ext)
    }

    /// Returns true if file size is within configured limits.
    fn is_within_size(&self, path: &Path) -> bool {
        match fs::metadata(path) {
            Ok(meta) => meta.len() as usize <= self.config.max_file_size,
            Err(_) => false,
        }
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{PathBuf};

    #[test]
    fn scan_src_folder_finds_rust_files() {
        let mut config = ScanConfig::default();
        config.allowed_extensions = vec!["rs".to_string()];
        config.include_hidden = false;

        let scanner = Scanner::new(config);

        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
        let files = scanner.scan(&root).expect("scanner should parse src folder");

        println!("All the files: {:?}", files);

        assert!(
            !files.is_empty(),
            "scanner returned no files for {}",
            root.display()
        );

        assert!(
            files.iter().all(|p| p.extension().and_then(|e| e.to_str()) == Some("rs")),
            "scanner returned non-.rs files: {files:#?}"
        );

        assert!(
            files.iter().any(|p| p.file_name().and_then(|n| n.to_str()) == Some("main.rs"))
                || files.iter().any(|p| p.file_name().and_then(|n| n.to_str()) == Some("lib.rs")),
            "scanner did not find main.rs or lib.rs in src: {files:#?}"
        );
    }

    #[test]
    fn is_excluded_dir_matches_default_config() {
        let scanner = Scanner::new(ScanConfig::default());

        // --- should match excluded dirs ---
        assert!(scanner.is_excluded_dir("target"));
        assert!(scanner.is_excluded_dir("node_modules"));
        assert!(scanner.is_excluded_dir(".git"));

        // --- nested paths should also match ---
        assert!(scanner.is_excluded_dir("target/debug/file.rs"));
        assert!(scanner.is_excluded_dir("./target/release/build/foo.rs"));
        assert!(scanner.is_excluded_dir("/tmp/project/node_modules/pkg/index.js"));
        assert!(scanner.is_excluded_dir(".git/config"));

        // --- should NOT match ---
        assert!(!scanner.is_excluded_dir("src"));
        assert!(!scanner.is_excluded_dir("src/main.rs"));
        assert!(!scanner.is_excluded_dir("README.md"));

        // --- tricky cases ---
        assert!(!scanner.is_excluded_dir("targeting.rs")); // substring should NOT match
        assert!(!scanner.is_excluded_dir("my_target_dir/file.rs")); // not exact component
    }
}