//! Java modifier extraction from tree-sitter nodes.
//!
//! Handles: public, protected, private, abstract, static, final,
//! default, sealed, non-sealed, strictfp, transient, volatile,
//! synchronized, native.

use semver_analyzer_core::Visibility;
use tree_sitter::Node;

/// Extracted Java modifiers for a declaration.
#[derive(Debug, Clone)]
pub struct JavaModifiers {
    pub visibility: Visibility,
    pub is_abstract: bool,
    pub is_static: bool,
    pub is_final: bool,
    pub is_default: bool,
    pub is_sealed: bool,
}

impl Default for JavaModifiers {
    fn default() -> Self {
        Self {
            visibility: Visibility::Internal, // Java default: package-private
            is_abstract: false,
            is_static: false,
            is_final: false,
            is_default: false,
            is_sealed: false,
        }
    }
}

/// Extract modifiers from a declaration node.
///
/// Looks for a `modifiers` child node and parses its children
/// for visibility keywords and other modifiers.
pub fn extract_modifiers(node: Node, _source: &str) -> JavaModifiers {
    let mut mods = JavaModifiers::default();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "modifiers" {
            let mut mod_cursor = child.walk();
            for mod_child in child.children(&mut mod_cursor) {
                match mod_child.kind() {
                    "public" => mods.visibility = Visibility::Public,
                    "protected" => mods.visibility = Visibility::Protected,
                    "private" => mods.visibility = Visibility::Private,
                    "abstract" => mods.is_abstract = true,
                    "static" => mods.is_static = true,
                    "final" => mods.is_final = true,
                    "default" => mods.is_default = true,
                    "sealed" => mods.is_sealed = true,
                    // Skip annotations (handled separately) and other nodes
                    _ => {}
                }
            }
        }
    }

    mods
}
