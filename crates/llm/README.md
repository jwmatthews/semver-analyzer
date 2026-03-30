# semver-analyzer-llm

LLM-based behavioral analysis for the semver-analyzer. Implements the `BehaviorAnalyzer` trait from `semver-analyzer-core` to detect breaking behavioral changes that static analysis cannot catch -- changes in what functions *do*, not just their type signatures.

The crate is **agent-agnostic**: it shells out to any external LLM CLI tool (e.g., `goose`, `opencode`) via a configurable command string. It does not embed or link against any LLM SDK.

## Architecture

The system has four pillars:

1. **Agent-agnostic invocation** -- any CLI that accepts a prompt as its final argument
2. **Template-constrained prompts** -- structured JSON schemas reduce hallucination
3. **Tier 1 structural comparison** -- deterministic spec comparison without LLM
4. **Tier 2 LLM fallback** -- for ambiguous cases that structural comparison cannot resolve

## Usage

```rust
use semver_analyzer_llm::LlmBehaviorAnalyzer;
use semver_analyzer_core::BehaviorAnalyzer;

let analyzer = LlmBehaviorAnalyzer::new("goose run --no-session -q -t");

// Infer a function's behavioral spec
let spec = analyzer.infer_spec(&function_body, &signature)?;

// Infer with test context (higher confidence)
let spec = analyzer.infer_spec_with_test_context(&body, &sig, &test_diff)?;

// Compare two specs for breaking changes
let verdict = analyzer.specs_are_breaking(&old_spec, &new_spec)?;

// Check if a break propagates through a caller
let propagates = analyzer.check_propagation(&caller_body, &caller_sig, "callee", &evidence)?;
```

## How Invocation Works

1. The `llm_command` string (e.g., `"goose run --no-session -q -t"`) is split on whitespace
2. The prompt is appended as the final argument
3. The command is executed as a subprocess with a configurable timeout (default 120s)
4. JSON is extracted from the response using three fallback strategies:
   - Fenced JSON blocks (`` ```json ... ``` ``)
   - Largest valid JSON object found via regex
   - Manual brace-matching parser

## Spec Comparison (Tier 1)

The deterministic structural comparison runs first, avoiding an LLM call when possible:

| Spec Field | Breaking If... | Not Breaking If... |
|------------|----------------|-------------------|
| Preconditions | New precondition added; condition tightened | Precondition removed (more permissive) |
| Postconditions | Postcondition removed; return value changed | New postcondition added |
| Error behavior | Error type changed; new error case added | Error case removed |
| Side effects | Side effect removed; action changed | New side effect added |

If Tier 1 detects a break (confidence >= 0.80), it returns immediately. Otherwise, if both specs have non-empty `notes` fields, it falls through to Tier 2 LLM comparison.

## Additional Analysis Capabilities

Beyond the core `BehaviorAnalyzer` trait, the analyzer provides:

| Method | Purpose |
|--------|---------|
| `analyze_file_diff` | File-level behavioral + API analysis (1 LLM call per file) |
| `analyze_composition_patterns` | Detect JSX nesting structure changes |
| `infer_constant_renames` | Identify regex-based constant rename patterns |
| `infer_interface_renames` | Map removed interfaces to their replacements |
| `infer_component_hierarchy` | Infer parent-child composition hierarchy |
| `infer_suffix_renames` | Identify CSS physical-to-logical property suffix renames |

## Prompt System

All prompts produce structured JSON output matching exact schemas. Key prompts:

- **Spec inference** -- forces `FunctionSpec` JSON (preconditions, postconditions, error_behavior, side_effects)
- **Spec inference with tests** -- grounds the spec with test assertion diffs as truth
- **File behavioral analysis** -- categorizes changes into 8 behavioral + 6 API change types
- **Propagation check** -- determines if a callee's break propagates through a caller
- **Constant/interface rename inference** -- identifies systematic rename patterns
- **Hierarchy inference** -- infers component parent-child composition from source

## Dependencies

| Crate | Purpose |
|-------|---------|
| `semver-analyzer-core` | Core traits (`BehaviorAnalyzer`) and types (`FunctionSpec`, `BreakingVerdict`, etc.) |
| `serde`, `serde_json` | JSON serialization for prompts and response parsing |
| `anyhow` | Error handling |
| `regex` | JSON extraction from free-text LLM output |
| `tracing` | Structured logging |

## License

Apache-2.0
