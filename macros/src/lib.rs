use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{
    DeriveInput, Error, Ident, Meta, Result, Token, parse_macro_input, parse_quote,
    punctuated::Punctuated,
};

#[proc_macro_derive(Component)]
pub fn component_derive(input: TokenStream) -> TokenStream {
    let mut ast = parse_macro_input!(input as DeriveInput);

    ast.generics
        .make_where_clause()
        .predicates
        .push(parse_quote! { Self: Sized + Send + Sync + 'static });

    let struct_name = &ast.ident;
    let (impl_generics, type_generics, where_clause) = &ast.generics.split_for_impl();

    let output = quote! {
        unsafe impl #impl_generics ssecs::component::Component for #struct_name #type_generics
        #where_clause
        {
            fn id() -> ssecs::entity::Entity {
                #[linkme::distributed_slice(ssecs::component::COMPONENT_ENTRIES)]
                static ENTRY: ssecs::component::ComponentEntry = #struct_name::init;
                let begin = ssecs::component::COMPONENT_ENTRIES[..].as_ptr() as u64;
                let end = &raw const ENTRY as u64;
                unsafe {
                    ssecs::entity::Entity::from_offset(
                        (end - begin) / size_of::<ssecs::component::ComponentEntry>() as u64,
                    )
                }
            }

            fn init(_: &ssecs::world::World) {
                // world.component_with_id::<Player>(Player::id())
            }

            fn info() -> ssecs::component::ComponentInfo {
                ssecs::component::ComponentInfo {
                    size: std::mem::size_of::<#struct_name>(),
                    id: #struct_name::id(),
                }
            }
        }
    };

    output.into()
}
