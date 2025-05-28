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
                let begin = ssecs::component::COMPONENT_ENTRIES[..].as_ptr() as u64;
                let end = &raw const ENTRY as u64;
                unsafe {
                    ssecs::entity::Entity::from_offset(
                        (end - begin) / size_of::<ssecs::component::ComponentEntry>() as u64,
                    )
                }
            }

            fn init(world: &mut ssecs::world::World) {
                world.insert(#struct_name::info(), #struct_name::id());
            }

            fn info() -> ssecs::component::ComponentInfo {
                unsafe {
                    ssecs::component::ComponentInfo::new(
                        std::any::type_name::<#struct_name>(),
                        std::mem::align_of::<#struct_name>(),
                        std::mem::size_of::<#struct_name>(),
                        #struct_name::id(),
                        #struct_name::drop,
                    )
                }
            }

            fn drop(bytes: &mut [std::mem::MaybeUninit<u8>]) {
                unsafe { (bytes.as_ptr() as *mut #struct_name).drop_in_place() }
            }
        }
    };

    output.into()
}
