//! Case conversion for `rename_all`.
//!
//! Two entry-points:
//! * `apply_field` — input is Rust `snake_case` (struct / named-variant fields)
//! * `apply_variant` — input is Rust `PascalCase` (enum variant identifiers)

use crate::attrs::RenameAll;

/// Apply a rename rule to a **snake_case** Rust field name.
pub fn apply(name: &str, rule: RenameAll) -> String {
    match rule {
        RenameAll::LowerCase          => name.to_ascii_lowercase(),
        RenameAll::UpperCase          => name.to_ascii_uppercase(),
        RenameAll::SnakeCase          => name.to_owned(),
        RenameAll::ScreamingSnakeCase => name.to_ascii_uppercase(),
        RenameAll::KebabCase          => name.replace('_', "-"),
        RenameAll::ScreamingKebabCase => name.to_ascii_uppercase().replace('_', "-"),
        RenameAll::CamelCase          => snake_to_camel(name, false),
        RenameAll::PascalCase         => snake_to_camel(name, true),
    }
}

/// Apply a rename rule to a **PascalCase** Rust enum variant name.
pub fn apply_variant(name: &str, rule: RenameAll) -> String {
    match rule {
        RenameAll::LowerCase          => name.to_ascii_lowercase(),
        RenameAll::UpperCase          => name.to_ascii_uppercase(),
        RenameAll::PascalCase         => name.to_owned(),
        RenameAll::CamelCase          => {
            let mut chars = name.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_ascii_lowercase().to_string() + chars.as_str(),
            }
        }
        RenameAll::SnakeCase          => pascal_to_snake(name),
        RenameAll::ScreamingSnakeCase => pascal_to_snake(name).to_ascii_uppercase(),
        RenameAll::KebabCase          => pascal_to_snake(name).replace('_', "-"),
        RenameAll::ScreamingKebabCase => pascal_to_snake(name).to_ascii_uppercase().replace('_', "-"),
    }
}

fn snake_to_camel(name: &str, capitalise_first: bool) -> String {
    let mut result = String::with_capacity(name.len());
    let mut next_upper = capitalise_first;
    for ch in name.chars() {
        if ch == '_' {
            next_upper = true;
        } else if next_upper {
            result.extend(ch.to_uppercase());
            next_upper = false;
        } else {
            result.push(ch);
        }
    }
    result
}

/// Convert `PascalCase` → `snake_case`.
fn pascal_to_snake(name: &str) -> String {
    let mut result = String::with_capacity(name.len() + 4);
    for (i, ch) in name.chars().enumerate() {
        if ch.is_ascii_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(ch.to_ascii_lowercase());
    }
    result
}
