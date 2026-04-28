//! Cross-file Java index for call graph walking and reference search.
//!
//! Provides the `JavaIndex` struct which pre-parses all `.java` files in a
//! project directory using tree-sitter and builds lookup tables for:
//! - Which types are declared in each file
//! - What imports each file has (for unqualified name resolution)
//! - Method invocations and type references across files
//!
//! This enables the BU pipeline to trace private function breakage up
//! to public API surfaces via `find_callers`, and to find all usage
//! sites of a symbol via `find_references`.

use anyhow::{Context, Result};
use semver_analyzer_core::{Caller, Reference, Visibility};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tree_sitter::{Node, Parser};

/// Information about a Java type declared in a file.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct TypeInfo {
    /// Simple class name (e.g., `"Foo"`).
    name: String,
    /// Fully qualified name (e.g., `"com.example.Foo"`).
    qualified_name: String,
    /// Source file path (relative).
    file: PathBuf,
}

/// Information about a method declared in a file.
#[derive(Debug, Clone)]
struct MethodInfo {
    /// Simple method name.
    name: String,
    /// Qualified: `com.example.Foo::doThing`.
    qualified_name: String,
    /// Enclosing type's qualified name.
    enclosing_type: String,
    /// Source file path.
    file: PathBuf,
    /// Line number.
    line: usize,
    /// Visibility.
    visibility: Visibility,
    /// Method body source text.
    body: String,
    /// Method signature (everything before body).
    signature: String,
}

/// Pre-built index of a Java project for cross-file lookups.
#[allow(dead_code)]
pub struct JavaIndex {
    /// Map: simple type name → list of TypeInfo (may have collisions across packages).
    types_by_name: HashMap<String, Vec<TypeInfo>>,
    /// Map: file path → list of imports (simple name → qualified name).
    imports_by_file: HashMap<PathBuf, HashMap<String, String>>,
    /// Map: file path → package name.
    packages_by_file: HashMap<PathBuf, String>,
    /// All methods indexed, grouped by simple method name.
    methods_by_name: HashMap<String, Vec<MethodInfo>>,
}

impl JavaIndex {
    /// Build an index of all Java files under `root`.
    pub fn build(root: &Path) -> Result<Self> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_java::LANGUAGE.into())
            .context("Failed to set tree-sitter Java language")?;

        let java_files = find_java_files(root)?;

        let mut types_by_name: HashMap<String, Vec<TypeInfo>> = HashMap::new();
        let mut imports_by_file: HashMap<PathBuf, HashMap<String, String>> = HashMap::new();
        let mut packages_by_file: HashMap<PathBuf, String> = HashMap::new();
        let mut methods_by_name: HashMap<String, Vec<MethodInfo>> = HashMap::new();

        for file_path in &java_files {
            let source = match std::fs::read_to_string(file_path) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let relative = file_path
                .strip_prefix(root)
                .unwrap_or(file_path)
                .to_path_buf();

            let tree = match parser.parse(&source, None) {
                Some(t) => t,
                None => continue,
            };

            let root_node = tree.root_node();
            let package = extract_package(root_node, &source);
            let imports = extract_imports(root_node, &source);

            if let Some(ref pkg) = package {
                packages_by_file.insert(relative.clone(), pkg.clone());
            }
            imports_by_file.insert(relative.clone(), imports);

            // Extract type declarations and methods
            index_declarations(
                root_node,
                &source,
                &relative,
                &package,
                "",
                &mut types_by_name,
                &mut methods_by_name,
            );
        }

        Ok(Self {
            types_by_name,
            imports_by_file,
            packages_by_file,
            methods_by_name,
        })
    }

    /// Find all callers of a method with the given name in the project.
    ///
    /// Scans all indexed methods for invocations of `symbol_name`.
    /// Uses heuristic matching: finds `method_invocation` nodes where
    /// the method name matches, then resolves the receiver type via
    /// variable declarations and imports.
    pub fn find_callers(&self, source_file: &Path, symbol_name: &str) -> Result<Vec<Caller>> {
        // Determine the enclosing type of the target symbol to help
        // with receiver type matching
        let target_type = self.resolve_enclosing_type(source_file, symbol_name);

        let mut callers = Vec::new();

        for methods in self.methods_by_name.values() {
            for method in methods {
                // Don't report the method as its own caller
                if method.file == source_file && method.name == symbol_name {
                    continue;
                }

                // Check if this method's body calls the target symbol
                if self.body_calls_method(
                    &method.body,
                    symbol_name,
                    &method.file,
                    target_type.as_deref(),
                ) {
                    callers.push(Caller {
                        qualified_name: method.qualified_name.clone(),
                        file: method.file.clone(),
                        line: method.line,
                        visibility: method.visibility,
                        body: method.body.clone(),
                        signature: method.signature.clone(),
                    });
                }
            }
        }

        Ok(callers)
    }

    /// Find all references to a symbol across the project.
    pub fn find_references(
        &self,
        source_file: &Path,
        symbol_name: &str,
    ) -> Result<Vec<Reference>> {
        let mut references = Vec::new();

        // Look for the symbol in all indexed methods
        for methods in self.methods_by_name.values() {
            for method in methods {
                // Skip the declaring file
                if method.file == source_file && method.name == symbol_name {
                    continue;
                }

                // Check if the body references the symbol (as a method call,
                // type reference, or field access)
                if method.body.contains(symbol_name) {
                    references.push(Reference {
                        file: method.file.clone(),
                        line: method.line,
                        local_binding: symbol_name.to_string(),
                        enclosing_symbol: Some(method.qualified_name.clone()),
                    });
                }
            }
        }

        // Also check import declarations — if a type is imported, that's a reference
        for (file, imports) in &self.imports_by_file {
            if file == source_file {
                continue;
            }
            if imports.contains_key(symbol_name) {
                references.push(Reference {
                    file: file.clone(),
                    line: 0, // Import line not tracked
                    local_binding: symbol_name.to_string(),
                    enclosing_symbol: None,
                });
            }
        }

        Ok(references)
    }

    /// Resolve the enclosing type of a method by looking up what type
    /// declares a method with the given name in the given file.
    fn resolve_enclosing_type(&self, file: &Path, method_name: &str) -> Option<String> {
        for methods in self.methods_by_name.values() {
            for method in methods {
                if method.file == file && method.name == method_name {
                    return Some(method.enclosing_type.clone());
                }
            }
        }
        None
    }

    /// Check if a method body contains a call to `target_method`.
    ///
    /// Uses simple text matching plus optional receiver type checking.
    /// This is a heuristic — it can't do full type resolution without
    /// a compiler, but catches the common cases:
    /// - `target()` — unqualified call (same class or static import)
    /// - `obj.target()` — qualified call
    /// - `ClassName.target()` — static call
    fn body_calls_method(
        &self,
        body: &str,
        target_method: &str,
        caller_file: &Path,
        target_type: Option<&str>,
    ) -> bool {
        // Quick check: does the body contain the method name at all?
        if !body.contains(target_method) {
            return false;
        }

        // Check for method invocation patterns:
        // 1. `target(` — direct call
        // 2. `.target(` — qualified call
        // 3. `this.target(` — explicit this call
        let call_pattern = format!("{}(", target_method);
        let dot_call = format!(".{}(", target_method);

        if body.contains(&call_pattern) {
            // If no target type constraint, any call pattern matches
            if target_type.is_none() {
                return true;
            }

            // If we have a target type, check if the caller's file imports it
            // or is in the same package
            if let Some(target) = target_type {
                let target_simple = target.rsplit('.').next().unwrap_or(target);

                // Check if the caller file imports the target type
                if let Some(imports) = self.imports_by_file.get(caller_file) {
                    if imports.contains_key(target_simple) {
                        return true;
                    }
                }

                // Check if they're in the same package
                if let Some(caller_pkg) = self.packages_by_file.get(caller_file) {
                    let target_pkg = target.rsplit('.').skip(1).collect::<Vec<_>>();
                    let target_pkg = target_pkg.into_iter().rev().collect::<Vec<_>>().join(".");
                    if *caller_pkg == target_pkg {
                        return true;
                    }
                }

                // Still match if we see a dot-qualified call
                if body.contains(&dot_call) {
                    return true;
                }
            }
        }

        false
    }
}

// ── File discovery (reused from extract) ────────────────────────────────

fn find_java_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    find_java_files_recursive(root, root, &mut files)?;
    Ok(files)
}

fn find_java_files_recursive(root: &Path, dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return Ok(()),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if name_str.starts_with('.') {
            continue;
        }

        if path.is_dir() {
            let rel = path.strip_prefix(root).unwrap_or(&path);
            let rel_str = rel.to_string_lossy();

            // Skip build output and test directories
            if name_str == "target"
                || name_str == "build"
                || name_str == "node_modules"
                || rel_str.contains("/src/test/")
                || rel_str.starts_with("src/test/")
            {
                continue;
            }

            find_java_files_recursive(root, &path, files)?;
        } else if name_str.ends_with(".java")
            && name_str != "package-info.java"
            && name_str != "module-info.java"
        {
            files.push(path);
        }
    }

    Ok(())
}

// ── Tree-sitter extraction for indexing ─────────────────────────────────

fn extract_package(root: Node, source: &str) -> Option<String> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "package_declaration" {
            let mut inner = child.walk();
            for pkg_child in child.children(&mut inner) {
                if pkg_child.kind() == "scoped_identifier" || pkg_child.kind() == "identifier" {
                    return Some(node_text(pkg_child, source).to_string());
                }
            }
        }
    }
    None
}

fn extract_imports(root: Node, source: &str) -> HashMap<String, String> {
    let mut imports = HashMap::new();
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        if child.kind() == "import_declaration" {
            let text = node_text(child, source);
            let trimmed = text
                .trim_start_matches("import ")
                .trim_start_matches("static ")
                .trim_end_matches(';')
                .trim();

            if !trimmed.ends_with('*') {
                if let Some(dot_pos) = trimmed.rfind('.') {
                    let simple_name = &trimmed[dot_pos + 1..];
                    imports.insert(simple_name.to_string(), trimmed.to_string());
                }
            }
        }
    }

    imports
}

fn index_declarations(
    node: Node,
    source: &str,
    file_path: &Path,
    package: &Option<String>,
    parent_class: &str,
    types_by_name: &mut HashMap<String, Vec<TypeInfo>>,
    methods_by_name: &mut HashMap<String, Vec<MethodInfo>>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "class_declaration"
            | "interface_declaration"
            | "enum_declaration"
            | "record_declaration" => {
                let class_name = child
                    .child_by_field_name("name")
                    .map(|n| node_text(n, source))
                    .unwrap_or("");

                let qualified_class = if parent_class.is_empty() {
                    match package {
                        Some(pkg) => format!("{}.{}", pkg, class_name),
                        None => class_name.to_string(),
                    }
                } else {
                    format!("{}.{}", parent_class, class_name)
                };

                types_by_name
                    .entry(class_name.to_string())
                    .or_default()
                    .push(TypeInfo {
                        name: class_name.to_string(),
                        qualified_name: qualified_class.clone(),
                        file: file_path.to_path_buf(),
                    });

                // Index methods within this type
                index_methods_in_type(
                    child,
                    source,
                    file_path,
                    &qualified_class,
                    methods_by_name,
                );

                // Recurse into nested types
                index_declarations(
                    child,
                    source,
                    file_path,
                    package,
                    &qualified_class,
                    types_by_name,
                    methods_by_name,
                );
            }
            _ => {
                index_declarations(
                    child,
                    source,
                    file_path,
                    package,
                    parent_class,
                    types_by_name,
                    methods_by_name,
                );
            }
        }
    }
}

fn index_methods_in_type(
    type_node: Node,
    source: &str,
    file_path: &Path,
    enclosing_type: &str,
    methods_by_name: &mut HashMap<String, Vec<MethodInfo>>,
) {
    let body = match type_node
        .child_by_field_name("body")
        .or_else(|| find_body_node(type_node))
    {
        Some(b) => b,
        None => return,
    };

    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        if child.kind() == "method_declaration" || child.kind() == "constructor_declaration" {
            let name = child
                .child_by_field_name("name")
                .map(|n| node_text(n, source))
                .unwrap_or("");

            if name.is_empty() {
                continue;
            }

            let qualified_name = format!("{}::{}", enclosing_type, name);

            let method_body = find_child_by_kind(child, "block")
                .or_else(|| find_child_by_kind(child, "constructor_body"))
                .map(|n| node_text(n, source))
                .unwrap_or("")
                .to_string();

            let body_start = find_child_by_kind(child, "block")
                .or_else(|| find_child_by_kind(child, "constructor_body"))
                .map(|n| n.start_byte())
                .unwrap_or(child.end_byte());
            let signature = source[child.start_byte()..body_start].trim().to_string();

            let visibility = extract_visibility(child);

            methods_by_name
                .entry(name.to_string())
                .or_default()
                .push(MethodInfo {
                    name: name.to_string(),
                    qualified_name,
                    enclosing_type: enclosing_type.to_string(),
                    file: file_path.to_path_buf(),
                    line: child.start_position().row + 1,
                    visibility,
                    body: method_body,
                    signature,
                });
        }
    }
}

fn extract_visibility(node: Node) -> Visibility {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "modifiers" {
            let mut mod_cursor = child.walk();
            for mod_child in child.children(&mut mod_cursor) {
                match mod_child.kind() {
                    "public" => return Visibility::Public,
                    "protected" => return Visibility::Protected,
                    "private" => return Visibility::Private,
                    _ => {}
                }
            }
        }
    }
    Visibility::Internal
}

fn find_body_node(node: Node) -> Option<Node> {
    find_child_by_kind(node, "class_body")
        .or_else(|| find_child_by_kind(node, "interface_body"))
        .or_else(|| find_child_by_kind(node, "enum_body"))
}

fn node_text<'a>(node: Node, source: &'a str) -> &'a str {
    &source[node.byte_range()]
}

#[allow(clippy::manual_find)]
fn find_child_by_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == kind {
            return Some(child);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_project(files: &[(&str, &str)]) -> TempDir {
        let dir = TempDir::new().unwrap();
        for (path, content) in files {
            let full_path = dir.path().join(path);
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(&full_path, content).unwrap();
        }
        dir
    }

    #[test]
    fn test_index_finds_types() {
        let dir = setup_project(&[
            (
                "src/com/example/Foo.java",
                r#"
                package com.example;
                public class Foo {
                    public void doThing() {}
                }
                "#,
            ),
            (
                "src/com/example/Bar.java",
                r#"
                package com.example;
                public class Bar {
                    public void doOther() {}
                }
                "#,
            ),
        ]);

        let index = JavaIndex::build(dir.path()).unwrap();
        assert!(index.types_by_name.contains_key("Foo"));
        assert!(index.types_by_name.contains_key("Bar"));
        assert!(index.methods_by_name.contains_key("doThing"));
        assert!(index.methods_by_name.contains_key("doOther"));
    }

    #[test]
    fn test_find_callers_basic() {
        let dir = setup_project(&[
            (
                "src/com/example/Service.java",
                r#"
                package com.example;
                public class Service {
                    private void helper() {
                        // private method
                    }
                    public void process() {
                        helper();
                    }
                }
                "#,
            ),
        ]);

        let index = JavaIndex::build(dir.path()).unwrap();
        let callers = index
            .find_callers(Path::new("src/com/example/Service.java"), "helper")
            .unwrap();

        assert_eq!(callers.len(), 1);
        assert!(callers[0].qualified_name.contains("process"));
        assert_eq!(callers[0].visibility, Visibility::Public);
    }

    #[test]
    fn test_find_callers_cross_file() {
        let dir = setup_project(&[
            (
                "src/com/example/Util.java",
                r#"
                package com.example;
                public class Util {
                    public static String format(String s) { return s.trim(); }
                }
                "#,
            ),
            (
                "src/com/example/App.java",
                r#"
                package com.example;
                public class App {
                    public void run() {
                        String result = Util.format("hello");
                    }
                }
                "#,
            ),
        ]);

        let index = JavaIndex::build(dir.path()).unwrap();
        let callers = index
            .find_callers(Path::new("src/com/example/Util.java"), "format")
            .unwrap();

        assert_eq!(callers.len(), 1);
        assert!(callers[0].qualified_name.contains("run"));
    }

    #[test]
    fn test_find_references_via_import() {
        let dir = setup_project(&[
            (
                "src/com/example/Foo.java",
                r#"
                package com.example;
                public class Foo {}
                "#,
            ),
            (
                "src/com/other/Bar.java",
                r#"
                package com.other;
                import com.example.Foo;
                public class Bar {
                    public void use() {
                        Foo f = new Foo();
                    }
                }
                "#,
            ),
        ]);

        let index = JavaIndex::build(dir.path()).unwrap();
        let refs = index
            .find_references(Path::new("src/com/example/Foo.java"), "Foo")
            .unwrap();

        // Should find the import reference
        assert!(!refs.is_empty());
    }
}
