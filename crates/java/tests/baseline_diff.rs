//! Baseline integration tests for the Java diff engine.
//!
//! These tests construct Java API surfaces, run `diff_surfaces_with_semantics`
//! with `Java` semantics, and capture the output as insta snapshots.

mod helpers;

use helpers::*;
use semver_analyzer_core::diff::diff_surfaces_with_semantics;
use semver_analyzer_java::Java;

fn diff(old: &ApiSurface, new: &ApiSurface) -> Vec<NormalizedChange> {
    normalize(&diff_surfaces_with_semantics(
        old,
        new,
        &Java::new(),
    ))
}

// ── Class-level changes ─────────────────────────────────────────────

#[test]
fn baseline_class_removed() {
    let old = surface(vec![java_class("UserService", "com.example")]);
    let new = surface(vec![]);
    insta::assert_yaml_snapshot!(diff(&old, &new));
}

#[test]
fn baseline_class_added() {
    let old = surface(vec![]);
    let new = surface(vec![java_class("UserService", "com.example")]);
    insta::assert_yaml_snapshot!(diff(&old, &new));
}

#[test]
fn baseline_no_changes() {
    let mut cls = java_class("UserService", "com.example");
    cls.members.push(java_method(
        "findUser",
        "com.example.UserService",
        vec![param("id", "long")],
        "User",
    ));
    let s = surface(vec![cls]);
    insta::assert_yaml_snapshot!(diff(&s, &s));
}

// ── Method-level changes ────────────────────────────────────────────

#[test]
fn baseline_method_removed() {
    let mut old_cls = java_class("Service", "com.example");
    old_cls.members.push(java_method(
        "process",
        "com.example.Service",
        vec![param("input", "String")],
        "void",
    ));

    let new_cls = java_class("Service", "com.example");
    insta::assert_yaml_snapshot!(diff(
        &surface(vec![old_cls]),
        &surface(vec![new_cls]),
    ));
}

#[test]
fn baseline_method_added_to_interface() {
    let old_iface = java_interface("Repository", "com.example");

    let mut new_iface = java_interface("Repository", "com.example");
    new_iface.members.push(java_method(
        "findById",
        "com.example.Repository",
        vec![param("id", "long")],
        "Optional<Entity>",
    ));

    insta::assert_yaml_snapshot!(diff(
        &surface(vec![old_iface]),
        &surface(vec![new_iface]),
    ));
}

#[test]
fn baseline_method_return_type_changed() {
    let mut old_cls = java_class("Service", "com.example");
    old_cls.members.push(java_method(
        "getData",
        "com.example.Service",
        vec![],
        "List<String>",
    ));

    let mut new_cls = java_class("Service", "com.example");
    new_cls.members.push(java_method(
        "getData",
        "com.example.Service",
        vec![],
        "Set<String>",
    ));

    insta::assert_yaml_snapshot!(diff(
        &surface(vec![old_cls]),
        &surface(vec![new_cls]),
    ));
}

#[test]
fn baseline_method_parameter_added() {
    let mut old_cls = java_class("Service", "com.example");
    old_cls.members.push(java_method(
        "process",
        "com.example.Service",
        vec![param("input", "String")],
        "void",
    ));

    let mut new_cls = java_class("Service", "com.example");
    new_cls.members.push(java_method(
        "process",
        "com.example.Service",
        vec![param("input", "String"), param("options", "Map<String, Object>")],
        "void",
    ));

    insta::assert_yaml_snapshot!(diff(
        &surface(vec![old_cls]),
        &surface(vec![new_cls]),
    ));
}

// ── Annotation changes (via diff_language_data) ─────────────────────

#[test]
fn baseline_annotation_removed_breaking() {
    let mut old_cls = java_class("Config", "com.example");
    old_cls.members.push(with_annotation(
        java_method("dataSource", "com.example.Config", vec![], "DataSource"),
        "Bean",
    ));

    let mut new_cls = java_class("Config", "com.example");
    new_cls.members.push(java_method(
        "dataSource",
        "com.example.Config",
        vec![],
        "DataSource",
    ));

    insta::assert_yaml_snapshot!(diff(
        &surface(vec![old_cls]),
        &surface(vec![new_cls]),
    ));
}

#[test]
fn baseline_annotation_added_non_breaking() {
    let mut old_cls = java_class("Service", "com.example");
    old_cls.members.push(java_method(
        "process",
        "com.example.Service",
        vec![],
        "void",
    ));

    let mut new_cls = java_class("Service", "com.example");
    new_cls.members.push(with_annotation(
        java_method("process", "com.example.Service", vec![], "void"),
        "Deprecated",
    ));

    insta::assert_yaml_snapshot!(diff(
        &surface(vec![old_cls]),
        &surface(vec![new_cls]),
    ));
}

// ── Throws clause changes ───────────────────────────────────────────

#[test]
fn baseline_throws_added() {
    let mut old_cls = java_class("Reader", "com.example");
    old_cls.members.push(java_method(
        "read",
        "com.example.Reader",
        vec![],
        "String",
    ));

    let mut new_cls = java_class("Reader", "com.example");
    new_cls.members.push(with_throws(
        java_method("read", "com.example.Reader", vec![], "String"),
        vec!["IOException"],
    ));

    insta::assert_yaml_snapshot!(diff(
        &surface(vec![old_cls]),
        &surface(vec![new_cls]),
    ));
}

// ── Final / sealed changes ──────────────────────────────────────────

#[test]
fn baseline_class_became_final() {
    let old = surface(vec![java_class("Base", "com.example")]);
    let new = surface(vec![with_final(java_class("Base", "com.example"))]);
    insta::assert_yaml_snapshot!(diff(&old, &new));
}

#[test]
fn baseline_class_became_sealed() {
    let old = surface(vec![java_class("Shape", "com.example")]);
    let new = surface(vec![with_sealed(
        java_class("Shape", "com.example"),
        vec!["Circle", "Rectangle"],
    )]);
    insta::assert_yaml_snapshot!(diff(&old, &new));
}

// ── Relocation (package move) ───────────────────────────────────────

#[test]
fn baseline_class_relocated() {
    let mut old_cls = java_class("CacheManager", "com.example.cache");
    old_cls.import_path = Some("com.example.cache.CacheManager".into());

    let mut new_cls = java_class("CacheManager", "com.example.cache.auto");
    new_cls.import_path = Some("com.example.cache.auto.CacheManager".into());

    insta::assert_yaml_snapshot!(diff(
        &surface(vec![old_cls]),
        &surface(vec![new_cls]),
    ));
}

// ── Inheritance changes ─────────────────────────────────────────────

#[test]
fn baseline_superclass_changed() {
    let old = surface(vec![with_extends(
        java_class("MyList", "com.example"),
        "AbstractList",
    )]);
    let new = surface(vec![with_extends(
        java_class("MyList", "com.example"),
        "AbstractCollection",
    )]);
    insta::assert_yaml_snapshot!(diff(&old, &new));
}

// ── Constant value changed ──────────────────────────────────────────

#[test]
fn baseline_constant_value_changed() {
    let mut old_cls = java_class("Config", "com.example");
    old_cls.members.push(java_constant(
        "MAX_SIZE",
        "com.example.Config",
        "int",
        "100",
    ));

    let mut new_cls = java_class("Config", "com.example");
    new_cls.members.push(java_constant(
        "MAX_SIZE",
        "com.example.Config",
        "int",
        "200",
    ));

    insta::assert_yaml_snapshot!(diff(
        &surface(vec![old_cls]),
        &surface(vec![new_cls]),
    ));
}
