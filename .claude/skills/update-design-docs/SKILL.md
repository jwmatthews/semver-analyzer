---
name: update-design-docs
description: Use when code changes have landed and design docs or onboarding docs may be stale - performs a deep dive into recent code changes and updates design/ and docs/ to match current reality
---

# Update Design Docs

Analyze code changes, compare against existing design documentation, and update docs so they accurately reflect the current codebase.

## Modes

This skill has two modes:

| Mode | When to use | What it does |
|------|-------------|-------------|
| **Update** (default) | After code changes land | Full 7-phase process: identify changes, audit affected docs, update, verify |
| **Audit** | Periodic accuracy check, pre-release validation, adversarial review | Run Deep Verify (Phase 3) + Adversarial Self-Check (Phase 7) across ALL docs. Produce structured report. Make zero edits. |

In **audit mode**, skip Phases 1, 2, 4, 5, 6. Run Phase 3 against all design docs (not just changed ones), then run Phase 7 Parts A and B.

## When to Use

- After a feature branch is merged or a batch of commits lands
- Before cutting a release, to ensure docs are current
- When you notice design docs referencing stale types, file paths, or algorithms
- Periodically as a maintenance sweep
- When asked to "audit" or "verify" documentation accuracy

## Overview

This project has two documentation tiers:

| Directory | Audience | Style |
|-----------|----------|-------|
| `design/` | AI agents | Precise: file maps, type signatures, algorithm details, invariants |
| `docs/` | Human engineers | Narrative: mental models, "when you'll touch it", worked examples |

Both must stay in sync with code. `CLAUDE.md` and `AGENTS.md` are the top-level entry points and also need updating if new docs are added or the doc index changes.

## Process

### Phase 1: Identify What Changed (update mode only)

Determine the scope of code changes to analyze:

```bash
# Changes since a specific ref (e.g., last docs update, last release)
git diff --stat <base-ref>..HEAD -- 'src/' 'crates/'

# Changes in last N commits
git log --oneline -20 --stat -- 'src/' 'crates/'

# If no ref is obvious, diff against main
git diff --stat main..HEAD -- 'src/' 'crates/'
```

Build a change map: which crates and which files were touched.

### Phase 2: Map Changes to Docs (update mode only)

Each crate/area maps to specific design docs:

| Changed area | Design doc | Onboarding section |
|-------------|------------|-------------------|
| `crates/core/src/types/` | `design/data-types.md` | "Key Concepts" |
| `crates/core/src/traits.rs` | `design/01-traits.md`, `design/architecture.md`, `design/core-crate.md` | "Architecture at a Glance" |
| `crates/core/src/diff/` | `design/core-crate.md` (Diff Engine section) | "Key Concepts > Rename Detection" |
| `crates/ts/src/extract/` | `design/ts-crate.md` (Extraction section) | "Where to Start Reading Code" |
| `crates/ts/src/source_profile/` | `design/ts-crate.md` (Source Profile section) | "SD Pipeline" |
| `crates/ts/src/composition/` | `design/ts-crate.md` (Composition section), `design/05-composition-tree-v2.md`, `design/composition-ground-truth.md` | "Composition Trees" |
| `crates/ts/src/sd_pipeline.rs` | `design/pipelines.md`, `design/ts-crate.md` | "SD Pipeline" |
| `crates/ts/src/konveyor*.rs` | `design/ts-crate.md` (Konveyor section) | N/A |
| `crates/java/` | `design/java-crate.md` | "Languages Supported" |
| `crates/llm/` | `design/llm-crate.md` | "BU Pipeline" |
| `crates/konveyor-core/` | `design/architecture.md` | N/A |
| `src/orchestrator.rs` | `design/architecture.md` (Orchestrator section), `design/pipelines.md` | "Architecture at a Glance" |
| `src/cli/` or `src/main.rs` | `design/architecture.md` (Configuration section) | "Development Workflow" |
| Test files (`**/tests.rs`, `tests/`) | `design/testing.md` | N/A |
| New feature patterns | `design/adding-features.md` | N/A |

If a change doesn't map to any doc, it may not need doc updates -- but check if it introduces a new module, type, or algorithm that should be documented.

### Phase 3: Deep Verify (both modes)

For each affected design doc (update mode) or ALL design docs (audit mode), run these 10 verification checks. Each check has a concrete command, a pass/fail criterion, and the docs most likely to fail it.

#### Check 1: Generic Parameters

Types that are generic in code must appear with their generic parameter in docs (at least in code blocks and type signature contexts).

```bash
# Find generic types in core
grep -n 'pub struct\|pub enum\|pub trait' crates/core/src/types/surface.rs crates/core/src/types/report.rs crates/core/src/types/envelope.rs crates/core/src/traits.rs | grep '<'

# For each, verify docs use the generic form in code contexts
for type in "ApiSurface" "Symbol" "LanguageSemantics" "AnalysisReport" "TypeSummary" "PackageChanges" "BehavioralChange" "ManifestChange" "LanguageReport" "LanguageBehavioralChange" "LanguageManifestChange"; do
  code_form=$(grep -m1 "pub struct $type\|pub enum $type\|pub trait $type" crates/core/src/types/*.rs crates/core/src/traits.rs 2>/dev/null)
  if echo "$code_form" | grep -q '<'; then
    for doc in design/*.md; do
      bare=$(grep -cP "\b${type}\b" "$doc" 2>/dev/null || echo 0)
      generic=$(grep -cP "${type}<" "$doc" 2>/dev/null || echo 0)
      delta=$((bare - generic))
      if [ "$delta" -gt 0 ]; then
        echo "WARN: $doc uses bare '$type' ~$delta times — verify these are in prose context, not code blocks"
      fi
    done
  fi
done
```

**Pass criterion**: No bare generic types appear inside code blocks or type signature tables. Bare usage in prose headings is acceptable.
**Most likely to fail**: `01-traits.md`, `02-types.md`, `04-language-implementation-guide.md`

#### Check 2: Associated Type Count on Language Trait

```bash
# Count associated types in the Language trait
awk '/^pub trait Language/,/^}/' crates/core/src/traits.rs | grep -c '^\s*type [A-Z]'

# Check what docs claim
grep -n 'associated type' design/01-traits.md design/04-language-implementation-guide.md design/architecture.md 2>/dev/null
```

**Pass criterion**: Doc count matches code count.
**Most likely to fail**: `01-traits.md`, `04-language-implementation-guide.md`

#### Check 3: Phantom Types/Traits

Every trait, struct, or enum name referenced in docs as existing in the codebase must actually exist.

```bash
# Check commonly-phantom names
for name in "ApiExtractor" "DiffParser" "CallGraphBuilder" "TestAnalyzer" "ComponentSummary" "PropertySummary"; do
  if ! grep -rq "pub trait $name\|pub struct $name\|pub enum $name" crates/ src/; then
    hits=$(grep -rl "$name" design/*.md 2>/dev/null)
    if [ -n "$hits" ]; then
      echo "PHANTOM: '$name' referenced in docs but not defined in code. Docs: $hits"
    fi
  fi
done
```

**Pass criterion**: Zero phantom references found.
**Most likely to fail**: `01-traits.md`, `04-language-implementation-guide.md`

#### Check 4: Const vs Method (Language::NAME)

```bash
# Check whether NAME is const or fn in the Language trait
grep -n 'fn name\|const NAME' crates/core/src/traits.rs | head -5

# Check docs
grep -rn 'fn name()\|L::name()' design/*.md
```

**Pass criterion**: If code uses `const NAME`, no doc should show `fn name()` or `L::name()`.
**Most likely to fail**: `01-traits.md`, `04-language-implementation-guide.md`

#### Check 5: Field Types (Option Wrapping)

Key structs have fields that are `Option<T>` in code but sometimes documented as bare `T`.

```bash
# Extract actual field types for high-drift structs
for struct in "BehavioralChange" "ChangedFunction" "StructuralChange" "MigrationTarget"; do
  echo "=== $struct ==="
  grep -A 50 "pub struct $struct" crates/core/src/types/*.rs 2>/dev/null | grep 'pub ' | head -20
done
```

**Pass criterion**: Every field shown in docs matches the exact type in code, including `Option<>` wrapping.
**Most likely to fail**: `02-types.md`, `03-report-envelope.md`

#### Check 6: EdgeStrength Signal Assignments

The composition tree signal table must match the actual `EdgeStrength::` variant used per step.

```bash
# Extract actual EdgeStrength assignments per step in the composition builder
grep -n 'EdgeStrength::' crates/ts/src/composition/mod.rs | head -40
```

Cross-reference each step's comment (e.g., "Step 1", "Step 2") with the EdgeStrength variant used in the adjacent code.

**Pass criterion**: Signal table in each doc matches code assignments exactly.
**Most likely to fail**: `05-composition-tree-v2.md`, `agent-guide.md`

#### Check 7: Collapse Algorithm

```bash
# Show the actual collapse logic
grep -A 15 'pub fn collapse_chain' crates/ts/src/sd_types.rs
grep -B2 -A 20 'fn collapse_internal_nodes\|Wrapper =>\|Allowed =>' crates/ts/src/sd_pipeline.rs | head -40
```

**Pass criterion**: Docs describe the three-branch logic (Wrapper passthrough, Allowed passthrough, AND-based `collapse_chain()`) — not "take the stronger."
**Most likely to fail**: `05-composition-tree-v2.md`, `agent-guide.md`

#### Check 8: Test Counts

```bash
# Count #[test] functions per test file
for file in crates/ts/tests/baseline_diff.rs crates/ts/tests/baseline_manifest.rs crates/ts/tests/baseline_behavioral.rs crates/ts/tests/baseline_migration.rs crates/java/tests/baseline_diff.rs crates/java/tests/baseline_konveyor.rs crates/java/tests/baseline_manifest.rs crates/java/tests/baseline_sd.rs crates/konveyor-core/tests/token_rename_pipeline.rs; do
  count=$(grep -c '#\[test\]' "$file" 2>/dev/null || echo "FILE NOT FOUND")
  echo "$file: $count"
done
```

**Pass criterion**: Every test count in `testing.md` matches the actual `#[test]` count.
**Most likely to fail**: `testing.md`

#### Check 9: Deleted Function References

```bash
# Extract function names referenced in docs and check they exist in code
grep -ohP '`[a-z_]+\(\)`' design/*.md | sort -u | while read -r func; do
  bare=$(echo "$func" | tr -d '`()')
  if [ ${#bare} -gt 3 ] && ! grep -rq "fn $bare" crates/ src/; then
    echo "DELETED: $func — not found as 'fn $bare' in code"
    grep -rn "$func" design/*.md | head -3
  fi
done
```

**Pass criterion**: Zero deleted function references.
**Most likely to fail**: `agent-guide.md`

#### Check 10: Method/Variant/Step Counts

```bash
# Count items in key code structures and compare against doc claims
echo "LanguageSemantics methods:"
awk '/^pub trait LanguageSemantics/,/^}/' crates/core/src/traits.rs | grep -c 'fn '

echo "Language trait methods:"
awk '/^pub trait Language:/,/^}/' crates/core/src/traits.rs | grep -c 'fn '

echo "Language trait associated types:"
awk '/^pub trait Language:/,/^}/' crates/core/src/traits.rs | grep -c '^\s*type '

echo "SymbolKind variants:"
awk '/pub enum SymbolKind/,/^}/' crates/core/src/types/surface.rs | grep -c '^\s*[A-Z]'

echo "ChangeSubject variants:"
awk '/pub enum ChangeSubject/,/^}/' crates/core/src/types/change_subject.rs | grep -c '^\s*[A-Z]'

echo "EdgeStrength variants:"
awk '/pub enum EdgeStrength/,/^}/' crates/ts/src/sd_types.rs | grep -c '^\s*[A-Z]'

echo "Composition builder steps:"
grep -c '── Step' crates/ts/src/composition/mod.rs

echo "LLM parsers:"
grep -c 'pub fn parse_' crates/llm/src/invoke.rs

echo "JavaSourceCategory variants:"
awk '/pub enum JavaSourceCategory/,/^}/' crates/java/src/sd_types.rs | grep -c '^\s*[A-Z]'

echo "SourceLevelCategory variants:"
awk '/pub enum SourceLevelCategory/,/^}/' crates/ts/src/sd_types.rs | grep -c '^\s*[A-Z]'
```

**Pass criterion**: Every count claimed in any doc matches the actual code count.
**Most likely to fail**: `architecture.md`, `pipelines.md`, `llm-crate.md`, `testing.md`

### Phase 4: Update Design Docs (update mode only)

For each doc that needs changes:

1. **Read the full current doc** to understand its structure
2. **Read the relevant source files** to understand current reality
3. **Edit surgically** -- update only what changed; preserve the doc's existing style and structure
4. **Preserve invariants sections** -- if an invariant changed, flag it prominently since CLAUDE.md may reference it

Design doc style rules:
- Use file maps with one-line descriptions
- Include actual type signatures for key types (abbreviated if long)
- State algorithm steps with numbers and concrete threshold values
- Tables for structured information (variants, phases, signals)
- No narrative -- just facts an agent needs

### Phase 5: Update Onboarding Doc (update mode only)

If changes affect concepts covered in `docs/onboarding.md`:

1. **Read the current onboarding doc**
2. **Update affected sections** -- keep the narrative, conversational style
3. **Update the "Where to Start Reading Code" table** if file paths changed
4. **Update "Known Limitations"** if any were fixed or new ones discovered

Onboarding doc style rules:
- Explain "what" and "why", not implementation details
- Use analogies and comparisons ("think of it as...")
- Keep code blocks to CLI usage and architecture diagrams
- Write for someone who hasn't seen the codebase

### Phase 6: Update Entry Points (update mode only)

If new design docs were added or the doc index changed:

1. Update the "Deep Dive References" tables in both `CLAUDE.md` and `AGENTS.md`
2. Ensure every design doc is listed with a one-line purpose

### Phase 7: Adversarial Self-Check (both modes)

#### Part A: Automated Re-Verification

Re-run ALL 10 checks from Phase 3 on the updated (or audited) docs. Any failure must be resolved before the skill completes.

```bash
# Quick re-run of critical checks
# 1. File references still valid
grep -oP '`[^`]*\.rs`' design/*.md | while IFS=: read -r doc ref; do
  path=$(echo "$ref" | tr -d '`')
  if [ ! -f "$path" ] && ! find . -path "*/$path" -print -quit 2>/dev/null | grep -q .; then
    echo "BROKEN REF in $doc: $path"
  fi
done

# 2. No phantom traits
for name in "ApiExtractor" "DiffParser" "CallGraphBuilder" "TestAnalyzer"; do
  if grep -rlq "$name" design/*.md 2>/dev/null; then
    echo "PHANTOM TRAIT still referenced: $name"
  fi
done

# 3. No fn name() references (should be const NAME)
if grep -rq 'fn name()' design/*.md; then
  echo "STALE: 'fn name()' found — should be 'const NAME'"
fi
```

#### Part B: Cross-Doc Consistency

Verify that facts stated in multiple docs agree with each other:

```bash
# Find claims about method counts across docs
grep -rn 'methods.*LanguageSemantics\|LanguageSemantics.*method' design/*.md

# Find claims about associated type counts
grep -rn 'associated type\|6 associated\|4 associated' design/*.md

# Find EdgeStrength-related claims
grep -rn 'EdgeStrength.*variant\|4 variant.*EdgeStrength' design/*.md

# Find composition step counts
grep -rn 'signal step\|Step 10\|~19\|~20.*step\|10.*step' design/*.md
```

**Pass criterion**: If two docs make the same factual claim, they must agree. Contradictions are failures.

## Audit Mode Report Format

When running in audit mode, produce this structured report:

```markdown
## Audit Report — [date]

### Summary
| Check | Pass | Fail | Docs Affected |
|-------|------|------|--------------|
| 1. Generic parameters | ... | ... | ... |
| 2. Associated type count | ... | ... | ... |
| 3. Phantom types/traits | ... | ... | ... |
| 4. Const vs fn | ... | ... | ... |
| 5. Field types | ... | ... | ... |
| 6. EdgeStrength assignments | ... | ... | ... |
| 7. Collapse algorithm | ... | ... | ... |
| 8. Test counts | ... | ... | ... |
| 9. Deleted functions | ... | ... | ... |
| 10. Method/variant counts | ... | ... | ... |

### Failures
#### Check N: [name]
- `design/file.md` line X: [what's wrong] — code shows [what's right] at `file.rs:line`
...

### Cross-Doc Consistency Issues
- [fact]: `doc1.md` says X, `doc2.md` says Y, code says Z
...
```

## Dispatching with Parallel Agents

For large change sets spanning multiple crates, dispatch parallel agents -- one per design doc:

```
Agent 1: Audit + update design/core-crate.md against crates/core/
Agent 2: Audit + update design/ts-crate.md against crates/ts/
Agent 3: Audit + update design/java-crate.md against crates/java/
Agent 4: Audit + update design/architecture.md against src/ and cross-crate changes
Agent 5: Audit + update docs/onboarding.md (run after agents 1-4 complete)
```

Each agent should:
1. Read the design doc
2. Read the relevant source files
3. Run the applicable Phase 3 checks
4. Make targeted edits (update mode) or report findings (audit mode)

## Drift-Resistant Patterns

These facts drift fastest and must be re-verified on every audit:

| Drift-Prone Fact | How to Verify | Why It Drifts |
|-----------------|---------------|---------------|
| Test counts | `grep -c '#\[test\]' file.rs` | New tests added without doc update |
| Method/variant counts | `awk` + `grep -c` on trait/enum blocks | Methods/variants added incrementally |
| SD pipeline phase ordering | Read the main function in `sd_pipeline.rs` top-to-bottom | Phases inserted between existing phases |
| Generic parameters | grep for bare type names in docs | Easy to omit `<M>` when writing prose |
| EdgeStrength per step | grep `EdgeStrength::` near step comments | Strengths change during composition algorithm tuning |
| Collapse algorithm | Read `collapse_chain()` and its callers | Logic refined as edge cases are discovered |
| Function existence | grep `fn funcname` in crates/ | Functions renamed or inlined during refactors |

## What NOT to Document

- Git history or changelog entries (that's what `git log` is for)
- Temporary debugging code or feature flags
- Test fixture data (just reference the fixture directory)
- Per-PR context ("added for issue #X") -- these rot as the codebase evolves
