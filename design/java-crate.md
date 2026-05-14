# Java Crate (`crates/java/`)

Implements `Language` for Java. Behind the `java` cargo feature flag.

## File Map

```
crates/java/src/
  lib.rs                  Re-exports
  language.rs             Language trait impl for Java
  cli.rs                  Java-specific CLI args
  types.rs                JavaSymbolData, JavaAnnotation, JavaCategory, etc.
  extensions.rs           JavaAnalysisExtensions wrapper
  sd_types.rs             JavaClassProfile, JavaSourceChange, 22 categories
  sd_pipeline.rs          Java source-level diff pipeline
  report.rs               build_report() for Java
  extract/
    mod.rs                Tree-sitter based .java extraction
    modifiers.rs          Java modifier extraction
    module_info.rs        module-info.java parsing
  manifest/
    mod.rs                pom.xml and build.gradle diffing
    pom.rs                XML event-driven pom.xml parser
  index/
    mod.rs                Cross-file call graph (JavaIndex)
  konveyor.rs             Java Konveyor rule generation + namespace migrations
  worktree/
    mod.rs, guard.rs, error.rs  Git worktree + Maven/Gradle build
  test_analyzer/
    mod.rs                JUnit/TestNG/AssertJ test discovery + assertion diff
  diff_parser/
    mod.rs                Git diff -> ChangedFunction (with overload disambiguation)
```

## Language-Specific Semantics

### `is_member_addition_breaking()`
- Adding abstract methods to interfaces or abstract classes: **breaking**
- Adding default methods to interfaces: **not breaking**
- Adding concrete methods to classes: **not breaking**

### `same_family()`
Same Java package (e.g., `com.example.auth.UserService` and `com.example.auth.AuthProvider` are same family).

### `visibility_rank()`
`Private(0) < Internal/package-private(1) < Protected(2) < Public(3)`

### `diff_language_data()`
Detects changes in Java-specific modifiers and metadata:
- Annotation add/remove/attribute changes
- `throws` clause changes
- `final`, `sealed`, `non-sealed` modifier changes
- `synchronized`, `transient`, `volatile`, `native` modifier changes
- `permits` clause changes

Annotation removal breaking rules: `@Bean`, `@Service`, `@Component`, `@Repository`, `@Controller`, `@RestController`, `@Autowired`, `@Inject`, `@Override`, `@FunctionalInterface`, `@Deprecated`, `@Nullable`, `@NonNull`, `@NotNull` are considered breaking when removed.

## JavaSymbolData

Per-symbol metadata:
```
JavaSymbolData
  annotations: Vec<JavaAnnotation>     // @Bean, @Override, etc.
  throws: Vec<String>                  // checked exceptions
  is_record: bool
  is_annotation_type: bool
  is_default: bool                     // default interface method
  is_sealed: bool
  permits: Vec<String>
  is_final: bool
  is_non_sealed: bool
  is_synchronized: bool
  is_transient: bool
  is_volatile: bool
  is_native: bool
```

## Extraction (`extract/mod.rs`)

Uses tree-sitter for Java source parsing (not compiled output like TS).

### Process
1. Recursively find `.java` files (skips `target/`, `build/`, `test/`, `generated/`)
2. For each file: parse with tree-sitter, extract package declaration and imports
3. Walk top-level type declarations: classes, interfaces, enums, records, annotation types
4. Extract members: methods, constructors, fields, annotation elements, nested types
5. Special handling for records: synthesizes canonical constructor + accessor methods from record components
6. Enum constants: captures constructor arguments
7. `module-info.java`: extracts exports, requires, opens, provides, uses directives

### Import Resolution
`ImportMap` tracks exact imports (`String` -> `java.lang.String`) and wildcard prefixes (`java.util.*`). Used for annotation qualified name resolution.

## SD Pipeline (`sd_pipeline.rs`)

### Phases

**Phase A**: Changed file detection and profile diffing
- `git diff --name-only` for changed `.java` files
- Extract `JavaClassProfile` at both refs
- `diff_class_profiles()` compares: annotations, final/sealed/abstract, inheritance, constructor params, methods

**Phase B**: Full extraction at to-ref

**Phase B.5**: Inheritance chain resolution
- `resolve_serializable_classes()` uses transitive closure: if class extends a `Serializable` class, it's also serializable
- Fixed-point iteration until no new classes are added

**Phase B1**: Build inheritance summary

**Phase B3**: Module system diff
- Compare `module-info.java` directives between refs
- Track exports, requires, opens, provides changes

### 22 Source-Level Change Categories

`AnnotationRemoved`, `AnnotationAdded`, `AnnotationChanged`, `DelegationChanged`, `ExceptionAdded`, `ExceptionRemoved`, `SynchronizationRemoved`, `SynchronizationAdded`, `SerializationFieldAdded`, `SerializationFieldRemoved`, `SerializationFieldTypeChanged`, `TransientChanged`, `OverrideRemoved`, `OverrideAdded`, `ConstructorDependencyChanged`, `ModuleExportRemoved`, `ModuleExportAdded`, `ModuleRequiresChanged`, `FinalAdded`, `FinalRemoved`, `SealedChanged`, `InheritanceChanged`, `NativeRemoved`

## Manifest Diffing (`manifest/`)

Supports both Maven (`pom.xml`) and Gradle (`build.gradle`, `build.gradle.kts`).

### Maven POM Parser (`pom.rs`)
Event-driven XML parsing via `quick-xml`. Tracks XML path stack to correctly identify nested elements. Skips `dependencyManagement` section. Extracts: project identity, parent, dependencies (group:artifact:version:scope), properties, plugins.

### Gradle Parser
Regex-based extraction of dependency declarations. Less precise than POM parsing but handles common patterns.

### Changes Detected
`DependencyAdded`, `DependencyRemoved`, `DependencyVersionChanged`, `ParentVersionChanged`, `PropertyChanged`, `PluginChanged`, `ProjectIdentityChanged`, `DependencyScopeChanged`

## Cross-File Index (`index/mod.rs`)

`JavaIndex` provides project-wide call graph analysis:
```
JavaIndex
  types_by_name: HashMap<String, Vec<TypeInfo>>
  imports_by_file: HashMap<PathBuf, ImportMap>
  packages_by_file: HashMap<PathBuf, String>
  methods_by_name: HashMap<String, Vec<MethodInfo>>
```

`find_callers()` searches all indexed method bodies for references to the target symbol. Uses heuristic text matching (`target(`, `.target(`) plus optional receiver type checking.

## Konveyor Rules (`konveyor.rs`)

### TD Rules
Generated from `AnalysisReport<Java>`:
- Class/interface renamed, removed, relocated
- Method signature changed, return type changed
- Annotation removed (breaking ones only)
- Dependency changes

### SD Rules
From `JavaSdPipelineResult`:
- Annotation removed/changed rules
- Synchronized removed rules
- Module export/requires changed rules
- Exception added rules
- Final/sealed changed rules

### Namespace Migration Rules
Parse `"old.ns=new.ns"` or `"old.ns=new.ns@group:artifact:version"` format. Generate import relocation rules with optional dependency addition rules. Common use case: `javax.servlet=jakarta.servlet@jakarta.servlet:jakarta.servlet-api:6.0.0`

### Class Migration Rules
For classes with 5+ removed methods: generate a single comprehensive rule with member mapping table instead of individual per-method rules.

## Worktree Lifecycle

1. Create git worktree
2. Auto-detect Maven or Gradle from build files
3. If build not skipped: run `mvn compile` or `gradle compileJava`
4. Build failures are non-fatal warnings (extraction continues with source-only parsing)
5. Cleanup on drop

## Test Analyzer (`test_analyzer/mod.rs`)

### Test Discovery (3 strategies)
1. Maven/Gradle standard layout: `src/main/java/` -> `src/test/java/`
2. Sibling test files
3. Recursive search of test directories

### Test File Detection
Checks suffixes: `Test`, `Tests`, `IT`, `ITCase`, `Spec`; and prefix: `Test`.

### Assertion Detection
Matches patterns from: JUnit 4/5 (`assertEquals`, `assertThrows`), AssertJ (`assertThat`), Hamcrest (`assertThat` + matchers), TestNG, Mockito (`verify`, `when`), Google Truth (`assertThat` + `Truth`).

## Diff Parser (`diff_parser/mod.rs`)

Parses `git diff --name-status -M30` to find changed Java files. Key difference from TS: uses `overload_key` (includes parameter types) for disambiguating overloaded methods. Also extracts `static final` constant fields for value-change detection. `normalize_body()` strips comments and blank lines, properly handling string literals to avoid false stripping.
