//! Baseline integration tests for the Java SD pipeline.
//!
//! Tests profile extraction from source and profile diffing with insta snapshots.

use semver_analyzer_java::sd_types::*;
use serde::Serialize;

// ── Normalized SD change for snapshotting ────────────────────────────

#[derive(Debug, Serialize)]
struct NormalizedSdChange {
    class_name: String,
    category: String,
    description: String,
    is_breaking: bool,
    method: Option<String>,
    old_value: Option<String>,
    new_value: Option<String>,
}

impl From<&JavaSourceChange> for NormalizedSdChange {
    fn from(c: &JavaSourceChange) -> Self {
        NormalizedSdChange {
            class_name: c.class_name.clone(),
            category: format!("{:?}", c.category),
            description: c.description.clone(),
            is_breaking: c.is_breaking,
            method: c.method.clone(),
            old_value: c.old_value.clone(),
            new_value: c.new_value.clone(),
        }
    }
}

fn normalize_sd(changes: &[JavaSourceChange]) -> Vec<NormalizedSdChange> {
    changes.iter().map(NormalizedSdChange::from).collect()
}

// ── Profile diffing helper ──────────────────────────────────────────

fn diff_profiles(old: &JavaClassProfile, new: &JavaClassProfile) -> Vec<NormalizedSdChange> {
    let mut changes = Vec::new();
    semver_analyzer_java::sd_pipeline::diff_class_profiles(old, new, &mut changes);
    normalize_sd(&changes)
}

fn method(name: &str, qname: &str) -> MethodProfile {
    MethodProfile {
        name: name.to_string(),
        qualified_name: qname.to_string(),
        is_synchronized: false,
        is_native: false,
        is_override: false,
        is_default: false,
        is_abstract: false,
        thrown_exceptions: Vec::new(),
        annotations: Vec::new(),
        delegations: Vec::new(),
        return_type: None,
        param_types: Vec::new(),
    }
}

// ── Annotation changes ──────────────────────────────────────────────

#[test]
fn sd_annotation_removed_from_class() {
    let old = JavaClassProfile {
        qualified_name: "com.example.Service".into(),
        name: "Service".into(),
        annotations: vec![ProfileAnnotation {
            name: "Component".into(),
            qualified_name: None,
            attributes: vec![],
        }],
        ..Default::default()
    };
    let new = JavaClassProfile {
        qualified_name: "com.example.Service".into(),
        name: "Service".into(),
        ..Default::default()
    };

    insta::assert_yaml_snapshot!(diff_profiles(&old, &new));
}

#[test]
fn sd_annotation_changed_attributes() {
    let old = JavaClassProfile {
        qualified_name: "com.example.Config".into(),
        name: "Config".into(),
        annotations: vec![ProfileAnnotation {
            name: "RequestMapping".into(),
            qualified_name: None,
            attributes: vec![("value".into(), "\"/api/v1\"".into())],
        }],
        ..Default::default()
    };
    let new = JavaClassProfile {
        qualified_name: "com.example.Config".into(),
        name: "Config".into(),
        annotations: vec![ProfileAnnotation {
            name: "RequestMapping".into(),
            qualified_name: None,
            attributes: vec![("value".into(), "\"/api/v2\"".into())],
        }],
        ..Default::default()
    };

    insta::assert_yaml_snapshot!(diff_profiles(&old, &new));
}

// ── Synchronized changes ────────────────────────────────────────────

#[test]
fn sd_synchronized_removed() {
    let old = JavaClassProfile {
        qualified_name: "com.example.Cache".into(),
        name: "Cache".into(),
        methods: vec![MethodProfile {
            is_synchronized: true,
            ..method("update", "com.example.Cache.update")
        }],
        ..Default::default()
    };
    let new = JavaClassProfile {
        qualified_name: "com.example.Cache".into(),
        name: "Cache".into(),
        methods: vec![method("update", "com.example.Cache.update")],
        ..Default::default()
    };

    insta::assert_yaml_snapshot!(diff_profiles(&old, &new));
}

// ── Exception changes ───────────────────────────────────────────────

#[test]
fn sd_exception_added() {
    let old = JavaClassProfile {
        qualified_name: "com.example.IO".into(),
        name: "IO".into(),
        methods: vec![method("read", "com.example.IO.read")],
        ..Default::default()
    };
    let new = JavaClassProfile {
        qualified_name: "com.example.IO".into(),
        name: "IO".into(),
        methods: vec![MethodProfile {
            thrown_exceptions: vec!["IOException".into()],
            ..method("read", "com.example.IO.read")
        }],
        ..Default::default()
    };

    insta::assert_yaml_snapshot!(diff_profiles(&old, &new));
}

// ── Override changes ────────────────────────────────────────────────

#[test]
fn sd_override_removed() {
    let old = JavaClassProfile {
        qualified_name: "com.example.Sub".into(),
        name: "Sub".into(),
        methods: vec![MethodProfile {
            is_override: true,
            ..method("toString", "com.example.Sub.toString")
        }],
        ..Default::default()
    };
    let new = JavaClassProfile {
        qualified_name: "com.example.Sub".into(),
        name: "Sub".into(),
        methods: vec![method("toString", "com.example.Sub.toString")],
        ..Default::default()
    };

    insta::assert_yaml_snapshot!(diff_profiles(&old, &new));
}

// ── Final / sealed ──────────────────────────────────────────────────

#[test]
fn sd_class_became_final() {
    let old = JavaClassProfile {
        qualified_name: "com.example.Base".into(),
        name: "Base".into(),
        ..Default::default()
    };
    let new = JavaClassProfile {
        qualified_name: "com.example.Base".into(),
        name: "Base".into(),
        is_final: true,
        ..Default::default()
    };

    insta::assert_yaml_snapshot!(diff_profiles(&old, &new));
}

#[test]
fn sd_class_became_sealed() {
    let old = JavaClassProfile {
        qualified_name: "com.example.Shape".into(),
        name: "Shape".into(),
        ..Default::default()
    };
    let new = JavaClassProfile {
        qualified_name: "com.example.Shape".into(),
        name: "Shape".into(),
        is_sealed: true,
        permits: vec!["Circle".into(), "Rectangle".into()],
        ..Default::default()
    };

    insta::assert_yaml_snapshot!(diff_profiles(&old, &new));
}

// ── Constructor dependency changed ──────────────────────────────────

#[test]
fn sd_constructor_dependency_changed() {
    let old = JavaClassProfile {
        qualified_name: "com.example.Service".into(),
        name: "Service".into(),
        constructor_params: vec!["UserRepository".into()],
        ..Default::default()
    };
    let new = JavaClassProfile {
        qualified_name: "com.example.Service".into(),
        name: "Service".into(),
        constructor_params: vec!["UserRepository".into(), "EventPublisher".into()],
        ..Default::default()
    };

    insta::assert_yaml_snapshot!(diff_profiles(&old, &new));
}

// ── Inheritance changed ─────────────────────────────────────────────

#[test]
fn sd_superclass_changed() {
    let old = JavaClassProfile {
        qualified_name: "com.example.MyList".into(),
        name: "MyList".into(),
        extends: Some("AbstractList".into()),
        ..Default::default()
    };
    let new = JavaClassProfile {
        qualified_name: "com.example.MyList".into(),
        name: "MyList".into(),
        extends: Some("AbstractCollection".into()),
        ..Default::default()
    };

    insta::assert_yaml_snapshot!(diff_profiles(&old, &new));
}

// ── Delegation changed ──────────────────────────────────────────────

#[test]
fn sd_delegation_changed() {
    let old = JavaClassProfile {
        qualified_name: "com.example.Processor".into(),
        name: "Processor".into(),
        methods: vec![MethodProfile {
            delegations: vec!["validator.validate".into(), "repo.save".into()],
            ..method("process", "com.example.Processor.process")
        }],
        ..Default::default()
    };
    let new = JavaClassProfile {
        qualified_name: "com.example.Processor".into(),
        name: "Processor".into(),
        methods: vec![MethodProfile {
            delegations: vec!["validator.validate".into(), "eventBus.publish".into()],
            ..method("process", "com.example.Processor.process")
        }],
        ..Default::default()
    };

    insta::assert_yaml_snapshot!(diff_profiles(&old, &new));
}

// ── Serialization changes ───────────────────────────────────────────

#[test]
fn sd_serialization_field_removed() {
    let old = JavaClassProfile {
        qualified_name: "com.example.Data".into(),
        name: "Data".into(),
        is_serializable: true,
        fields: vec![
            FieldProfile {
                name: "name".into(),
                field_type: "String".into(),
                is_transient: false,
                is_volatile: false,
                is_static: false,
                is_final: false,
            },
            FieldProfile {
                name: "age".into(),
                field_type: "int".into(),
                is_transient: false,
                is_volatile: false,
                is_static: false,
                is_final: false,
            },
        ],
        ..Default::default()
    };
    let new = JavaClassProfile {
        qualified_name: "com.example.Data".into(),
        name: "Data".into(),
        is_serializable: true,
        fields: vec![FieldProfile {
            name: "name".into(),
            field_type: "String".into(),
            is_transient: false,
            is_volatile: false,
            is_static: false,
            is_final: false,
        }],
        ..Default::default()
    };

    // Use diff_serialization directly
    let changes = semver_analyzer_java::sd_pipeline::diff_serialization(&old, &new);
    insta::assert_yaml_snapshot!(normalize_sd(&changes));
}
