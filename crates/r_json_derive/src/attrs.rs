//! Attribute parsing for both `#[serde(...)]` and `#[rjson(...)]` namespaces.
//!
//! We mirror every serde container / field attribute relevant to JSON so that
//! structs annotated purely with `#[derive(serde::Serialize, serde::Deserialize)]`
//! and `#[serde(…)]` work with r_json out of the box — users need not add any
//! new annotations.  r_json-specific extensions live under `#[rjson(…)]`.

use syn::{Attribute, Expr, ExprPath, LitStr, Result};

// ── RenameAll ─────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RenameAll {
    LowerCase,
    UpperCase,
    PascalCase,
    CamelCase,
    SnakeCase,
    ScreamingSnakeCase,
    KebabCase,
    ScreamingKebabCase,
}

impl RenameAll {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "lowercase"            => Some(Self::LowerCase),
            "UPPERCASE"            => Some(Self::UpperCase),
            "PascalCase"           => Some(Self::PascalCase),
            "camelCase"            => Some(Self::CamelCase),
            "snake_case"           => Some(Self::SnakeCase),
            "SCREAMING_SNAKE_CASE" => Some(Self::ScreamingSnakeCase),
            "kebab-case"           => Some(Self::KebabCase),
            "SCREAMING-KEBAB-CASE" => Some(Self::ScreamingKebabCase),
            _ => None,
        }
    }
}

// ── FieldDefault ──────────────────────────────────────────────────────────────

#[derive(Clone, Default)]
pub enum FieldDefault {
    #[default]
    None,
    /// `#[serde(default)]` — call `Default::default()`
    Default,
    /// `#[serde(default = "path")]` — call the given function
    Path(ExprPath),
    /// `#[rjson(default_value = expr)]` — use an inline const expression
    /// (only available under the `unstable` feature)
    #[allow(dead_code)]
    Value(Box<Expr>),
}

// ── ContainerAttrs ────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct ContainerAttrs {
    pub rename_all: Option<RenameAll>,
    pub deny_unknown_fields: bool,
    /// `#[serde(default)]` at the container level — missing fields use Default
    pub default: bool,
    /// `#[serde(tag = "…")]` — internally tagged enum
    pub tag: Option<String>,
    /// `#[serde(content = "…")]` — adjacently tagged enum
    pub content: Option<String>,
    /// `#[serde(untagged)]`
    pub untagged: bool,
    /// `#[rjson(trie_dispatch)]` — use compile-time trie for field dispatch
    pub trie_dispatch: bool,
}

// ── FieldAttrs ────────────────────────────────────────────────────────────────

#[derive(Default, Clone)]
pub struct FieldAttrs {
    pub rename: Option<String>,
    pub aliases: Vec<String>,
    pub skip: bool,
    pub skip_serializing: bool,
    pub skip_deserializing: bool,
    pub skip_serializing_if: Option<ExprPath>,
    pub default: FieldDefault,
    pub flatten: bool,
    pub borrow: bool,
}

// ── parsing ───────────────────────────────────────────────────────────────────

fn is_serde_or_rjson(attr: &Attribute) -> Option<bool> {
    if attr.path().is_ident("serde")  { return Some(false); }
    if attr.path().is_ident("rjson")  { return Some(true);  }
    None
}

pub fn parse_container_attrs(attrs: &[Attribute]) -> Result<ContainerAttrs> {
    let mut out = ContainerAttrs::default();
    for attr in attrs {
        let Some(is_rjson) = is_serde_or_rjson(attr) else { continue };
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename_all") {
                let s: LitStr = meta.value()?.parse()?;
                out.rename_all = RenameAll::from_str(&s.value());
            } else if meta.path.is_ident("deny_unknown_fields") {
                out.deny_unknown_fields = true;
            } else if meta.path.is_ident("default") {
                out.default = true;
            } else if meta.path.is_ident("tag") {
                let s: LitStr = meta.value()?.parse()?;
                out.tag = Some(s.value());
            } else if meta.path.is_ident("content") {
                let s: LitStr = meta.value()?.parse()?;
                out.content = Some(s.value());
            } else if meta.path.is_ident("untagged") {
                out.untagged = true;
            } else if is_rjson && meta.path.is_ident("trie_dispatch") {
                out.trie_dispatch = true;
            }
            // Silently ignore unrecognised attrs (e.g. `bound`, `crate`, …)
            Ok(())
        })?;
    }
    Ok(out)
}

pub fn parse_field_attrs(attrs: &[Attribute]) -> Result<FieldAttrs> {
    let mut out = FieldAttrs::default();
    for attr in attrs {
        let Some(is_rjson) = is_serde_or_rjson(attr) else { continue };
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename") {
                let s: LitStr = meta.value()?.parse()?;
                out.rename = Some(s.value());
            } else if meta.path.is_ident("alias") {
                let s: LitStr = meta.value()?.parse()?;
                out.aliases.push(s.value());
            } else if meta.path.is_ident("skip") {
                out.skip = true;
            } else if meta.path.is_ident("skip_serializing") {
                out.skip_serializing = true;
            } else if meta.path.is_ident("skip_deserializing") {
                out.skip_deserializing = true;
            } else if meta.path.is_ident("skip_serializing_if") {
                let s: LitStr = meta.value()?.parse()?;
                let path: ExprPath = s.parse()?;
                out.skip_serializing_if = Some(path);
            } else if meta.path.is_ident("default") {
                if meta.input.peek(syn::Token![=]) {
                    let s: LitStr = meta.value()?.parse()?;
                    let path: ExprPath = s.parse()?;
                    out.default = FieldDefault::Path(path);
                } else {
                    out.default = FieldDefault::Default;
                }
            } else if meta.path.is_ident("flatten") {
                out.flatten = true;
            } else if meta.path.is_ident("borrow") {
                out.borrow = true;
            } else if is_rjson && meta.path.is_ident("default_value") {
                // #[rjson(default_value = <expr>)]
                let expr: Expr = meta.value()?.parse()?;
                out.default = FieldDefault::Value(Box::new(expr));
            }
            Ok(())
        })?;
    }
    Ok(out)
}
