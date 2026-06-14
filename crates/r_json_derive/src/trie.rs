//! Compile-time trie construction and code-generation for field-name dispatch.
//!
//! Instead of a linear `match key { b"field1" => …, b"field2" => … }`, we
//! generate a nested decision tree that first branches on `key.len()`, then on
//! individual bytes, making average-case dispatch O(1) per character rather
//! than O(N * key_len) for N fields.
//!
//! The algorithm:
//!   1. Group field names by length (O(1) comparison).
//!   2. Within each length group, build a character-trie keyed on byte
//!      positions, branching only where ambiguity exists.
//!   3. Emit the result as nested Rust `match` expressions that the compiler
//!      can compile to jump tables or short SIMD sequences.

use proc_macro2::TokenStream;
use quote::quote;

/// One entry in the dispatch table.
pub struct Entry {
    /// JSON key bytes (after rename / rename_all is applied).
    pub key: Vec<u8>,
    /// Field index (used as the result returned by the dispatch block).
    pub idx: usize,
}

/// Generate an expression that evaluates to the field index (`usize`) for
/// the given byte-slice `key`, or `usize::MAX` for an unknown field.
///
/// `key_var` is the name of the `&[u8]` variable that holds the parsed key
/// at runtime.
pub fn generate_dispatch(entries: &[Entry], key_var: &syn::Ident) -> TokenStream {
    if entries.is_empty() {
        return quote! { usize::MAX };
    }

    // Group by length.
    let max_len = entries.iter().map(|e| e.key.len()).max().unwrap_or(0);
    let mut by_len: Vec<Vec<&Entry>> = vec![Vec::new(); max_len + 1];
    for e in entries {
        by_len[e.key.len()].push(e);
    }

    let len_arms: Vec<TokenStream> = by_len
        .iter()
        .enumerate()
        .filter(|(_, group)| !group.is_empty())
        .map(|(len, group)| {
            let body = if group.len() == 1 {
                let e = group[0];
                let key_bytes: Vec<u8> = e.key.clone();
                let idx = e.idx;
                quote! {
                    if #key_var == &[#(#key_bytes),*] { #idx } else { usize::MAX }
                }
            } else {
                trie_node(group, 0, key_var)
            };
            quote! { #len => { #body } }
        })
        .collect();

    quote! {
        match #key_var.len() {
            #(#len_arms,)*
            _ => usize::MAX,
        }
    }
}

/// Recursively generate trie branches for a group of entries that all have
/// the same length (already handled by the outer match on `len`), starting at
/// byte position `pos`.
fn trie_node(group: &[&Entry], pos: usize, key_var: &syn::Ident) -> TokenStream {
    if group.len() == 1 {
        let e = group[0];
        let key_bytes: Vec<u8> = e.key.clone();
        let idx = e.idx;
        // All remaining bytes must match — do a full slice comparison.
        return quote! {
            if #key_var == &[#(#key_bytes),*] { #idx } else { usize::MAX }
        };
    }

    // Branch on byte at position `pos`.
    // Group entries by their byte at `pos`.
    let mut by_byte: std::collections::BTreeMap<u8, Vec<&Entry>> = std::collections::BTreeMap::new();
    for e in group {
        by_byte.entry(e.key[pos]).or_default().push(e);
    }

    // All entries share the same length so `pos` is always < key.len().
    let pos_lit = pos;
    let arms: Vec<TokenStream> = by_byte
        .into_iter()
        .map(|(byte, sub_group)| {
            let body = trie_node(&sub_group, pos + 1, key_var);
            quote! { #byte => { #body } }
        })
        .collect();

    quote! {
        match #key_var[#pos_lit] {
            #(#arms,)*
            _ => usize::MAX,
        }
    }
}
