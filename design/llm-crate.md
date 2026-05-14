# LLM Crate (`crates/llm/`)

Language-agnostic LLM behavioral analysis. Shells out to an external CLI command (default: goose) and parses structured JSON responses.

## File Map

```
crates/llm/src/
  lib.rs            LlmBehaviorAnalyzer struct + BehaviorAnalyzer trait impl
  invoke.rs         CLI execution + response parsing (6 parsers)
  prompts.rs        Prompt builders (7 prompts + FunctionSpec JSON schema)
  spec_compare.rs   Tier 1 structural spec comparison (no LLM needed)
```

## LlmBehaviorAnalyzer

```rust
pub struct LlmBehaviorAnalyzer {
    llm_command: String,    // e.g., "goose run --no-session -q -t"
    timeout_secs: u64,      // default 120
}
```

### Methods

| Method | Purpose | Used By |
|--------|---------|---------|
| `infer_spec()` | Infer behavioral spec from function body | BU pipeline |
| `infer_spec_with_test_context()` | Infer spec with test assertion context | BU pipeline |
| `specs_are_breaking()` | Compare two specs for breaking changes | BU pipeline |
| `check_propagation()` | Check if breaking change propagates through caller | BU pipeline |
| `analyze_file_diff()` | Analyze entire file diff for behavioral + API changes | BU Phase 2 |
| `infer_constant_renames()` | Detect constant rename patterns from examples | Orchestrator |
| `infer_hierarchy_from_prompt()` | Infer component hierarchy from source | Orchestrator |
| `infer_suffix_renames_from_prompt()` | Detect suffix rename patterns | Orchestrator |
| `infer_interface_renames()` | Detect interface rename mappings | Orchestrator |

## CLI Contract

The LLM command receives the prompt as the last argument:
```
<llm_command> "<prompt>"
```

Expected behavior:
- Prompt is the final CLI argument
- Response on stdout as JSON
- Exit code 0 on success
- Timeout handling (default 120s)

Example with goose: `goose run --no-session -q -t "What are the breaking changes in this diff?"`

## FunctionSpec (Core Data Model)

```json
{
  "preconditions": [{ "parameter": "x", "condition": "must be positive", "on_violation": "throws Error" }],
  "postconditions": [{ "condition": "returns sorted array", "returns": "number[]" }],
  "error_behavior": [{ "trigger": "empty input", "error_type": "Error", "message_pattern": "cannot be empty" }],
  "side_effects": [{ "target": "database", "action": "writes record", "condition": "on success" }],
  "notes": ["thread-safe", "idempotent"]
}
```

## Two-Tier Breaking Change Detection

### Tier 1: Structural Comparison (`spec_compare.rs`)

No LLM needed. Compares two `FunctionSpec` instances using normalized string matching on identity fields:

| Aspect | Breaking If |
|--------|------------|
| Preconditions | New precondition added (tightened contract) |
| Postconditions | Postcondition removed (weakened guarantee) |
| Postconditions | Return type changed |
| Error behavior | Error type changed |
| Error behavior | New error case added |
| Side effects | Side effect removed |
| Side effects | Action changed |

Matching strategy: lowercase + trim + collapse whitespace. Match preconditions by `parameter`, postconditions by `condition`, errors by `trigger`, side effects by `target+action`.

Returns confidence 0.80 for breaking, 0.0 for not. If structural comparison finds no breaking changes, falls through to Tier 2.

### Tier 2: LLM Comparison

Only invoked when Tier 1 finds no issues but the specs have differing `notes`. Asks the LLM to compare the two specs directly.

## Response Parsing (`invoke.rs`)

`extract_json()` uses 3 strategies in order:
1. Fenced JSON block (` ```json ... ``` `)
2. Largest valid JSON object via regex
3. Brace-matching with string-literal awareness

`resolve_goose_overflow()` handles goose CLI's truncation: when stdout contains `... (N more lines -> /path)`, reads the overflow file.

## Prompt Construction (`prompts.rs`)

| Prompt | Purpose | Input |
|--------|---------|-------|
| `build_spec_inference_prompt()` | Infer behavioral spec from function body | function source code |
| `build_spec_inference_with_test_prompt()` | Infer spec with test context | function + test assertions |
| `build_spec_comparison_prompt()` | Compare two specs for breaking changes | old spec + new spec |
| `build_propagation_check_prompt()` | Check if break propagates through caller | caller source + break description |
| `build_file_behavioral_prompt()` | Analyze file diff (main production prompt) | file diff + dynamic categories |
| `build_constant_rename_prompt()` | Detect rename patterns from examples | removed + added constant lists |
| `build_interface_rename_prompt()` | Detect interface renames | removed + added interface lists |

The `build_file_behavioral_prompt()` is the most complex: truncates large diffs (>15K chars), builds dynamic category sections from `LlmCategoryDefinition`, and produces a single JSON response with both behavioral and API-level changes.

## Cost Considerations

LLM analysis is only used with `--behavioral` flag. The SD pipeline (default) is fully deterministic with zero LLM cost.

When using `--behavioral`:
- Each changed function gets 2-3 LLM calls (spec inference + comparison + optional propagation)
- File-level analysis adds 1 call per changed file
- Rename/hierarchy inference adds a handful of calls
- Concurrency capped at `LLM_CONCURRENCY = 5`
- Circuit breaker triggers after excessive failures

Estimated cost per analysis (from PLAN.md): ~$2-10 USD depending on repository size and provider.
