use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, parse_macro_input};

#[proc_macro_attribute]
pub fn main(args: TokenStream, item: TokenStream) -> TokenStream {
    let function = parse_macro_input!(item as ItemFn);
    let signature = &function.sig;
    if !args.is_empty()
        || !signature.inputs.is_empty()
        || signature.asyncness.is_some()
        || matches!(signature.safety, syn::Safety::Unsafe(_))
        || signature.abi.is_some()
        || !signature.generics.params.is_empty()
        || signature.generics.where_clause.is_some()
        || signature.variadic.is_some()
    {
        return syn::Error::new_spanned(signature, "expected a safe synchronous fn main() -> impl reflex::Application, without attribute arguments or generics")
            .to_compile_error().into();
    }
    let name = &signature.ident;
    let startup = include_str!("startup.S");
    quote! {
        #function
        #[cfg(not(all(target_arch = "arm", target_os = "none")))]
        compile_error!("#[reflex::main] requires armv7a-none-eabi");
        core::arch::global_asm!(#startup, options(raw));
        #[global_allocator]
        static __REFLEX_ALLOCATOR: reflex::allocator::SystemAllocator =
            reflex::allocator::SystemAllocator::new();
        #[unsafe(no_mangle)]
        unsafe extern "C" fn __reflex_main(context: *const reflex::app::AppContext) {
            unsafe { reflex::lifecycle::start(context, #name, &__REFLEX_ALLOCATOR); }
        }
        #[panic_handler]
        fn __reflex_panic(info: &core::panic::PanicInfo<'_>) -> ! {
            reflex::lifecycle::panic(info)
        }
    }
    .into()
}
