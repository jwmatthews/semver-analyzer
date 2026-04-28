//! Shared test helpers for Java baseline integration tests.
//!
//! Provides construction helpers for `Symbol<JavaSymbolData>`,
//! `ApiSurface<JavaSymbolData>`, and normalized snapshot types.
#![allow(dead_code)]

use semver_analyzer_core::*;
use semver_analyzer_java::language::Java;
use semver_analyzer_java::types::{JavaAnnotation, JavaSymbolData};
use serde::Serialize;

// Type aliases for Java tests.
pub type Symbol = semver_analyzer_core::Symbol<JavaSymbolData>;
pub type ApiSurface = semver_analyzer_core::ApiSurface<JavaSymbolData>;

// ── Normalized change for snapshotting ───────────────────────────────

/// Semantic representation of a structural change for snapshot comparison.
#[derive(Debug, Serialize)]
pub struct NormalizedChange {
    pub symbol: String,
    pub qualified_name: String,
    pub kind: String,
    pub change_type: String,
    pub is_breaking: bool,
    pub description: String,
    pub before: Option<String>,
    pub after: Option<String>,
    pub has_migration_target: bool,
}

impl From<&StructuralChange> for NormalizedChange {
    fn from(c: &StructuralChange) -> Self {
        NormalizedChange {
            symbol: c.symbol.clone(),
            qualified_name: c.qualified_name.clone(),
            kind: format!("{:?}", c.kind),
            change_type: format!("{:?}", c.change_type),
            is_breaking: c.is_breaking,
            description: c.description.clone(),
            before: c.before.clone(),
            after: c.after.clone(),
            has_migration_target: c.migration_target.is_some(),
        }
    }
}

pub fn normalize(changes: &[StructuralChange]) -> Vec<NormalizedChange> {
    changes.iter().map(NormalizedChange::from).collect()
}

// ── Normalized manifest change ──────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct NormalizedManifestChange {
    pub field: String,
    pub change_type: String,
    pub is_breaking: bool,
    pub description: String,
    pub before: Option<String>,
    pub after: Option<String>,
}

impl From<&ManifestChange<Java>> for NormalizedManifestChange {
    fn from(c: &ManifestChange<Java>) -> Self {
        NormalizedManifestChange {
            field: c.field.clone(),
            change_type: format!("{:?}", c.change_type),
            is_breaking: c.is_breaking,
            description: c.description.clone(),
            before: c.before.clone(),
            after: c.after.clone(),
        }
    }
}

pub fn normalize_manifest(changes: &[ManifestChange<Java>]) -> Vec<NormalizedManifestChange> {
    changes.iter().map(NormalizedManifestChange::from).collect()
}

// ── Symbol construction helpers ─────────────────────────────────────

/// Create a basic Java symbol with JavaSymbolData default.
pub fn java_sym(name: &str, qname: &str, kind: SymbolKind) -> Symbol {
    let mut s = semver_analyzer_core::Symbol::new(
        name,
        qname,
        kind,
        Visibility::Public,
        "Test.java",
        1,
    );
    s.language_data = JavaSymbolData::default();
    s
}

/// Create a Java class symbol with package.
pub fn java_class(name: &str, package: &str) -> Symbol {
    let qname = format!("{}.{}", package, name);
    let mut s = java_sym(name, &qname, SymbolKind::Class);
    s.package = Some(package.to_string());
    s.import_path = Some(qname);
    s
}

/// Create a Java interface symbol.
pub fn java_interface(name: &str, package: &str) -> Symbol {
    let qname = format!("{}.{}", package, name);
    let mut s = java_sym(name, &qname, SymbolKind::Interface);
    s.package = Some(package.to_string());
    s.import_path = Some(qname);
    s
}

/// Create a Java enum symbol.
pub fn java_enum(name: &str, package: &str) -> Symbol {
    let qname = format!("{}.{}", package, name);
    let mut s = java_sym(name, &qname, SymbolKind::Enum);
    s.package = Some(package.to_string());
    s.import_path = Some(qname);
    s
}

/// Create a method member symbol.
pub fn java_method(name: &str, parent_qname: &str, params: Vec<Parameter>, ret: &str) -> Symbol {
    let qname = format!("{}.{}", parent_qname, name);
    let mut s = java_sym(name, &qname, SymbolKind::Method);
    s.signature = Some(Signature {
        parameters: params,
        return_type: Some(ret.to_string()),
        type_parameters: Vec::new(),
        is_async: false,
    });
    s
}

/// Create a property/field member symbol.
pub fn java_field(name: &str, parent_qname: &str, ty: &str) -> Symbol {
    let qname = format!("{}.{}", parent_qname, name);
    let mut s = java_sym(name, &qname, SymbolKind::Property);
    s.signature = Some(Signature {
        parameters: Vec::new(),
        return_type: Some(ty.to_string()),
        type_parameters: Vec::new(),
        is_async: false,
    });
    s
}

/// Create a constant member symbol.
pub fn java_constant(name: &str, parent_qname: &str, ty: &str, value: &str) -> Symbol {
    let qname = format!("{}.{}", parent_qname, name);
    let mut s = java_sym(name, &qname, SymbolKind::Constant);
    s.is_static = true;
    s.is_readonly = true;
    s.signature = Some(Signature {
        parameters: vec![Parameter {
            name: "value".into(),
            type_annotation: Some(ty.to_string()),
            optional: false,
            has_default: true,
            default_value: Some(value.to_string()),
            is_variadic: false,
        }],
        return_type: Some(ty.to_string()),
        type_parameters: Vec::new(),
        is_async: false,
    });
    s
}

/// Create a parameter.
pub fn param(name: &str, ty: &str) -> Parameter {
    Parameter {
        name: name.to_string(),
        type_annotation: Some(ty.to_string()),
        optional: false,
        has_default: false,
        default_value: None,
        is_variadic: false,
    }
}

/// Add an annotation to a symbol.
pub fn with_annotation(mut sym: Symbol, name: &str) -> Symbol {
    sym.language_data.annotations.push(JavaAnnotation {
        name: name.to_string(),
        qualified_name: None,
        attributes: Vec::new(),
    });
    sym
}

/// Add an annotation with attributes.
pub fn with_annotation_attrs(
    mut sym: Symbol,
    name: &str,
    attrs: Vec<(&str, &str)>,
) -> Symbol {
    sym.language_data.annotations.push(JavaAnnotation {
        name: name.to_string(),
        qualified_name: None,
        attributes: attrs
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    });
    sym
}

/// Make a symbol final.
pub fn with_final(mut sym: Symbol) -> Symbol {
    sym.language_data.is_final = true;
    sym.is_readonly = true;
    sym
}

/// Make a symbol sealed with permits.
pub fn with_sealed(mut sym: Symbol, permits: Vec<&str>) -> Symbol {
    sym.language_data.is_sealed = true;
    sym.language_data.permits = permits.into_iter().map(|s| s.to_string()).collect();
    sym
}

/// Add throws clause to a method.
pub fn with_throws(mut sym: Symbol, exceptions: Vec<&str>) -> Symbol {
    sym.language_data.throws = exceptions.into_iter().map(|s| s.to_string()).collect();
    sym
}

/// Set extends on a symbol.
pub fn with_extends(mut sym: Symbol, parent: &str) -> Symbol {
    sym.extends = Some(parent.to_string());
    sym
}

/// Set implements on a symbol.
pub fn with_implements(mut sym: Symbol, ifaces: Vec<&str>) -> Symbol {
    sym.implements = ifaces.into_iter().map(|s| s.to_string()).collect();
    sym
}

/// Make a symbol abstract.
pub fn with_abstract(mut sym: Symbol) -> Symbol {
    sym.is_abstract = true;
    sym
}

/// Construct an API surface from symbols.
pub fn surface(symbols: Vec<Symbol>) -> ApiSurface {
    ApiSurface { symbols }
}
