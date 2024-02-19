use std::str::FromStr;

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use quote::quote_spanned;
use quote::ToTokens;
use syn::__private::TokenStream2;
use syn::parse::Parser;
use syn::ItemFn;

fn get_runtime_name() -> &'static str {
    if cfg!(feature = "tokio") {
        "tokio"
    } else if cfg!(feature = "monoio") {
        "monoio"
    } else {
        "tokio"
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum RuntimeFlavor {
    CurrentThread,
    Threaded,
}

impl RuntimeFlavor {
    fn from_str(s: &str) -> Result<RuntimeFlavor, String> {
        match s {
            "current_thread" => Ok(RuntimeFlavor::CurrentThread),
            "multi_thread" => Ok(RuntimeFlavor::Threaded),
            _ => Err(format!(
                "No such runtime flavor `{}`. The runtime flavors are `current_thread` and `multi_thread`.",
                s
            )),
        }
    }
}

#[derive(Debug)]
struct FinalConfig {
    flavor: RuntimeFlavor,
    worker_threads: Option<usize>,
    start_paused: Option<bool>,
    init: Option<(String, Span)>,
    tracing_span: Option<(String, Span)>,
    tracing_lib: Option<(String, Span)>,
}

struct Configuration {
    rt_multi_thread_available: bool,
    default_flavor: RuntimeFlavor,
    flavor: Option<RuntimeFlavor>,
    worker_threads: Option<(usize, Span)>,
    start_paused: Option<(bool, Span)>,
    is_test: bool,
    init: Option<(String, Span)>,
    tracing_span: Option<(String, Span)>,

    /// Import `tracing` and `tracing_future` in another crate or mod, e.g. `tracing_lib::tracing`, instead of using `tracing`.
    tracing_lib: Option<(String, Span)>,
}

impl Configuration {
    fn new(is_test: bool, rt_multi_thread: bool) -> Self {
        Configuration {
            rt_multi_thread_available: rt_multi_thread,
            default_flavor: match is_test {
                true => RuntimeFlavor::CurrentThread,
                false => RuntimeFlavor::Threaded,
            },
            flavor: None,
            worker_threads: None,
            start_paused: None,
            is_test,
            init: None,
            tracing_span: None,
            tracing_lib: None,
        }
    }

    fn set_flavor(&mut self, runtime: syn::Lit, span: Span) -> Result<(), syn::Error> {
        if self.flavor.is_some() {
            return Err(syn::Error::new(span, "`flavor` set multiple times."));
        }

        let runtime_str = parse_string(runtime, span, "flavor")?;
        let runtime = RuntimeFlavor::from_str(&runtime_str).map_err(|err| syn::Error::new(span, err))?;
        self.flavor = Some(runtime);
        Ok(())
    }

    fn set_init(&mut self, init_fn: syn::Lit, span: Span) -> Result<(), syn::Error> {
        if self.init.is_some() {
            return Err(syn::Error::new(span, "`init` set multiple times."));
        }

        let init_expr = parse_string(init_fn, span, "init")?;
        self.init = Some((init_expr, span));

        Ok(())
    }

    fn set_tracing_span(&mut self, level: syn::Lit, span: Span) -> Result<(), syn::Error> {
        if self.tracing_span.is_some() {
            return Err(syn::Error::new(span, "`tracing_span` set multiple times."));
        }

        let tracing_span = parse_string(level, span, "tracing_span")?;
        self.tracing_span = Some((tracing_span, span));

        Ok(())
    }

    // TODO: test internal, test-async-entry
    fn set_tracing_lib(&mut self, level: syn::Lit, span: Span) -> Result<(), syn::Error> {
        if self.tracing_lib.is_some() {
            return Err(syn::Error::new(span, "`tracing_lib` set multiple times."));
        }

        let tracing_lib = parse_string(level, span, "tracing_lib")?;
        self.tracing_lib = Some((tracing_lib, span));

        Ok(())
    }

    fn set_worker_threads(&mut self, worker_threads: syn::Lit, span: Span) -> Result<(), syn::Error> {
        if self.worker_threads.is_some() {
            return Err(syn::Error::new(span, "`worker_threads` set multiple times."));
        }

        let worker_threads = parse_int(worker_threads, span, "worker_threads")?;
        if worker_threads == 0 {
            self.flavor = Some(RuntimeFlavor::CurrentThread);
            self.worker_threads = None;
        } else {
            self.flavor = Some(RuntimeFlavor::Threaded);
            self.worker_threads = Some((worker_threads, span));
        }

        Ok(())
    }

    fn set_start_paused(&mut self, start_paused: syn::Lit, span: Span) -> Result<(), syn::Error> {
        if self.start_paused.is_some() {
            return Err(syn::Error::new(span, "`start_paused` set multiple times."));
        }

        let start_paused = parse_bool(start_paused, span, "start_paused")?;
        self.start_paused = Some((start_paused, span));
        Ok(())
    }

    fn macro_name(&self) -> &'static str {
        if self.is_test {
            match get_runtime_name() {
                "tokio" => "tokio::test",
                "monoio" => "monoio::test",
                _ => unreachable!(),
            }
        } else {
            match get_runtime_name() {
                "tokio" => "tokio::main",
                "monoio" => "monoio::main",
                _ => unreachable!(),
            }
        }
    }

    fn build(&self) -> Result<FinalConfig, syn::Error> {
        let flavor = self.flavor.unwrap_or(self.default_flavor);
        use RuntimeFlavor::*;

        let worker_threads = match (flavor, self.worker_threads) {
            (CurrentThread, Some((_, worker_threads_span))) => {
                let msg = format!(
                    "The `worker_threads` option requires the `multi_thread` runtime flavor. Use `#[{}(flavor = \"multi_thread\")]`",
                    self.macro_name(),
                );
                return Err(syn::Error::new(worker_threads_span, msg));
            }
            (CurrentThread, None) => None,
            (Threaded, worker_threads) if self.rt_multi_thread_available => worker_threads.map(|(val, _span)| val),
            (Threaded, _) => {
                let msg = if self.flavor.is_none() {
                    "The default runtime flavor is `multi_thread`, but the `rt-multi-thread` feature is disabled."
                } else {
                    "The runtime flavor `multi_thread` requires the `rt-multi-thread` feature."
                };
                return Err(syn::Error::new(Span::call_site(), msg));
            }
        };

        let start_paused = match (flavor, self.start_paused) {
            (Threaded, Some((_, start_paused_span))) => {
                let msg = format!(
                    "The `start_paused` option requires the `current_thread` runtime flavor. Use `#[{}(flavor = \"current_thread\")]`",
                    self.macro_name(),
                );
                return Err(syn::Error::new(start_paused_span, msg));
            }
            (CurrentThread, Some((start_paused, _))) => Some(start_paused),
            (_, None) => None,
        };

        Ok(FinalConfig {
            flavor,
            worker_threads,
            start_paused,
            init: self.init.clone(),
            tracing_span: self.tracing_span.clone(),
            tracing_lib: self.tracing_lib.clone(),
        })
    }
}

fn parse_int(int: syn::Lit, span: Span, field: &str) -> Result<usize, syn::Error> {
    match int {
        syn::Lit::Int(lit) => match lit.base10_parse::<usize>() {
            Ok(value) => Ok(value),
            Err(e) => Err(syn::Error::new(
                span,
                format!("Failed to parse value of `{}` as integer: {}", field, e),
            )),
        },
        _ => Err(syn::Error::new(
            span,
            format!("Failed to parse value of `{}` as integer.", field),
        )),
    }
}

fn parse_string(int: syn::Lit, span: Span, field: &str) -> Result<String, syn::Error> {
    match int {
        syn::Lit::Str(s) => Ok(s.value()),
        syn::Lit::Verbatim(s) => Ok(s.to_string()),
        _ => Err(syn::Error::new(
            span,
            format!("Failed to parse value of `{}` as string.", field),
        )),
    }
}

fn parse_bool(bool: syn::Lit, span: Span, field: &str) -> Result<bool, syn::Error> {
    match bool {
        syn::Lit::Bool(b) => Ok(b.value),
        _ => Err(syn::Error::new(
            span,
            format!("Failed to parse value of `{}` as bool.", field),
        )),
    }
}

fn build_config(args: AttributeArgs, rt_multi_thread: bool) -> Result<FinalConfig, syn::Error> {
    let mut config = Configuration::new(true, rt_multi_thread);
    let macro_name = config.macro_name();

    for arg in args {
        match arg {
            syn::NestedMeta::Meta(syn::Meta::NameValue(namevalue)) => {
                let ident = namevalue
                    .path
                    .get_ident()
                    .ok_or_else(|| syn::Error::new_spanned(&namevalue, "Must have specified ident"))?
                    .to_string()
                    .to_lowercase();
                match ident.as_str() {
                    "worker_threads" => {
                        config
                            .set_worker_threads(namevalue.lit.clone(), syn::spanned::Spanned::span(&namevalue.lit))?;
                    }
                    "flavor" => {
                        config.set_flavor(namevalue.lit.clone(), syn::spanned::Spanned::span(&namevalue.lit))?;
                    }
                    "start_paused" => {
                        config.set_start_paused(namevalue.lit.clone(), syn::spanned::Spanned::span(&namevalue.lit))?;
                    }
                    "init" => {
                        config.set_init(namevalue.lit.clone(), syn::spanned::Spanned::span(&namevalue.lit))?;
                    }
                    "tracing_span" => {
                        config.set_tracing_span(namevalue.lit.clone(), syn::spanned::Spanned::span(&namevalue.lit))?;
                    }
                    "tracing_lib" => {
                        config.set_tracing_lib(namevalue.lit.clone(), syn::spanned::Spanned::span(&namevalue.lit))?;
                    }

                    name => {
                        let msg = format!(
                            "Unknown attribute {} is specified; expected one of: `flavor`, `worker_threads`, `start_paused`, `init`, `tracing_span`",
                            name,
                        );
                        return Err(syn::Error::new_spanned(namevalue, msg));
                    }
                }
            }
            syn::NestedMeta::Meta(syn::Meta::Path(path)) => {
                let name = path
                    .get_ident()
                    .ok_or_else(|| syn::Error::new_spanned(&path, "Must have specified ident"))?
                    .to_string()
                    .to_lowercase();
                let msg = match name.as_str() {
                    "threaded_scheduler" | "multi_thread" => {
                        format!(
                            "Set the runtime flavor with #[{}(flavor = \"multi_thread\")].",
                            macro_name
                        )
                    }
                    "basic_scheduler" | "current_thread" | "single_threaded" => {
                        format!(
                            "Set the runtime flavor with #[{}(flavor = \"current_thread\")].",
                            macro_name
                        )
                    }
                    "flavor" | "worker_threads" | "start_paused" => {
                        format!("The `{}` attribute requires an argument.", name)
                    }
                    "init" => {
                        format!(
                            "The `{}` attribute requires an argument in string of the initializing statement to run.",
                            name
                        )
                    }
                    "tracing_span" => {
                        format!(
                            "The `{}` attribute requires an argument of level of the span, e.g. `debug` or `info`.",
                            name
                        )
                    }
                    "tracing_lib" => {
                        format!(
                            "The `{}` attribute requires an argument of level of the span, e.g. \"my_lib::\" or \"::\" or \"\".",
                            name
                        )
                    }
                    name => {
                        format!("Unknown attribute {} is specified; expected one of: `flavor`, `worker_threads`, `start_paused`, `init`, `tracing_span`, `tracing_lib`", name)
                    }
                };
                return Err(syn::Error::new_spanned(path, msg));
            }
            other => {
                return Err(syn::Error::new_spanned(other, "Unknown attribute inside the macro"));
            }
        }
    }

    config.build()
}

type AttributeArgs = syn::punctuated::Punctuated<syn::NestedMeta, syn::Token![,]>;

/// Marks async function to be executed by async runtime, suitable to test environment
///
/// It supports:
/// - [tokio](https://tokio.rs/)
/// - [monoio](https://github.com/bytedance/monoio)
///
/// By default it uses `tokio` runtime. Switch runtime with feature flags:
/// - `tokio`: tokio runtime;
/// - `monoio`: monoio runtime;
///
/// ## Usage for tokio runtime
///
/// ### Multi-thread runtime
///
/// ```no_run
/// #[async_entry::test(flavor = "multi_thread", worker_threads = 1)]
/// async fn my_test() {
///     assert!(true);
/// }
/// ```
///
/// `worker_threads>0` implies `flavor="multi_thread"`.
/// `worker_threads==0` implies `flavor="current_thread"`.
///
/// ### Using default
///
/// The default test runtime is single-threaded.
///
/// ```no_run
/// #[async_entry::test]
/// async fn my_test() {
///     assert!(true);
/// }
/// ```
///
/// ### Configure the runtime to start with time paused
///
/// ```no_run
/// #[async_entry::test(start_paused = true)]
/// async fn my_test() {
///     assert!(true);
/// }
/// ```
///
/// Note that `start_paused` requires the `test-util` feature to be enabled.
///
/// ### Add initialization statement
///
/// ```no_run
/// #[async_entry::test(init = "init_log!()")]
/// async fn my_test() {
///     assert!(true);
/// }
/// // Will produce:
/// //
/// // fn my_test() {
/// //
/// //     let _g = init_log!();  // Add init statement
/// //
/// //     let body = async { assert!(true); };
/// //     let rt = ...
/// //     rt.block_on(body);
/// // }
/// ```
///
/// ### Add tracing span over the test fn
///
/// ```no_run
/// #[async_entry::test(tracing_span = "info")]
/// async fn my_test() {
///     assert!(true);
/// }
/// // Will produce:
/// //
/// // fn my_test() {
/// //     let body = async { assert!(true); };
/// //
/// //     use ::tracing::Instrument;                       // Add tracing span
/// //     let body_span = ::tracing::info_span("my_test"); //
/// //     let body = body.instrument(body_span);           //
/// //
/// //     let rt = ...
/// //     rt.block_on(body);
/// // }
/// ```
///
/// ### Use other lib to import `tracing` and `tracing_future`
///
/// ```no_run
/// #[async_entry::test(tracing_span = "info" ,tracing_lib="my_lib::")]
/// async fn my_test() {
///     assert!(true);
/// }
/// // Will produce:
/// //
/// // fn my_test() {
/// //     let body = async { assert!(true); };
/// //
/// //     use my_lib::tracing::Instrument;                         // Add tracing span
/// //     let body_span = my_lib::tracing::info_span("my_test");   //
/// //     let body = body.instrument(body_span);                   //
/// //
/// //     let rt = ...
/// //     rt.block_on(body);
/// // }
/// ```
///
/// ## Usage for monoio runtime
///
/// **When using `monoio` runtime with feature flag `monoio` enabled:
/// `flavor`, `worker_threads` and `start_paused` are ignored**.
///
/// It is the same as using `tokio` runtime, except the runtime is `monoio`:
///
/// ```no_run
/// #[async_entry::test()]
/// async fn my_test() {
///     assert!(true);
/// }
/// // Will produce:
/// //
/// // fn my_test() {
/// //     // ...
/// //
/// //     let body = async { assert!(true); };
/// //     let rt = monoio::RuntimeBuilder::<_>::new()...
/// //     rt.block_on(body);
/// // }
/// ```
///
/// ### NOTE:
///
/// If you rename the async_entry crate in your dependencies this macro will not work.
#[proc_macro_attribute]
pub fn test(args: TokenStream, item: TokenStream) -> TokenStream {
    let tokens = entry_test(args, item.clone());

    match tokens {
        Ok(x) => x,
        Err(e) => token_stream_with_error(item, e),
    }
}

/// Entry of async test fn.
fn entry_test(args: TokenStream, item: TokenStream) -> Result<TokenStream, syn::Error> {
    let input = parse_item_fn(item)?;

    // parse attribute arguments in the parentheses:
    let parsed_args = AttributeArgs::parse_terminated.parse(args)?;
    let config = build_config(parsed_args, true)?;

    let item = build_test_fn(input, config)?;
    Ok(item)
}

// Copied from tokio-macros:
// `fn parse_knobs()` in `tokio/tokio-macros/src/entry.rs`.
fn build_test_fn(mut item_fn: ItemFn, config: FinalConfig) -> Result<TokenStream, syn::Error> {
    item_fn.sig.asyncness = None;

    let fn_name = item_fn.sig.ident.to_string();

    let (last_stmt_start_span, last_stmt_end_span) = {
        let mut last_stmt = item_fn.block.stmts.last().map(ToTokens::into_token_stream).unwrap_or_default().into_iter();
        // `Span` on stable Rust has a limitation that only points to the first
        // token, not the whole tokens. We can work around this limitation by
        // using the first/last span of the tokens like
        // `syn::Error::new_spanned` does.
        let start = last_stmt.next().map_or_else(Span::call_site, |t| t.span());
        let end = last_stmt.last().map_or(start, |t| t.span());
        (start, end)
    };

    let test_attr = quote! { #[::core::prelude::v1::test] };

    let rt = build_runtime(last_stmt_start_span, &config)?;

    let init = if let Some(init) = config.init {
        let init_str = format!("let _g = {};", init.0);
        let init_tokens = str_to_p2tokens(&init_str, init.1)?;

        quote! { #init_tokens }
    } else {
        quote! {}
    };

    let body_tracing_span = if let Some(tspan) = config.tracing_span {
        let tracing_lib = if let Some(l) = config.tracing_lib {
            l.0.clone()
        } else {
            "".to_string()
        };

        let level = tspan.0;
        let add_tracing_span = format!(
            r#"
            use {} tracing::Instrument;
            let body_span = {} tracing::{}_span!("{}");
            let body = body.instrument(body_span);
        "#,
            tracing_lib, tracing_lib, level, fn_name
        );

        let tracing_span = str_to_p2tokens(&add_tracing_span, tspan.1)?;
        quote! { #tracing_span }
    } else {
        quote! {}
    };

    let old_body = &item_fn.block;
    let old_brace = old_body.brace_token;
    let (tail_return, tail_semicolon) = match old_body.stmts.last() {
        Some(syn::Stmt::Semi(syn::Expr::Return(_), _)) => (quote! { return }, quote! { ; }),
        Some(syn::Stmt::Semi(..)) | Some(syn::Stmt::Local(..)) | None => {
            match &item_fn.sig.output {
                syn::ReturnType::Type(_, ty) if matches!(&**ty, syn::Type::Tuple(ty) if ty.elems.is_empty()) => {
                    (quote! {}, quote! { ; }) // unit
                }
                syn::ReturnType::Default => (quote! {}, quote! { ; }), // unit
                syn::ReturnType::Type(..) => (quote! {}, quote! {}),   // ! or another
            }
        }
        _ => (quote! {}, quote! {}),
    };

    // Assemble new body

    let body = quote_spanned! {last_stmt_end_span=>
        {
            #init

            let body = async #old_body;

            #body_tracing_span

            #[allow(unused_mut)]
            let mut rt = #rt;

            #[allow(clippy::expect_used)]
            #tail_return rt.block_on(body) #tail_semicolon

        }
    };

    item_fn.block = syn::parse2(body).expect("parsing failure:::");
    item_fn.block.brace_token = old_brace;

    let res = quote! {
        #test_attr
        #item_fn
    };

    let x: TokenStream = res.into_token_stream().into();
    Ok(x)
}

/// Build a statement that builds a async runtime,
/// e.g. `let rt = Builder::new_multi_thread().build().expect("");`
fn build_runtime(span: Span, config: &FinalConfig) -> Result<TokenStream2, syn::Error> {
    let rt_builder = {
        match get_runtime_name() {
            "tokio" => {
                let mut rt_builder = quote! { tokio::runtime::Builder };

                rt_builder = match config.flavor {
                    RuntimeFlavor::CurrentThread => quote_spanned! {span=>
                        #rt_builder::new_current_thread()
                    },
                    RuntimeFlavor::Threaded => quote_spanned! {span=>
                        #rt_builder::new_multi_thread()
                    },
                };

                if let Some(v) = config.worker_threads {
                    rt_builder = quote! { #rt_builder.worker_threads(#v) };
                }

                if let Some(v) = config.start_paused {
                    rt_builder = quote! { #rt_builder.start_paused(#v) };
                }
                rt_builder
            }
            "monoio" => {
                let rt_builder = quote! { monoio::RuntimeBuilder::<monoio::FusionDriver>::new() };
                rt_builder
            }
            _ => unreachable!(),
        }
    };

    let rt: TokenStream2 = quote! {
        #rt_builder
            .enable_all()
            .build()
            .expect("Failed building the Runtime")
    };

    Ok(rt)
}

/// Parse TokenStream of some fn
fn parse_item_fn(item: TokenStream) -> Result<ItemFn, syn::Error> {
    let input = syn::parse::<ItemFn>(item.clone())?;

    if input.sig.asyncness.is_none() {
        let msg = "the `async` keyword is missing from the function declaration";
        return Err(syn::Error::new_spanned(input.sig.fn_token, msg));
    }

    check_dup_test_attr(&input)?;

    Ok(input)
}

/// Check if there is already a `#[test]` attribute for input function
fn check_dup_test_attr(input: &ItemFn) -> Result<(), syn::Error> {
    let mut attrs = input.attrs.iter();
    let found = attrs.find(|a| a.path.is_ident("test"));
    if let Some(attr) = found {
        return Err(syn::Error::new_spanned(attr, "dup test"));
    }

    Ok(())
}

/// Parse rust source code in str and produce a TokenStream
fn str_to_p2tokens(s: &str, span: Span) -> Result<proc_macro2::TokenStream, syn::Error> {
    let toks = proc_macro2::TokenStream::from_str(s).map_err(|e| syn::Error::new(span, e))?;
    Ok(toks)
}

fn token_stream_with_error(mut item: TokenStream, e: syn::Error) -> TokenStream {
    item.extend(TokenStream::from(e.into_compile_error()));
    item
}
