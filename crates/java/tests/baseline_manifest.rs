//! Baseline integration tests for Java manifest diffing.
//!
//! Tests pom.xml and build.gradle parsing and diffing with insta snapshots.

mod helpers;

use helpers::*;
use semver_analyzer_java::Java;
use semver_analyzer_core::Language;

fn diff_manifest(old: &str, new: &str) -> Vec<NormalizedManifestChange> {
    normalize_manifest(&Java::diff_manifest_content(old, new))
}

// ── pom.xml tests ───────────────────────────────────────────────────

#[test]
fn baseline_pom_dependency_added() {
    let old = r#"<?xml version="1.0"?>
<project>
    <dependencies>
        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-web</artifactId>
        </dependency>
    </dependencies>
</project>"#;

    let new = r#"<?xml version="1.0"?>
<project>
    <dependencies>
        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-web</artifactId>
        </dependency>
        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-data-jpa</artifactId>
        </dependency>
    </dependencies>
</project>"#;

    insta::assert_yaml_snapshot!(diff_manifest(old, new));
}

#[test]
fn baseline_pom_dependency_removed() {
    let old = r#"<?xml version="1.0"?>
<project>
    <dependencies>
        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-web</artifactId>
        </dependency>
        <dependency>
            <groupId>javax.servlet</groupId>
            <artifactId>javax.servlet-api</artifactId>
        </dependency>
    </dependencies>
</project>"#;

    let new = r#"<?xml version="1.0"?>
<project>
    <dependencies>
        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-web</artifactId>
        </dependency>
    </dependencies>
</project>"#;

    insta::assert_yaml_snapshot!(diff_manifest(old, new));
}

#[test]
fn baseline_pom_dependency_version_changed() {
    let old = r#"<?xml version="1.0"?>
<project>
    <dependencies>
        <dependency>
            <groupId>com.google.guava</groupId>
            <artifactId>guava</artifactId>
            <version>31.0-jre</version>
        </dependency>
    </dependencies>
</project>"#;

    let new = r#"<?xml version="1.0"?>
<project>
    <dependencies>
        <dependency>
            <groupId>com.google.guava</groupId>
            <artifactId>guava</artifactId>
            <version>33.0-jre</version>
        </dependency>
    </dependencies>
</project>"#;

    insta::assert_yaml_snapshot!(diff_manifest(old, new));
}

#[test]
fn baseline_pom_parent_version_changed() {
    let old = r#"<?xml version="1.0"?>
<project>
    <parent>
        <groupId>org.springframework.boot</groupId>
        <artifactId>spring-boot-starter-parent</artifactId>
        <version>3.2.0</version>
    </parent>
</project>"#;

    let new = r#"<?xml version="1.0"?>
<project>
    <parent>
        <groupId>org.springframework.boot</groupId>
        <artifactId>spring-boot-starter-parent</artifactId>
        <version>4.0.0</version>
    </parent>
</project>"#;

    insta::assert_yaml_snapshot!(diff_manifest(old, new));
}

#[test]
fn baseline_pom_no_changes() {
    let pom = r#"<?xml version="1.0"?>
<project>
    <dependencies>
        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-web</artifactId>
        </dependency>
    </dependencies>
</project>"#;

    insta::assert_yaml_snapshot!(diff_manifest(pom, pom));
}

// ── build.gradle tests ──────────────────────────────────────────────

#[test]
fn baseline_gradle_dependency_added() {
    let old = r#"
plugins {
    id 'java'
}
dependencies {
    implementation 'org.springframework.boot:spring-boot-starter-web:3.2.0'
}
"#;

    let new = r#"
plugins {
    id 'java'
}
dependencies {
    implementation 'org.springframework.boot:spring-boot-starter-web:3.2.0'
    implementation 'org.springframework.boot:spring-boot-starter-data-jpa:3.2.0'
}
"#;

    insta::assert_yaml_snapshot!(diff_manifest(old, new));
}

#[test]
fn baseline_gradle_dependency_version_changed() {
    let old = r#"
dependencies {
    implementation 'com.google.guava:guava:31.0-jre'
}
"#;

    let new = r#"
dependencies {
    implementation 'com.google.guava:guava:33.0-jre'
}
"#;

    insta::assert_yaml_snapshot!(diff_manifest(old, new));
}
