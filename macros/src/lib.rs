use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input, parse_quote};

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
                let begin = ssecs::component::COMPONENT_ENTRIES[..].as_ptr() as u32;
                let end = &raw const ENTRY as u32;
                unsafe {
                    ssecs::entity::Entity::from_offset(
                        (end - begin) / size_of::<ssecs::component::ComponentEntry>() as u32,
                    )
                }
            }

            fn init(world: &ssecs::world::World) {
                world.entity(#struct_name::id()).insert(#struct_name::info());
            }

            fn info() -> ssecs::component::ComponentInfo {
                unsafe {
                    ssecs::component::ComponentInfo {
                        name: std::any::type_name::<#struct_name>(),
                        align: std::mem::align_of::<#struct_name>(),
                        size: std::mem::size_of::<#struct_name>(),
                        id: #struct_name::id(),
                        clone: #struct_name::get_erased_clone(),
                        default: #struct_name::get_erased_default(),
                        drop: #struct_name::erased_drop,
                        on_insert: #struct_name::get_on_insert(),
                        on_remove: #struct_name::get_on_remove(),
                    }
                }
            }
        }
    };

    output.into()
}
