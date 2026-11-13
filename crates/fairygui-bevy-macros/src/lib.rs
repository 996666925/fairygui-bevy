//! Derive named child bindings for ECS-owned FairyGUI package extensions.
use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, LitStr, parse_macro_input};

/// Bind Entity / Option<Entity> fields through #[gui(path = "child/path")].
/// Other fields use Default. The type requires #[gui(url = "ui://...")];
/// optional on_construct = function_path attaches observers via EntityCommands.
#[proc_macro_derive(GuiExtension, attributes(gui))]
pub fn derive_gui_extension(input: TokenStream) -> TokenStream {
    expand(parse_macro_input!(input as DeriveInput))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn expand(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let mut url: Option<LitStr> = None;
    let mut hook: Option<syn::Path> = None;
    for attribute in input
        .attrs
        .iter()
        .filter(|attribute| attribute.path().is_ident("gui"))
    {
        attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("url") {
                if url.is_some() {
                    return Err(meta.error("duplicate extension url"));
                }
                url = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("on_construct") {
                if hook.is_some() {
                    return Err(meta.error("duplicate on_construct hook"));
                }
                hook = Some(meta.value()?.parse()?);
            } else {
                return Err(meta.error("expected url or on_construct"));
            }
            Ok(())
        })?;
    }
    let url = url.ok_or_else(|| {
        syn::Error::new_spanned(
            &input.ident,
            "GuiExtension requires #[gui(url = \"ui://Package/Item\")]",
        )
    })?;
    if !url.value().starts_with("ui://") || url.value().len() <= 5 {
        return Err(syn::Error::new_spanned(
            url,
            "expected a nonempty ui:// resource URL",
        ));
    }
    let syn::Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "GuiExtension supports structs",
        ));
    };
    let build = match &data.fields {
        syn::Fields::Named(fields) => {
            let mut bindings = Vec::new();
            for field in &fields.named {
                let name = field.ident.as_ref().unwrap();
                let mut path: Option<LitStr> = None;
                for attribute in field
                    .attrs
                    .iter()
                    .filter(|attribute| attribute.path().is_ident("gui"))
                {
                    attribute.parse_nested_meta(|meta| {
                        if !meta.path.is_ident("path") {
                            return Err(meta.error("expected path"));
                        }
                        if path.is_some() {
                            return Err(meta.error("duplicate child path"));
                        }
                        path = Some(meta.value()?.parse()?);
                        Ok(())
                    })?;
                }
                let value = if let Some(path) = path {
                    let optional = matches!(&field.ty, syn::Type::Path(ty) if ty.path.segments.last().is_some_and(|segment| segment.ident == "Option"));
                    if optional {
                        quote!(gui.child_by_path(entity, #path))
                    } else {
                        quote!(gui.child_by_path(entity, #path).ok_or_else(|| ::fairygui_bevy::GuiExtensionError::MissingChild(#path.into()))?)
                    }
                } else {
                    quote!(::core::default::Default::default())
                };
                bindings.push(quote!(#name: #value));
            }
            quote!(Self { #(#bindings),* })
        }
        syn::Fields::Unit => quote!(Self),
        syn::Fields::Unnamed(_) => {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "use named fields or a unit struct for GuiExtension",
            ));
        }
    };
    let hook = hook.map(|hook| {
        quote! {
            fn on_construct(entity: &mut ::bevy::ecs::system::EntityCommands) { #hook(entity); }
        }
    });
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    Ok(quote! {
        impl #impl_generics ::fairygui_bevy::GuiExtension for #name #ty_generics #where_clause {
            const URL: &'static str = #url;
            fn construct(entity: ::bevy::prelude::Entity, gui: &::fairygui_bevy::GuiQuery) -> ::core::result::Result<Self, ::fairygui_bevy::GuiExtensionError> {
                ::core::result::Result::Ok(#build)
            }
            #hook
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_bindings_produce_compile_diagnostics() {
        for source in [
            "struct Missing;",
            "#[gui(url = \"plain\")] struct Bad;",
            "#[gui(url = \"ui://A/B\", url = \"ui://A/C\")] struct Duplicate;",
            "#[gui(url = \"ui://A/B\")] enum Wrong { A }",
            "#[gui(url = \"ui://A/B\")] struct Tuple(u32);",
            "#[gui(url = \"ui://A/B\")] struct WrongField { #[gui(unknown = \"x\")] child: Entity }",
        ] {
            assert!(expand(syn::parse_str(source).unwrap()).is_err(), "{source}");
        }
    }
}
