//! The `farhelm_testtrace::test` attribute keeps libtest's public contract
//! while moving per-test tracing and Tokio ownership into the support crate.
//!
//! It deliberately recognizes only the Tokio test options used by Farhelm.
//! Accepting and then ignoring another test macro's option would make a test
//! look configured while running under different semantics.
//! Rust resolves `cfg_attr` before invoking this attribute. The effective
//! `should_panic` declaration therefore supplies both libtest and capture metadata;
//! integration contracts exercise enabled and disabled conditions.

use proc_macro::TokenStream;
use quote::quote;
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{
    Attribute, Error, ItemFn, LitBool, LitInt, LitStr, Meta, Result, Token, parse_macro_input,
};

/// Expands a synchronous or Tokio-style test into a test-owned capture scope.
#[proc_macro_attribute]
pub fn test(args: TokenStream, item: TokenStream) -> TokenStream {
    match expand(args.into(), parse_macro_input!(item as ItemFn)) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.into_compile_error().into(),
    }
}

/// Parsed Tokio options, kept separate from syntax so validation precedes code generation.
#[derive(Default)]
struct RuntimeOptions {
    /// Distinguishes an explicit current-thread flavor from the implicit default.
    flavor_seen: bool,
    /// Selects Tokio's worker scheduler when true.
    multi_thread: bool,
    /// Optional fixed worker count accepted only for the worker scheduler.
    worker_threads: Option<usize>,
    /// Optional clock declaration, including an explicit false value.
    start_paused: Option<bool>,
}

/// Preserves the outer item contract while replacing only runtime and capture ownership.
fn expand(args: proc_macro2::TokenStream, function: ItemFn) -> Result<proc_macro2::TokenStream> {
    let is_async = function.sig.asyncness.is_some();
    let expected_panic = expected_panic_tokens(&function.attrs)?;
    reject_other_test_attributes(&function.attrs)?;
    reject_function_shape(&function)?;

    let runtime = parse_options(args, is_async)?;
    let ident = &function.sig.ident;
    let attrs = &function.attrs;
    let visibility = &function.vis;
    let mut signature = function.sig.clone();
    let body = &function.block;

    if is_async {
        signature.asyncness = None;
        let runtime = runtime_tokens(&runtime);
        Ok(quote! {
            #(#attrs)*
            #[::core::prelude::v1::test]
            #visibility #signature {
                ::farhelm_testtrace::run_async(concat!(module_path!(), "::", stringify!(#ident)), #expected_panic, #runtime, async move #body)
            }
        })
    } else {
        Ok(quote! {
            #(#attrs)*
            #[::core::prelude::v1::test]
            #visibility #signature {
                ::farhelm_testtrace::run_sync(concat!(module_path!(), "::", stringify!(#ident)), #expected_panic, || #body)
            }
        })
    }
}

/// Rejects shapes libtest cannot call as a zero-argument ordinary test function.
fn reject_function_shape(function: &ItemFn) -> Result<()> {
    if !function.sig.inputs.is_empty() {
        return Err(Error::new_spanned(
            &function.sig.inputs,
            "farhelm_testtrace::test functions cannot take parameters",
        ));
    }
    if function.sig.constness.is_some()
        || function.sig.unsafety.is_some()
        || function.sig.abi.is_some()
    {
        return Err(Error::new_spanned(
            &function.sig,
            "farhelm_testtrace::test requires an ordinary safe function",
        ));
    }
    if !function.sig.generics.params.is_empty() || function.sig.generics.where_clause.is_some() {
        return Err(Error::new_spanned(
            &function.sig.generics,
            "farhelm_testtrace::test functions cannot be generic",
        ));
    }
    Ok(())
}

/// Rejects any direct or conditionally applied test macro whose semantics would be ambiguous.
fn reject_other_test_attributes(attrs: &[Attribute]) -> Result<()> {
    for attr in attrs {
        if meta_contains_test(&attr.meta)? {
            return Err(Error::new_spanned(
                attr,
                "farhelm_testtrace::test replaces #[test] or #[tokio::test]; do not stack test attributes",
            ));
        }
    }
    Ok(())
}

/// Finds nested test attributes inside `cfg_attr` without interpreting its condition.
fn meta_contains_test(meta: &Meta) -> Result<bool> {
    meta_contains_named(meta, "test")
}

/// Finds one named attribute inside recursively nested `cfg_attr` declarations.
fn meta_contains_named(meta: &Meta, name: &str) -> Result<bool> {
    if meta
        .path()
        .segments
        .last()
        .is_some_and(|segment| segment.ident == name)
    {
        return Ok(true);
    }
    if !meta.path().is_ident("cfg_attr") {
        return Ok(false);
    }
    let nested = match meta {
        Meta::List(list) => {
            list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?
        }
        _ => return Ok(false),
    };
    for candidate in nested.iter().skip(1) {
        if meta_contains_named(candidate, name)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Preserves whether `should_panic` is bare or carries an expected substring.
fn expected_panic_tokens(attrs: &[Attribute]) -> Result<proc_macro2::TokenStream> {
    let Some(attribute) = attrs
        .iter()
        .find(|attribute| attribute.path().is_ident("should_panic"))
    else {
        return Ok(quote!(::farhelm_testtrace::ExpectedPanic::None));
    };
    match &attribute.meta {
        Meta::Path(_) => Ok(quote!(::farhelm_testtrace::ExpectedPanic::Any)),
        Meta::List(list) => {
            let mut expected = None;
            list.parse_nested_meta(|meta| {
                if !meta.path.is_ident("expected") {
                    return Err(meta.error("should_panic supports only expected = \"...\""));
                }
                if expected.is_some() {
                    return Err(meta.error("duplicate should_panic expected value"));
                }
                expected = Some(meta.value()?.parse::<LitStr>()?);
                Ok(())
            })?;
            if let Some(expected) = expected {
                Ok(quote!(::farhelm_testtrace::ExpectedPanic::Expected(
                    ::std::borrow::Cow::Borrowed(#expected)
                )))
            } else {
                Ok(quote!(::farhelm_testtrace::ExpectedPanic::Any))
            }
        }
        Meta::NameValue(_) => Err(Error::new_spanned(
            attribute,
            "should_panic must be bare or use expected = \"...\"",
        )),
    }
}

/// Parses the deliberately small Tokio option surface and rejects semantic conflicts.
fn parse_options(args: proc_macro2::TokenStream, is_async: bool) -> Result<RuntimeOptions> {
    if args.is_empty() {
        return Ok(RuntimeOptions::default());
    }
    if !is_async {
        return Err(Error::new_spanned(
            args,
            "synchronous farhelm_testtrace::test functions do not accept runtime options",
        ));
    }
    let mut options = RuntimeOptions::default();
    syn::meta::parser(|meta| {
        if meta.path.is_ident("flavor") {
            if options.flavor_seen {
                return Err(meta.error("duplicate flavor option"));
            }
            options.flavor_seen = true;
            let value: LitStr = meta.value()?.parse()?;
            match value.value().as_str() {
                "current_thread" => {}
                "multi_thread" => options.multi_thread = true,
                _ => return Err(meta.error(
                    "farhelm_testtrace::test supports only current_thread or multi_thread flavor",
                )),
            }
        } else if meta.path.is_ident("worker_threads") {
            if options.worker_threads.is_some() {
                return Err(meta.error("duplicate worker_threads option"));
            }
            let value: LitInt = meta.value()?.parse()?;
            let workers = value.base10_parse()?;
            if workers == 0 {
                return Err(meta.error("worker_threads must be at least one"));
            }
            options.worker_threads = Some(workers);
        } else if meta.path.is_ident("start_paused") {
            if options.start_paused.is_some() {
                return Err(meta.error("duplicate start_paused option"));
            }
            let value: LitBool = meta.value()?.parse()?;
            options.start_paused = Some(value.value);
        } else {
            return Err(meta.error("unsupported farhelm_testtrace::test option"));
        }
        Ok(())
    })
    .parse2(args.clone())?;
    if options.worker_threads.is_some() && !options.multi_thread {
        return Err(Error::new_spanned(
            args,
            "worker_threads requires flavor = \"multi_thread\"",
        ));
    }
    if options.start_paused.is_some_and(|paused| paused) && options.multi_thread {
        return Err(Error::new_spanned(
            args,
            "start_paused = true requires the current_thread flavor",
        ));
    }
    Ok(options)
}

/// Lowers validated source options to the support crate's runtime configuration.
fn runtime_tokens(options: &RuntimeOptions) -> proc_macro2::TokenStream {
    let flavor = if options.multi_thread {
        quote!(::farhelm_testtrace::RuntimeFlavor::MultiThread)
    } else {
        quote!(::farhelm_testtrace::RuntimeFlavor::CurrentThread)
    };
    let workers = match options.worker_threads {
        Some(workers) => quote!(Some(#workers)),
        None => quote!(None),
    };
    let paused = options.start_paused.unwrap_or(false);
    quote!(::farhelm_testtrace::RuntimeConfig { flavor: #flavor, worker_threads: #workers, start_paused: #paused })
}
