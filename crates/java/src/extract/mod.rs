//! Java API surface extraction using tree-sitter-java.
//!
//! Parses `.java` source files directly (no build step needed) and extracts
//! the public API surface: classes, interfaces, enums, records, annotation
//! types, methods, constructors, fields, and their modifiers/annotations.

mod modifiers;
pub mod module_info;

use crate::types::{JavaAnnotation, JavaSymbolData};
use anyhow::{Context, Result};
use semver_analyzer_core::{
    ApiSurface, Parameter, Signature, Symbol, SymbolKind, TypeParameter, Visibility,
};
use std::path::{Path, PathBuf};
use tree_sitter::{Node, Parser};

/// Java API surface extractor using tree-sitter.
pub struct JavaExtractor {
    parser: Parser,
}

impl JavaExtractor {
    pub fn new() -> Result<Self> {
        let mut parser = Parser::new();
        let language = tree_sitter_java::LANGUAGE;
        parser
            .set_language(&language.into())
            .context("Failed to set tree-sitter Java language")?;
        Ok(Self { parser })
    }

    /// Extract the API surface from all `.java` files in a directory.
    pub fn extract_from_dir(&mut self, root: &Path) -> Result<ApiSurface<JavaSymbolData>> {
        let java_files = find_java_files(root)?;
        let mut symbols = Vec::new();

        for file_path in &java_files {
            let source = std::fs::read_to_string(file_path)
                .with_context(|| format!("Failed to read {}", file_path.display()))?;

            let relative = file_path
                .strip_prefix(root)
                .unwrap_or(file_path)
                .to_path_buf();

            match self.extract_file(&source, &relative) {
                Ok(mut file_symbols) => symbols.append(&mut file_symbols),
                Err(e) => {
                    tracing::warn!(file = %relative.display(), error = %e, "Failed to parse Java file");
                }
            }
        }

        Ok(ApiSurface { symbols })
    }

    /// Extract symbols from a single Java source file.
    pub fn extract_file(
        &mut self,
        source: &str,
        file_path: &Path,
    ) -> Result<Vec<Symbol<JavaSymbolData>>> {
        let tree = self
            .parser
            .parse(source, None)
            .context("tree-sitter failed to parse")?;

        let root = tree.root_node();
        let mut symbols = Vec::new();

        // Check for module-info.java
        let file_name = file_path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();
        if file_name == "module-info.java" {
            if let Some(module_sym) =
                module_info::extract_module_info(root, source, file_path)
            {
                symbols.push(module_sym);
            }
            return Ok(symbols);
        }

        let package = extract_package(root, source);
        let imports = extract_imports(root, source);

        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            match child.kind() {
                "class_declaration"
                | "interface_declaration"
                | "enum_declaration"
                | "record_declaration"
                | "annotation_type_declaration" => {
                    if let Some(sym) =
                        extract_type_declaration(child, source, file_path, &package, &imports)
                    {
                        if sym.visibility != Visibility::Private {
                            symbols.push(sym);
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(symbols)
    }
}

// ── File discovery ──────────────────────────────────────────────────────

fn find_java_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    find_java_files_recursive(root, root, &mut files)?;
    files.sort();
    Ok(files)
}

fn find_java_files_recursive(root: &Path, dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            tracing::debug!(dir = %dir.display(), error = %e, "Skipping unreadable directory");
            return Ok(());
        }
    };

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if name_str.starts_with('.') {
            continue;
        }

        if path.is_dir() {
            let rel = path.strip_prefix(root).unwrap_or(&path);
            let rel_str = rel.to_string_lossy();

            if name_str == "target"
                || name_str == "build"
                || name_str == "generated"
                || name_str == "generated-sources"
                || name_str == "node_modules"
                || rel_str.contains("/src/test/")
                || rel_str.starts_with("src/test/")
                || (name_str == "test" && !rel_str.contains("/java/"))
                || (name_str == "tests" && !rel_str.contains("/java/"))
            {
                continue;
            }

            find_java_files_recursive(root, &path, files)?;
        } else if name_str.ends_with(".java") && name_str != "package-info.java" {
            files.push(path);
        }
    }

    Ok(())
}

// ── Package declaration ─────────────────────────────────────────────────

fn extract_package(root: Node, source: &str) -> Option<String> {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "package_declaration" {
            let mut inner_cursor = child.walk();
            for pkg_child in child.children(&mut inner_cursor) {
                if pkg_child.kind() == "scoped_identifier" || pkg_child.kind() == "identifier" {
                    return Some(node_text(pkg_child, source).to_string());
                }
            }
        }
    }
    None
}

// ── Import declarations ─────────────────────────────────────────────────

/// Resolved imports: maps simple names to fully-qualified names,
/// plus wildcard import prefixes for fallback resolution.
struct ImportMap {
    /// Exact imports: `"Service"` → `"org.springframework.stereotype.Service"`.
    exact: std::collections::HashMap<String, String>,
    /// Wildcard import prefixes: `"org.springframework.stereotype"` from
    /// `import org.springframework.stereotype.*`.
    wildcard_prefixes: Vec<String>,
}

fn extract_imports(root: Node, source: &str) -> ImportMap {
    let mut exact = std::collections::HashMap::new();
    let mut wildcard_prefixes = Vec::new();
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        if child.kind() == "import_declaration" {
            let text = node_text(child, source);
            let trimmed = text
                .trim_start_matches("import ")
                .trim_start_matches("static ")
                .trim_end_matches(';')
                .trim();

            if let Some(prefix) = trimmed.strip_suffix(".*") {
                // Wildcard import: keep the package prefix for fallback resolution
                wildcard_prefixes.push(prefix.to_string());
            } else if let Some(dot_pos) = trimmed.rfind('.') {
                let simple_name = &trimmed[dot_pos + 1..];
                exact.insert(simple_name.to_string(), trimmed.to_string());
            }
        }
    }

    ImportMap {
        exact,
        wildcard_prefixes,
    }
}

// ── Type declaration extraction ─────────────────────────────────────────

fn extract_type_declaration(
    node: Node,
    source: &str,
    file_path: &Path,
    package: &Option<String>,
    imports: &ImportMap,
) -> Option<Symbol<JavaSymbolData>> {
    let name = find_child_by_field(node, "name").map(|n| node_text(n, source).to_string())?;

    let qualified_name = match package {
        Some(pkg) => format!("{}.{}", pkg, name),
        None => name.clone(),
    };

    let mods = modifiers::extract_modifiers(node, source);
    let kind = match node.kind() {
        "class_declaration" => SymbolKind::Class,
        "interface_declaration" => SymbolKind::Interface,
        "enum_declaration" => SymbolKind::Enum,
        "record_declaration" => SymbolKind::Class,
        "annotation_type_declaration" => SymbolKind::Interface,
        _ => return None,
    };

    let mut sym = Symbol::new(
        &name,
        &qualified_name,
        kind,
        mods.visibility,
        file_path,
        node.start_position().row + 1,
    );

    sym.is_abstract = mods.is_abstract;
    sym.is_static = mods.is_static;
    sym.is_readonly = mods.is_final;

    if let Some(ref pkg) = package {
        sym.package = Some(pkg.clone());
    }
    sym.import_path = Some(qualified_name.clone());

    let mut lang_data = JavaSymbolData {
        annotations: extract_annotations(node, source, imports),
        is_record: node.kind() == "record_declaration",
        is_annotation_type: node.kind() == "annotation_type_declaration",
        is_final: mods.is_final,
        is_sealed: mods.is_sealed,
        is_non_sealed: mods.is_non_sealed,
        ..Default::default()
    };

    if let Some(superclass) = find_child_by_field(node, "superclass") {
        let type_node = superclass
            .child(superclass.child_count().saturating_sub(1))
            .unwrap_or(superclass);
        sym.extends = Some(node_text(type_node, source).to_string());
    }

    if let Some(interfaces) = find_child_by_field(node, "interfaces") {
        sym.implements = extract_type_list(interfaces, source);
    }

    if let Some(permits) = find_child_by_field(node, "permits") {
        lang_data.permits = extract_type_list(permits, source);
    }

    if let Some(type_params) = find_child_by_field(node, "type_parameters") {
        let tps = extract_type_parameters(type_params, source);
        if !tps.is_empty() {
            sym.signature = Some(Signature {
                parameters: Vec::new(),
                return_type: None,
                type_parameters: tps,
                is_async: false,
            });
        }
    }

    let body_node = find_child_by_kind(node, "class_body")
        .or_else(|| find_child_by_kind(node, "interface_body"))
        .or_else(|| find_child_by_kind(node, "enum_body"))
        .or_else(|| find_child_by_kind(node, "annotation_type_body"))
        .or_else(|| find_child_by_kind(node, "record_declaration_body"));

    if let Some(body) = body_node {
        extract_members(
            body,
            source,
            file_path,
            &qualified_name,
            package,
            imports,
            &mut sym.members,
        );
    }

    if kind == SymbolKind::Interface {
        for member in &mut sym.members {
            if member.visibility == Visibility::Internal {
                member.visibility = Visibility::Public;
            }
        }
    }

    if node.kind() == "enum_declaration" {
        if let Some(body) = find_child_by_kind(node, "enum_body") {
            extract_enum_constants(body, source, file_path, &qualified_name, &mut sym.members);
        }
    }

    if node.kind() == "record_declaration" {
        if let Some(params) = find_child_by_field(node, "parameters") {
            let record_params = extract_formal_parameters(params, source);

            // Check if a compact constructor was already extracted from the body.
            // Compact constructors have no explicit parameters — they implicitly
            // take the record's component parameters.
            let has_compact_ctor = sym.members.iter().any(|m| {
                m.kind == SymbolKind::Constructor
                    && m.name == name
                    && m.signature
                        .as_ref()
                        .map(|s| s.parameters.is_empty())
                        .unwrap_or(true)
            });

            if has_compact_ctor {
                // Update the compact constructor's signature with record params
                for member in &mut sym.members {
                    if member.kind == SymbolKind::Constructor
                        && member.name == name
                        && member
                            .signature
                            .as_ref()
                            .map(|s| s.parameters.is_empty())
                            .unwrap_or(true)
                    {
                        member.signature = Some(Signature {
                            parameters: record_params.clone(),
                            return_type: None,
                            type_parameters: Vec::new(),
                            is_async: false,
                        });
                        break;
                    }
                }
            } else {
                // Synthesize a canonical constructor
                let mut ctor = Symbol::new(
                    &name,
                    format!("{}.{}", qualified_name, name),
                    SymbolKind::Constructor,
                    Visibility::Public,
                    file_path,
                    node.start_position().row + 1,
                );
                ctor.signature = Some(Signature {
                    parameters: record_params.clone(),
                    return_type: None,
                    type_parameters: Vec::new(),
                    is_async: false,
                });
                ctor.language_data = JavaSymbolData::default();
                sym.members.push(ctor);
            }

            // Synthesize accessor methods for each record component
            for param in &record_params {
                // Skip if an explicit accessor was already declared in the body
                let already_declared = sym.members.iter().any(|m| {
                    m.kind == SymbolKind::Method && m.name == param.name
                });
                if already_declared {
                    continue;
                }

                let mut accessor = Symbol::new(
                    &param.name,
                    format!("{}.{}", qualified_name, param.name),
                    SymbolKind::Method,
                    Visibility::Public,
                    file_path,
                    node.start_position().row + 1,
                );
                accessor.signature = Some(Signature {
                    parameters: Vec::new(),
                    return_type: param.type_annotation.clone(),
                    type_parameters: Vec::new(),
                    is_async: false,
                });
                accessor.language_data = JavaSymbolData::default();
                sym.members.push(accessor);
            }
        }
    }

    sym.language_data = lang_data;
    Some(sym)
}

// ── Member extraction ───────────────────────────────────────────────────

fn extract_members(
    body: Node,
    source: &str,
    file_path: &Path,
    parent_qualified_name: &str,
    _package: &Option<String>,
    imports: &ImportMap,
    members: &mut Vec<Symbol<JavaSymbolData>>,
) {
    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        match child.kind() {
            "method_declaration" => {
                if let Some(sym) =
                    extract_method(child, source, file_path, parent_qualified_name, imports)
                {
                    if sym.visibility != Visibility::Private {
                        members.push(sym);
                    }
                }
            }
            "constructor_declaration" | "compact_constructor_declaration" => {
                if let Some(sym) =
                    extract_constructor(child, source, file_path, parent_qualified_name, imports)
                {
                    if sym.visibility != Visibility::Private {
                        members.push(sym);
                    }
                }
            }
            "field_declaration" | "constant_declaration" => {
                let mut field_syms =
                    extract_field(child, source, file_path, parent_qualified_name, imports);
                for sym in field_syms.drain(..) {
                    if sym.visibility != Visibility::Private {
                        members.push(sym);
                    }
                }
            }
            "annotation_type_element_declaration" => {
                if let Some(sym) = extract_annotation_element(
                    child,
                    source,
                    file_path,
                    parent_qualified_name,
                    imports,
                ) {
                    members.push(sym);
                }
            }
            "class_declaration"
            | "interface_declaration"
            | "enum_declaration"
            | "record_declaration"
            | "annotation_type_declaration" => {
                if let Some(mut sym) =
                    extract_type_declaration(child, source, file_path, _package, imports)
                {
                    // Override qualified_name to reflect nesting: Parent.Inner
                    sym.qualified_name = format!("{}.{}", parent_qualified_name, sym.name);
                    // Fix import_path to match the corrected qualified_name
                    sym.import_path = Some(sym.qualified_name.clone());
                    if sym.visibility != Visibility::Private {
                        members.push(sym);
                    }
                }
            }
            _ => {}
        }
    }
}

fn extract_method(
    node: Node,
    source: &str,
    file_path: &Path,
    parent_qname: &str,
    imports: &ImportMap,
) -> Option<Symbol<JavaSymbolData>> {
    let name = find_child_by_field(node, "name").map(|n| node_text(n, source).to_string())?;
    let qualified_name = format!("{}.{}", parent_qname, name);
    let mods = modifiers::extract_modifiers(node, source);

    let mut sym = Symbol::new(
        &name,
        &qualified_name,
        SymbolKind::Method,
        mods.visibility,
        file_path,
        node.start_position().row + 1,
    );

    sym.is_abstract = mods.is_abstract;
    sym.is_static = mods.is_static;

    let return_type = find_child_by_field(node, "type").map(|n| node_text(n, source).to_string());
    let params = find_child_by_field(node, "parameters")
        .map(|n| extract_formal_parameters(n, source))
        .unwrap_or_default();
    let type_params = find_child_by_field(node, "type_parameters")
        .map(|n| extract_type_parameters(n, source))
        .unwrap_or_default();

    sym.signature = Some(Signature {
        parameters: params,
        return_type,
        type_parameters: type_params,
        is_async: false,
    });

    let mut lang_data = JavaSymbolData {
        annotations: extract_annotations(node, source, imports),
        is_default: mods.is_default,
        is_synchronized: mods.is_synchronized,
        is_native: mods.is_native,
        ..Default::default()
    };

    let throws = extract_throws(node, source);
    if !throws.is_empty() {
        lang_data.throws = throws;
    }

    sym.language_data = lang_data;
    Some(sym)
}

fn extract_constructor(
    node: Node,
    source: &str,
    file_path: &Path,
    parent_qname: &str,
    imports: &ImportMap,
) -> Option<Symbol<JavaSymbolData>> {
    let name = find_child_by_field(node, "name").map(|n| node_text(n, source).to_string())?;
    let qualified_name = format!("{}.{}", parent_qname, name);
    let mods = modifiers::extract_modifiers(node, source);

    let mut sym = Symbol::new(
        &name,
        &qualified_name,
        SymbolKind::Constructor,
        mods.visibility,
        file_path,
        node.start_position().row + 1,
    );

    let params = find_child_by_field(node, "parameters")
        .map(|n| extract_formal_parameters(n, source))
        .unwrap_or_default();
    let type_params = find_child_by_field(node, "type_parameters")
        .map(|n| extract_type_parameters(n, source))
        .unwrap_or_default();

    sym.signature = Some(Signature {
        parameters: params,
        return_type: None,
        type_parameters: type_params,
        is_async: false,
    });

    let mut lang_data = JavaSymbolData {
        annotations: extract_annotations(node, source, imports),
        ..Default::default()
    };
    let throws = extract_throws(node, source);
    if !throws.is_empty() {
        lang_data.throws = throws;
    }

    sym.language_data = lang_data;
    Some(sym)
}

fn extract_field(
    node: Node,
    source: &str,
    file_path: &Path,
    parent_qname: &str,
    imports: &ImportMap,
) -> Vec<Symbol<JavaSymbolData>> {
    let mods = modifiers::extract_modifiers(node, source);
    let annotations = extract_annotations(node, source, imports);
    let type_str = find_child_by_field(node, "type").map(|n| node_text(n, source).to_string());

    let mut symbols = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "variable_declarator" {
            if let Some(name_node) = find_child_by_field(child, "name") {
                let name = node_text(name_node, source).to_string();
                let qualified_name = format!("{}.{}", parent_qname, name);

                let kind = if mods.is_static && mods.is_final {
                    SymbolKind::Constant
                } else {
                    SymbolKind::Property
                };

                let mut sym = Symbol::new(
                    &name,
                    &qualified_name,
                    kind,
                    mods.visibility,
                    file_path,
                    child.start_position().row + 1,
                );

                sym.is_static = mods.is_static;
                sym.is_readonly = mods.is_final;

                sym.signature = Some(Signature {
                    parameters: Vec::new(),
                    return_type: type_str.clone(),
                    type_parameters: Vec::new(),
                    is_async: false,
                });

                if kind == SymbolKind::Constant {
                    if let Some(value_node) = find_child_by_field(child, "value") {
                        let value = node_text(value_node, source).to_string();
                        if let Some(ref mut sig) = sym.signature {
                            if sig.parameters.is_empty() {
                                sig.parameters.push(Parameter {
                                    name: "value".into(),
                                    type_annotation: type_str.clone(),
                                    optional: false,
                                    has_default: true,
                                    default_value: Some(value),
                                    is_variadic: false,
                                });
                            }
                        }
                    }
                }

                sym.language_data = JavaSymbolData {
                    annotations: annotations.clone(),
                    is_final: mods.is_final,
                    is_transient: mods.is_transient,
                    is_volatile: mods.is_volatile,
                    ..Default::default()
                };

                symbols.push(sym);
            }
        }
    }

    symbols
}

fn extract_annotation_element(
    node: Node,
    source: &str,
    file_path: &Path,
    parent_qname: &str,
    imports: &ImportMap,
) -> Option<Symbol<JavaSymbolData>> {
    let name = find_child_by_field(node, "name").map(|n| node_text(n, source).to_string())?;
    let qualified_name = format!("{}.{}", parent_qname, name);

    let return_type = find_child_by_field(node, "type").map(|n| node_text(n, source).to_string());

    let mut sym = Symbol::new(
        &name,
        &qualified_name,
        SymbolKind::Method,
        Visibility::Public,
        file_path,
        node.start_position().row + 1,
    );

    let has_default = find_child_by_field(node, "value").is_some()
        || find_child_by_kind(node, "default_value").is_some();

    let default_value = find_child_by_field(node, "value")
        .or_else(|| find_child_by_kind(node, "default_value"))
        .map(|n| node_text(n, source).to_string());

    sym.signature = Some(Signature {
        parameters: vec![Parameter {
            name: "value".into(),
            type_annotation: return_type.clone(),
            optional: has_default,
            has_default,
            default_value,
            is_variadic: false,
        }],
        return_type,
        type_parameters: Vec::new(),
        is_async: false,
    });

    sym.language_data = JavaSymbolData {
        annotations: extract_annotations(node, source, imports),
        ..Default::default()
    };

    Some(sym)
}

fn extract_enum_constants(
    body: Node,
    source: &str,
    file_path: &Path,
    parent_qname: &str,
    members: &mut Vec<Symbol<JavaSymbolData>>,
) {
    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        if child.kind() == "enum_constant" {
            if let Some(name_node) = find_child_by_field(child, "name") {
                let name = node_text(name_node, source).to_string();
                let qualified_name = format!("{}.{}", parent_qname, name);

                let mut sym = Symbol::new(
                    &name,
                    &qualified_name,
                    SymbolKind::EnumMember,
                    Visibility::Public,
                    file_path,
                    child.start_position().row + 1,
                );

                // Extract enum constant constructor arguments (e.g., MERCURY(3.303e+23, 2.4397e6))
                if let Some(args) = find_child_by_field(child, "arguments") {
                    let mut arg_params = Vec::new();
                    let mut arg_cursor = args.walk();
                    let mut arg_index = 0;
                    for arg_child in args.children(&mut arg_cursor) {
                        // Skip parentheses and commas
                        if arg_child.kind() == "(" || arg_child.kind() == ")" || arg_child.kind() == "," {
                            continue;
                        }
                        let value = node_text(arg_child, source).to_string();
                        arg_params.push(Parameter {
                            name: format!("arg{}", arg_index),
                            type_annotation: None,
                            optional: false,
                            has_default: true,
                            default_value: Some(value),
                            is_variadic: false,
                        });
                        arg_index += 1;
                    }
                    if !arg_params.is_empty() {
                        sym.signature = Some(Signature {
                            parameters: arg_params,
                            return_type: None,
                            type_parameters: Vec::new(),
                            is_async: false,
                        });
                    }
                }

                sym.language_data = JavaSymbolData::default();
                members.push(sym);
            }
        }
    }
}

// ── Annotation extraction ───────────────────────────────────────────────

fn extract_annotations(
    node: Node,
    source: &str,
    imports: &ImportMap,
) -> Vec<JavaAnnotation> {
    let mut annotations = Vec::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if child.kind() == "modifiers" {
            let mut mod_cursor = child.walk();
            for mod_child in child.children(&mut mod_cursor) {
                if mod_child.kind() == "marker_annotation" || mod_child.kind() == "annotation" {
                    if let Some(ann) = parse_annotation(mod_child, source, imports) {
                        annotations.push(ann);
                    }
                }
            }
        }
    }

    annotations
}

fn parse_annotation(
    node: Node,
    source: &str,
    imports: &ImportMap,
) -> Option<JavaAnnotation> {
    let name_node = find_child_by_field(node, "name")?;
    let name = node_text(name_node, source).to_string();
    // Try exact import first, then fall back to wildcard prefix resolution
    let qualified_name = imports.exact.get(&name).cloned().or_else(|| {
        // For wildcard imports like `import org.springframework.stereotype.*`,
        // try `org.springframework.stereotype.{name}` as a best-effort resolution.
        // We can't verify the type actually exists without classpath resolution,
        // but this gives a useful qualified name for annotation matching.
        if imports.wildcard_prefixes.len() == 1 {
            // Unambiguous: only one wildcard import
            Some(format!("{}.{}", imports.wildcard_prefixes[0], name))
        } else {
            // Multiple wildcard imports: ambiguous, can't resolve
            // (could try heuristics but risk false matches)
            None
        }
    });

    let mut attributes = Vec::new();
    if let Some(args) = find_child_by_field(node, "arguments") {
        let mut cursor = args.walk();
        for child in args.children(&mut cursor) {
            match child.kind() {
                "element_value_pair" => {
                    let key = find_child_by_field(child, "key")
                        .map(|n| node_text(n, source).to_string())
                        .unwrap_or_else(|| "value".into());
                    let value = find_child_by_field(child, "value")
                        .map(|n| node_text(n, source).to_string())
                        .unwrap_or_default();
                    attributes.push((key, value));
                }
                _ if child.kind() != "(" && child.kind() != ")" => {
                    let value = node_text(child, source).to_string();
                    if !value.is_empty() && value != "(" && value != ")" {
                        attributes.push(("value".into(), value));
                    }
                }
                _ => {}
            }
        }
    }

    Some(JavaAnnotation {
        name,
        qualified_name,
        attributes,
    })
}

// ── Parameter extraction ────────────────────────────────────────────────

fn extract_formal_parameters(node: Node, source: &str) -> Vec<Parameter> {
    let mut params = Vec::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if child.kind() == "formal_parameter" || child.kind() == "spread_parameter" {
            let is_variadic = child.kind() == "spread_parameter";
            let type_ann =
                find_child_by_field(child, "type").map(|n| node_text(n, source).to_string());
            let name = find_child_by_field(child, "name")
                .map(|n| node_text(n, source).to_string())
                .unwrap_or_else(|| format!("arg{}", params.len()));

            params.push(Parameter {
                name,
                type_annotation: type_ann,
                optional: false,
                has_default: false,
                default_value: None,
                is_variadic,
            });
        } else if child.kind() == "record_component" {
            let type_ann =
                find_child_by_field(child, "type").map(|n| node_text(n, source).to_string());
            let name = find_child_by_field(child, "name")
                .map(|n| node_text(n, source).to_string())
                .unwrap_or_else(|| format!("arg{}", params.len()));

            params.push(Parameter {
                name,
                type_annotation: type_ann,
                optional: false,
                has_default: false,
                default_value: None,
                is_variadic: false,
            });
        }
    }

    params
}

// ── Type parameter extraction ───────────────────────────────────────────

fn extract_type_parameters(node: Node, source: &str) -> Vec<TypeParameter> {
    let mut type_params = Vec::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if child.kind() == "type_parameter" {
            let name = child
                .child(0)
                .map(|n| node_text(n, source).to_string())
                .unwrap_or_default();

            let constraint =
                find_child_by_kind(child, "type_bound").map(|n| node_text(n, source).to_string());

            type_params.push(TypeParameter {
                name,
                constraint,
                default: None,
            });
        }
    }

    type_params
}

// ── Throws clause extraction ────────────────────────────────────────────

fn extract_throws(node: Node, source: &str) -> Vec<String> {
    let mut throws = Vec::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if child.kind() == "throws" {
            let mut throws_cursor = child.walk();
            for tc in child.children(&mut throws_cursor) {
                if tc.kind() == "type_identifier"
                    || tc.kind() == "scoped_type_identifier"
                    || tc.kind() == "generic_type"
                {
                    throws.push(node_text(tc, source).to_string());
                }
            }
        }
    }

    throws
}

fn extract_type_list(node: Node, source: &str) -> Vec<String> {
    let mut types = Vec::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        match child.kind() {
            "type_identifier" | "scoped_type_identifier" | "generic_type" => {
                types.push(node_text(child, source).to_string());
            }
            "type_list" => {
                let mut inner = extract_type_list(child, source);
                types.append(&mut inner);
            }
            _ => {}
        }
    }

    types
}

// ── Tree-sitter node helpers ────────────────────────────────────────────

fn node_text<'a>(node: Node, source: &'a str) -> &'a str {
    &source[node.byte_range()]
}

fn find_child_by_field<'a>(node: Node<'a>, field: &str) -> Option<Node<'a>> {
    node.child_by_field_name(field)
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

    fn parse(source: &str) -> Vec<Symbol<JavaSymbolData>> {
        let mut extractor = JavaExtractor::new().unwrap();
        extractor
            .extract_file(source, Path::new("Test.java"))
            .unwrap()
    }

    #[test]
    fn test_simple_class() {
        let syms = parse(
            r#"
            package com.example;
            public class Foo {
                public void doThing(String name) {}
                private void internal() {}
            }
            "#,
        );
        assert_eq!(syms.len(), 1);
        let foo = &syms[0];
        assert_eq!(foo.name, "Foo");
        assert_eq!(foo.qualified_name, "com.example.Foo");
        assert_eq!(foo.kind, SymbolKind::Class);
        assert_eq!(foo.visibility, Visibility::Public);
        assert_eq!(foo.members.len(), 1);
        assert_eq!(foo.members[0].name, "doThing");
    }

    #[test]
    fn test_interface_with_default_method() {
        let syms = parse(
            r#"
            package com.example;
            public interface Greeter {
                String greet(String name);
                default String greetAll() { return "Hello all"; }
            }
            "#,
        );
        assert_eq!(syms.len(), 1);
        let iface = &syms[0];
        assert_eq!(iface.kind, SymbolKind::Interface);
        assert_eq!(iface.members.len(), 2);

        let default_method = iface.members.iter().find(|m| m.name == "greetAll").unwrap();
        assert!(default_method.language_data.is_default);
    }

    #[test]
    fn test_enum() {
        let syms = parse(
            r#"
            package com.example;
            public enum Color {
                RED, GREEN, BLUE;
            }
            "#,
        );
        assert_eq!(syms.len(), 1);
        let e = &syms[0];
        assert_eq!(e.kind, SymbolKind::Enum);
        let enum_members: Vec<&str> = e.members.iter().map(|m| m.name.as_str()).collect();
        assert!(enum_members.contains(&"RED"));
        assert!(enum_members.contains(&"GREEN"));
        assert!(enum_members.contains(&"BLUE"));
    }

    #[test]
    fn test_record() {
        let syms = parse(
            r#"
            package com.example;
            public record Point(int x, int y) {}
            "#,
        );
        assert_eq!(syms.len(), 1);
        let rec = &syms[0];
        assert_eq!(rec.kind, SymbolKind::Class);
        assert!(rec.language_data.is_record);
        assert_eq!(rec.members.len(), 3); // ctor + 2 accessors
    }

    #[test]
    fn test_annotations() {
        let syms = parse(
            r#"
            package com.example;
            import org.springframework.stereotype.Service;
            @Service
            @Deprecated(since = "3.2", forRemoval = true)
            public class OldService {}
            "#,
        );
        assert_eq!(syms.len(), 1);
        let svc = &syms[0];
        assert_eq!(svc.language_data.annotations.len(), 2);

        let service_ann = &svc.language_data.annotations[0];
        assert_eq!(service_ann.name, "Service");
        assert_eq!(
            service_ann.qualified_name.as_deref(),
            Some("org.springframework.stereotype.Service")
        );
    }

    #[test]
    fn test_static_final_constant() {
        let syms = parse(
            r#"
            package com.example;
            public class Constants {
                public static final int MAX_SIZE = 100;
                protected String name;
            }
            "#,
        );
        assert_eq!(syms.len(), 1);
        let cls = &syms[0];
        assert_eq!(cls.members.len(), 2);
        assert_eq!(cls.members[0].kind, SymbolKind::Constant);
        assert_eq!(cls.members[1].kind, SymbolKind::Property);
    }

    #[test]
    fn test_throws_clause() {
        let syms = parse(
            r#"
            package com.example;
            public class Foo {
                public void read() throws java.io.IOException {}
            }
            "#,
        );
        let method = &syms[0].members[0];
        assert_eq!(method.language_data.throws.len(), 1);
    }

    #[test]
    fn test_visibility_filtering() {
        let syms = parse(
            r#"
            package com.example;
            public class Foo {
                public void publicMethod() {}
                protected void protectedMethod() {}
                void packagePrivateMethod() {}
                private void privateMethod() {}
            }
            "#,
        );
        let members = &syms[0].members;
        assert_eq!(members.len(), 3);
    }

    #[test]
    fn test_inner_class_import_path() {
        let syms = parse(
            r#"
            package com.example;
            public class Outer {
                public static class Inner {
                    public void innerMethod() {}
                }
                public interface InnerIface {
                    void doThing();
                }
            }
            "#,
        );
        assert_eq!(syms.len(), 1);
        let outer = &syms[0];
        assert_eq!(outer.qualified_name, "com.example.Outer");
        assert_eq!(outer.import_path.as_deref(), Some("com.example.Outer"));

        // Inner class should have correct qualified_name and import_path
        let inner_class = outer
            .members
            .iter()
            .find(|m| m.name == "Inner")
            .expect("Inner class not found");
        assert_eq!(inner_class.qualified_name, "com.example.Outer.Inner");
        assert_eq!(
            inner_class.import_path.as_deref(),
            Some("com.example.Outer.Inner")
        );

        // Inner interface too
        let inner_iface = outer
            .members
            .iter()
            .find(|m| m.name == "InnerIface")
            .expect("InnerIface not found");
        assert_eq!(inner_iface.qualified_name, "com.example.Outer.InnerIface");
        assert_eq!(
            inner_iface.import_path.as_deref(),
            Some("com.example.Outer.InnerIface")
        );
    }

    #[test]
    fn test_enum_with_arguments() {
        let syms = parse(
            r#"
            package com.example;
            public enum Planet {
                MERCURY(3.303e+23, 2.4397e6),
                VENUS(4.869e+24, 6.0518e6);
                private final double mass;
                Planet(double mass, double radius) { this.mass = mass; }
            }
            "#,
        );
        assert_eq!(syms.len(), 1);
        let e = &syms[0];
        assert_eq!(e.kind, SymbolKind::Enum);

        let mercury = e.members.iter().find(|m| m.name == "MERCURY").unwrap();
        assert_eq!(mercury.kind, SymbolKind::EnumMember);
        // Should have constructor arguments captured
        let sig = mercury.signature.as_ref().expect("MERCURY should have arguments");
        assert_eq!(sig.parameters.len(), 2);
        assert_eq!(sig.parameters[0].default_value.as_deref(), Some("3.303e+23"));
        assert_eq!(sig.parameters[1].default_value.as_deref(), Some("2.4397e6"));

        let venus = e.members.iter().find(|m| m.name == "VENUS").unwrap();
        let sig = venus.signature.as_ref().expect("VENUS should have arguments");
        assert_eq!(sig.parameters.len(), 2);

        // Simple enum constants (no args) should have no signature
        // (this test has only MERCURY and VENUS with args)
    }

    #[test]
    fn test_record_with_compact_constructor() {
        let syms = parse(
            r#"
            package com.example;
            public record Range(int start, int end) {
                public Range {
                    if (start > end) throw new IllegalArgumentException();
                }
            }
            "#,
        );
        assert_eq!(syms.len(), 1);
        let rec = &syms[0];
        assert!(rec.language_data.is_record);

        // Should have: compact constructor (with params from record), 2 accessors
        let ctor = rec
            .members
            .iter()
            .find(|m| m.kind == SymbolKind::Constructor)
            .expect("constructor not found");
        assert_eq!(ctor.name, "Range");
        // The compact constructor should have the record's parameters
        let params = &ctor.signature.as_ref().unwrap().parameters;
        assert_eq!(params.len(), 2);
        assert_eq!(params[0].name, "start");
        assert_eq!(params[1].name, "end");

        // Should have 2 accessor methods
        let accessors: Vec<_> = rec
            .members
            .iter()
            .filter(|m| m.kind == SymbolKind::Method)
            .collect();
        assert_eq!(accessors.len(), 2);
    }

    #[test]
    fn test_package_private_class() {
        let syms = parse(
            r#"
            package com.example;
            class PackagePrivateClass {
                public void doThing() {}
            }
            "#,
        );
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].visibility, Visibility::Internal);
    }
}
