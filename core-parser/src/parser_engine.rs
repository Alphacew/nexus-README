use std::collections::HashSet;
use std::path::{Path, PathBuf};
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser, Query, QueryCursor};

// Compile-time static queries for AST extraction

const TS_QUERY: &str = r#"
;; Classes
(class_declaration
  name: (type_identifier) @class.name)

;; Interfaces
(interface_declaration
  name: (type_identifier) @interface.name)

;; Type Aliases
(type_alias_declaration
  name: (type_identifier) @type.name)

;; Exported Function Declarations
(export_statement
  declaration: (function_declaration
    name: (identifier) @function.exported.name))

;; Exported Generator Functions
(export_statement
  declaration: (generator_function_declaration
    name: (identifier) @function.exported.name))

;; Exported Arrow Functions / Variables
(export_statement
  declaration: (lexical_declaration
    (variable_declarator
      name: (identifier) @function.exported.name
      value: [(arrow_function) (function_expression)])))

;; Imports for Internal Dependency Extraction
(import_statement
  source: (string) @import.source)

;; Environment Variables (matching process.env.VAR)
(member_expression
  object: (member_expression
    object: (identifier) @obj_process
    property: (property_identifier) @prop_env)
  property: (property_identifier) @env.var)
"#;

const PY_QUERY: &str = r#"
;; Module docstring
(module . (expression_statement (string) @module.docstring))

;; Classes
(class_definition
  name: (identifier) @class.name)

;; Functions
(function_definition
  name: (identifier) @function.name)

;; Imports
(import_statement
  (dotted_name) @import.name)
(import_from_statement
  (dotted_name) @import.module)

;; os.environ.get("VAR")
(call
  function: (attribute
    object: (attribute
      object: (identifier) @obj_os
      attribute: (identifier) @attr_environ)
    attribute: (identifier) @method_get)
  arguments: (argument_list (string) @env.var))

;; os.environ["VAR"]
(subscript
  value: (attribute
    object: (identifier) @obj_os
    attribute: (identifier) @attr_environ)
  subscript: (string) @env.var)
"#;

const RS_QUERY: &str = r#"
;; Structs
(struct_item
  name: (type_identifier) @struct.name)

;; Traits
(trait_item
  name: (type_identifier) @trait.name)

;; Pub impl functions
(impl_item
  (declaration_list
    (function_item
      (visibility_modifier) @vis
      (identifier) @function.pub.name)))

;; Use statements (Imports)
(use_declaration
  _ @use.path)

;; Environment Variables (matching env!("VAR"))
(macro_invocation
  (identifier) @macro_name
  (token_tree (string_literal) @env.var))

;; env::var("VAR") or std::env::var("VAR")
(call_expression
  [
    (identifier) @func_name
    (scoped_identifier
      (identifier) @mod_name
      (identifier) @func_name)
    (scoped_identifier
      (scoped_identifier
        (identifier) @std_name
        (identifier) @mod_name)
      (identifier) @func_name)
  ]
  (arguments (string_literal) @env.var))
"#;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ExportInfo {
    pub name: String,
    pub r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ParsedModule {
    pub file_path: PathBuf,
    pub language: String,
    pub exports: Vec<ExportInfo>,
    pub internal_dependencies: Vec<String>,
    pub env_variables: Vec<String>,
    pub docstring: Option<String>,
}

/// Holds pre-compiled Tree-sitter Queries for supported languages.
pub struct ASTAnalyzer {
    ts_lang: Language,
    ts_query: Query,
    py_lang: Language,
    py_query: Query,
    rs_lang: Language,
    rs_query: Query,
}

impl ASTAnalyzer {
    pub fn new() -> Self {
        let ts_lang = Language::from(tree_sitter_typescript::LANGUAGE_TYPESCRIPT);
        let ts_query = Query::new(&ts_lang, TS_QUERY).expect("Failed to parse TS query");

        let py_lang = Language::from(tree_sitter_python::LANGUAGE);
        let py_query = Query::new(&py_lang, PY_QUERY).expect("Failed to parse Python query");

        let rs_lang = Language::from(tree_sitter_rust::LANGUAGE);
        let rs_query = Query::new(&rs_lang, RS_QUERY).expect("Failed to parse Rust query");

        Self {
            ts_lang,
            ts_query,
            py_lang,
            py_query,
            rs_lang,
            rs_query,
        }
    }

    /// Detects language from file extension.
    pub fn detect_language(&self, path: &Path) -> Option<&'static str> {
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("rs") => Some("rust"),
            Some("py") => Some("python"),
            Some("ts") | Some("tsx") | Some("js") | Some("jsx") => Some("typescript"),
            _ => None,
        }
    }

    /// Analyzes a single file's AST using the reusable thread-local parser/cursor state.
    pub fn analyze_file(
        &self,
        path: &Path,
        root_path: &Path,
        parser: &mut Parser,
        cursor: &mut QueryCursor,
    ) -> Result<ParsedModule, Box<dyn std::error::Error + Send + Sync>> {
        let content = std::fs::read_to_string(path)?;
        let source_bytes = content.as_bytes();

        let lang_str = self
            .detect_language(path)
            .ok_or_else(|| format!("Unsupported file type: {:?}", path))?;

        let (lang, query) = match lang_str {
            "rust" => (self.rs_lang.clone(), &self.rs_query),
            "python" => (self.py_lang.clone(), &self.py_query),
            "typescript" => (self.ts_lang.clone(), &self.ts_query),
            _ => unreachable!(),
        };

        parser.set_language(&lang)?;
        let tree = parser
            .parse(&content, None)
            .ok_or_else(|| format!("Failed to parse file: {:?}", path))?;

        let mut exports = Vec::new();
        let mut imports = Vec::new();
        let mut env_variables = HashSet::new();
        let mut docstring = None;

        let mut matches = cursor.matches(query, tree.root_node(), source_bytes);

        while let Some(r#match) = matches.next() {
            let mut captures = std::collections::HashMap::new();
            for capture in r#match.captures {
                let name = &query.capture_names()[capture.index as usize];
                captures.insert(*name, capture.node);
            }

            // Extract Environment Variables
            if let Some(env_node) = captures.get("env.var") {
                let mut env_name = env_node.utf8_text(source_bytes)?.to_string();
                // Strip quotes if literal string is captured
                if (env_name.starts_with('"') && env_name.ends_with('"'))
                    || (env_name.starts_with('\'') && env_name.ends_with('\''))
                {
                    env_name = env_name[1..env_name.len() - 1].to_string();
                }

                // Check language-specific environment conditions
                let is_valid = match lang_str {
                    "typescript" => {
                        captures
                            .get("obj_process")
                            .is_some_and(|n| n.utf8_text(source_bytes).unwrap_or("") == "process")
                            && captures
                                .get("prop_env")
                                .is_some_and(|n| n.utf8_text(source_bytes).unwrap_or("") == "env")
                    }
                    "python" => {
                        captures
                            .get("obj_os")
                            .is_some_and(|n| n.utf8_text(source_bytes).unwrap_or("") == "os")
                            && captures.get("attr_environ").is_some_and(|n| {
                                n.utf8_text(source_bytes).unwrap_or("") == "environ"
                            })
                    }
                    "rust" => {
                        if let Some(macro_node) = captures.get("macro_name") {
                            macro_node.utf8_text(source_bytes).unwrap_or("") == "env"
                        } else if let Some(func_node) = captures.get("func_name") {
                            func_node.utf8_text(source_bytes).unwrap_or("") == "var"
                                && captures.get("mod_name").is_none_or(|n| {
                                    n.utf8_text(source_bytes).unwrap_or("") == "env"
                                })
                        } else {
                            false
                        }
                    }
                    _ => false,
                };

                if is_valid {
                    env_variables.insert(env_name);
                }
            }

            // Extract Imports / Dependencies
            if let Some(import_node) = captures.get("import.source") {
                let mut source = import_node.utf8_text(source_bytes)?.to_string();
                if (source.starts_with('"') && source.ends_with('"'))
                    || (source.starts_with('\'') && source.ends_with('\''))
                {
                    source = source[1..source.len() - 1].to_string();
                }
                imports.push(source);
            } else if let Some(import_node) = captures.get("import.name") {
                imports.push(import_node.utf8_text(source_bytes)?.to_string());
            } else if let Some(import_node) = captures.get("import.module") {
                imports.push(import_node.utf8_text(source_bytes)?.to_string());
            } else if let Some(use_node) = captures.get("use.path") {
                imports.push(use_node.utf8_text(source_bytes)?.to_string());
            }

            // Extract Docstrings
            if let Some(doc_node) = captures.get("module.docstring") {
                let mut doc = doc_node.utf8_text(source_bytes)?.to_string();
                // Strip python triple quotes (e.g. """ or ''')
                if (doc.starts_with("\"\"\"") && doc.ends_with("\"\"\""))
                    || (doc.starts_with("'''") && doc.ends_with("'''"))
                {
                    doc = doc[3..doc.len() - 3].trim().to_string();
                } else if (doc.starts_with('"') && doc.ends_with('"'))
                    || (doc.starts_with('\'') && doc.ends_with('\''))
                {
                    doc = doc[1..doc.len() - 1].trim().to_string();
                }
                docstring = Some(doc);
            }

            // Extract Exports (Classes, Functions, Structs, etc.)
            let mut export_entry = None;

            if let Some(name_node) = captures.get("class.name") {
                export_entry = Some(ExportInfo {
                    name: name_node.utf8_text(source_bytes)?.to_string(),
                    r#type: "class".to_string(),
                    description: None,
                    meta: None,
                });
            } else if let Some(name_node) = captures.get("interface.name") {
                export_entry = Some(ExportInfo {
                    name: name_node.utf8_text(source_bytes)?.to_string(),
                    r#type: "interface".to_string(),
                    description: None,
                    meta: None,
                });
            } else if let Some(name_node) = captures.get("type.name") {
                export_entry = Some(ExportInfo {
                    name: name_node.utf8_text(source_bytes)?.to_string(),
                    r#type: "type".to_string(),
                    description: None,
                    meta: None,
                });
            } else if let Some(name_node) = captures.get("function.exported.name") {
                export_entry = Some(ExportInfo {
                    name: name_node.utf8_text(source_bytes)?.to_string(),
                    r#type: "function".to_string(),
                    description: None,
                    meta: None,
                });
            } else if let Some(name_node) = captures.get("function.name") {
                // In Python, we extract all functions (and optionally classes)
                export_entry = Some(ExportInfo {
                    name: name_node.utf8_text(source_bytes)?.to_string(),
                    r#type: "function".to_string(),
                    description: None,
                    meta: None,
                });
            } else if let Some(name_node) = captures.get("struct.name") {
                export_entry = Some(ExportInfo {
                    name: name_node.utf8_text(source_bytes)?.to_string(),
                    r#type: "struct".to_string(),
                    description: None,
                    meta: None,
                });
            } else if let Some(name_node) = captures.get("trait.name") {
                export_entry = Some(ExportInfo {
                    name: name_node.utf8_text(source_bytes)?.to_string(),
                    r#type: "trait".to_string(),
                    description: None,
                    meta: None,
                });
            } else if let Some(name_node) = captures.get("function.pub.name") {
                // Rust pub impl functions: check if vis node is actually "pub"
                let is_pub = captures
                    .get("vis")
                    .is_some_and(|v| v.utf8_text(source_bytes).unwrap_or("") == "pub");
                if is_pub {
                    export_entry = Some(ExportInfo {
                        name: name_node.utf8_text(source_bytes)?.to_string(),
                        r#type: "function".to_string(),
                        description: None,
                        meta: None,
                    });
                }
            }

            if let Some(entry) = export_entry {
                // Deduplicate exports just in case (e.g. multiple matches from overlapping rules)
                if !exports
                    .iter()
                    .any(|e: &ExportInfo| e.name == entry.name && e.r#type == entry.r#type)
                {
                    exports.push(entry);
                }
            }
        }

        // Get relative path for clean CodebaseTopology serialization
        let rel_path = path.strip_prefix(root_path).unwrap_or(path).to_path_buf();

        Ok(ParsedModule {
            file_path: rel_path,
            language: lang_str.to_string(),
            exports,
            internal_dependencies: imports,
            env_variables: env_variables.into_iter().collect(),
            docstring,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;
    use tree_sitter::{Parser, QueryCursor};

    #[test]
    fn test_ast_extraction_contract() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();

        // 1. Create a TypeScript file with exported entities and imports
        let ts_code = r#"
            import { Helper } from "./utils";
            export interface Config {
                port: number;
            }
            export class Server {
                start() {
                    const token = process.env.API_TOKEN;
                }
            }
            export function runServer(): void {}
        "#;
        let ts_path = root.join("server.ts");
        fs::write(&ts_path, ts_code).unwrap();

        // 2. Create a Python file with docstring, classes, functions, and env vars
        let py_code = r#"
            """Module docstring for python server"""
            import sys
            import os

            class Client:
                def __init__(self):
                    self.key = os.environ.get("SECRET_KEY")
                    self.token = os.environ["API_TOKEN"]

            def connect():
                pass
        "#;
        let py_path = root.join("client.py");
        fs::write(&py_path, py_code).unwrap();

        // 3. Create a Rust file
        let rs_code = r#"
            use std::env;
            pub struct DB;
            pub trait Connection {}
            impl DB {
                pub fn connect() {
                    let db_url = env!("DATABASE_URL");
                    let host = env::var("DB_HOST").unwrap();
                }
            }
        "#;
        let rs_path = root.join("db.rs");
        fs::write(&rs_path, rs_code).unwrap();

        // 4. Initialize ASTAnalyzer and process these files
        let analyzer = ASTAnalyzer::new();

        let mut parser = Parser::new();
        let mut cursor = QueryCursor::new();

        let ts_info = analyzer
            .analyze_file(&ts_path, root, &mut parser, &mut cursor)
            .unwrap();
        let py_info = analyzer
            .analyze_file(&py_path, root, &mut parser, &mut cursor)
            .unwrap();
        let rs_info = analyzer
            .analyze_file(&rs_path, root, &mut parser, &mut cursor)
            .unwrap();

        // 5. Verification of TypeScript exports and env variables
        assert_eq!(ts_info.language, "typescript");
        assert!(
            ts_info
                .exports
                .iter()
                .any(|e| e.name == "Config" && e.r#type == "interface")
        );
        assert!(
            ts_info
                .exports
                .iter()
                .any(|e| e.name == "Server" && e.r#type == "class")
        );
        assert!(
            ts_info
                .exports
                .iter()
                .any(|e| e.name == "runServer" && e.r#type == "function")
        );
        assert!(ts_info.env_variables.contains(&"API_TOKEN".to_string()));
        assert!(
            ts_info
                .internal_dependencies
                .contains(&"./utils".to_string())
        );

        // 6. Verification of Python docstring and env variables
        assert_eq!(py_info.language, "python");
        assert_eq!(
            py_info.docstring.as_deref(),
            Some("Module docstring for python server")
        );
        assert!(
            py_info
                .exports
                .iter()
                .any(|e| e.name == "Client" && e.r#type == "class")
        );
        assert!(
            py_info
                .exports
                .iter()
                .any(|e| e.name == "connect" && e.r#type == "function")
        );
        assert!(py_info.env_variables.contains(&"SECRET_KEY".to_string()));
        assert!(py_info.env_variables.contains(&"API_TOKEN".to_string()));
        assert!(py_info.internal_dependencies.contains(&"sys".to_string()));
        assert!(py_info.internal_dependencies.contains(&"os".to_string()));

        // 7. Verification of Rust exports and env variables
        assert_eq!(rs_info.language, "rust");
        assert!(
            rs_info
                .exports
                .iter()
                .any(|e| e.name == "DB" && e.r#type == "struct")
        );
        assert!(
            rs_info
                .exports
                .iter()
                .any(|e| e.name == "Connection" && e.r#type == "trait")
        );
        assert!(
            rs_info
                .exports
                .iter()
                .any(|e| e.name == "connect" && e.r#type == "function")
        );
        assert!(rs_info.env_variables.contains(&"DATABASE_URL".to_string()));
        assert!(rs_info.env_variables.contains(&"DB_HOST".to_string()));
        assert!(
            rs_info
                .internal_dependencies
                .contains(&"std::env".to_string())
        );
    }
}
