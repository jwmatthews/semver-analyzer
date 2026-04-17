//! Deprecated replacement detection and structural change transformation.
//!
//! Moved from `src/orchestrator.rs` during genericization (Phase 2).
//! These functions are TypeScript/React-specific because they analyze
//! `SourceLevelCategory::RenderedComponent` changes from the SD pipeline.
//!
//! Two detection strategies are used:
//! 1. **Rendering swap** (primary): Host components that stopped rendering
//!    the deprecated component and started rendering the replacement
//!    (e.g., ToolbarFilter switched from Chip to Label).
//! 2. **Commit co-change** (fallback): The git commit that deprecated a
//!    component also modified source files in the replacement component's
//!    directory (e.g., the Tile deprecation commit modified Card/CardHeader.tsx).

use crate::sd_types::{
    DeprecatedReplacement, ReplacementEvidence, SdPipelineResult, SourceLevelCategory,
};
use semver_analyzer_core::{ChangeSubject, StructuralChange, StructuralChangeType, SymbolKind};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use tracing::debug;

/// Detect deprecated components that have differently-named replacements.
///
/// When a component is relocated to `/deprecated/` AND host components in the
/// codebase switched from rendering the old component to rendering a new one
/// (e.g., ToolbarFilter stopped rendering `Chip` and started rendering `Label`
/// — a clear 1:1 swap), record the replacement relationship.
///
/// Example: ToolbarFilter and MultiTypeaheadSelect both stopped rendering
/// `Chip` and started rendering `Label` → `Chip` is replaced by `Label`.
pub fn detect_deprecated_replacements(
    structural_changes: &[StructuralChange],
    sd: &SdPipelineResult,
) -> Vec<DeprecatedReplacement> {
    // Step 1: Collect component names that were relocated to deprecated.
    // Only look at variable/constant kinds (the component itself, not its Props interface).
    let relocated_components: HashSet<String> = structural_changes
        .iter()
        .filter(|sc| matches!(sc.change_type, StructuralChangeType::Relocated { .. }))
        .filter(|sc| sc.description.contains("moved to deprecated"))
        .filter(|sc| matches!(sc.kind, SymbolKind::Variable | SymbolKind::Constant))
        .map(|sc| sc.symbol.clone())
        .collect();

    if relocated_components.is_empty() {
        return vec![];
    }

    // Step 2: Build per-host rendering swap maps from SD source-level changes.
    // For each host component, track what it stopped and started rendering.
    let mut stopped_by_host: HashMap<String, HashSet<String>> = HashMap::new();
    let mut started_by_host: HashMap<String, HashSet<String>> = HashMap::new();

    for slc in &sd.source_level_changes {
        if slc.category != SourceLevelCategory::RenderedComponent {
            continue;
        }
        if let Some(ref old_val) = slc.old_value {
            if slc.new_value.is_none() {
                // "X no longer internally renders Y" — old_value = Y
                stopped_by_host
                    .entry(slc.component.clone())
                    .or_default()
                    .insert(old_val.clone());
            }
        }
        if let Some(ref new_val) = slc.new_value {
            if slc.old_value.is_none() {
                // "X now internally renders Y" — new_value = Y
                started_by_host
                    .entry(slc.component.clone())
                    .or_default()
                    .insert(new_val.clone());
            }
        }
    }

    // Step 3: For each relocated component, find hosts that stopped rendering
    // it and started rendering something new. The intersection of "started"
    // sets across hosts is the candidate replacement.
    let mut replacements = Vec::new();

    for old_comp in &relocated_components {
        // Find all hosts that stopped rendering this component
        let mut candidate_counts: HashMap<String, Vec<String>> = HashMap::new();

        for (host, stopped) in &stopped_by_host {
            if !stopped.contains(old_comp) {
                continue;
            }
            // This host stopped rendering old_comp — what did it start rendering?
            if let Some(started) = started_by_host.get(host) {
                for new_comp in started {
                    // Skip generic wrappers (Fragment, etc.) and the relocated
                    // component itself, and other relocated components.
                    if new_comp == "Fragment"
                        || new_comp == "React.Fragment"
                        || relocated_components.contains(new_comp)
                        || new_comp == old_comp
                    {
                        continue;
                    }
                    candidate_counts
                        .entry(new_comp.clone())
                        .or_default()
                        .push(host.clone());
                }
            }
        }

        // Pick the candidate with the most host evidence.
        // Tiebreaker: prefer candidates whose structural shape matches
        // (e.g., Chip → Label not LabelGroup; ChipGroup → LabelGroup not Label).
        let old_is_group = old_comp.ends_with("Group");
        if let Some((best_replacement, hosts)) =
            candidate_counts
                .into_iter()
                .max_by(|(name_a, hosts_a), (name_b, hosts_b)| {
                    hosts_a.len().cmp(&hosts_b.len()).then_with(|| {
                        // Prefer matching "Group" shape
                        let a_matches = name_a.ends_with("Group") == old_is_group;
                        let b_matches = name_b.ends_with("Group") == old_is_group;
                        a_matches.cmp(&b_matches)
                    })
                })
        {
            replacements.push(DeprecatedReplacement {
                old_component: old_comp.clone(),
                new_component: best_replacement,
                evidence_hosts: hosts,
                evidence_source: ReplacementEvidence::RenderingSwap,
            });
        }
    }

    replacements
}

/// Transform structural changes based on detected deprecated replacements.
///
/// For each component with a deprecated replacement:
/// 1. Convert the `Relocated` entry into a `Changed` entry with the
///    replacement component name in `after` and a descriptive message.
/// 2. Suppress the `signature_changed` entry for the Props interface
///    (base class change is a consequence of the replacement, not an
///    independent migration action).
pub fn apply_deprecated_replacements(
    structural_changes: Arc<Vec<StructuralChange>>,
    replacements: &[DeprecatedReplacement],
) -> Arc<Vec<StructuralChange>> {
    if replacements.is_empty() {
        return structural_changes;
    }

    // Build lookup: old component name → replacement info
    let replacement_map: HashMap<&str, &DeprecatedReplacement> = replacements
        .iter()
        .map(|r| (r.old_component.as_str(), r))
        .collect();

    // Also build a set of Props interface names to suppress signature-changed
    // entries for (e.g., "ChipProps" when "Chip" has a replacement).
    let suppressed_signature_changes: HashSet<String> = replacements
        .iter()
        .map(|r| format!("{}Props", r.old_component))
        .collect();

    let original = Arc::try_unwrap(structural_changes).unwrap_or_else(|arc| (*arc).clone());

    let mut result = Vec::with_capacity(original.len());

    for sc in original {
        // Check if this is a relocation for a replaced component
        if matches!(sc.change_type, StructuralChangeType::Relocated { .. }) {
            if let Some(repl) = replacement_map.get(sc.symbol.as_str()) {
                // Transform: Relocated → Changed (component replacement)
                result.push(StructuralChange {
                    change_type: StructuralChangeType::Changed(ChangeSubject::Symbol {
                        kind: sc.kind,
                    }),
                    before: Some(repl.old_component.clone()),
                    after: Some(repl.new_component.clone()),
                    description: format!(
                        "Component `{}` was deprecated and replaced by `{}`. \
                         Migrate from `<{}>` to `<{}>`.",
                        repl.old_component,
                        repl.new_component,
                        repl.old_component,
                        repl.new_component,
                    ),
                    ..sc
                });
                continue;
            }
            // Also check Props interfaces (e.g., ChipProps → LabelProps)
            let props_base = sc.symbol.strip_suffix("Props");
            if let Some(base) = props_base {
                if let Some(repl) = replacement_map.get(base) {
                    // Transform: Relocated ChipProps → Changed pointing to LabelProps
                    result.push(StructuralChange {
                        change_type: StructuralChangeType::Changed(ChangeSubject::Symbol {
                            kind: sc.kind,
                        }),
                        before: Some(format!("{}Props", repl.old_component)),
                        after: Some(format!("{}Props", repl.new_component)),
                        description: format!(
                            "Interface `{}Props` was deprecated and replaced by `{}Props`. \
                             Migrate from `{}Props` to `{}Props`.",
                            repl.old_component,
                            repl.new_component,
                            repl.old_component,
                            repl.new_component,
                        ),
                        ..sc
                    });
                    continue;
                }
            }
        }

        // Suppress signature-changed entries for Props of replaced components.
        // e.g., "ChipProps base class changed from X to LabelProps" is redundant
        // once we know Chip → Label.
        if matches!(sc.change_type, StructuralChangeType::Changed(_))
            && suppressed_signature_changes.contains(&sc.symbol)
            && sc.description.contains("base class changed")
        {
            debug!(
                symbol = %sc.symbol,
                "Suppressing signature-changed entry for replaced component Props"
            );
            continue;
        }

        result.push(sc);
    }

    Arc::new(result)
}

/// Detect deprecated replacements via git commit co-change analysis.
///
/// This is a **fallback** strategy that runs only for relocated components
/// not already detected by the rendering swap method. It analyzes the git
/// commits that deprecated each component to find which other component
/// families had source files modified in the same commit.
///
/// **Algorithm:**
/// 1. Collect relocated-to-deprecated component names from structural changes
/// 2. Filter out components already detected by rendering swap
/// 3. For each remaining component, find the commit(s) that deprecated it
/// 4. For each deprecation commit, find co-changed component families
/// 5. If exactly 1 candidate family → create a `DeprecatedReplacement`
/// 6. If multiple candidates → log at debug level, skip (conservative)
/// 7. If 0 candidates → skip
///
/// **Example:** The commit that deprecated Tile (`548cd3474`) also modified
/// `Card/CardHeader.tsx`. Card is the only non-self family with source file
/// changes → `Tile → Card` replacement detected.
pub fn detect_deprecated_replacements_from_commits(
    repo: &Path,
    from_ref: &str,
    to_ref: &str,
    structural_changes: &[StructuralChange],
    already_detected: &HashSet<&str>,
) -> Vec<DeprecatedReplacement> {
    use semver_analyzer_core::git::{commit_co_changed_families, find_deprecation_commits};

    // Step 1: Collect component names that were relocated to deprecated.
    let relocated_components: HashSet<String> = structural_changes
        .iter()
        .filter(|sc| matches!(sc.change_type, StructuralChangeType::Relocated { .. }))
        .filter(|sc| sc.description.contains("moved to deprecated"))
        .filter(|sc| matches!(sc.kind, SymbolKind::Variable | SymbolKind::Constant))
        .map(|sc| sc.symbol.clone())
        .collect();

    if relocated_components.is_empty() {
        return vec![];
    }

    // Step 2: Filter out already-detected components (rendering swap found them)
    let undetected: Vec<&str> = relocated_components
        .iter()
        .filter(|name| !already_detected.contains(name.as_str()))
        .map(|s| s.as_str())
        .collect();

    if undetected.is_empty() {
        debug!("All relocated components already have rendering swap replacements");
        return vec![];
    }

    // Step 3: Find deprecation commits in the git history
    let deprecation_commits = find_deprecation_commits(repo, from_ref, to_ref);
    if deprecation_commits.is_empty() {
        debug!("No deprecation commits found between {from_ref} and {to_ref}");
        return vec![];
    }

    let mut replacements = Vec::new();

    // Step 4: For each undetected component, find co-changed families
    for comp_name in &undetected {
        // Find commits that deprecated this component
        let relevant_commits: Vec<_> = deprecation_commits
            .iter()
            .filter(|dc| dc.component == *comp_name)
            .collect();

        if relevant_commits.is_empty() {
            debug!(
                component = comp_name,
                "No deprecation commit found for component"
            );
            continue;
        }

        // Collect co-changed families across all deprecation commits for this component
        let mut all_candidates: HashSet<String> = HashSet::new();
        let mut commit_shas: Vec<String> = Vec::new();

        for dc in &relevant_commits {
            let families = commit_co_changed_families(repo, &dc.sha, comp_name);
            for family in families {
                all_candidates.insert(family);
            }
            commit_shas.push(dc.sha.clone());
        }

        // Step 5: Conservative: only accept single-candidate results
        match all_candidates.len() {
            0 => {
                debug!(
                    component = comp_name,
                    commits = ?commit_shas,
                    "No co-changed component families found in deprecation commit(s)"
                );
            }
            1 => {
                let replacement = all_candidates.into_iter().next().unwrap();
                debug!(
                    old = comp_name,
                    new = %replacement,
                    commits = ?commit_shas,
                    "Deprecated replacement detected via commit co-change"
                );
                replacements.push(DeprecatedReplacement {
                    old_component: comp_name.to_string(),
                    new_component: replacement,
                    evidence_hosts: commit_shas,
                    evidence_source: ReplacementEvidence::CommitCoChange,
                });
            }
            n => {
                debug!(
                    component = comp_name,
                    candidates = ?all_candidates,
                    count = n,
                    "Multiple co-changed families found — skipping (ambiguous)"
                );
            }
        }
    }

    replacements
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sd_types::{SourceLevelCategory, SourceLevelChange};

    /// Helper: build a Relocated structural change for a component variable.
    fn relocated_component(name: &str) -> StructuralChange {
        StructuralChange {
            symbol: name.to_string(),
            qualified_name: format!("pkg/src/components/{name}/{name}.{name}"),
            kind: SymbolKind::Variable,
            package: Some("@patternfly/react-core".to_string()),
            change_type: StructuralChangeType::Relocated {
                from: ChangeSubject::Symbol {
                    kind: SymbolKind::Variable,
                },
                to: ChangeSubject::Symbol {
                    kind: SymbolKind::Variable,
                },
            },
            before: Some(format!("pkg/src/components/{name}/{name}.{name}")),
            after: Some(format!(
                "pkg/src/deprecated/components/{name}/{name}.{name}"
            )),
            description: format!("variable `{name}` moved to deprecated exports"),
            is_breaking: true,
            impact: None,
            migration_target: None,
        }
    }

    /// Helper: build a Relocated structural change for a Props interface.
    fn relocated_props(name: &str) -> StructuralChange {
        let props_name = format!("{name}Props");
        StructuralChange {
            symbol: props_name.clone(),
            qualified_name: format!("pkg/src/components/{name}/{name}.{props_name}"),
            kind: SymbolKind::Interface,
            package: Some("@patternfly/react-core".to_string()),
            change_type: StructuralChangeType::Relocated {
                from: ChangeSubject::Symbol {
                    kind: SymbolKind::Interface,
                },
                to: ChangeSubject::Symbol {
                    kind: SymbolKind::Interface,
                },
            },
            before: Some(format!("pkg/src/components/{name}/{name}.{props_name}")),
            after: Some(format!(
                "pkg/src/deprecated/components/{name}/{name}.{props_name}"
            )),
            description: format!("interface `{props_name}` moved to deprecated exports"),
            is_breaking: true,
            impact: None,
            migration_target: None,
        }
    }

    /// Helper: build a signature-changed structural change for Props base class.
    fn signature_changed_props(name: &str, old_base: &str, new_base: &str) -> StructuralChange {
        let props_name = format!("{name}Props");
        StructuralChange {
            symbol: props_name.clone(),
            qualified_name: format!("pkg/src/components/{name}/{name}.{props_name}"),
            kind: SymbolKind::Interface,
            package: Some("@patternfly/react-core".to_string()),
            change_type: StructuralChangeType::Changed(ChangeSubject::Symbol {
                kind: SymbolKind::Interface,
            }),
            before: Some(old_base.to_string()),
            after: Some(new_base.to_string()),
            description: format!("`{props_name}` base class changed from {old_base} to {new_base}"),
            is_breaking: true,
            impact: None,
            migration_target: None,
        }
    }

    /// Helper: build a RenderedComponent source-level change for "stopped rendering".
    fn stopped_rendering(host: &str, component: &str) -> SourceLevelChange {
        SourceLevelChange {
            component: host.to_string(),
            category: SourceLevelCategory::RenderedComponent,
            description: format!("{host} no longer internally renders {component}"),
            old_value: Some(component.to_string()),
            new_value: None,
            has_test_implications: false,
            test_description: None,
            element: None,
            migration_from: None,
            dependency_chain: None,
        }
    }

    /// Helper: build a RenderedComponent source-level change for "started rendering".
    fn started_rendering(host: &str, component: &str) -> SourceLevelChange {
        SourceLevelChange {
            component: host.to_string(),
            category: SourceLevelCategory::RenderedComponent,
            description: format!("{host} now internally renders {component}"),
            old_value: None,
            new_value: Some(component.to_string()),
            has_test_implications: false,
            test_description: None,
            element: None,
            migration_from: None,
            dependency_chain: None,
        }
    }

    /// Helper: build a non-RenderedComponent source-level change (e.g., CssToken).
    fn css_token_change(host: &str, desc: &str) -> SourceLevelChange {
        SourceLevelChange {
            component: host.to_string(),
            category: SourceLevelCategory::CssToken,
            description: desc.to_string(),
            old_value: None,
            new_value: None,
            has_test_implications: false,
            test_description: None,
            element: None,
            migration_from: None,
            dependency_chain: None,
        }
    }

    fn make_sd(source_level_changes: Vec<SourceLevelChange>) -> SdPipelineResult {
        SdPipelineResult {
            source_level_changes,
            ..Default::default()
        }
    }

    // ── Detection tests ─────────────────────────────────────────────

    #[test]
    fn test_chip_to_label_detected_via_rendering_swap() {
        let structural_changes = vec![
            relocated_component("Chip"),
            relocated_props("Chip"),
            relocated_component("ChipGroup"),
            relocated_props("ChipGroup"),
        ];

        let sd = make_sd(vec![
            stopped_rendering("ToolbarFilter", "Chip"),
            stopped_rendering("ToolbarFilter", "ChipGroup"),
            started_rendering("ToolbarFilter", "Label"),
            started_rendering("ToolbarFilter", "LabelGroup"),
            started_rendering("ToolbarFilter", "Fragment"),
            stopped_rendering("MultiTypeaheadSelect", "Chip"),
            stopped_rendering("MultiTypeaheadSelect", "ChipGroup"),
            started_rendering("MultiTypeaheadSelect", "Label"),
            started_rendering("MultiTypeaheadSelect", "LabelGroup"),
        ]);

        let result = detect_deprecated_replacements(&structural_changes, &sd);

        assert_eq!(
            result.len(),
            2,
            "Should detect Chip→Label and ChipGroup→LabelGroup"
        );

        let chip_repl = result.iter().find(|r| r.old_component == "Chip");
        assert!(chip_repl.is_some(), "Should find Chip replacement");
        let chip_repl = chip_repl.unwrap();
        assert_eq!(chip_repl.new_component, "Label");
        assert_eq!(chip_repl.evidence_hosts.len(), 2);
        assert!(chip_repl
            .evidence_hosts
            .contains(&"ToolbarFilter".to_string()));
        assert!(chip_repl
            .evidence_hosts
            .contains(&"MultiTypeaheadSelect".to_string()));

        let group_repl = result.iter().find(|r| r.old_component == "ChipGroup");
        assert!(group_repl.is_some(), "Should find ChipGroup replacement");
        let group_repl = group_repl.unwrap();
        assert_eq!(group_repl.new_component, "LabelGroup");
        assert_eq!(group_repl.evidence_hosts.len(), 2);
    }

    #[test]
    fn test_modal_not_detected_no_rendering_swap() {
        let structural_changes = vec![
            relocated_component("Modal"),
            relocated_props("Modal"),
            relocated_component("ModalBox"),
            relocated_props("ModalBox"),
            relocated_component("ModalBoxBody"),
            relocated_props("ModalBoxBody"),
        ];

        let sd = make_sd(vec![
            stopped_rendering("ModalContent", "Modal"),
            stopped_rendering("ModalContent", "ModalBox"),
            stopped_rendering("ModalContent", "ModalBoxBody"),
        ]);

        let result = detect_deprecated_replacements(&structural_changes, &sd);
        assert!(
            result.is_empty(),
            "Modal should not be detected — no rendering swap"
        );
    }

    #[test]
    fn test_dual_list_selector_not_detected_no_external_swap() {
        let structural_changes = vec![
            relocated_component("DualListSelector"),
            relocated_component("DualListSelectorPane"),
            relocated_component("DualListSelectorList"),
            relocated_component("DualListSelectorControl"),
        ];

        let sd = make_sd(vec![
            stopped_rendering("DualListSelectorPane", "DualListSelector"),
            stopped_rendering("DualListSelectorPane", "DualListSelectorList"),
            stopped_rendering("DualListSelector", "DualListSelectorControl"),
            stopped_rendering("DualListSelector", "DualListSelectorPane"),
        ]);

        let result = detect_deprecated_replacements(&structural_changes, &sd);
        assert!(
            result.is_empty(),
            "DualListSelector should not be detected — sub-components are also relocated"
        );
    }

    #[test]
    fn test_tile_not_detected_no_swap() {
        let structural_changes = vec![relocated_component("Tile"), relocated_props("Tile")];

        let sd = make_sd(vec![css_token_change(
            "Tile",
            "Tile no longer uses CSS token foo",
        )]);

        let result = detect_deprecated_replacements(&structural_changes, &sd);
        assert!(
            result.is_empty(),
            "Tile should not be detected — no rendering swap"
        );
    }

    #[test]
    fn test_fragment_only_swap_not_detected() {
        let structural_changes = vec![relocated_component("SomeComponent")];

        let sd = make_sd(vec![
            stopped_rendering("HostComponent", "SomeComponent"),
            started_rendering("HostComponent", "Fragment"),
            started_rendering("HostComponent", "React.Fragment"),
        ]);

        let result = detect_deprecated_replacements(&structural_changes, &sd);
        assert!(
            result.is_empty(),
            "Fragment-only swaps should not produce a replacement"
        );
    }

    #[test]
    fn test_no_relocations_returns_empty() {
        let structural_changes = vec![StructuralChange {
            symbol: "SomeProps".to_string(),
            qualified_name: "pkg/SomeProps".to_string(),
            kind: SymbolKind::Interface,
            package: None,
            change_type: StructuralChangeType::Changed(ChangeSubject::Symbol {
                kind: SymbolKind::Interface,
            }),
            before: Some("OldType".to_string()),
            after: Some("NewType".to_string()),
            description: "type changed".to_string(),
            is_breaking: true,
            impact: None,
            migration_target: None,
        }];

        let sd = make_sd(vec![
            stopped_rendering("Host", "Foo"),
            started_rendering("Host", "Bar"),
        ]);

        let result = detect_deprecated_replacements(&structural_changes, &sd);
        assert!(result.is_empty());
    }

    #[test]
    fn test_single_host_swap_detected() {
        let structural_changes = vec![relocated_component("OldWidget")];

        let sd = make_sd(vec![
            stopped_rendering("Dashboard", "OldWidget"),
            started_rendering("Dashboard", "NewWidget"),
        ]);

        let result = detect_deprecated_replacements(&structural_changes, &sd);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].old_component, "OldWidget");
        assert_eq!(result[0].new_component, "NewWidget");
        assert_eq!(result[0].evidence_hosts, vec!["Dashboard".to_string()]);
    }

    #[test]
    fn test_props_interface_relocation_not_counted_as_component() {
        let structural_changes = vec![relocated_props("SomeWidget")];

        let sd = make_sd(vec![
            stopped_rendering("Host", "SomeWidget"),
            started_rendering("Host", "NewWidget"),
        ]);

        let result = detect_deprecated_replacements(&structural_changes, &sd);
        assert!(
            result.is_empty(),
            "Props-only relocations should not trigger detection"
        );
    }

    #[test]
    fn test_relocated_component_swapped_for_another_relocated_component_ignored() {
        let structural_changes = vec![relocated_component("OldA"), relocated_component("OldB")];

        let sd = make_sd(vec![
            stopped_rendering("Host", "OldA"),
            started_rendering("Host", "OldB"),
        ]);

        let result = detect_deprecated_replacements(&structural_changes, &sd);
        assert!(
            result.is_empty(),
            "Swapping one relocated component for another should not count"
        );
    }

    #[test]
    fn test_best_candidate_wins_with_most_hosts() {
        let structural_changes = vec![relocated_component("OldComp")];

        let sd = make_sd(vec![
            stopped_rendering("Host1", "OldComp"),
            started_rendering("Host1", "BetterReplacement"),
            started_rendering("Host1", "WeakerCandidate"),
            stopped_rendering("Host2", "OldComp"),
            started_rendering("Host2", "BetterReplacement"),
        ]);

        let result = detect_deprecated_replacements(&structural_changes, &sd);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].new_component, "BetterReplacement");
        assert_eq!(result[0].evidence_hosts.len(), 2);
    }

    // ── Transformation tests ────────────────────────────────────────

    #[test]
    fn test_apply_transforms_relocation_to_changed() {
        let changes = Arc::new(vec![
            relocated_component("Chip"),
            relocated_props("Chip"),
            signature_changed_props("Chip", "React.HTMLProps<HTMLDivElement>", "LabelProps"),
        ]);

        let replacements = vec![DeprecatedReplacement {
            old_component: "Chip".to_string(),
            new_component: "Label".to_string(),
            evidence_hosts: vec!["ToolbarFilter".to_string()],
            evidence_source: ReplacementEvidence::RenderingSwap,
        }];

        let result = apply_deprecated_replacements(changes, &replacements);

        assert_eq!(
            result.len(),
            2,
            "Expected 2 entries (component + props), got {}",
            result.len()
        );

        let comp = &result[0];
        assert_eq!(comp.symbol, "Chip");
        assert!(matches!(comp.change_type, StructuralChangeType::Changed(_)));
        assert_eq!(comp.before.as_deref(), Some("Chip"));
        assert_eq!(comp.after.as_deref(), Some("Label"));
        assert!(comp.description.contains("replaced by `Label`"));

        let props = &result[1];
        assert_eq!(props.symbol, "ChipProps");
        assert!(matches!(
            props.change_type,
            StructuralChangeType::Changed(_)
        ));
        assert_eq!(props.before.as_deref(), Some("ChipProps"));
        assert_eq!(props.after.as_deref(), Some("LabelProps"));
        assert!(props.description.contains("replaced by `LabelProps`"));
    }

    #[test]
    fn test_apply_suppresses_signature_changed_for_replaced_props() {
        let changes = Arc::new(vec![
            StructuralChange {
                symbol: "OtherProps".to_string(),
                qualified_name: "pkg/OtherProps".to_string(),
                kind: SymbolKind::Interface,
                package: None,
                change_type: StructuralChangeType::Changed(ChangeSubject::Symbol {
                    kind: SymbolKind::Interface,
                }),
                before: Some("OldBase".to_string()),
                after: Some("NewBase".to_string()),
                description: "`OtherProps` base class changed from OldBase to NewBase".to_string(),
                is_breaking: true,
                impact: None,
                migration_target: None,
            },
            signature_changed_props("Chip", "React.HTMLProps<HTMLDivElement>", "LabelProps"),
        ]);

        let replacements = vec![DeprecatedReplacement {
            old_component: "Chip".to_string(),
            new_component: "Label".to_string(),
            evidence_hosts: vec!["ToolbarFilter".to_string()],
            evidence_source: ReplacementEvidence::RenderingSwap,
        }];

        let result = apply_deprecated_replacements(changes, &replacements);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].symbol, "OtherProps");
    }

    #[test]
    fn test_apply_no_replacements_returns_unchanged() {
        let original = vec![relocated_component("Modal"), relocated_props("Modal")];
        let changes = Arc::new(original.clone());

        let result = apply_deprecated_replacements(changes, &[]);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].symbol, "Modal");
        assert!(matches!(
            result[0].change_type,
            StructuralChangeType::Relocated { .. }
        ));
    }

    #[test]
    fn test_apply_preserves_non_replaced_relocations() {
        let changes = Arc::new(vec![
            relocated_component("Chip"),
            relocated_component("Modal"),
            relocated_props("Chip"),
            relocated_props("Modal"),
        ]);

        let replacements = vec![DeprecatedReplacement {
            old_component: "Chip".to_string(),
            new_component: "Label".to_string(),
            evidence_hosts: vec!["ToolbarFilter".to_string()],
            evidence_source: ReplacementEvidence::RenderingSwap,
        }];

        let result = apply_deprecated_replacements(changes, &replacements);
        assert_eq!(result.len(), 4);

        let chip = result.iter().find(|s| s.symbol == "Chip").unwrap();
        assert!(matches!(chip.change_type, StructuralChangeType::Changed(_)));

        let modal = result.iter().find(|s| s.symbol == "Modal").unwrap();
        assert!(matches!(
            modal.change_type,
            StructuralChangeType::Relocated { .. }
        ));
    }

    #[test]
    fn test_full_patternfly_scenario() {
        let structural_changes = vec![
            relocated_component("Chip"),
            relocated_props("Chip"),
            relocated_component("ChipGroup"),
            relocated_props("ChipGroup"),
            signature_changed_props("Chip", "React.HTMLProps<HTMLDivElement>", "LabelProps"),
            signature_changed_props(
                "ChipGroup",
                "React.HTMLProps<HTMLUListElement>",
                "Omit<LabelGroupProps, 'ref'>",
            ),
            relocated_component("Modal"),
            relocated_props("Modal"),
            relocated_component("ModalBox"),
            relocated_component("Tile"),
            relocated_props("Tile"),
            relocated_component("DualListSelector"),
            relocated_props("DualListSelector"),
        ];

        let sd = make_sd(vec![
            stopped_rendering("ToolbarFilter", "Chip"),
            stopped_rendering("ToolbarFilter", "ChipGroup"),
            started_rendering("ToolbarFilter", "Label"),
            started_rendering("ToolbarFilter", "LabelGroup"),
            started_rendering("ToolbarFilter", "Fragment"),
            stopped_rendering("MultiTypeaheadSelect", "Chip"),
            stopped_rendering("MultiTypeaheadSelect", "ChipGroup"),
            started_rendering("MultiTypeaheadSelect", "Label"),
            started_rendering("MultiTypeaheadSelect", "LabelGroup"),
            stopped_rendering("ModalContent", "Modal"),
            stopped_rendering("ModalContent", "ModalBox"),
            stopped_rendering("DualListSelectorPane", "DualListSelector"),
        ]);

        let replacements = detect_deprecated_replacements(&structural_changes, &sd);
        assert_eq!(
            replacements.len(),
            2,
            "Only Chip and ChipGroup should be detected"
        );

        let chip = replacements
            .iter()
            .find(|r| r.old_component == "Chip")
            .unwrap();
        assert_eq!(chip.new_component, "Label");

        let group = replacements
            .iter()
            .find(|r| r.old_component == "ChipGroup")
            .unwrap();
        assert_eq!(group.new_component, "LabelGroup");

        let changes = Arc::new(structural_changes);
        let result = apply_deprecated_replacements(changes, &replacements);
        assert_eq!(
            result.len(),
            11,
            "Expected 11 entries (4 Changed + 7 Relocated), got {}",
            result.len()
        );

        let chip_entries: Vec<_> = result
            .iter()
            .filter(|s| s.symbol == "Chip" || s.symbol == "ChipProps")
            .collect();
        assert_eq!(chip_entries.len(), 2);
        for entry in &chip_entries {
            assert!(
                matches!(entry.change_type, StructuralChangeType::Changed(_)),
                "{} should be Changed",
                entry.symbol
            );
        }

        let modal_entries: Vec<_> = result
            .iter()
            .filter(|s| s.symbol == "Modal" || s.symbol == "ModalProps")
            .collect();
        assert_eq!(modal_entries.len(), 2);
        for entry in &modal_entries {
            assert!(
                matches!(entry.change_type, StructuralChangeType::Relocated { .. }),
                "{} should remain Relocated",
                entry.symbol
            );
        }

        let sig_changed: Vec<_> = result
            .iter()
            .filter(|s| {
                (s.symbol == "ChipProps" || s.symbol == "ChipGroupProps")
                    && s.description.contains("base class changed")
            })
            .collect();
        assert!(
            sig_changed.is_empty(),
            "Signature-changed entries for replaced Props should be suppressed"
        );
    }

    // ── Commit co-change detection tests ────────────────────────────

    #[test]
    fn test_commit_cochange_skips_already_detected() {
        // If all relocated components are already detected via rendering swap,
        // the commit co-change analysis should return empty.
        let structural_changes = vec![relocated_component("Chip"), relocated_component("Tile")];

        let already_detected: HashSet<&str> = ["Chip", "Tile"].iter().copied().collect();

        let result = detect_deprecated_replacements_from_commits(
            std::path::Path::new("/nonexistent"),
            "v5",
            "v6",
            &structural_changes,
            &already_detected,
        );

        assert!(
            result.is_empty(),
            "Should skip all components already detected"
        );
    }

    #[test]
    fn test_commit_cochange_empty_when_no_relocations() {
        // No relocated components → should return empty immediately
        let structural_changes = vec![StructuralChange {
            symbol: "SomeProps".to_string(),
            qualified_name: "pkg/SomeProps".to_string(),
            kind: SymbolKind::Interface,
            package: None,
            change_type: StructuralChangeType::Changed(ChangeSubject::Symbol {
                kind: SymbolKind::Interface,
            }),
            before: Some("OldType".to_string()),
            after: Some("NewType".to_string()),
            description: "type changed".to_string(),
            is_breaking: true,
            impact: None,
            migration_target: None,
        }];

        let already_detected: HashSet<&str> = HashSet::new();

        let result = detect_deprecated_replacements_from_commits(
            std::path::Path::new("/nonexistent"),
            "v5",
            "v6",
            &structural_changes,
            &already_detected,
        );

        assert!(result.is_empty());
    }

    #[test]
    fn test_rendering_swap_sets_evidence_source() {
        let structural_changes = vec![relocated_component("OldWidget")];

        let sd = make_sd(vec![
            stopped_rendering("Dashboard", "OldWidget"),
            started_rendering("Dashboard", "NewWidget"),
        ]);

        let result = detect_deprecated_replacements(&structural_changes, &sd);
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].evidence_source,
            ReplacementEvidence::RenderingSwap
        );
    }

    /// Integration test: verify Tile → Card detection via commit co-change
    /// using the real PatternFly React repository.
    ///
    /// Requires the PF repo at `/tmp/semver-pipeline-v2/repos/patternfly-react`.
    #[test]
    #[ignore]
    fn test_tile_detected_via_commit_cochange_integration() {
        let repo = std::path::Path::new("/tmp/semver-pipeline-v2/repos/patternfly-react");
        if !repo.exists() {
            eprintln!("Skipping: PF repo not found at {}", repo.display());
            return;
        }

        let structural_changes = vec![
            relocated_component("Tile"),
            relocated_props("Tile"),
            // Also include Chip (will be in already_detected)
            relocated_component("Chip"),
            relocated_props("Chip"),
        ];

        // Simulate Chip already detected by rendering swap
        let already_detected: HashSet<&str> = ["Chip"].iter().copied().collect();

        let result = detect_deprecated_replacements_from_commits(
            repo,
            "v5.4.0",
            "v6.4.1",
            &structural_changes,
            &already_detected,
        );

        // Tile should be detected via commit co-change → Card
        let tile_repl = result.iter().find(|r| r.old_component == "Tile");
        assert!(
            tile_repl.is_some(),
            "Tile should be detected via commit co-change. Got: {:?}",
            result
        );
        let tile_repl = tile_repl.unwrap();
        assert_eq!(tile_repl.new_component, "Card");
        assert_eq!(
            tile_repl.evidence_source,
            ReplacementEvidence::CommitCoChange
        );
        assert!(
            !tile_repl.evidence_hosts.is_empty(),
            "Should have at least one commit SHA as evidence"
        );

        // Chip should NOT appear (already detected)
        let chip_repl = result.iter().find(|r| r.old_component == "Chip");
        assert!(
            chip_repl.is_none(),
            "Chip should be skipped — already detected via rendering swap"
        );
    }
}
