//! Code-generation for `#[derive(ToJson)]`.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Error, Fields, Result};

use crate::attrs::{self, FieldAttrs};
use crate::rename;

pub fn expand(input: &DeriveInput) -> Result<TokenStream> {
    match &input.data {
        Data::Struct(_) => expand_struct(input),
        Data::Enum(_) => expand_enum(input),
        Data::Union(_) => Err(Error::new_spanned(input, "ToJson does not support unions")),
    }
}

// ── struct ────────────────────────────────────────────────────────────────────

fn expand_struct(input: &DeriveInput) -> Result<TokenStream> {
    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let container = attrs::parse_container_attrs(&input.attrs)?;

    let fields = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(f) => &f.named,
            Fields::Unit => {
                return Ok(quote! {
                    #[automatically_derived]
                    impl #impl_generics ::r_json::ToJson for #ident #ty_generics #where_clause {
                        #[inline(always)]
                        fn json_write(&self, w: &mut ::std::vec::Vec<u8>) {
                            w.extend_from_slice(b"{}");
                        }
                        #[inline(always)]
                        fn json_size_hint(&self) -> usize { 2 }
                    }
                });
            }
            _ => return Err(Error::new_spanned(ident, "ToJson only supports named-field structs")),
        },
        _ => unreachable!(),
    };

    let mut writes: Vec<TokenStream> = Vec::new();
    // `open_brace_fused`: true when the first unconditional field fuses '{' into its key write.
    // When false, we emit `w.push(b'{')` separately before the writes.
    let mut first = true;
    let mut open_brace_fused = false;

    // Accumulate per-field info for json_size_hint generation.
    // Each entry is a TokenStream fragment `overhead + ::r_json::ToJson::json_size_hint(&self.field)`.
    let mut hint_parts: Vec<TokenStream> = Vec::new();
    // Track whether any field is a flatten; those fields can't be counted in the static hint.
    let mut has_flatten = false;

    // Count serializable (non-skipped, non-flatten) fields for inline annotation.
    let mut serializable_field_count = 0usize;

    for field in fields {
        let fname = field.ident.as_ref().unwrap();
        let fattrs: FieldAttrs = attrs::parse_field_attrs(&field.attrs)?;

        if fattrs.skip || fattrs.skip_serializing {
            continue;
        }

        if fattrs.flatten {
            // Flatten: write inner fields directly (stripping outer braces).
            let comma = if first { quote! {} } else { quote! { w.push(b','); } };
            writes.push(quote! {
                #comma
                {
                    let mut _flat_buf = ::std::vec::Vec::new();
                    ::r_json::ToJson::json_write(&self.#fname, &mut _flat_buf);
                    // Strip outer `{` and `}`
                    if _flat_buf.len() > 2 {
                        if !first_written { w.extend_from_slice(&_flat_buf[1.._flat_buf.len()-1]); }
                        else { w.push(b','); w.extend_from_slice(&_flat_buf[1.._flat_buf.len()-1]); }
                    }
                }
            });
            first = false;
            has_flatten = true;
            continue;
        }

        serializable_field_count += 1;

        // Resolve the JSON key name.
        let json_key = if let Some(r) = &fattrs.rename {
            r.clone()
        } else if let Some(rule) = container.rename_all {
            rename::apply(&fname.to_string(), rule)
        } else {
            fname.to_string()
        };

        if first {
            first = false;
            if let Some(predicate) = &fattrs.skip_serializing_if {
                // Conditional first field: cannot fuse '{', emit it separately.
                // Fuse `"key":` as a b"..." literal for single memcpy.
                let key_literal = format!("\"{}\":", json_key);
                let key_lit_bytes = proc_macro2::Literal::byte_string(key_literal.as_bytes());
                // Use a unique const name per field index to avoid shadowing.
                let const_name = proc_macro2::Ident::new(
                    &format!("_K{}", serializable_field_count - 1),
                    proc_macro2::Span::call_site(),
                );
                writes.push(quote! {
                    if !#predicate(&self.#fname) {
                        const #const_name: &[u8] = #key_lit_bytes;
                        w.extend_from_slice(#const_name);
                        ::r_json::ToJson::json_write(&self.#fname, w);
                    }
                });
                // open_brace_fused stays false — '{' is emitted before the loop output.
                // Conditional field: don't count in hint (might be absent).
            } else {
                // Unconditional first field: fuse '{' + '"key":' into a single const byte slice.
                let fused_key = format!("{{\"{}\":", json_key);
                let fused_lit = proc_macro2::Literal::byte_string(fused_key.as_bytes());
                let const_name = proc_macro2::Ident::new(
                    &format!("_K{}", serializable_field_count - 1),
                    proc_macro2::Span::call_site(),
                );
                writes.push(quote! {
                    {
                        const #const_name: &[u8] = #fused_lit;
                        w.extend_from_slice(#const_name);
                        ::r_json::ToJson::json_write(&self.#fname, w);
                    }
                });
                open_brace_fused = true;
                // key overhead: `{"key":` length (covers the opening brace + key + colon).
                // We count braces as 2 separately in `hint`, and this overhead is for the
                // fused slice minus the `{` (1 byte), so overhead = key_len + 3 (`"` + key + `":`).
                // But since we fuse `{` into _K0 and count `2usize` for braces already, we must
                // NOT double-count. The fused key `{"key":` has length = 1 + 1 + key_len + 2 = key_len+4.
                // The `2usize` already accounts for both braces, so we only add key_overhead = key_len+3
                // (the `"key":` portion without the `{`).
                let key_overhead: usize = json_key.len() + 3; // `"` + key + `":`
                hint_parts.push(quote! {
                    #key_overhead + ::r_json::ToJson::json_size_hint(&self.#fname)
                });
            }
        } else {
            // Subsequent fields: fuse ',' + '"key":' into a single const byte slice.
            if let Some(predicate) = &fattrs.skip_serializing_if {
                let fused_key = format!(",\"{}\":", json_key);
                let fused_lit = proc_macro2::Literal::byte_string(fused_key.as_bytes());
                let const_name = proc_macro2::Ident::new(
                    &format!("_K{}", serializable_field_count - 1),
                    proc_macro2::Span::call_site(),
                );
                // Single predicate evaluation; no double-evaluation of self.field.
                writes.push(quote! {
                    if !#predicate(&self.#fname) {
                        const #const_name: &[u8] = #fused_lit;
                        w.extend_from_slice(#const_name);
                        ::r_json::ToJson::json_write(&self.#fname, w);
                    }
                });
                // Conditional field: don't count in hint (might be absent).
            } else {
                let fused_key = format!(",\"{}\":", json_key);
                let fused_lit = proc_macro2::Literal::byte_string(fused_key.as_bytes());
                let const_name = proc_macro2::Ident::new(
                    &format!("_K{}", serializable_field_count - 1),
                    proc_macro2::Span::call_site(),
                );
                writes.push(quote! {
                    {
                        const #const_name: &[u8] = #fused_lit;
                        w.extend_from_slice(#const_name);
                        ::r_json::ToJson::json_write(&self.#fname, w);
                    }
                });
                // key overhead: `,"key":` = 1 + 1 + key_len + 2 = key_len + 4
                let key_overhead: usize = json_key.len() + 4;
                hint_parts.push(quote! {
                    #key_overhead + ::r_json::ToJson::json_size_hint(&self.#fname)
                });
            }
        }
    }

    // Emit opening brace explicitly when not fused (empty struct, conditional first field, or
    // first field was flatten).
    let open_brace = if open_brace_fused {
        quote! {}
    } else {
        quote! { w.push(b'{'); }
    };

    // Build json_size_hint: 2 (braces) + sum of field contributions.
    // When there are flatten fields or no hint parts, fall back to a generous default.
    let size_hint_impl = if has_flatten || hint_parts.is_empty() {
        quote! {
            #[inline]
            fn json_size_hint(&self) -> usize { 256 }
        }
    } else {
        quote! {
            #[inline]
            fn json_size_hint(&self) -> usize {
                2usize #(+ #hint_parts)*
            }
        }
    };

    // Choose inline annotation based on number of serializable fields.
    let inline_attr = match serializable_field_count {
        0..=4  => quote! { #[inline(always)] },
        5..=16 => quote! { #[inline] },
        _      => quote! {},
    };

    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics ::r_json::ToJson for #ident #ty_generics #where_clause {
            #inline_attr
            fn json_write(&self, w: &mut ::std::vec::Vec<u8>) {
                #open_brace
                #(#writes)*
                w.push(b'}');
            }

            #size_hint_impl
        }
    })
}

// ── enum ──────────────────────────────────────────────────────────────────────

fn expand_enum(input: &DeriveInput) -> Result<TokenStream> {
    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let container = attrs::parse_container_attrs(&input.attrs)?;

    let variants = match &input.data {
        Data::Enum(e) => &e.variants,
        _ => unreachable!(),
    };

    // Decide tagging strategy.
    let tag = container.tag.as_deref();
    let content = container.content.as_deref();
    let untagged = container.untagged;

    let arms: Vec<TokenStream> = variants
        .iter()
        .map(|v| {
            let vident = &v.ident;
            let vattrs = attrs::parse_field_attrs(&v.attrs)?;

            let variant_name = if let Some(r) = &vattrs.rename {
                r.clone()
            } else if let Some(rule) = container.rename_all {
                rename::apply_variant(&vident.to_string(), rule)
            } else {
                vident.to_string()
            };

            match &v.fields {
                Fields::Unit => {
                    // Unit variant → JSON string representation.
                    // Use b"..." literal so LLVM sees a static string reference.
                    let quoted_name = format!("\"{}\"", variant_name);
                    let vname_lit = proc_macro2::Literal::byte_string(quoted_name.as_bytes());
                    Ok(quote! {
                        Self::#vident => w.extend_from_slice(#vname_lit),
                    })
                }
                Fields::Named(f) => {
                    let field_writes = build_variant_field_writes(
                        f.named.iter(),
                        &container,
                        true,
                    )?;
                    let arm = if let Some(tag_key) = tag {
                        if let Some(content_key) = content {
                            // Adjacently tagged: {"tag":"Name","content":{…}}
                            let tag_payload = format!("{{\"{}\":\"{}\",\"{}\":", tag_key, variant_name, content_key);
                            let tag_lit = proc_macro2::Literal::byte_string(tag_payload.as_bytes());
                            let field_names: Vec<&syn::Ident> = f.named.iter().map(|f| f.ident.as_ref().unwrap()).collect();
                            quote! {
                                Self::#vident { #(#field_names),* } => {
                                    w.extend_from_slice(#tag_lit);
                                    w.push(b'{');
                                    #(#field_writes)*
                                    w.extend_from_slice(b"}}");
                                }
                            }
                        } else {
                            // Internally tagged: {"tag":"Name","field":…}
                            let tag_payload = format!("{{\"{}\":\"{}\",", tag_key, variant_name);
                            let tag_lit = proc_macro2::Literal::byte_string(tag_payload.as_bytes());
                            let field_names: Vec<&syn::Ident> = f.named.iter().map(|f| f.ident.as_ref().unwrap()).collect();
                            quote! {
                                Self::#vident { #(#field_names),* } => {
                                    w.extend_from_slice(#tag_lit);
                                    #(#field_writes)*
                                    w.push(b'}');
                                }
                            }
                        }
                    } else if untagged {
                        let field_names: Vec<&syn::Ident> = f.named.iter().map(|f| f.ident.as_ref().unwrap()).collect();
                        quote! {
                            Self::#vident { #(#field_names),* } => {
                                w.push(b'{');
                                #(#field_writes)*
                                w.push(b'}');
                            }
                        }
                    } else {
                        // Externally tagged: {"VariantName":{…}}
                        let tag_payload = format!("{{\"{}\":{{", variant_name);
                        let tag_lit = proc_macro2::Literal::byte_string(tag_payload.as_bytes());
                        let field_names: Vec<&syn::Ident> = f.named.iter().map(|f| f.ident.as_ref().unwrap()).collect();
                        quote! {
                            Self::#vident { #(#field_names),* } => {
                                w.extend_from_slice(#tag_lit);
                                #(#field_writes)*
                                w.extend_from_slice(b"}}");
                            }
                        }
                    };
                    Ok(arm)
                }
                Fields::Unnamed(_) => Err(Error::new_spanned(
                    vident,
                    "ToJson does not support tuple enum variants",
                )),
            }
        })
        .collect::<Result<_>>()?;

    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics ::r_json::ToJson for #ident #ty_generics #where_clause {
            fn json_write(&self, w: &mut ::std::vec::Vec<u8>) {
                match self {
                    #(#arms)*
                }
            }
        }
    })
}

fn build_variant_field_writes<'a>(
    fields: impl Iterator<Item = &'a syn::Field>,
    container: &attrs::ContainerAttrs,
    _is_named: bool,
) -> Result<Vec<TokenStream>> {
    let mut writes = Vec::new();
    let mut first = true;
    let mut field_idx = 0usize;
    for field in fields {
        let fname = field.ident.as_ref().unwrap();
        let fattrs = attrs::parse_field_attrs(&field.attrs)?;
        if fattrs.skip || fattrs.skip_serializing { continue; }

        let json_key = if let Some(r) = &fattrs.rename {
            r.clone()
        } else if let Some(rule) = container.rename_all {
            rename::apply(&fname.to_string(), rule)
        } else {
            fname.to_string()
        };

        // Fuse comma (or none for first) + `"key":` into a single b"..." literal.
        let fused_key = if first {
            first = false;
            format!("\"{}\":", json_key)
        } else {
            format!(",\"{}\":", json_key)
        };
        let fused_lit = proc_macro2::Literal::byte_string(fused_key.as_bytes());
        let const_name = proc_macro2::Ident::new(
            &format!("_VK{}", field_idx),
            proc_macro2::Span::call_site(),
        );
        field_idx += 1;

        writes.push(quote! {
            {
                const #const_name: &[u8] = #fused_lit;
                w.extend_from_slice(#const_name);
                ::r_json::ToJson::json_write(#fname, w);
            }
        });
    }
    Ok(writes)
}
