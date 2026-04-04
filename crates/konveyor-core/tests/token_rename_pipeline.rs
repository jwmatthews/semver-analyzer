//! Integration test: full pipeline for design-token renames.
//!
//! Loads the complete set of 4028 token renames extracted from the real
//! PatternFly v5.4.0 → v6.4.1 migration report and verifies that:
//!
//! 1. `api_change_to_strategy` produces a `Rename` strategy for every entry
//!    with clean symbol names (no symbol_summary pollution).
//! 2. Rules built from those strategies consolidate correctly.
//! 3. The consolidated fix-strategy mappings all contain clean from/to names.
//! 4. A lookup by old token name always succeeds in the mappings.

use std::collections::HashMap;

use semver_analyzer_core::{ApiChange, ApiChangeKind, ApiChangeType};
use semver_analyzer_konveyor_core::{
    api_change_to_strategy, consolidate_rules, extract_fix_strategies, extract_name_from_summary,
    KonveyorCondition, KonveyorRule, RenamePatterns,
};

/// A single entry from the fixture file.
#[derive(serde::Deserialize)]
struct TokenRenameEntry {
    file: String,
    symbol: String,
    before: String,
    after: String,
}

fn load_fixture() -> Vec<TokenRenameEntry> {
    let data = include_str!("fixtures/token_renames.json");
    serde_json::from_str(data).expect("failed to parse token_renames.json fixture")
}

// ── 1. api_change_to_strategy produces correct Rename for every token ────

#[test]
fn test_every_token_rename_produces_rename_strategy() {
    let entries = load_fixture();
    assert!(
        entries.len() > 4000,
        "Expected 4000+ token renames in fixture, got {}",
        entries.len()
    );

    let patterns = RenamePatterns::empty();
    let member_renames = HashMap::new();

    let mut failures = Vec::new();

    for entry in &entries {
        let change = ApiChange {
            symbol: entry.symbol.clone(),
            qualified_name: String::new(),
            kind: ApiChangeKind::Constant,
            change: ApiChangeType::Renamed,
            before: Some(entry.before.clone()),
            after: Some(entry.after.clone()),
            description: format!("Exported constant `{}` was renamed", entry.symbol),
            migration_target: None,
            removal_disposition: None,
            renders_element: None,
        };

        let strat = api_change_to_strategy(&change, &patterns, &member_renames, &entry.file);

        match strat {
            None => {
                failures.push(format!("{}: no strategy produced", entry.symbol));
            }
            Some(s) => {
                if s.strategy != "Rename" {
                    failures.push(format!(
                        "{}: expected Rename, got {}",
                        entry.symbol, s.strategy
                    ));
                    continue;
                }

                // from must be the clean symbol name
                let from = s.from.as_deref().unwrap_or("");
                if from != entry.symbol {
                    failures.push(format!(
                        "{}: from mismatch: expected '{}', got '{}'",
                        entry.symbol, entry.symbol, from
                    ));
                }

                // to must be a clean name, not a symbol_summary string
                let to = s.to.as_deref().unwrap_or("");
                if to.contains("variable: ") || to.contains("constant: ") {
                    failures.push(format!(
                        "{}: 'to' is a raw symbol_summary: {}",
                        entry.symbol, to
                    ));
                }

                // to must match what extract_name_from_summary returns
                let expected_new = extract_name_from_summary(&entry.after);
                if to != expected_new {
                    failures.push(format!(
                        "{}: to mismatch: expected '{}', got '{}'",
                        entry.symbol, expected_new, to
                    ));
                }
            }
        }
    }

    assert!(
        failures.is_empty(),
        "{} / {} token renames failed:\n{}",
        failures.len(),
        entries.len(),
        failures[..failures.len().min(20)].join("\n")
    );
}

// ── 2. Consolidation + fix-strategy mappings are all clean ───────────────

#[test]
fn test_consolidated_token_strategies_have_clean_mappings() {
    let entries = load_fixture();
    let patterns = RenamePatterns::empty();
    let member_renames = HashMap::new();

    // Build one KonveyorRule per token rename (mimicking generate_rules).
    let rules: Vec<KonveyorRule> = entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let change = ApiChange {
                symbol: entry.symbol.clone(),
                qualified_name: String::new(),
                kind: ApiChangeKind::Constant,
                change: ApiChangeType::Renamed,
                before: Some(entry.before.clone()),
                after: Some(entry.after.clone()),
                description: format!("Exported constant `{}` was renamed", entry.symbol),
                migration_target: None,
                removal_disposition: None,
                renders_element: None,
            };

            let fix_strategy =
                api_change_to_strategy(&change, &patterns, &member_renames, &entry.file);

            KonveyorRule {
                rule_id: format!("semver-token-rename-{}", i),
                labels: vec![
                    "change-type=renamed".to_string(),
                    "kind=constant".to_string(),
                    "has-codemod=true".to_string(),
                    format!("package=@patternfly/react-tokens"),
                ],
                effort: 1,
                category: "mandatory".to_string(),
                description: format!("Exported constant `{}` was renamed", entry.symbol),
                message: format!("File: {}\ntoken renamed", entry.file),
                links: vec![],
                when: KonveyorCondition::FileContent {
                    filecontent: semver_analyzer_konveyor_core::FileContentFields {
                        pattern: format!("\\b{}\\b", entry.symbol),
                        file_pattern: "*.{ts,tsx}".to_string(),
                    },
                },
                fix_strategy,
            }
        })
        .collect();

    let pre_count = rules.len();
    assert!(pre_count > 4000);

    // Consolidate
    let (consolidated, _id_map) = consolidate_rules(rules);

    // Should consolidate into far fewer rules
    let token_rules: Vec<&KonveyorRule> = consolidated
        .iter()
        .filter(|r| r.labels.contains(&"kind=constant".to_string()))
        .filter(|r| r.labels.contains(&"change-type=renamed".to_string()))
        .collect();

    let total_consolidated = token_rules.len();
    // has-codemod=true rules are kept as singletons (not consolidated)
    // to preserve their per-token Rename mappings. So the count should
    // remain the same or very close to the original.
    assert!(
        total_consolidated > 0,
        "Expected token rules to survive consolidation, got 0",
    );

    // Extract fix strategies
    let strategies = extract_fix_strategies(&consolidated);

    // Collect all mappings across all consolidated token rules.
    // Some PascalCase constants (e.g., Chart, ChartArea) stay as individual
    // rules because `consolidation_key` treats them differently. Count
    // mappings from both the big groups and the individual rules.
    let mut all_mappings = Vec::new();
    for rule in &token_rules {
        if let Some(strat) = strategies.get(&rule.rule_id) {
            assert_eq!(
                strat.strategy, "Rename",
                "Rule {} should have Rename strategy, got {}",
                rule.rule_id, strat.strategy
            );
            if strat.mappings.is_empty() {
                // Individual (non-consolidated) rule: from/to on the entry itself
                if let (Some(from), Some(to)) = (&strat.from, &strat.to) {
                    all_mappings.push((from.clone(), to.clone()));
                }
            } else {
                for m in &strat.mappings {
                    if let (Some(from), Some(to)) = (&m.from, &m.to) {
                        all_mappings.push((from.clone(), to.clone()));
                    }
                }
            }
        }
    }

    // Every original entry should have a mapping
    assert!(
        all_mappings.len() >= entries.len(),
        "Expected at least {} mappings across consolidated rules, got {}",
        entries.len(),
        all_mappings.len()
    );

    // Every mapping must have clean names (no symbol_summary strings)
    let mut dirty = Vec::new();
    for (from, to) in &all_mappings {
        if from.contains("variable: ") || from.contains("constant: ") {
            dirty.push(format!("from: {}", from));
        }
        if to.contains("variable: ") || to.contains("constant: ") {
            dirty.push(format!("to: {}", to));
        }
    }

    assert!(
        dirty.is_empty(),
        "{} mappings have symbol_summary pollution:\n{}",
        dirty.len(),
        dirty[..dirty.len().min(20)].join("\n")
    );
}

// ── 3. Every original symbol is findable in the consolidated mappings ────

#[test]
fn test_every_token_findable_in_consolidated_mappings() {
    let entries = load_fixture();
    let patterns = RenamePatterns::empty();
    let member_renames = HashMap::new();

    let rules: Vec<KonveyorRule> = entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let change = ApiChange {
                symbol: entry.symbol.clone(),
                qualified_name: String::new(),
                kind: ApiChangeKind::Constant,
                change: ApiChangeType::Renamed,
                before: Some(entry.before.clone()),
                after: Some(entry.after.clone()),
                description: format!("Exported constant `{}` was renamed", entry.symbol),
                migration_target: None,
                removal_disposition: None,
                renders_element: None,
            };

            let fix_strategy =
                api_change_to_strategy(&change, &patterns, &member_renames, &entry.file);

            KonveyorRule {
                rule_id: format!("semver-token-rename-{}", i),
                labels: vec![
                    "change-type=renamed".to_string(),
                    "kind=constant".to_string(),
                    "has-codemod=true".to_string(),
                    format!("package=@patternfly/react-tokens"),
                ],
                effort: 1,
                category: "mandatory".to_string(),
                description: format!("Exported constant `{}` was renamed", entry.symbol),
                message: format!("File: {}\ntoken renamed", entry.file),
                links: vec![],
                when: KonveyorCondition::FileContent {
                    filecontent: semver_analyzer_konveyor_core::FileContentFields {
                        pattern: format!("\\b{}\\b", entry.symbol),
                        file_pattern: "*.{ts,tsx}".to_string(),
                    },
                },
                fix_strategy,
            }
        })
        .collect();

    let (consolidated, _) = consolidate_rules(rules);
    let strategies = extract_fix_strategies(&consolidated);

    // Build a lookup: old_name → new_name from all consolidated mappings.
    // Include both grouped mappings and individual rule from/to fields.
    let mut rename_map: HashMap<String, String> = HashMap::new();
    for rule in &consolidated {
        if let Some(strat) = strategies.get(&rule.rule_id) {
            if strat.mappings.is_empty() {
                if let (Some(from), Some(to)) = (&strat.from, &strat.to) {
                    rename_map.insert(from.clone(), to.clone());
                }
            } else {
                for m in &strat.mappings {
                    if let (Some(from), Some(to)) = (&m.from, &m.to) {
                        rename_map.insert(from.clone(), to.clone());
                    }
                }
            }
        }
    }

    // Every unique symbol must be findable
    let unique_symbols: std::collections::HashSet<&str> =
        entries.iter().map(|e| e.symbol.as_str()).collect();

    let mut missing = Vec::new();
    let mut dirty_targets = Vec::new();

    for sym in &unique_symbols {
        match rename_map.get(*sym) {
            None => missing.push(sym.to_string()),
            Some(target) => {
                // Target must be a clean name (no symbol_summary strings)
                if target.contains("variable: ") || target.contains("constant: ") {
                    dirty_targets.push(format!("{} → {}", sym, target));
                }
            }
        }
    }

    assert!(
        missing.is_empty(),
        "{} / {} unique tokens not findable in consolidated mappings:\n{}",
        missing.len(),
        unique_symbols.len(),
        missing[..missing.len().min(20)].join("\n")
    );

    assert!(
        dirty_targets.is_empty(),
        "{} tokens have symbol_summary targets:\n{}",
        dirty_targets.len(),
        dirty_targets[..dirty_targets.len().min(20)].join("\n")
    );

    // Sanity: at least 2500 unique symbols exist in the map
    let found = unique_symbols
        .iter()
        .filter(|s| rename_map.contains_key(**s))
        .count();
    assert!(
        found >= 2500,
        "Expected at least 2500 unique tokens in rename map, got {}",
        found
    );
}
