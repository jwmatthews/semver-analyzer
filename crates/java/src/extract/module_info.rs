//! Java `module-info.java` extraction.
//!
//! Parses module declarations and extracts directives as symbols:
//! - `exports` → exported packages (removing one is breaking)
//! - `requires` → module dependencies (adding `requires transitive` affects downstream)
//! - `opens` → reflectively accessible packages
//! - `provides ... with` → service provider declarations
//! - `uses` → service consumer declarations

use crate::types::JavaSymbolData;
use semver_analyzer_core::{Symbol, SymbolKind, Visibility};
use std::path::Path;
use tree_sitter::Node;

/// Directive types in a module declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleDirectiveKind {
    Exports,
    ExportsTo,
    Requires,
    RequiresTransitive,
    Opens,
    OpensTo,
    Provides,
    Uses,
}

impl std::fmt::Display for ModuleDirectiveKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exports => write!(f, "exports"),
            Self::ExportsTo => write!(f, "exports-to"),
            Self::Requires => write!(f, "requires"),
            Self::RequiresTransitive => write!(f, "requires-transitive"),
            Self::Opens => write!(f, "opens"),
            Self::OpensTo => write!(f, "opens-to"),
            Self::Provides => write!(f, "provides"),
            Self::Uses => write!(f, "uses"),
        }
    }
}

/// Extract module declaration from a `module-info.java` file.
///
/// Returns a single `Symbol` with kind `Namespace` representing the module,
/// with each directive as a member symbol.
pub fn extract_module_info(
    root: Node,
    source: &str,
    file_path: &Path,
) -> Option<Symbol<JavaSymbolData>> {
    // Find the module_declaration node
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "module_declaration" {
            return extract_module_declaration(child, source, file_path);
        }
    }
    None
}

fn extract_module_declaration(
    node: Node,
    source: &str,
    file_path: &Path,
) -> Option<Symbol<JavaSymbolData>> {
    let module_name = node
        .child_by_field_name("name")
        .map(|n| node_text(n, source).to_string())?;

    let mut sym = Symbol::new(
        &module_name,
        &module_name,
        SymbolKind::Namespace,
        Visibility::Public,
        file_path,
        node.start_position().row + 1,
    );

    sym.language_data = JavaSymbolData::default();
    sym.import_path = Some(module_name.clone());

    // Extract directives from the module body
    if let Some(body) = node.child_by_field_name("body") {
        let mut body_cursor = body.walk();
        for directive in body.children(&mut body_cursor) {
            match directive.kind() {
                "requires_module_directive" => {
                    if let Some(member) =
                        extract_requires_directive(directive, source, file_path, &module_name)
                    {
                        sym.members.push(member);
                    }
                }
                "exports_module_directive" => {
                    if let Some(member) =
                        extract_exports_directive(directive, source, file_path, &module_name)
                    {
                        sym.members.push(member);
                    }
                }
                "opens_module_directive" => {
                    if let Some(member) =
                        extract_opens_directive(directive, source, file_path, &module_name)
                    {
                        sym.members.push(member);
                    }
                }
                "provides_module_directive" => {
                    if let Some(member) =
                        extract_provides_directive(directive, source, file_path, &module_name)
                    {
                        sym.members.push(member);
                    }
                }
                "uses_module_directive" => {
                    if let Some(member) =
                        extract_uses_directive(directive, source, file_path, &module_name)
                    {
                        sym.members.push(member);
                    }
                }
                _ => {}
            }
        }
    }

    Some(sym)
}

fn extract_requires_directive(
    node: Node,
    source: &str,
    file_path: &Path,
    module_name: &str,
) -> Option<Symbol<JavaSymbolData>> {
    let required_module = node
        .child_by_field_name("module")
        .map(|n| node_text(n, source).to_string())?;

    // Check for `transitive` modifier
    let is_transitive = has_child_kind(node, "requires_modifier");
    let directive_kind = if is_transitive {
        ModuleDirectiveKind::RequiresTransitive
    } else {
        ModuleDirectiveKind::Requires
    };

    let name = if is_transitive {
        format!("requires transitive {}", required_module)
    } else {
        format!("requires {}", required_module)
    };
    let qualified = format!("{}.requires.{}", module_name, required_module);

    let mut sym = Symbol::new(
        &name,
        &qualified,
        SymbolKind::Property, // directives are properties of the module
        Visibility::Public,
        file_path,
        node.start_position().row + 1,
    );

    sym.language_data = JavaSymbolData::default();
    // Store the directive kind in the extends field for easy access during diffing
    sym.extends = Some(directive_kind.to_string());

    Some(sym)
}

fn extract_exports_directive(
    node: Node,
    source: &str,
    file_path: &Path,
    module_name: &str,
) -> Option<Symbol<JavaSymbolData>> {
    let package = node
        .child_by_field_name("package")
        .map(|n| node_text(n, source).to_string())?;

    // Check for qualified export (`exports ... to ...`)
    let target_modules = node
        .child_by_field_name("modules")
        .map(|n| node_text(n, source).to_string());

    let directive_kind = if target_modules.is_some() {
        ModuleDirectiveKind::ExportsTo
    } else {
        ModuleDirectiveKind::Exports
    };

    let name = match &target_modules {
        Some(targets) => format!("exports {} to {}", package, targets),
        None => format!("exports {}", package),
    };
    let qualified = format!("{}.exports.{}", module_name, package);

    let mut sym = Symbol::new(
        &name,
        &qualified,
        SymbolKind::Property,
        Visibility::Public,
        file_path,
        node.start_position().row + 1,
    );

    sym.language_data = JavaSymbolData::default();
    sym.extends = Some(directive_kind.to_string());

    Some(sym)
}

fn extract_opens_directive(
    node: Node,
    source: &str,
    file_path: &Path,
    module_name: &str,
) -> Option<Symbol<JavaSymbolData>> {
    let package = node
        .child_by_field_name("package")
        .map(|n| node_text(n, source).to_string())?;

    let target_modules = node
        .child_by_field_name("modules")
        .map(|n| node_text(n, source).to_string());

    let directive_kind = if target_modules.is_some() {
        ModuleDirectiveKind::OpensTo
    } else {
        ModuleDirectiveKind::Opens
    };

    let name = match &target_modules {
        Some(targets) => format!("opens {} to {}", package, targets),
        None => format!("opens {}", package),
    };
    let qualified = format!("{}.opens.{}", module_name, package);

    let mut sym = Symbol::new(
        &name,
        &qualified,
        SymbolKind::Property,
        Visibility::Public,
        file_path,
        node.start_position().row + 1,
    );

    sym.language_data = JavaSymbolData::default();
    sym.extends = Some(directive_kind.to_string());

    Some(sym)
}

fn extract_provides_directive(
    node: Node,
    source: &str,
    file_path: &Path,
    module_name: &str,
) -> Option<Symbol<JavaSymbolData>> {
    let provided = node
        .child_by_field_name("provided")
        .map(|n| node_text(n, source).to_string())?;

    // Collect implementation classes after `with`
    let mut impls = Vec::new();
    let mut found_with = false;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "with" {
            found_with = true;
        } else if found_with
            && (child.kind() == "scoped_identifier" || child.kind() == "identifier")
        {
            impls.push(node_text(child, source).to_string());
        }
    }

    let name = format!("provides {} with {}", provided, impls.join(", "));
    let qualified = format!("{}.provides.{}", module_name, provided);

    let mut sym = Symbol::new(
        &name,
        &qualified,
        SymbolKind::Property,
        Visibility::Public,
        file_path,
        node.start_position().row + 1,
    );

    sym.language_data = JavaSymbolData::default();
    sym.extends = Some(ModuleDirectiveKind::Provides.to_string());
    sym.implements = impls;

    Some(sym)
}

fn extract_uses_directive(
    node: Node,
    source: &str,
    file_path: &Path,
    module_name: &str,
) -> Option<Symbol<JavaSymbolData>> {
    let service_type = node
        .child_by_field_name("type")
        .map(|n| node_text(n, source).to_string())?;

    let name = format!("uses {}", service_type);
    let qualified = format!("{}.uses.{}", module_name, service_type);

    let mut sym = Symbol::new(
        &name,
        &qualified,
        SymbolKind::Property,
        Visibility::Public,
        file_path,
        node.start_position().row + 1,
    );

    sym.language_data = JavaSymbolData::default();
    sym.extends = Some(ModuleDirectiveKind::Uses.to_string());

    Some(sym)
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn node_text<'a>(node: Node, source: &'a str) -> &'a str {
    &source[node.byte_range()]
}

fn has_child_kind(node: Node, kind: &str) -> bool {
    let mut cursor = node.walk();
    let result = node.children(&mut cursor).any(|c| c.kind() == kind);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use tree_sitter::Parser;

    fn parse_module(source: &str) -> Option<Symbol<JavaSymbolData>> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_java::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        extract_module_info(tree.root_node(), source, Path::new("module-info.java"))
    }

    #[test]
    fn test_basic_module() {
        let sym = parse_module(
            r#"module com.example.app {
    requires java.base;
    exports com.example.api;
}"#,
        )
        .expect("should parse module");

        assert_eq!(sym.name, "com.example.app");
        assert_eq!(sym.kind, SymbolKind::Namespace);
        assert_eq!(sym.members.len(), 2);

        let requires = &sym.members[0];
        assert_eq!(requires.name, "requires java.base");
        assert_eq!(requires.extends.as_deref(), Some("requires"));

        let exports = &sym.members[1];
        assert_eq!(exports.name, "exports com.example.api");
        assert_eq!(exports.extends.as_deref(), Some("exports"));
    }

    #[test]
    fn test_module_all_directives() {
        let sym = parse_module(
            r#"module com.example.app {
    requires java.base;
    requires transitive java.sql;
    exports com.example.api;
    exports com.example.internal to com.example.friend;
    opens com.example.model;
    provides com.example.spi.Service with com.example.impl.ServiceImpl;
    uses com.example.spi.Logger;
}"#,
        )
        .expect("should parse module");

        assert_eq!(sym.members.len(), 7);

        // Check requires transitive
        let req_trans = sym
            .members
            .iter()
            .find(|m| m.name.contains("transitive"))
            .unwrap();
        assert_eq!(req_trans.extends.as_deref(), Some("requires-transitive"));

        // Check qualified export
        let export_to = sym
            .members
            .iter()
            .find(|m| m.name.contains("internal"))
            .unwrap();
        assert_eq!(export_to.extends.as_deref(), Some("exports-to"));
        assert!(export_to.name.contains("to com.example.friend"));

        // Check provides
        let provides = sym
            .members
            .iter()
            .find(|m| m.name.starts_with("provides"))
            .unwrap();
        assert_eq!(provides.extends.as_deref(), Some("provides"));
        assert!(provides.implements.contains(&"com.example.impl.ServiceImpl".to_string()));

        // Check uses
        let uses = sym
            .members
            .iter()
            .find(|m| m.name.starts_with("uses"))
            .unwrap();
        assert_eq!(uses.extends.as_deref(), Some("uses"));
    }
}
