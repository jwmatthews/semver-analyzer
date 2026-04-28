//! Java test file discovery and assertion diff detection.
//!
//! Discovers JUnit/TestNG test files by convention (Maven/Gradle standard
//! layout) and detects assertion changes using text-based pattern matching.

use anyhow::{Context, Result};
use semver_analyzer_core::{TestConvention, TestDiff, TestFile};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Java test analyzer.
#[derive(Default)]
pub struct JavaTestAnalyzer;

impl JavaTestAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// Find test files associated with a Java source file.
    pub fn find_tests(&self, repo: &Path, source_file: &Path) -> Result<Vec<TestFile>> {
        let mut results = Vec::new();
        let mut seen = std::collections::HashSet::new();

        let source_str = source_file.to_string_lossy();
        let stem = source_file
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        if stem.is_empty() {
            return Ok(results);
        }

        // Strategy 1: Maven/Gradle standard layout
        if source_str.contains("/src/main/java/") {
            let test_base = source_str.replace("/src/main/java/", "/src/test/java/");
            let test_dir = Path::new(&test_base).parent().unwrap_or(Path::new(""));

            for suffix in &["Test", "Tests", "IT", "ITCase", "Spec"] {
                let test_path = test_dir.join(format!("{}{}.java", stem, suffix));
                let abs_path = repo.join(&test_path);
                if abs_path.exists() && seen.insert(abs_path.clone()) {
                    results.push(TestFile {
                        path: test_path,
                        convention: if *suffix == "IT" || *suffix == "ITCase" {
                            TestConvention::Suffix(suffix.to_string())
                        } else {
                            TestConvention::MirrorTree("src/test/java".to_string())
                        },
                    });
                }
            }

            // Also check TestFoo prefix pattern
            let test_prefix_path = test_dir.join(format!("Test{}.java", stem));
            let abs_prefix_path = repo.join(&test_prefix_path);
            if abs_prefix_path.exists() && seen.insert(abs_prefix_path.clone()) {
                results.push(TestFile {
                    path: test_prefix_path,
                    convention: TestConvention::MirrorTree("src/test/java".to_string()),
                });
            }
        }

        // Strategy 2: Sibling test file (same directory)
        if let Some(parent) = source_file.parent() {
            for suffix in &["Test", "Tests", "IT", "ITCase", "Spec"] {
                let test_path = parent.join(format!("{}{}.java", stem, suffix));
                let abs_path = repo.join(&test_path);
                if abs_path.exists() && seen.insert(abs_path.clone()) {
                    results.push(TestFile {
                        path: test_path,
                        convention: TestConvention::Suffix(suffix.to_string()),
                    });
                }
            }

            // TestFoo prefix
            let test_prefix_path = parent.join(format!("Test{}.java", stem));
            let abs_prefix_path = repo.join(&test_prefix_path);
            if abs_prefix_path.exists() && seen.insert(abs_prefix_path.clone()) {
                results.push(TestFile {
                    path: test_prefix_path,
                    convention: TestConvention::Suffix("Test".to_string()),
                });
            }
        }

        // Strategy 3: Search test directories
        let test_dirs = ["src/test/java", "src/test", "test"];
        for test_dir in &test_dirs {
            let abs_test_dir = repo.join(test_dir);
            if abs_test_dir.is_dir() {
                find_tests_recursive(repo, &abs_test_dir, &stem, &mut results, &mut seen)?;
            }
        }

        Ok(results)
    }

    /// Diff test assertions between two git refs.
    pub fn diff_test_assertions(
        &self,
        repo: &Path,
        test_file: &TestFile,
        from_ref: &str,
        to_ref: &str,
    ) -> Result<TestDiff> {
        let output = Command::new("git")
            .args([
                "diff",
                &format!("{}..{}", from_ref, to_ref),
                "--",
                &test_file.path.to_string_lossy(),
            ])
            .current_dir(repo)
            .output()
            .context("Failed to run git diff for test file")?;

        let diff_text = String::from_utf8_lossy(&output.stdout).to_string();

        let mut removed_assertions = Vec::new();
        let mut added_assertions = Vec::new();

        for line in diff_text.lines() {
            if line.starts_with('-') && !line.starts_with("---") {
                let content = &line[1..];
                if is_assertion_line(content) {
                    removed_assertions.push(content.trim().to_string());
                }
            } else if line.starts_with('+') && !line.starts_with("+++") {
                let content = &line[1..];
                if is_assertion_line(content) {
                    added_assertions.push(content.trim().to_string());
                }
            }
        }

        let has_changes = !removed_assertions.is_empty() || !added_assertions.is_empty();

        Ok(TestDiff {
            test_file: test_file.path.clone(),
            removed_assertions,
            added_assertions,
            has_assertion_changes: has_changes,
            full_diff: diff_text,
        })
    }
}

fn find_tests_recursive(
    repo: &Path,
    dir: &Path,
    stem: &str,
    results: &mut Vec<TestFile>,
    seen: &mut std::collections::HashSet<PathBuf>,
) -> Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            find_tests_recursive(repo, &path, stem, results, seen)?;
        } else {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            if name_str.ends_with(".java") {
                for suffix in &["Test", "Tests", "IT", "ITCase", "Spec"] {
                    let expected = format!("{}{}.java", stem, suffix);
                    if name_str.as_ref() == expected {
                        let rel_path = path.strip_prefix(repo).unwrap_or(&path).to_path_buf();
                        if seen.insert(path.clone()) {
                            results.push(TestFile {
                                path: rel_path,
                                convention: TestConvention::TestsDir,
                            });
                        }
                    }
                }
                // TestFoo prefix pattern
                let prefix_expected = format!("Test{}.java", stem);
                if name_str.as_ref() == prefix_expected {
                    let rel_path = path.strip_prefix(repo).unwrap_or(&path).to_path_buf();
                    if seen.insert(path.clone()) {
                        results.push(TestFile {
                            path: rel_path,
                            convention: TestConvention::TestsDir,
                        });
                    }
                }
            }
        }
    }

    Ok(())
}

fn is_assertion_line(line: &str) -> bool {
    let trimmed = line.trim();

    if trimmed.is_empty() || trimmed.starts_with("//") || trimmed.starts_with('*') {
        return false;
    }

    // JUnit 4/5
    if trimmed.contains("assertEquals(")
        || trimmed.contains("assertNotEquals(")
        || trimmed.contains("assertTrue(")
        || trimmed.contains("assertFalse(")
        || trimmed.contains("assertNull(")
        || trimmed.contains("assertNotNull(")
        || trimmed.contains("assertThrows(")
        || trimmed.contains("assertAll(")
        || trimmed.contains("assertInstanceOf(")
        || trimmed.contains("assertArrayEquals(")
        || trimmed.contains("assertSame(")
        || trimmed.contains("assertNotSame(")
        || trimmed.contains("assertDoesNotThrow(")
        || trimmed.contains("assertTimeout(")
        || trimmed.contains("assertTimeoutPreemptively(")
        || trimmed.contains("assertIterableEquals(")
        || trimmed.contains("assertLinesMatch(")
    {
        return true;
    }

    // AssertJ (comprehensive)
    if trimmed.contains("assertThat(")
        || trimmed.contains("assertThatThrownBy(")
        || trimmed.contains("assertThatCode(")
        || trimmed.contains("assertThatExceptionOfType(")
        || trimmed.contains("assertThatNoException(")
        || trimmed.contains(".isEqualTo(")
        || trimmed.contains(".isNotEqualTo(")
        || trimmed.contains(".isNull()")
        || trimmed.contains(".isNotNull()")
        || trimmed.contains(".isTrue()")
        || trimmed.contains(".isFalse()")
        || trimmed.contains(".isEmpty()")
        || trimmed.contains(".isNotEmpty()")
        || trimmed.contains(".isPresent()")
        || trimmed.contains(".isNotPresent()")
        || trimmed.contains(".hasSize(")
        || trimmed.contains(".hasSizeGreaterThan(")
        || trimmed.contains(".contains(")
        || trimmed.contains(".containsExactly(")
        || trimmed.contains(".containsExactlyInAnyOrder(")
        || trimmed.contains(".containsOnly(")
        || trimmed.contains(".doesNotContain(")
        || trimmed.contains(".containsKey(")
        || trimmed.contains(".containsValue(")
        || trimmed.contains(".containsEntry(")
        || trimmed.contains(".isInstanceOf(")
        || trimmed.contains(".isNotInstanceOf(")
        || trimmed.contains(".isExactlyInstanceOf(")
        || trimmed.contains(".extracting(")
        || trimmed.contains(".satisfies(")
        || trimmed.contains(".allMatch(")
        || trimmed.contains(".anyMatch(")
        || trimmed.contains(".noneMatch(")
        || trimmed.contains(".startsWith(")
        || trimmed.contains(".endsWith(")
        || trimmed.contains(".matches(")
        || trimmed.contains(".isGreaterThan(")
        || trimmed.contains(".isLessThan(")
        || trimmed.contains(".isGreaterThanOrEqualTo(")
        || trimmed.contains(".isLessThanOrEqualTo(")
        || trimmed.contains(".isBetween(")
        || trimmed.contains(".isZero()")
        || trimmed.contains(".isPositive()")
        || trimmed.contains(".isNegative()")
        || trimmed.contains(".isBlank()")
        || trimmed.contains(".isNotBlank()")
        || trimmed.contains(".hasCause(")
        || trimmed.contains(".hasMessage(")
        || trimmed.contains(".hasMessageContaining(")
        || trimmed.contains(".isCompletedWithValue(")
        || trimmed.contains(".isSameAs(")
        || trimmed.contains(".isNotSameAs(")
        || trimmed.contains(".usingRecursiveComparison(")
    {
        return true;
    }

    // Hamcrest
    if trimmed.contains("assertThat(") && trimmed.contains(", is(")
        || trimmed.contains(", equalTo(")
        || trimmed.contains(", hasItem(")
        || trimmed.contains(", hasItems(")
        || trimmed.contains(", hasSize(")
        || trimmed.contains(", containsString(")
        || trimmed.contains(", startsWith(")
        || trimmed.contains(", endsWith(")
        || trimmed.contains(", instanceOf(")
        || trimmed.contains(", notNullValue(")
        || trimmed.contains(", nullValue(")
        || trimmed.contains(", not(")
        || trimmed.contains(", allOf(")
        || trimmed.contains(", anyOf(")
        || trimmed.contains(", both(")
        || trimmed.contains(", either(")
        || trimmed.contains(", hasEntry(")
        || trimmed.contains(", hasKey(")
        || trimmed.contains(", hasValue(")
        || trimmed.contains(", closeTo(")
        || trimmed.contains(", greaterThan(")
        || trimmed.contains(", lessThan(")
        || trimmed.contains("MatcherAssert.assertThat(")
    {
        return true;
    }

    // TestNG
    if trimmed.contains("Assert.assertEquals(")
        || trimmed.contains("Assert.assertTrue(")
        || trimmed.contains("Assert.assertFalse(")
        || trimmed.contains("Assert.assertNull(")
        || trimmed.contains("Assert.assertNotNull(")
        || trimmed.contains("Assert.assertThrows(")
        || trimmed.contains("Assert.expectThrows(")
    {
        return true;
    }

    // Mockito
    if trimmed.contains("verify(")
        || trimmed.contains("verifyNoInteractions(")
        || trimmed.contains("verifyNoMoreInteractions(")
        || trimmed.contains("verifyZeroInteractions(")
    {
        return true;
    }

    // Google Truth
    if trimmed.contains("assertWithMessage(")
        || trimmed.contains("assertAbout(")
        || (trimmed.contains("Truth.assertThat(") || trimmed.contains("expect.that("))
    {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_assertion_line_junit() {
        assert!(is_assertion_line("  assertEquals(expected, actual);"));
        assert!(is_assertion_line(
            "  assertThrows(Exception.class, () -> foo());"
        ));
        assert!(is_assertion_line("  assertTrue(result.isPresent());"));
    }

    #[test]
    fn test_is_assertion_line_assertj() {
        assert!(is_assertion_line(
            "  assertThat(result).isEqualTo(expected);"
        ));
        assert!(is_assertion_line("  assertThat(list).hasSize(3);"));
    }

    #[test]
    fn test_is_assertion_line_hamcrest() {
        assert!(is_assertion_line(
            "  assertThat(result, is(equalTo(expected)));"
        ));
        assert!(is_assertion_line("  assertThat(list, hasItem(42));"));
        assert!(is_assertion_line(
            "  MatcherAssert.assertThat(x, notNullValue());"
        ));
    }

    #[test]
    fn test_is_assertion_line_assertj_extended() {
        assert!(is_assertion_line(
            "  assertThat(list).containsExactly(1, 2, 3);"
        ));
        assert!(is_assertion_line("  assertThat(str).startsWith(\"foo\");"));
        assert!(is_assertion_line(
            "  assertThat(opt).isPresent();"
        ));
        assert!(is_assertion_line(
            "  assertThatThrownBy(() -> foo()).isInstanceOf(Exception.class);"
        ));
        assert!(is_assertion_line(
            "  assertThat(result).extracting(\"name\").isEqualTo(\"test\");"
        ));
    }

    #[test]
    fn test_is_assertion_line_truth() {
        assert!(is_assertion_line(
            "  Truth.assertThat(result).isEqualTo(expected);"
        ));
        assert!(is_assertion_line(
            "  assertWithMessage(\"should be true\").that(x).isTrue();"
        ));
    }

    #[test]
    fn test_is_assertion_line_negative() {
        assert!(!is_assertion_line("  int x = 1;"));
        assert!(!is_assertion_line("  // assertEquals(a, b);"));
        assert!(!is_assertion_line(""));
        assert!(!is_assertion_line("  * assertEquals in javadoc"));
    }
}
