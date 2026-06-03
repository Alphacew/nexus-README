use ignore::{DirEntry, Error, ParallelVisitor, ParallelVisitorBuilder, WalkBuilder, WalkState};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Sender, channel};

/// A utility to check if a directory should be skipped.
fn should_exclude_dir(name: &str, rel_path: &Path, exclusions: &[String]) -> bool {
    let defaults = [
        ".git",
        "node_modules",
        "build",
        "target",
        "dist",
        ".next",
        ".venv",
        "venv",
        "__pycache__",
    ];

    if defaults.contains(&name) {
        return true;
    }

    if exclusions.iter().any(|ex| ex == name) {
        return true;
    }

    // Check if relative path equals any exclusion (with normalized slashes)
    let rel_path_str = rel_path.to_string_lossy().replace('\\', "/");
    if exclusions.iter().any(|ex| {
        let normalized_ex = ex.replace('\\', "/");
        rel_path_str == normalized_ex || rel_path_str.starts_with(&format!("{}/", normalized_ex))
    }) {
        return true;
    }

    false
}

/// A list of common binary file extensions to skip.
fn is_binary_extension(ext: &str) -> bool {
    let binary_extensions = [
        "png", "jpg", "jpeg", "gif", "webp", "ico", "tiff", "bmp", "pdf", "zip", "tar", "gz",
        "tgz", "xz", "bz2", "7z", "rar", "exe", "dll", "so", "dylib", "bin", "wasm", "pyc",
        "class", "jar", "o", "a", "woff", "woff2", "ttf", "otf", "eot", "mp3", "mp4", "mkv", "avi",
        "mov", "wav", "flac", "db", "sqlite", "sqlite3", "sqlitedb", "dmg", "iso", "apk", "ipa",
        "msi", "pdb", "ds_store",
    ];
    binary_extensions.contains(&ext.to_lowercase().as_str())
}

/// Filters candidate files by size, binary extensions, and custom exclusions.
fn should_keep_file(entry: &DirEntry, root_path: &Path, exclusions: &[String]) -> bool {
    let path = entry.path();

    // 1. Get relative path to check custom exclusions
    let rel_path = path.strip_prefix(root_path).unwrap_or(path);
    let rel_path_str = rel_path.to_string_lossy().replace('\\', "/");
    if exclusions.iter().any(|ex| {
        let normalized_ex = ex.replace('\\', "/");
        rel_path_str == normalized_ex || rel_path_str.starts_with(&format!("{}/", normalized_ex))
    }) {
        return false;
    }

    // 2. Check binary extensions
    if path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(is_binary_extension)
    {
        return false;
    }

    // 3. Check file size (cap at 2MB)
    if entry.metadata().is_ok_and(|m| m.len() > 2_000_000) {
        return false;
    }

    true
}

/// The workspace file crawler.
pub struct WorkspaceCrawler {
    root_path: PathBuf,
    exclusions: Vec<String>,
}

impl WorkspaceCrawler {
    pub fn new(root_path: PathBuf, exclusions: Vec<String>) -> Self {
        Self {
            root_path,
            exclusions,
        }
    }

    /// Crawls the workspace concurrently and returns a sorted list of relative paths.
    pub fn crawl(&self) -> Result<Vec<PathBuf>, ignore::Error> {
        let (tx, rx) = channel();

        let mut builder = WalkBuilder::new(&self.root_path);
        // Boundary enforcement: prevent traversing outside symlinks or loops
        builder.follow_links(false);
        // Traverse hidden files by default but filter out .git
        builder.hidden(false);
        builder.git_ignore(true);
        builder.git_global(true);
        builder.git_exclude(true);
        builder.parents(true);

        let walk_parallel = builder.build_parallel();

        let root_path_clone = Arc::new(self.root_path.clone());
        let exclusions_clone = Arc::new(self.exclusions.clone());

        struct CrawlerVisitor {
            tx: Sender<PathBuf>,
            root_path: Arc<PathBuf>,
            exclusions: Arc<Vec<String>>,
        }

        impl ParallelVisitor for CrawlerVisitor {
            fn visit(&mut self, entry: Result<DirEntry, Error>) -> WalkState {
                match entry {
                    Ok(dir_entry) => {
                        let path = dir_entry.path();
                        let rel_path = path.strip_prefix(self.root_path.as_ref()).unwrap_or(path);

                        if let Some(file_type) = dir_entry.file_type() {
                            if file_type.is_dir() {
                                if path
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .is_some_and(|name| {
                                        should_exclude_dir(name, rel_path, &self.exclusions)
                                    })
                                {
                                    return WalkState::Skip;
                                }
                            } else if file_type.is_file()
                                && should_keep_file(
                                    &dir_entry,
                                    self.root_path.as_ref(),
                                    &self.exclusions,
                                )
                            {
                                // Send relative path
                                let _ = self.tx.send(rel_path.to_path_buf());
                            }
                        }
                        WalkState::Continue
                    }
                    Err(_) => WalkState::Continue,
                }
            }
        }

        struct CrawlerVisitorBuilder {
            tx: Sender<PathBuf>,
            root_path: Arc<PathBuf>,
            exclusions: Arc<Vec<String>>,
        }

        impl<'s> ParallelVisitorBuilder<'s> for CrawlerVisitorBuilder {
            fn build(&mut self) -> Box<dyn ParallelVisitor + 's> {
                Box::new(CrawlerVisitor {
                    tx: self.tx.clone(),
                    root_path: self.root_path.clone(),
                    exclusions: self.exclusions.clone(),
                })
            }
        }

        let mut visitor_builder = CrawlerVisitorBuilder {
            tx,
            root_path: root_path_clone,
            exclusions: exclusions_clone,
        };

        walk_parallel.visit(&mut visitor_builder);
        drop(visitor_builder);

        // Gather all results
        let mut paths = Vec::new();
        while let Ok(path) = rx.recv() {
            paths.push(path);
        }

        // Sort to guarantee deterministic JSON output
        paths.sort();

        Ok(paths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_crawler_defaults_and_ignores() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create standard source files
        fs::create_dir_all(root.join("src")).unwrap();
        File::create(root.join("src/main.rs")).unwrap();
        File::create(root.join("src/lib.rs")).unwrap();

        // Create nested hidden files inside allowed folder
        fs::create_dir_all(root.join(".github/workflows")).unwrap();
        File::create(root.join(".github/workflows/ci.yml")).unwrap();

        // Create default ignored directory files
        fs::create_dir_all(root.join("node_modules/lodash")).unwrap();
        File::create(root.join("node_modules/lodash/index.js")).unwrap();

        fs::create_dir_all(root.join(".git/hooks")).unwrap();
        File::create(root.join(".git/hooks/pre-commit")).unwrap();

        // Create binary file
        File::create(root.join("src/image.png")).unwrap();

        // Create large file (> 2MB)
        let large_path = root.join("src/large.txt");
        let mut large_file = File::create(large_path).unwrap();
        large_file.write_all(&vec![0u8; 2_000_001]).unwrap();

        // Create file excluded by .gitignore
        let gitignore_path = root.join(".gitignore");
        let mut gitignore_file = File::create(gitignore_path).unwrap();
        writeln!(gitignore_file, "ignored.txt").unwrap();
        writeln!(gitignore_file, "target/").unwrap();

        File::create(root.join("ignored.txt")).unwrap();

        fs::create_dir_all(root.join("target/debug")).unwrap();
        File::create(root.join("target/debug/core-parser")).unwrap();

        // Run crawler
        let crawler = WorkspaceCrawler::new(root.to_path_buf(), vec![]);
        let paths = crawler.crawl().unwrap();

        // Expectations
        let expected = vec![
            PathBuf::from(".github/workflows/ci.yml"),
            PathBuf::from(".gitignore"),
            PathBuf::from("src/lib.rs"),
            PathBuf::from("src/main.rs"),
        ];

        assert_eq!(paths, expected);
    }

    #[test]
    fn test_crawler_custom_exclusions() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        fs::create_dir_all(root.join("src")).unwrap();
        File::create(root.join("src/main.rs")).unwrap();

        fs::create_dir_all(root.join("temp_config")).unwrap();
        File::create(root.join("temp_config/app.json")).unwrap();

        // Crawl with custom exclude for "temp_config"
        let crawler = WorkspaceCrawler::new(root.to_path_buf(), vec!["temp_config".to_string()]);
        let paths = crawler.crawl().unwrap();

        let expected = vec![PathBuf::from("src/main.rs")];
        assert_eq!(paths, expected);
    }
}
