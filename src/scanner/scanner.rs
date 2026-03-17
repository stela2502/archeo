use std::fs;
use std::path::{Path, PathBuf, Component};

use walkdir::WalkDir;

use super::scanner_config::ScanConfig;

#[derive(Debug, Clone)]
pub struct Scanner {
    config: ScanConfig,
}

impl Scanner {
    pub fn new(config: ScanConfig) -> Self {
        Self { config }
    }

    pub fn scan<P: AsRef<Path>>(&self, root: P) -> anyhow::Result<Vec<PathBuf>> {
        let root = root.as_ref();

        if !root.exists() {
            anyhow::bail!("Path does not exist: {}", root.display());
        }

        if !root.is_dir() {
            anyhow::bail!("Path is not a directory: {}", root.display());
        }

        let mut result = Vec::new();

        let walker = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            let path = entry.path();

            // always keep the root
            if path == root {
                return true;
            }

            // prune excluded directories before descent
            if entry.file_type().is_dir() {
                if self.is_excluded_dir(entry.path()) {
                    return false;
                }
                if !self.config.include_hidden {
                    if let Some(name) = entry.path().file_name().and_then(|s| s.to_str()) {
                        if self.is_hidden(name) {
                            return false;
                        }
                    }
                }
            }

            true
        });

        for entry in walker.filter_map(|e| e.ok()) {
            let path = entry.path();

            // skip root itself
            if path == root {
                continue;
            }

            // ignore directories in the result set
            if entry.file_type().is_dir() {
                continue;
            }

            // only files from here on
            if !entry.file_type().is_file() {
                continue;
            }

            // safety guard in case an excluded path still slips through
            if path.components().any(|c| {
                c.as_os_str()
                    .to_str()
                    .map(|s| self.is_excluded_dir(s))
                    .unwrap_or(false)
            }) {
                continue;
            }

            // hidden filter
            if !self.config.include_hidden && self.is_hidden(path) {
                continue;
            }

            // extension filter
            if !self.is_allowed_extension(path) {
                continue;
            }

            // size filter
            if !self.is_within_size(path) {
                continue;
            }

            result.push(path.to_path_buf());
        }

        Ok(result)
    }

    // --- helpers (methods, not free functions) ---

    fn is_hidden<P: AsRef<Path>>(&self, path: P) -> bool {
        path.as_ref()
            .file_name()
            .and_then(|name| name.to_str())
            .map(|s| s.starts_with('.'))
            .unwrap_or(false)
    }

    fn is_excluded_dir<P: AsRef<Path>>(&self, path: P) -> bool {
        let path = path.as_ref();

        path.components().any(|comp| {
            match comp {
                Component::Normal(name) => {
                    if let Some(s) = name.to_str() {
                        self.config.excluded_dirs.iter().any(|d| d == s)
                    } else {
                        false
                    }
                }
                _ => false,
            }
        })
    }

    fn is_allowed_extension(&self, path: &Path) -> bool {
        let ext = match path.extension().and_then(|e| e.to_str()) {
            Some(e) => e,
            None => return false,
        };

        self.config
            .allowed_extensions
            .iter()
            .any(|allowed| allowed == ext)
    }

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
    use std::path::{PathBuf, Path};

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