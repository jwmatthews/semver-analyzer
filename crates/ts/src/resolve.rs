//! Import path resolution for cross-file dependency analysis.
//!
//! Resolves TypeScript/JavaScript import specifiers to absolute file paths
//! using `oxc_resolver`, which supports:
//! - Relative imports (`./Foo`, `../components/Bar`)
//! - TypeScript path aliases from `tsconfig.json` (`@app/*` → `src/app/*`)
//! - Extension probing (`.tsx`, `.ts`, `.jsx`, `.js`)
//! - Index file resolution (directory → `index.tsx`, etc.)
//!
//! For monorepo projects with multiple `tsconfig.json` files, use
//! [`ResolverMap`] to create one resolver per tsconfig and route each
//! source file to the correct resolver based on path prefix matching.
//!
//! Ported from `frontend-analyzer-provider/crates/js-scanner/src/resolve.rs`
//! and extended with [`find_importers_of`] for the SD pipeline's transitive
//! behavioral change detection.

use oxc_resolver::{
    ResolveOptions, Resolver, TsconfigDiscovery, TsconfigOptions, TsconfigReferences,
};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

// ── ResolverMap ──────────────────────────────────────────────────────────

/// Routes source files to the correct `oxc_resolver::Resolver` based on
/// which `tsconfig.json` covers the file.
///
/// In monorepo projects with multiple tsconfig files (e.g., `client/`,
/// `common/`, `cypress/`), each tsconfig may define different path aliases.
/// `ResolverMap` creates one resolver per tsconfig and selects the right
/// one for each file being scanned.
///
/// Selection is by longest-prefix match on the tsconfig's directory:
/// a file at `client/src/app/Page.tsx` matches the resolver for
/// `client/tsconfig.json` because `client/` is a prefix of the file path.
pub struct ResolverMap {
    /// (tsconfig_dir, resolver) sorted by path depth descending
    /// so the most specific (longest) prefix match wins.
    resolvers: Vec<(PathBuf, Resolver)>,
    /// Fallback resolver with no tsconfig (for files outside any tsconfig scope).
    fallback: Resolver,
}

impl ResolverMap {
    /// Get the resolver whose tsconfig directory is the longest ancestor of
    /// `file_path`. Falls back to a resolver with no tsconfig if no match.
    pub fn resolver_for_file(&self, file_path: &Path) -> &Resolver {
        for (tsconfig_dir, resolver) in &self.resolvers {
            if file_path.starts_with(tsconfig_dir) {
                return resolver;
            }
        }
        &self.fallback
    }
}

/// Discover all `tsconfig.json` files under `root` (up to `max_depth`
/// levels deep) and create a [`ResolverMap`] with one resolver per tsconfig.
///
/// Skips `node_modules`, `.git`, `dist`, `build`, and `target` directories.
pub fn create_resolver_map(root: &Path, max_depth: usize) -> ResolverMap {
    let tsconfigs = find_all_tsconfigs_in_project(root, max_depth);

    if !tsconfigs.is_empty() {
        tracing::info!(
            "Found {} tsconfig.json file(s): {:?}",
            tsconfigs.len(),
            tsconfigs
        );
    }

    let mut resolvers: Vec<(PathBuf, Resolver)> = tsconfigs
        .iter()
        .filter_map(|tc| {
            let dir = tc.parent()?;
            let resolver = create_resolver(Some(tc));
            Some((dir.to_path_buf(), resolver))
        })
        .collect();

    // Sort by component count descending — most specific (deepest) paths first
    // so the longest-prefix match wins in resolver_for_file().
    resolvers.sort_by_key(|b| std::cmp::Reverse(b.0.components().count()));

    ResolverMap {
        resolvers,
        fallback: create_resolver(None),
    }
}

/// Find all `tsconfig.json` files in a project tree.
///
/// Searches `root` and its subdirectories up to `max_depth` levels deep.
/// Skips `node_modules`, `.git`, `dist`, `build`, and `target` directories.
pub fn find_all_tsconfigs_in_project(root: &Path, max_depth: usize) -> Vec<PathBuf> {
    let mut found = Vec::new();

    // Check root itself
    let candidate = root.join("tsconfig.json");
    if candidate.is_file() {
        found.push(candidate);
    }

    // BFS through subdirectories
    let mut queue: VecDeque<(PathBuf, usize)> = VecDeque::new();
    queue.push_back((root.to_path_buf(), 0));

    while let Some((dir, depth)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }

        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with('.')
                || name_str == "node_modules"
                || name_str == "dist"
                || name_str == "build"
                || name_str == "target"
            {
                continue;
            }

            let tsconfig = path.join("tsconfig.json");
            if tsconfig.is_file() {
                found.push(tsconfig);
            }

            queue.push_back((path, depth + 1));
        }
    }

    found
}

// ── Single resolver helpers ──────────────────────────────────────────────

/// Create a resolver configured for TypeScript/React projects.
///
/// Uses `TsconfigOptions` to read `compilerOptions.paths` aliases and
/// `baseUrl` from the project's `tsconfig.json`.
pub fn create_resolver(tsconfig_path: Option<&Path>) -> Resolver {
    let mut options = ResolveOptions {
        extensions: vec![
            ".tsx".into(),
            ".ts".into(),
            ".jsx".into(),
            ".js".into(),
            ".json".into(),
        ],
        main_files: vec!["index".into()],
        condition_names: vec!["node".into(), "import".into()],
        ..ResolveOptions::default()
    };

    if let Some(tsconfig) = tsconfig_path {
        options.tsconfig = Some(TsconfigDiscovery::Manual(TsconfigOptions {
            config_file: tsconfig.to_path_buf(),
            references: TsconfigReferences::Auto,
        }));
    }

    Resolver::new(options)
}

/// Resolve an import specifier to an absolute file path.
///
/// Returns `None` if the specifier can't be resolved.
pub fn resolve_import_with_resolver(
    resolver: &Resolver,
    importing_file: &Path,
    module_source: &str,
) -> Option<PathBuf> {
    let dir = importing_file.parent()?;
    match resolver.resolve(dir, module_source) {
        Ok(resolution) => Some(resolution.into_path_buf()),
        Err(_) => None,
    }
}

/// Check whether a resolved path is inside `node_modules`.
pub fn is_node_modules_path(path: &Path) -> bool {
    path.components().any(|c| c.as_os_str() == "node_modules")
}

// ── Cross-file import scanning ──────────────────────────────────────────

/// An import of a specific symbol from a resolved module.
#[derive(Debug, Clone)]
pub struct ImportBinding {
    /// The file that contains the import statement.
    pub importing_file: PathBuf,
    /// The local name used for the imported symbol (may differ from the
    /// exported name if aliased via `import { X as Y }`).
    pub local_name: String,
}

/// Find all files in `scan_files` that import `target_symbol` from a module
/// that resolves to `target_file`.
///
/// For each file in `scan_files`, parses its import declarations and
/// resolves each import source to an absolute path. If the resolved path
/// matches `target_file`, and the import includes `target_symbol` (as a
/// named import or aliased), the file is returned with its local binding
/// name.
///
/// # Arguments
///
/// * `resolver_map` - The resolver map for the project.
/// * `target_file` - Absolute path to the file containing the target symbol.
/// * `target_symbol` - The exported name of the symbol to search for.
/// * `scan_files` - List of source file paths to scan for imports.
///
/// # Returns
///
/// A list of `ImportBinding`s — one per file that imports the target symbol.
pub fn find_importers_of(
    resolver_map: &ResolverMap,
    target_file: &Path,
    target_symbol: &str,
    scan_files: &[PathBuf],
) -> Vec<ImportBinding> {
    let mut results = Vec::new();

    // Canonicalize target for comparison
    let target_canonical = match target_file.canonicalize() {
        Ok(p) => p,
        Err(_) => target_file.to_path_buf(),
    };

    for file_path in scan_files {
        let source = match std::fs::read_to_string(file_path) {
            Ok(s) => s,
            Err(_) => continue,
        };

        if let Some(binding) = check_file_imports(
            resolver_map,
            file_path,
            &source,
            &target_canonical,
            target_symbol,
        ) {
            results.push(binding);
        }
    }

    results
}

/// Check a single file's import declarations for an import of `target_symbol`
/// from a module resolving to `target_file`.
fn check_file_imports(
    resolver_map: &ResolverMap,
    file_path: &Path,
    source: &str,
    target_file: &Path,
    target_symbol: &str,
) -> Option<ImportBinding> {
    let allocator = oxc_allocator::Allocator::default();
    let source_type = oxc_span::SourceType::from_path(file_path).ok()?;
    let parsed = oxc_parser::Parser::new(&allocator, source, source_type).parse();

    let resolver = resolver_map.resolver_for_file(file_path);

    for item in &parsed.program.body {
        if let oxc_ast::ast::Statement::ImportDeclaration(import) = item {
            let module_source = import.source.value.as_str();

            // Skip bare specifiers (npm packages) — we only care about
            // project-internal imports that resolve to local files.
            if !module_source.starts_with('.')
                && !module_source.starts_with('/')
                && !module_source.starts_with('@')
            {
                continue;
            }

            // Resolve the import to an absolute path
            let resolved = match resolve_import_with_resolver(resolver, file_path, module_source) {
                Some(p) => p,
                None => continue,
            };

            // Skip node_modules
            if is_node_modules_path(&resolved) {
                continue;
            }

            // Canonicalize for comparison
            let resolved_canonical = resolved.canonicalize().unwrap_or(resolved);

            if resolved_canonical != *target_file {
                continue;
            }

            // Check if the import includes the target symbol
            if let Some(specifiers) = &import.specifiers {
                for spec in specifiers {
                    match spec {
                        oxc_ast::ast::ImportDeclarationSpecifier::ImportSpecifier(named) => {
                            let imported_name = match &named.imported {
                                oxc_ast::ast::ModuleExportName::IdentifierName(id) => {
                                    id.name.as_str()
                                }
                                oxc_ast::ast::ModuleExportName::IdentifierReference(id) => {
                                    id.name.as_str()
                                }
                                oxc_ast::ast::ModuleExportName::StringLiteral(s) => {
                                    s.value.as_str()
                                }
                            };
                            if imported_name == target_symbol {
                                return Some(ImportBinding {
                                    importing_file: file_path.to_path_buf(),
                                    local_name: named.local.name.as_str().to_string(),
                                });
                            }
                        }
                        oxc_ast::ast::ImportDeclarationSpecifier::ImportNamespaceSpecifier(_) => {
                            // `import * as helpers from '...'` — the symbol
                            // would be accessed as `helpers.getOUIAProps`.
                            // For Phase 1 we skip namespace imports.
                        }
                        oxc_ast::ast::ImportDeclarationSpecifier::ImportDefaultSpecifier(_) => {
                            // Default import — skip for now (helpers are
                            // typically named exports).
                        }
                    }
                }
            }
        }
    }

    None
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::slice;

    #[test]
    fn test_create_resolver_map_empty() {
        let dir = tempfile::tempdir().unwrap();
        let map = create_resolver_map(dir.path(), 3);
        assert!(map.resolvers.is_empty());
    }

    #[test]
    fn test_find_tsconfigs_in_project() {
        let dir = tempfile::tempdir().unwrap();
        let packages = dir.path().join("packages").join("react-core");
        fs::create_dir_all(&packages).unwrap();
        fs::write(
            packages.join("tsconfig.json"),
            r#"{ "compilerOptions": {} }"#,
        )
        .unwrap();

        let found = find_all_tsconfigs_in_project(dir.path(), 5);
        assert_eq!(found.len(), 1);
    }

    #[test]
    fn test_find_tsconfigs_skips_node_modules() {
        let dir = tempfile::tempdir().unwrap();
        let nm = dir.path().join("node_modules").join("pkg");
        fs::create_dir_all(&nm).unwrap();
        fs::write(nm.join("tsconfig.json"), "{}").unwrap();

        let found = find_all_tsconfigs_in_project(dir.path(), 3);
        assert!(found.is_empty());
    }

    #[test]
    fn test_resolve_relative_import() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        fs::create_dir_all(&src).unwrap();
        let target = src.join("helpers.ts");
        fs::write(&target, "export function getOUIAProps() {}").unwrap();
        let importing = src.join("Button.tsx");
        fs::write(&importing, "").unwrap();

        let resolver = create_resolver(None);
        let resolved = resolve_import_with_resolver(&resolver, &importing, "./helpers");
        assert!(resolved.is_some());
        assert!(resolved.unwrap().ends_with("helpers.ts"));
    }

    #[test]
    fn test_find_importers_of_named_import() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        let helpers_dir = src.join("helpers");
        let components_dir = src.join("components");
        fs::create_dir_all(&helpers_dir).unwrap();
        fs::create_dir_all(&components_dir).unwrap();

        // Helper file
        let helper_file = helpers_dir.join("ouia.ts");
        fs::write(
            &helper_file,
            "export function getOUIAProps(name: string) { return {}; }",
        )
        .unwrap();

        // Component that imports the helper
        let button_file = components_dir.join("Button.tsx");
        fs::write(
            &button_file,
            r#"import { getOUIAProps } from '../helpers/ouia';
export const Button = () => null;"#,
        )
        .unwrap();

        // Component that does NOT import the helper
        let alert_file = components_dir.join("Alert.tsx");
        fs::write(&alert_file, "export const Alert = () => null;").unwrap();

        let resolver_map = create_resolver_map(dir.path(), 3);
        let scan_files = vec![button_file.clone(), alert_file];

        let importers = find_importers_of(&resolver_map, &helper_file, "getOUIAProps", &scan_files);

        assert_eq!(importers.len(), 1);
        assert_eq!(importers[0].importing_file, button_file);
        assert_eq!(importers[0].local_name, "getOUIAProps");
    }

    #[test]
    fn test_find_importers_of_aliased_import() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        fs::create_dir_all(&src).unwrap();

        let helper_file = src.join("helpers.ts");
        fs::write(&helper_file, "export function getOUIAProps() {}").unwrap();

        let consumer = src.join("Consumer.tsx");
        fs::write(
            &consumer,
            "import { getOUIAProps as getProps } from './helpers';",
        )
        .unwrap();

        let resolver_map = create_resolver_map(dir.path(), 3);
        let importers = find_importers_of(
            &resolver_map,
            &helper_file,
            "getOUIAProps",
            slice::from_ref(&consumer),
        );

        assert_eq!(importers.len(), 1);
        assert_eq!(importers[0].local_name, "getProps");
    }

    #[test]
    fn test_find_importers_no_match() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        fs::create_dir_all(&src).unwrap();

        let helper_file = src.join("helpers.ts");
        fs::write(&helper_file, "export function getOUIAProps() {}").unwrap();

        // Import a different symbol
        let consumer = src.join("Consumer.tsx");
        fs::write(&consumer, "import { otherFunction } from './helpers';").unwrap();

        let resolver_map = create_resolver_map(dir.path(), 3);
        let importers = find_importers_of(&resolver_map, &helper_file, "getOUIAProps", &[consumer]);

        assert!(importers.is_empty());
    }

    #[test]
    fn test_is_node_modules_path() {
        assert!(is_node_modules_path(Path::new(
            "/project/node_modules/@patternfly/react-core/dist/index.js"
        )));
        assert!(!is_node_modules_path(Path::new(
            "/project/src/components/Foo.tsx"
        )));
    }
}
