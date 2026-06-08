#![allow(clippy::collapsible_if)]

mod crawler;
mod parser_engine;

use clap::Parser;
use crawler::WorkspaceCrawler;
use parser_engine::{ASTAnalyzer, ExportInfo, ParsedModule};
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(
    name = "core-parser",
    version,
    about = "High-performance codebase analysis engine"
)]
struct Args {
    /// The root workspace directory to crawl
    #[arg(default_value = ".")]
    dir: PathBuf,

    /// Custom directories/paths to exclude (comma-separated list)
    #[arg(short, long, value_delimiter = ',')]
    exclude: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct GitMetadata {
    latest_commits: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct TopologyModule {
    file_path: String,
    language: String,
    exports: Vec<ExportInfo>,
    internal_dependencies: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CodebaseTopology {
    project_name: String,
    entry_points: Vec<String>,
    dependencies: HashMap<String, String>,
    modules: Vec<TopologyModule>,
    environment_variables: Vec<String>,
    git_metadata: GitMetadata,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Verify target directory exists
    if !args.dir.exists() {
        return Err(format!("Target directory does not exist: {:?}", args.dir).into());
    }

    // 1. Crawl workspace to gather file list
    let crawler = WorkspaceCrawler::new(args.dir.clone(), args.exclude);
    let file_list = crawler.crawl()?;

    // 2. Initialize AST parser engine
    let analyzer = Arc::new(ASTAnalyzer::new());

    // 3. Process candidate files in parallel using rayon with thread-local parser state
    let parsed_modules: Vec<ParsedModule> = file_list
        .par_iter()
        .map_init(
            || (tree_sitter::Parser::new(), tree_sitter::QueryCursor::new()),
            |state, file| {
                let (parser, cursor) = state;
                let full_path = args.dir.join(file);
                analyzer
                    .analyze_file(&full_path, &args.dir, parser, cursor)
                    .ok()
            },
        )
        .flatten()
        .collect();

    // 4. Resolve environment variables across all modules
    let mut environment_variables = HashSet::new();
    for module in &parsed_modules {
        for env in &module.env_variables {
            environment_variables.insert(env.clone());
        }
    }
    let mut environment_variables: Vec<String> = environment_variables.into_iter().collect();
    environment_variables.sort();

    // 5. Map ParsedModule into TopologyModule with resolved internal dependencies
    let file_set: HashSet<PathBuf> = file_list.iter().cloned().collect();
    let file_strs: Vec<String> = file_list
        .iter()
        .map(|f| f.to_string_lossy().replace('\\', "/"))
        .collect();

    let modules: Vec<TopologyModule> = parsed_modules
        .iter()
        .map(|module| {
            let internal_deps = resolve_internal_dependencies(
                &module.file_path,
                &module.internal_dependencies,
                &file_set,
                &file_strs,
            );
            TopologyModule {
                file_path: module.file_path.to_string_lossy().replace('\\', "/"),
                language: module.language.clone(),
                exports: module.exports.clone(),
                internal_dependencies: internal_deps,
            }
        })
        .collect();

    // 6. Gather general metadata
    let project_name = get_project_name(&args.dir);
    let entry_points = get_entry_points(&args.dir, &file_list);
    let dependencies = get_dependencies(&args.dir);
    let git_metadata = get_git_metadata(&args.dir);

    // 7. Assemble Topology
    let topology = CodebaseTopology {
        project_name,
        entry_points,
        dependencies,
        modules,
        environment_variables,
        git_metadata,
    };

    // 8. Output strict JSON format
    let json_output = serde_json::to_string_pretty(&topology)?;
    println!("{}", json_output);

    Ok(())
}

/// Resolves raw imports to relative workspace file paths.
fn resolve_internal_dependencies(
    module_path: &Path,
    raw_imports: &[String],
    file_set: &HashSet<PathBuf>,
    file_strs: &[String],
) -> Vec<String> {
    let mut resolved = HashSet::new();
    let module_dir = module_path.parent().unwrap_or_else(|| Path::new(""));

    for import in raw_imports {
        let import_trimmed = import.trim();
        if import_trimmed.is_empty() {
            continue;
        }

        // Case 1: Relative imports (e.g. `./utils`, `../helper`)
        if import_trimmed.starts_with('.') {
            let potential_rel = module_dir.join(import_trimmed);

            // Try matching as is, and with various source extensions
            for ext in &["", ".ts", ".tsx", ".js", ".jsx", ".rs", ".py"] {
                let check_path = if ext.is_empty() {
                    potential_rel.clone()
                } else {
                    let mut p = potential_rel.clone();
                    if let Some(os_str) = p.file_name() {
                        let mut new_name = os_str.to_os_string();
                        new_name.push(ext);
                        p.set_file_name(new_name);
                    }
                    p
                };

                let clean_path = clean_path_components(&check_path);
                if file_set.contains(&clean_path) {
                    resolved.insert(clean_path.to_string_lossy().replace('\\', "/"));
                    break;
                }
            }
            continue;
        }

        // Rust module path resolution (e.g. `crate::crawler::WorkspaceCrawler` or `crawler::WorkspaceCrawler`)
        if import_trimmed.contains("::") {
            let segments: Vec<&str> = import_trimmed.split("::").collect();
            let mut start_idx = 0;
            while start_idx < segments.len()
                && (segments[start_idx] == "crate"
                    || segments[start_idx] == "self"
                    || segments[start_idx] == "super")
            {
                start_idx += 1;
            }
            if start_idx < segments.len() {
                let mod_name = segments[start_idx];
                let suffix1 = format!("/{}.rs", mod_name);
                let suffix2 = format!("/{}/mod.rs", mod_name);
                let exact = format!("{}.rs", mod_name);

                for file_str in file_strs {
                    if file_str.ends_with(&suffix1)
                        || file_str.ends_with(&suffix2)
                        || file_str == &exact
                    {
                        resolved.insert(file_str.clone());
                        break;
                    }
                }
            }
            continue;
        }

        // Case 2: Full module paths matching workspace layout (suffix checks)
        let import_normalized = import_trimmed.replace('\\', "/");
        let mut extensions = vec![import_normalized.clone()];
        for ext in &[".ts", ".tsx", ".js", ".jsx", ".rs", ".py"] {
            extensions.push(format!("{}{}", import_normalized, ext));
        }

        let mut found = false;
        for file_str in file_strs {
            for test_import in &extensions {
                if file_str == test_import || file_str.ends_with(&format!("/{}", test_import)) {
                    resolved.insert(file_str.clone());
                    found = true;
                    break;
                }
            }
            if found {
                break;
            }
        }
    }

    let mut result: Vec<String> = resolved.into_iter().collect();
    result.sort();
    result
}

/// Normalizes path steps (e.g., resolves dots and parent levels).
fn clean_path_components(path: &Path) -> PathBuf {
    let mut clean = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                clean.pop();
            }
            std::path::Component::Normal(c) => {
                clean.push(c);
            }
            std::path::Component::CurDir => {}
            _ => {}
        }
    }
    clean
}

/// Extracts project name from configuration files or folder name.
fn get_project_name(root_path: &Path) -> String {
    if let Ok(toml_content) = std::fs::read_to_string(root_path.join("Cargo.toml")) {
        for line in toml_content.lines() {
            let line = line.trim();
            if line.starts_with("name") {
                if let Some(val) = line.split('=').nth(1) {
                    let name = val.trim().trim_matches('"').trim_matches('\'').trim();
                    if !name.is_empty() {
                        return name.to_string();
                    }
                }
            }
        }
    }

    if let Ok(json_content) = std::fs::read_to_string(root_path.join("package.json")) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_content) {
            if let Some(name) = val.get("name").and_then(|n| n.as_str()) {
                if !name.is_empty() {
                    return name.to_string();
                }
            }
        }
    }

    root_path
        .canonicalize()
        .unwrap_or_else(|_| root_path.to_path_buf())
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown-project")
        .to_string()
}

/// Searches for standard codebase entry files.
fn get_entry_points(_root_path: &Path, file_list: &[PathBuf]) -> Vec<String> {
    let common_entry_files = [
        "src/main.rs",
        "src/lib.rs",
        "src/index.ts",
        "src/index.js",
        "src/main.ts",
        "src/main.js",
        "main.py",
        "app.py",
        "index.js",
        "index.ts",
    ];
    let mut entries = Vec::new();
    for entry in common_entry_files {
        let p = Path::new(entry);
        if file_list.iter().any(|f| f == p) {
            entries.push(entry.to_string());
        }
    }
    entries
}

/// Gathers dependency list from manifest files.
fn get_dependencies(root_path: &Path) -> HashMap<String, String> {
    let mut deps = HashMap::new();

    // Parse Cargo.toml dependencies
    if let Ok(toml_content) = std::fs::read_to_string(root_path.join("Cargo.toml")) {
        let mut in_deps = false;
        let mut in_dev_deps = false;
        for line in toml_content.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                in_deps = line.contains("dependencies");
                in_dev_deps = line.contains("dev-dependencies");
                continue;
            }
            if (in_deps || in_dev_deps) && !line.is_empty() && !line.starts_with('#') {
                if let Some(idx) = line.find('=') {
                    let key = line[..idx].trim().to_string();
                    let val = line[idx + 1..]
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'')
                        .trim_matches('{')
                        .trim_matches('}')
                        .trim()
                        .to_string();
                    deps.insert(key, val);
                }
            }
        }
    }

    // Parse package.json dependencies
    if let Ok(json_content) = std::fs::read_to_string(root_path.join("package.json")) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_content) {
            if let Some(obj) = val.get("dependencies").and_then(|d| d.as_object()) {
                for (k, v) in obj {
                    if let Some(v_str) = v.as_str() {
                        deps.insert(k.clone(), v_str.to_string());
                    }
                }
            }
            if let Some(obj) = val.get("devDependencies").and_then(|d| d.as_object()) {
                for (k, v) in obj {
                    if let Some(v_str) = v.as_str() {
                        deps.insert(k.clone(), v_str.to_string());
                    }
                }
            }
        }
    }

    // Parse requirements.txt (Python)
    if let Ok(reqs_content) = std::fs::read_to_string(root_path.join("requirements.txt")) {
        for line in reqs_content.lines() {
            let line = line.trim();
            if !line.is_empty() && !line.starts_with('#') {
                let parts: Vec<&str> = if line.contains("==") {
                    line.split("==").collect()
                } else if line.contains(">=") {
                    line.split(">=").collect()
                } else {
                    vec![line, "*"]
                };
                if !parts[0].is_empty() {
                    let key = parts[0].trim().to_string();
                    let val = parts.get(1).map(|v| v.trim()).unwrap_or("*").to_string();
                    deps.insert(key, val);
                }
            }
        }
    }

    deps
}

/// Runs localized Git commands to extract history.
fn get_git_metadata(root_path: &Path) -> GitMetadata {
    let mut latest_commits = Vec::new();

    if root_path.join(".git").exists() {
        if let Ok(output) = std::process::Command::new("git")
            .arg("log")
            .arg("-n")
            .arg("50")
            .arg("--format=%s")
            .current_dir(root_path)
            .output()
        {
            if output.status.success() {
                let commits_str = String::from_utf8_lossy(&output.stdout);
                for line in commits_str.lines() {
                    let commit_msg = line.trim().to_string();
                    if !commit_msg.is_empty() {
                        latest_commits.push(commit_msg);
                    }
                }
            }
        }
    }

    GitMetadata { latest_commits }
}
