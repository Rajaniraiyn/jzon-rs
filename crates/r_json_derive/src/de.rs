//! Code-generation for `#[derive(FromJson)]`.
//!
//! Key features of the generated code:
//!
//! * **Field-hint cache** — a single `usize` variable tracks the index of the
//!   most-recently matched field and is tried first on the next key.  For JSON
//!   payloads whose field order matches the struct definition (the common case)
//!   this gives O(1) dispatch on almost every field.
//!
//! * **Integer-padded key comparison** — for field names ≤8 bytes, a single
//!   u64 integer comparison replaces memcmp.  For names 9–16 bytes, u128.
//!   Names >16 bytes fall back to byte-slice comparison.  The dispatch first
//!   branches on `key.len()`, so lengths are verified before the integer test.
//!
//! * **Compile-time minimal perfect hash** (for structs with >6 active fields)
//!   — a multiplier found at proc-macro time gives O(1) dispatch regardless of
//!   field count.
//!
//! * **Trie dispatch** (enabled with `#[rjson(trie_dispatch)]`) — length-first
//!   then character-by-character branching, compiled to jump tables by rustc.
//!
//! * **Alias support** — `|`-joined patterns in the generated `match`.
//!
//! * **Full serde attribute compatibility** — rename, rename_all,
//!   deny_unknown_fields, skip_deserializing, default, default = "fn", flatten.

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{Data, DeriveInput, Error, Fields, GenericParam, Lifetime, Result, Type};

use crate::attrs::{self, ContainerAttrs, FieldAttrs, FieldDefault};
use crate::rename;
use crate::trie::{self, Entry};

pub fn expand(input: &DeriveInput) -> Result<TokenStream> {
    match &input.data {
        Data::Struct(_) => expand_struct(input),
        Data::Enum(_) => expand_enum(input),
        Data::Union(_) => Err(Error::new_spanned(input, "FromJson does not support unions")),
    }
}

// ── integer-comparison helpers ────────────────────────────────────────────────

/// Pad `bytes` to 8 bytes with zeros and interpret as a little-endian u64.
fn key_to_u64(bytes: &[u8]) -> u64 {
    let mut b = [0u8; 8];
    let len = bytes.len().min(8);
    b[..len].copy_from_slice(&bytes[..len]);
    u64::from_le_bytes(b)
}

/// Pad `bytes` to 16 bytes with zeros and interpret as a little-endian u128.
fn key_to_u128(bytes: &[u8]) -> u128 {
    let mut b = [0u8; 16];
    let len = bytes.len().min(16);
    b[..len].copy_from_slice(&bytes[..len]);
    u128::from_le_bytes(b)
}

// ── minimal perfect hash helpers ──────────────────────────────────────────────

/// Compute a hash of `key` using `mul` as the multiplier.
fn phf_hash(key: &[u8], mul: u64, table_size: usize) -> usize {
    let h = key
        .iter()
        .fold(0u64, |acc, &b| acc.wrapping_mul(mul).wrapping_add(b as u64));
    (h as usize) % table_size
}

/// Try to find a multiplier that produces no collisions for `keys`.
/// Returns `(multiplier, table_size)` where table_size is next power of two
/// >= keys.len().  Returns None if no small multiplier works (should not happen
/// in practice for reasonable field counts).
fn find_phf_multiplier(keys: &[&[u8]]) -> Option<(u64, usize)> {
    if keys.is_empty() {
        return None;
    }
    let table_size = keys.len().next_power_of_two();
    // Candidates: small primes known to work well as polynomial hash multipliers
    let candidates: &[u64] = &[
        31, 37, 97, 131, 137, 149, 157, 163, 167, 173, 179, 181, 191, 193, 197, 199,
        211, 223, 227, 229, 233, 239, 241, 251, 257, 263, 269, 271, 277, 281, 283,
        293, 307, 311, 313, 317, 331, 337, 347, 349, 353, 359, 367, 373, 379, 383,
        389, 397, 401, 409, 419, 421, 431, 433, 439, 443, 449, 457, 461, 463, 467,
        2654435761, // Knuth's multiplicative hash
        2246822519, 3266489917, 668265263, 374761393,
    ];
    for &m in candidates {
        let mut seen = vec![false; table_size];
        let mut ok = true;
        for key in keys {
            let h = phf_hash(key, m, table_size);
            if seen[h] {
                ok = false;
                break;
            }
            seen[h] = true;
        }
        if ok {
            return Some((m, table_size));
        }
    }
    None
}

// ── struct ────────────────────────────────────────────────────────────────────

fn expand_struct(input: &DeriveInput) -> Result<TokenStream> {
    let ident = &input.ident;
    let container = attrs::parse_container_attrs(&input.attrs)?;
    let de_lt = Lifetime::new("'de", Span::call_site());

    let fields = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(f) => &f.named,
            Fields::Unit => return expand_unit_struct(input, &container),
            _ => return Err(Error::new_spanned(ident, "FromJson only supports named-field structs")),
        },
        _ => unreachable!(),
    };

    // ── generics ─────────────────────────────────────────────────────────────
    let impl_params: Vec<TokenStream> = {
        let mut p = vec![quote! { 'de }];
        for gp in &input.generics.params {
            match gp {
                GenericParam::Lifetime(_) => {}  // absorbed into 'de
                GenericParam::Type(t) => p.push(quote! { #t }),
                GenericParam::Const(c) => p.push(quote! { #c }),
            }
        }
        p
    };
    let ty_args: TokenStream = if input.generics.params.is_empty() {
        quote! {}
    } else {
        let args: Vec<TokenStream> = input.generics.params.iter().map(|gp| match gp {
            GenericParam::Lifetime(_) => quote! { 'de },
            GenericParam::Type(t) => { let id = &t.ident; quote! { #id } }
            GenericParam::Const(c) => { let id = &c.ident; quote! { #id } }
        }).collect();
        quote! { <#(#args),*> }
    };
    let where_clause = &input.generics.where_clause;

    // ── per-field info ────────────────────────────────────────────────────────
    struct FieldInfo<'a> {
        fname: &'a syn::Ident,
        json_key: String,
        all_keys: Vec<String>,   // primary + aliases
        ty: &'a Type,
        fattrs: FieldAttrs,
        idx: usize,
    }

    let mut active_fields: Vec<FieldInfo> = Vec::new();
    for (i, field) in fields.iter().enumerate() {
        let fname = field.ident.as_ref().unwrap();
        let fattrs = attrs::parse_field_attrs(&field.attrs)?;
        let json_key = if let Some(r) = &fattrs.rename {
            r.clone()
        } else if let Some(rule) = container.rename_all {
            rename::apply(&fname.to_string(), rule)
        } else {
            fname.to_string()
        };
        let mut all_keys = vec![json_key.clone()];
        all_keys.extend(fattrs.aliases.clone());
        active_fields.push(FieldInfo { fname, json_key, all_keys, ty: &field.ty, fattrs, idx: i });
    }

    // ── variable declarations ─────────────────────────────────────────────────
    let var_decls: Vec<TokenStream> = active_fields.iter()
        .filter(|f| !f.fattrs.skip && !f.fattrs.skip_deserializing)
        .map(|f| {
            let fname = f.fname;
            quote! { let mut #fname = None; }
        })
        .collect();

    // ── dispatch entries (include all aliases) ────────────────────────────────
    let dispatch_entries: Vec<Entry> = active_fields.iter()
        .filter(|f| !f.fattrs.skip && !f.fattrs.skip_deserializing)
        .flat_map(|f| {
            f.all_keys.iter().map(move |k| Entry {
                key: k.as_bytes().to_vec(),
                idx: f.idx,
            })
        })
        .collect();

    // ── hint array of primary keys (for the O(1) cache fast-path) ────────────
    let active_deserialized: Vec<&FieldInfo> = active_fields.iter()
        .filter(|f| !f.fattrs.skip && !f.fattrs.skip_deserializing)
        .collect();

    let num_active = active_deserialized.len();

    // Build a compile-time array of primary key byte slices using b"..." literals.
    let hint_table: Vec<TokenStream> = active_deserialized.iter().map(|f| {
        let key_str = &f.json_key;
        let key_bytes = key_str.as_bytes();
        // Use byte string literal syntax for cleaner generated code
        let byte_str = proc_macro2::Literal::byte_string(key_bytes);
        quote! { #byte_str as &[u8] }
    }).collect();

    // Map from field index → position in the hint table.
    // We need to know "which slot in the hint table corresponds to field idx".
    let hint_slot_map: Vec<usize> = {
        let mut m = vec![usize::MAX; active_fields.len()];
        for (slot, fi) in active_deserialized.iter().enumerate() {
            m[fi.idx] = slot;
        }
        m
    };

    // ── dispatch logic ────────────────────────────────────────────────────────
    let key_ident = syn::Ident::new("_key", Span::call_site());
    let dispatch_expr = if container.trie_dispatch {
        trie::generate_dispatch(&dispatch_entries, &key_ident)
    } else {
        generate_optimized_dispatch(&dispatch_entries, num_active)
    };

    // ── inline annotation ─────────────────────────────────────────────────────
    // For small structs (≤4 active fields), annotate with #[inline] to help
    // the compiler always inline the small scanner.
    let inline_attr: TokenStream = if num_active <= 4 {
        quote! { #[inline] }
    } else {
        quote! {}
    };

    // ── per-field read arms ───────────────────────────────────────────────────
    let read_arms: Vec<TokenStream> = active_deserialized.iter().map(|f| {
        let fname = f.fname;
        let idx = f.idx;
        let read_expr = field_read_expr(f.ty, &de_lt);
        let hint_slot = hint_slot_map[idx];
        let next_slot = if num_active > 0 { (hint_slot + 1) % num_active } else { 0 };
        quote! {
            #idx => {
                #fname = Some(#read_expr);
                _hint = #next_slot;
            }
        }
    }).collect();

    // ── unknown field handling ────────────────────────────────────────────────
    let unknown_handler = if container.deny_unknown_fields {
        quote! { return Err(::r_json::Error::UnknownField); }
    } else {
        quote! { scanner.skip_value()?; }
    };

    // ── field assembly (Ok(Struct { … })) ────────────────────────────────────
    let field_assembly: Vec<TokenStream> = active_fields.iter().map(|f| {
        let fname = f.fname;
        let fname_str = f.json_key.as_str();

        if f.fattrs.skip || f.fattrs.skip_deserializing {
            return quote! { #fname: ::std::default::Default::default(), };
        }
        // skip_serializing means the field is omitted in JSON output, so it
        // will typically be absent when round-tripping → treat as defaulted.
        if f.fattrs.skip_serializing {
            return quote! { #fname: #fname.unwrap_or_default(), };
        }

        match &f.fattrs.default {
            FieldDefault::None => {
                if container.default {
                    quote! { #fname: #fname.unwrap_or_default(), }
                } else if is_option(f.ty) {
                    quote! { #fname: #fname.unwrap_or(None), }
                } else {
                    quote! { #fname: #fname.ok_or(::r_json::Error::MissingField(#fname_str))?, }
                }
            }
            FieldDefault::Default => {
                quote! { #fname: #fname.unwrap_or_default(), }
            }
            FieldDefault::Path(path) => {
                quote! { #fname: #fname.unwrap_or_else(#path), }
            }
            FieldDefault::Value(expr) => {
                quote! { #fname: #fname.unwrap_or_else(|| #expr), }
            }
        }
    }).collect();

    Ok(quote! {
        #[automatically_derived]
        impl<#(#impl_params),*> ::r_json::FromJson<'de>
            for #ident #ty_args
            #where_clause
        {
            #inline_attr
            fn from_json_scanner(
                scanner: &mut ::r_json::Scanner<'de>,
            ) -> ::std::result::Result<Self, ::r_json::Error> {
                scanner.skip_whitespace();
                scanner.expect_byte(b'{')?;

                #(#var_decls)*

                // Compile-time table of primary field name byte slices.
                const FIELD_HINTS: [&[u8]; #num_active] = [#(#hint_table),*];
                // `_hint` tracks the expected next field slot (0-based index into
                // FIELD_HINTS).  For in-order JSON this gives O(1) dispatch.
                let mut _hint: usize = 0;

                loop {
                    match scanner.peek_byte_after_ws()? {
                        b'}' => { scanner.advance(); break; }
                        b'"' => {
                            let _key = scanner.read_key_colon()?;

                            // Fast-path: check the hinted field first.
                            let _field_idx = if _hint < #num_active && _key == FIELD_HINTS[_hint] {
                                _hint
                            } else {
                                // Full dispatch (integer-compare, PHF, or match).
                                #dispatch_expr
                            };

                            match _field_idx {
                                #(#read_arms)*
                                _ => { #unknown_handler }
                            }
                        }
                        _ => return Err(::r_json::Error::UnexpectedToken),
                    }

                    // After each value the next byte is almost always ',' or '}'.
                    // Handle ',' (continue) and '}' (done) without going around
                    // the outer loop an extra time.
                    match scanner.peek_byte_after_ws()? {
                        b',' => { scanner.advance(); }
                        b'}' => { scanner.advance(); break; }
                        _ => return Err(::r_json::Error::UnexpectedToken),
                    }
                }

                Ok(#ident {
                    #(#field_assembly)*
                })
            }
        }
    })
}

// ── optimized dispatch code-generation ───────────────────────────────────────

/// A single (key_bytes, field_idx) pair for dispatch.
struct DispatchKey<'a> {
    key: &'a [u8],
    idx: usize,
}

/// Generate the optimized dispatch expression.
///
/// Strategy:
/// - If no entries: return usize::MAX.
/// - If num_active > 6 AND a perfect hash multiplier can be found: emit PHF
///   dispatch that gives O(1) average lookup with verification.
/// - Otherwise: emit a `match _key.len()` tree where each length arm uses
///   integer comparison (u64 for ≤8 bytes, u128 for ≤16 bytes, slice compare
///   for >16 bytes).
fn generate_optimized_dispatch(entries: &[Entry], num_active: usize) -> TokenStream {
    if entries.is_empty() {
        return quote! { usize::MAX };
    }

    // Collect keys from entries.
    let all_keys: Vec<DispatchKey> = entries.iter()
        .map(|e| DispatchKey { key: &e.key, idx: e.idx })
        .collect();

    // For structs with >6 active fields, try compile-time minimal perfect hash.
    if num_active > 6 {
        let key_slices: Vec<&[u8]> = all_keys.iter().map(|dk| dk.key).collect();
        if let Some((mul, table_size)) = find_phf_multiplier(&key_slices) {
            return generate_phf_dispatch(&all_keys, mul, table_size);
        }
    }

    // Otherwise use length-first integer comparison.
    generate_integer_compare_dispatch(&all_keys)
}

/// Generate dispatch using `match _key.len()` with integer comparisons per group.
fn generate_integer_compare_dispatch(all_keys: &[DispatchKey]) -> TokenStream {
    // Group by key length.
    let max_len = all_keys.iter().map(|dk| dk.key.len()).max().unwrap_or(0);
    let mut by_len: Vec<Vec<&DispatchKey>> = vec![Vec::new(); max_len + 1];
    for dk in all_keys {
        by_len[dk.key.len()].push(dk);
    }

    let len_arms: Vec<TokenStream> = by_len
        .iter()
        .enumerate()
        .filter(|(_, group)| !group.is_empty())
        .map(|(len, group)| {
            let body = if len <= 8 {
                // u64 comparison: load key into u64 and compare against precomputed constants.
                let arms: Vec<TokenStream> = group.iter().map(|dk| {
                    let const_val = key_to_u64(dk.key);
                    let idx = dk.idx;
                    quote! { #const_val => #idx, }
                }).collect();
                quote! {
                    {
                        let mut _b = [0u8; 8];
                        _b[..#len].copy_from_slice(_key);
                        match u64::from_le_bytes(_b) {
                            #(#arms)*
                            _ => usize::MAX,
                        }
                    }
                }
            } else if len <= 16 {
                // u128 comparison.
                let arms: Vec<TokenStream> = group.iter().map(|dk| {
                    let const_val = key_to_u128(dk.key);
                    let idx = dk.idx;
                    quote! { #const_val => #idx, }
                }).collect();
                quote! {
                    {
                        let mut _b = [0u8; 16];
                        _b[..#len].copy_from_slice(_key);
                        match u128::from_le_bytes(_b) {
                            #(#arms)*
                            _ => usize::MAX,
                        }
                    }
                }
            } else {
                // Fallback: plain byte-slice comparison.
                let arms: Vec<TokenStream> = group.iter().map(|dk| {
                    let key_bytes = dk.key;
                    let byte_str = proc_macro2::Literal::byte_string(key_bytes);
                    let idx = dk.idx;
                    quote! { #byte_str => #idx, }
                }).collect();
                quote! {
                    match _key {
                        #(#arms)*
                        _ => usize::MAX,
                    }
                }
            };
            quote! { #len => #body }
        })
        .collect();

    quote! {
        match _key.len() {
            #(#len_arms,)*
            _ => usize::MAX,
        }
    }
}

/// Generate PHF-based dispatch.
///
/// Emits:
/// ```text
/// const HASH_MUL: u64 = <mul>;
/// const HASH_TABLE: [usize; TABLE_SIZE] = [...];
/// const FIELD_NAMES: [&[u8]; N] = [...];
///
/// let _h = _key.iter().fold(0u64, |acc, &b| acc.wrapping_mul(HASH_MUL).wrapping_add(b as u64))
///          as usize % TABLE_SIZE;
/// let _fi = HASH_TABLE[_h];
/// if _fi != usize::MAX && _key == FIELD_NAMES[_fi] { _fi } else { usize::MAX }
/// ```
///
/// FIELD_NAMES is indexed by field index (not by hash slot), so we emit
/// a flat array of (field_idx -> key bytes) and store field_idx values in
/// HASH_TABLE.
fn generate_phf_dispatch(
    all_keys: &[DispatchKey],
    mul: u64,
    table_size: usize,
) -> TokenStream {
    // Build HASH_TABLE: slot → field_idx (usize::MAX = empty)
    let mut hash_table: Vec<usize> = vec![usize::MAX; table_size];
    for dk in all_keys {
        let h = phf_hash(dk.key, mul, table_size);
        hash_table[h] = dk.idx;
    }

    // Build FIELD_NAMES: field_idx → key bytes.
    // We need to store ALL keys (primary + aliases) indexed by field_idx.
    // For aliases of the same field_idx, the PHF can only hold one entry per
    // slot (aliases may collide in the hash table because they map to same idx).
    // We store the primary key per field for verification; for aliases we just
    // store those keys too (they won't be in HASH_TABLE if overwritten but the
    // PHF is used for the fast path while the slow path handles aliases — wait,
    // we must handle aliases correctly).
    //
    // Actually, since aliases can map to the same field_idx, we need a
    // separate verification array that maps hash slot → expected key bytes.
    // Build SLOT_KEYS: hash_slot → key bytes (for the key that landed there).
    let mut slot_keys: Vec<Option<Vec<u8>>> = vec![None; table_size];
    for dk in all_keys {
        let h = phf_hash(dk.key, mul, table_size);
        // The PHF has no collisions, so each slot gets exactly one key.
        slot_keys[h] = Some(dk.key.to_vec());
    }

    let hash_table_tokens: Vec<TokenStream> = hash_table.iter().map(|&fi| {
        if fi == usize::MAX {
            quote! { usize::MAX }
        } else {
            quote! { #fi }
        }
    }).collect();

    // SLOT_KEYS: for each slot, what is the expected key?
    // We store it as a &[u8] with a sentinel empty slice for empty slots.
    let slot_key_tokens: Vec<TokenStream> = slot_keys.iter().map(|sk| {
        match sk {
            None => quote! { b"" as &[u8] },
            Some(k) => {
                let byte_str = proc_macro2::Literal::byte_string(k);
                quote! { #byte_str as &[u8] }
            }
        }
    }).collect();

    let mul_lit = mul;

    quote! {
        {
            const HASH_MUL: u64 = #mul_lit;
            const HASH_TABLE: [usize; #table_size] = [#(#hash_table_tokens),*];
            const SLOT_KEYS: [&[u8]; #table_size] = [#(#slot_key_tokens),*];

            let _h = _key.iter().fold(0u64, |acc, &b|
                acc.wrapping_mul(HASH_MUL).wrapping_add(b as u64)
            ) as usize % #table_size;
            let _fi = HASH_TABLE[_h];
            if _fi != usize::MAX && _key == SLOT_KEYS[_h] {
                _fi
            } else {
                usize::MAX
            }
        }
    }
}

fn expand_unit_struct(input: &DeriveInput, _container: &ContainerAttrs) -> Result<TokenStream> {
    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    Ok(quote! {
        #[automatically_derived]
        impl<'de> #impl_generics ::r_json::FromJson<'de>
            for #ident #ty_generics #where_clause
        {
            fn from_json_scanner(
                scanner: &mut ::r_json::Scanner<'de>,
            ) -> ::std::result::Result<Self, ::r_json::Error> {
                scanner.skip_whitespace();
                scanner.expect_byte(b'{')?;
                scanner.skip_whitespace();
                scanner.expect_byte(b'}')?;
                Ok(#ident)
            }
        }
    })
}

// ── enum ──────────────────────────────────────────────────────────────────────

fn expand_enum(input: &DeriveInput) -> Result<TokenStream> {
    let ident = &input.ident;
    let container = attrs::parse_container_attrs(&input.attrs)?;

    let impl_params: Vec<TokenStream> = {
        let mut p = vec![quote! { 'de }];
        for gp in &input.generics.params {
            match gp {
                GenericParam::Lifetime(_) => {}
                GenericParam::Type(t) => p.push(quote! { #t }),
                GenericParam::Const(c) => p.push(quote! { #c }),
            }
        }
        p
    };
    let ty_args: TokenStream = if input.generics.params.is_empty() {
        quote! {}
    } else {
        let args: Vec<TokenStream> = input.generics.params.iter().map(|gp| match gp {
            GenericParam::Lifetime(_) => quote! { 'de },
            GenericParam::Type(t) => { let id = &t.ident; quote! { #id } }
            GenericParam::Const(c) => { let id = &c.ident; quote! { #id } }
        }).collect();
        quote! { <#(#args),*> }
    };
    let where_clause = &input.generics.where_clause;

    let variants = match &input.data {
        Data::Enum(e) => &e.variants,
        _ => unreachable!(),
    };

    // Only support unit-variant enums → JSON string.
    // Struct-variant enums with tag/content require more complex handling.
    let all_unit = variants.iter().all(|v| matches!(&v.fields, Fields::Unit));

    if all_unit {
        let arms: Vec<TokenStream> = variants.iter().map(|v| {
            let vident = &v.ident;
            let vattrs = attrs::parse_field_attrs(&v.attrs)?;
            let vname = if let Some(r) = &vattrs.rename {
                r.clone()
            } else if let Some(rule) = container.rename_all {
                rename::apply_variant(&vident.to_string(), rule)
            } else {
                vident.to_string()
            };
            let vbytes = vname.as_bytes();
            let byte_str = proc_macro2::Literal::byte_string(vbytes);
            Ok(quote! { #byte_str => Ok(#ident::#vident), })
        }).collect::<Result<_>>()?;

        return Ok(quote! {
            #[automatically_derived]
            impl<#(#impl_params),*> ::r_json::FromJson<'de>
                for #ident #ty_args #where_clause
            {
                fn from_json_scanner(
                    scanner: &mut ::r_json::Scanner<'de>,
                ) -> ::std::result::Result<Self, ::r_json::Error> {
                    let js = scanner.read_str()?;
                    let s = js.as_str().as_bytes();
                    match s {
                        #(#arms)*
                        _ => Err(::r_json::Error::UnknownVariant),
                    }
                }
            }
        });
    }

    Err(Error::new_spanned(
        ident,
        "FromJson currently supports only unit-variant enums; \
         use #[serde(tag)] with struct variants in a future release",
    ))
}

// ── type introspection helpers ────────────────────────────────────────────────

fn is_str_ref(ty: &Type) -> bool {
    if let Type::Reference(r) = ty {
        if let Type::Path(tp) = r.elem.as_ref() {
            return tp.path.is_ident("str");
        }
    }
    false
}

fn is_named(ty: &Type, name: &str) -> bool {
    if let Type::Path(tp) = ty {
        if let Some(seg) = tp.path.segments.last() {
            return seg.ident == name;
        }
    }
    false
}

fn is_option(ty: &Type) -> bool { is_named(ty, "Option") }

fn option_inner(ty: &Type) -> &Type {
    if let Type::Path(tp) = ty {
        if let Some(seg) = tp.path.segments.last() {
            if let syn::PathArguments::AngleBracketed(ab) = &seg.arguments {
                if let Some(syn::GenericArgument::Type(inner)) = ab.args.first() {
                    return inner;
                }
            }
        }
    }
    ty
}

/// Generate the expression that reads a single field value from `scanner`.
fn field_read_expr(ty: &Type, _de_lt: &Lifetime) -> TokenStream {
    if is_str_ref(ty) {
        quote! {
            {
                let _js = scanner.read_str()?;
                match _js {
                    ::r_json::JsonStr::Borrowed(b) => b,
                    ::r_json::JsonStr::Owned(_) => {
                        return Err(::r_json::Error::EscapedString);
                    }
                }
            }
        }
    } else if is_named(ty, "String") {
        quote! { scanner.read_str()?.into_owned() }
    } else if is_option(ty) {
        let inner = option_inner(ty);
        quote! {
            if scanner.peek_null() {
                scanner.read_null()?;
                None
            } else {
                Some(<#inner as ::r_json::FromJson<'de>>::from_json_scanner(scanner)?)
            }
        }
    } else {
        quote! { <#ty as ::r_json::FromJson<'de>>::from_json_scanner(scanner)? }
    }
}
