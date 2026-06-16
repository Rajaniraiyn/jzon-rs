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

fn expand_struct(input: &DeriveInput) -> Result<TokenStream> {
    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let container = attrs::parse_container_attrs(&input.attrs)?;

    let fields = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(f) => {
                if container.transparent {
                    let active: Vec<_> = f.named.iter().filter(|field| {
                        let fa = attrs::parse_field_attrs(&field.attrs).unwrap_or_default();
                        !fa.skip && !fa.skip_serializing
                    }).collect();
                    if active.len() != 1 {
                        return Err(Error::new_spanned(
                            ident,
                            "#[serde(transparent)] requires exactly one non-skipped field",
                        ));
                    }
                    let single = active[0].ident.as_ref().unwrap();
                    return Ok(quote! {
                        #[automatically_derived]
                        impl #impl_generics ::jzon::ToJson for #ident #ty_generics #where_clause {
                            #[inline(always)]
                            fn json_write(&self, w: &mut ::std::vec::Vec<u8>) {
                                ::jzon::ToJson::json_write(&self.#single, w);
                            }
                            #[inline(always)]
                            fn json_size_hint(&self) -> usize {
                                ::jzon::ToJson::json_size_hint(&self.#single)
                            }
                        }
                    });
                }
                &f.named
            }
            Fields::Unit => {
                return Ok(quote! {
                    #[automatically_derived]
                    impl #impl_generics ::jzon::ToJson for #ident #ty_generics #where_clause {
                        #[inline(always)]
                        fn json_write(&self, w: &mut ::std::vec::Vec<u8>) {
                            w.extend_from_slice(b"{}");
                        }
                        #[inline(always)]
                        fn json_size_hint(&self) -> usize { 2 }
                    }
                });
            }
            Fields::Unnamed(f) => {
                let n = f.unnamed.len();
                if n == 0 {
                    return Ok(quote! {
                        #[automatically_derived]
                        impl #impl_generics ::jzon::ToJson for #ident #ty_generics #where_clause {
                            #[inline(always)]
                            fn json_write(&self, w: &mut ::std::vec::Vec<u8>) {
                                w.extend_from_slice(b"[]");
                            }
                            #[inline(always)]
                            fn json_size_hint(&self) -> usize { 2 }
                        }
                    });
                }
                if n == 1 {
                    return Ok(quote! {
                        #[automatically_derived]
                        impl #impl_generics ::jzon::ToJson for #ident #ty_generics #where_clause {
                            #[inline(always)]
                            fn json_write(&self, w: &mut ::std::vec::Vec<u8>) {
                                ::jzon::ToJson::json_write(&self.0, w);
                            }
                            #[inline(always)]
                            fn json_size_hint(&self) -> usize {
                                ::jzon::ToJson::json_size_hint(&self.0)
                            }
                        }
                    });
                }
                let indices: Vec<syn::Index> = (0..n).map(syn::Index::from).collect();
                let first_idx = &indices[0];
                let rest_writes: Vec<TokenStream> = indices[1..].iter().map(|i| {
                    quote! {
                        w.push(b',');
                        ::jzon::ToJson::json_write(&self.#i, w);
                    }
                }).collect();
                let hint_parts: Vec<TokenStream> = indices.iter().map(|i| {
                    quote! { ::jzon::ToJson::json_size_hint(&self.#i) }
                }).collect();
                let _n_commas = n - 1;
                return Ok(quote! {
                    #[automatically_derived]
                    impl #impl_generics ::jzon::ToJson for #ident #ty_generics #where_clause {
                        #[inline]
                        fn json_write(&self, w: &mut ::std::vec::Vec<u8>) {
                            w.push(b'[');
                            ::jzon::ToJson::json_write(&self.#first_idx, w);
                            #(#rest_writes)*
                            w.push(b']');
                        }
                        #[inline]
                        fn json_size_hint(&self) -> usize {
                            2usize + #(#hint_parts)+* + (#n - 1)
                        }
                    }
                });
            }
        },
        _ => unreachable!(),
    };

    let mut writes: Vec<TokenStream> = Vec::new();
    let mut first = true;
    let mut open_brace_fused = false;
    // Compile-time key overhead: sum of all key-related constant bytes.
    // Tracks the known-at-compile-time portion of the output size:
    //   `{` (1) + `}` (1) + per non-skipped, non-conditional field: `"key":` (key.len()+3)
    //   + separating commas between always-present fields (counted below).
    let mut compile_time_key_overhead: usize = 2; // `{` + `}`
    // Runtime field size hints (only for fields that are always serialized).
    let mut runtime_hint_parts: Vec<TokenStream> = Vec::new();
    let mut serializable_field_count = 0usize;
    // Track whether we have at least one always-present field for comma logic.
    let mut always_present_count = 0usize;

    for field in fields {
        let fname = field.ident.as_ref().unwrap();
        let fattrs: FieldAttrs = attrs::parse_field_attrs(&field.attrs)?;

        if fattrs.skip || fattrs.skip_serializing {
            continue;
        }

        if fattrs.flatten {
            return Err(Error::new_spanned(
                &field.ident,
                "#[serde(flatten)] is not yet supported by jzon ToJson; use jzon_serde (Mode B)",
            ));
        }

        serializable_field_count += 1;

        let json_key = if let Some(r) = &fattrs.rename {
            r.clone()
        } else if let Some(rule) = container.rename_all {
            rename::apply(&fname.to_string(), rule)
        } else {
            fname.to_string()
        };

        // Build the value-writing expression: custom path or default ToJson.
        let write_value: TokenStream = if let Some(path) = &fattrs.serialize_with {
            quote! { #path(&self.#fname, w); }
        } else {
            quote! { ::jzon::ToJson::json_write(&self.#fname, w); }
        };

        if first {
            first = false;
            if let Some(predicate) = &fattrs.skip_serializing_if {
                let key_literal = format!("\"{}\":", json_key);
                let key_lit_bytes = proc_macro2::Literal::byte_string(key_literal.as_bytes());
                let const_name = proc_macro2::Ident::new(
                    &format!("_K{}", serializable_field_count - 1),
                    proc_macro2::Span::call_site(),
                );
                writes.push(quote! {
                    if !#predicate(&self.#fname) {
                        const #const_name: &[u8] = #key_lit_bytes;
                        w.extend_from_slice(#const_name);
                        #write_value
                    }
                });
                // Conditional field: does not contribute to compile-time overhead
                // (it may or may not appear). Skip it in size hint.
            } else {
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
                        #write_value
                    }
                });
                open_brace_fused = true;
                // `"key":` overhead: 1 (`"`) + key.len() + 2 (`":`) = key.len() + 3
                if fattrs.serialize_with.is_none() {
                    compile_time_key_overhead += json_key.len() + 3;
                    always_present_count += 1;
                    runtime_hint_parts.push(quote! {
                        ::jzon::ToJson::json_size_hint(&self.#fname)
                    });
                }
            }
        } else {
            if let Some(predicate) = &fattrs.skip_serializing_if {
                let fused_key = format!(",\"{}\":", json_key);
                let fused_lit = proc_macro2::Literal::byte_string(fused_key.as_bytes());
                let const_name = proc_macro2::Ident::new(
                    &format!("_K{}", serializable_field_count - 1),
                    proc_macro2::Span::call_site(),
                );
                writes.push(quote! {
                    if !#predicate(&self.#fname) {
                        const #const_name: &[u8] = #fused_lit;
                        w.extend_from_slice(#const_name);
                        #write_value
                    }
                });
                // Conditional field: does not contribute to compile-time overhead.
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
                        #write_value
                    }
                });
                // `,"key":` overhead: 1 (`,`) + 1 (`"`) + key.len() + 2 (`":`) = key.len() + 4
                if fattrs.serialize_with.is_none() {
                    compile_time_key_overhead += json_key.len() + 4;
                    always_present_count += 1;
                    runtime_hint_parts.push(quote! {
                        ::jzon::ToJson::json_size_hint(&self.#fname)
                    });
                }
            }
        }
    }

    let open_brace = if open_brace_fused {
        quote! {}
    } else {
        quote! { w.push(b'{'); }
    };

    let size_hint_impl = if always_present_count == 0 {
        // All fields are either skipped or conditional: fall back to a fixed
        // conservative estimate so we still pre-allocate something reasonable.
        quote! {
            #[inline]
            fn json_size_hint(&self) -> usize { 256 }
        }
    } else {
        // Emit the compile-time key overhead as a named constant so the
        // compiler can fold it into a single addend rather than recomputing
        // it at every call site.
        quote! {
            #[inline]
            fn json_size_hint(&self) -> usize {
                // KEY_OVERHEAD is a compile-time constant: `{` + `}` +
                // sum of `"key":` (and leading `,`) lengths for every
                // always-serialized field.
                const KEY_OVERHEAD: usize = #compile_time_key_overhead;
                KEY_OVERHEAD #(+ #runtime_hint_parts)*
            }
        }
    };

    let inline_attr = match serializable_field_count {
        0..=4  => quote! { #[inline(always)] },
        5..=16 => quote! { #[inline] },
        _      => quote! {},
    };

    Ok(quote! {
        #[automatically_derived]
        impl #impl_generics ::jzon::ToJson for #ident #ty_generics #where_clause {
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

fn expand_enum(input: &DeriveInput) -> Result<TokenStream> {
    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    let container = attrs::parse_container_attrs(&input.attrs)?;

    let variants = match &input.data {
        Data::Enum(e) => &e.variants,
        _ => unreachable!(),
    };

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
        impl #impl_generics ::jzon::ToJson for #ident #ty_generics #where_clause {
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
                ::jzon::ToJson::json_write(#fname, w);
            }
        });
    }
    Ok(writes)
}
