use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Expr, ExprRange, Fields, Lit};

#[proc_macro_derive(Inspectable, attributes(component, inspect))]
pub fn derive_inspectable(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = &input.ident;

    let mut category = "General".to_string();
    for attr in &input.attrs {
        if attr.path().is_ident("component") {
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("category") {
                    let value = meta.value()?;
                    let lit: Lit = value.parse()?;
                    if let Lit::Str(s) = lit {
                        category = s.value();
                    }
                }
                Ok(())
            });
        }
    }

    let mut field_reflect_calls = Vec::new();

    if let Data::Struct(data_struct) = &input.data {
        if let Fields::Named(fields_named) = &data_struct.fields {
            for field in &fields_named.named {
                let field_name = &field.ident;
                let field_str = field_name.as_ref().unwrap().to_string();

                let mut min_expr = quote! { None };
                let mut max_expr = quote! { None };
                let mut speed_expr = quote! { None };
                let mut is_color = false;
                let mut skip = false;

                for attr in &field.attrs {
                    if attr.path().is_ident("inspect") {
                        let _ = attr.parse_nested_meta(|meta| {
                            if meta.path.is_ident("skip") {
                                skip = true;
                            } else if meta.path.is_ident("color") {
                                is_color = true;
                            } else if meta.path.is_ident("speed") {
                                let val = meta.value()?;
                                let expr: Expr = val.parse()?;
                                speed_expr = quote! { Some(#expr as f32) };
                            } else if meta.path.is_ident("range") {
                                let val = meta.value()?;
                                let expr: Expr = val.parse()?;
                                if let Expr::Range(ExprRange { start, end, .. }) = expr {
                                    if let Some(s) = start {
                                        min_expr = quote! { Some(#s as f32) };
                                    }
                                    if let Some(e) = end {
                                        max_expr = quote! { Some(#e as f32) };
                                    }
                                }
                            }
                            Ok(())
                        });
                    }
                }

                if !skip {
                    field_reflect_calls.push(quote! {
                        {
                            let meta = ::ecs::FieldMeta {
                                range: match (#min_expr, #max_expr) {
                                    (Some(min), Some(max)) => Some((min, max)),
                                    _ => None,
                                },
                                speed: #speed_expr,
                                is_color: #is_color,
                            };
                            ::ecs::ReflectField::reflect_field(
                                &mut self.#field_name,
                                #field_str,
                                &meta,
                                visitor
                            );
                        }
                    });
                }
            }
        }
    }

    let expanded = quote! {
        impl ::ecs::Inspectable for #struct_name {
            const NAME: &'static str = stringify!(#struct_name);
            const CATEGORY: &'static str = #category;

            fn inspect_fields(&mut self, visitor: &mut dyn ::ecs::FieldVisitor) {
                #(#field_reflect_calls)*
            }
        }

        ::ecs::inventory::submit! {
            ::ecs::ComponentRegistryEntry {
                name: stringify!(#struct_name),
                category: #category,
                register_fn: |world| {
                    world.register_component::<#struct_name>();
                },
            }
        }
    };

    TokenStream::from(expanded)
}