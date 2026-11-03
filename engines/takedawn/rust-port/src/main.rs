//! Maintainer-only normalization of C2Rust output. No instruction translation
//! or replacement game/runtime behavior is implemented here.
use anyhow::{ensure, Context, Result};
use quote::{format_ident, ToTokens};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
use syn::{parse_quote, visit_mut::VisitMut, Expr, ForeignItem, Item};

#[derive(Default)]
struct Normalize {
    tls: BTreeMap<String, String>,
    locals: Option<BTreeSet<String>>,
    unsupported_intrinsics: BTreeSet<String>,
    atomic_calls: BTreeMap<String, usize>,
    streams: BTreeSet<String>,
    stream_functions: BTreeSet<String>,
    invalid_file_value: bool,
    fixed_constants: BTreeMap<String, usize>,
}
impl VisitMut for Normalize {
    fn visit_type_mut(&mut self, ty: &mut syn::Type) {
        if !self.streams.is_empty() {
            // The C runtime owns FILE's layout. Only raw pointers may cross
            // this boundary; by-value use, sizeof and arrays require a real
            // layout and must fail preparation rather than silently use a ZST.
            let opaque = |ty: &syn::Type| {
                matches!(ty, syn::Type::Path(p)
                if p.qself.is_none() && p.path.segments.len() == 1
                && matches!(p.path.segments[0].ident.to_string().as_str(), "FILE" | "_IO_FILE"))
            };
            if let syn::Type::Ptr(pointer) = ty {
                if opaque(&pointer.elem) {
                    return;
                }
            }
            if opaque(ty) {
                self.invalid_file_value = true;
                return;
            }
        }
        syn::visit_mut::visit_type_mut(self, ty);
    }
    fn visit_item_type_mut(&mut self, item: &mut syn::ItemType) {
        if !self.streams.is_empty() && item.ident == "FILE" {
            return; // The reviewed FILE = _IO_FILE alias was checked below.
        }
        syn::visit_mut::visit_item_type_mut(self, item);
    }
    fn visit_item_fn_mut(&mut self, function: &mut syn::ItemFn) {
        let enclosing = self.locals.replace(BTreeSet::new());
        syn::visit_mut::visit_item_fn_mut(self, function);
        let used = self.locals.take().unwrap();
        self.locals = enclosing;
        let declarations = used.into_iter().map(|name| {
            let pointer = format_ident!("__gamepkg_tls_{name}");
            let getter = format_ident!("gamepkg_tls_{name}");
            parse_quote!(let #pointer = unsafe { #getter() };)
        });
        // Cache addresses, never values: callees share the same live TLS cells.
        // Each native function remains on its OS thread for its lifetime.
        function.block.stmts.splice(0..0, declarations);
    }
    fn visit_expr_mut(&mut self, expr: &mut Expr) {
        if let Expr::Struct(value) = expr {
            if !self.streams.is_empty()
                && value
                    .path
                    .segments
                    .last()
                    .is_some_and(|s| s.ident == "FILE" || s.ident == "_IO_FILE")
            {
                self.invalid_file_value = true;
            }
        }
        syn::visit_mut::visit_expr_mut(self, expr);
        if let Expr::Call(call) = expr {
            if let Expr::Path(path) = call.func.as_ref() {
                let segments = &path.path.segments;
                if segments.len() == 3
                    && segments[0].ident == "core"
                    && segments[1].ident == "intrinsics"
                {
                    let name = segments[2].ident.to_string();
                    if name == "atomic_xadd_seqcst" && call.args.len() == 2 {
                        let pointer = &call.args[0];
                        let value = &call.args[1];
                        *self.atomic_calls.entry(name).or_default() += 1;
                        *expr = parse_quote!({
                            let __gamepkg_atomic_ptr = #pointer;
                            let __gamepkg_atomic_value = #value;
                            ::core::sync::atomic::AtomicU32::from_ptr(__gamepkg_atomic_ptr)
                                .fetch_add(__gamepkg_atomic_value, ::core::sync::atomic::Ordering::SeqCst)
                        });
                        return;
                    }
                    if name == "atomic_cxchg_seqcst_seqcst" && call.args.len() == 3 {
                        let pointer = &call.args[0];
                        let expected = &call.args[1];
                        let replacement = &call.args[2];
                        *self.atomic_calls.entry(name).or_default() += 1;
                        *expr = parse_quote!({
                            let __gamepkg_atomic_ptr = #pointer;
                            let __gamepkg_atomic_expected = #expected;
                            let __gamepkg_atomic_replacement = #replacement;
                            match ::core::sync::atomic::AtomicU32::from_ptr(__gamepkg_atomic_ptr)
                                .compare_exchange(__gamepkg_atomic_expected, __gamepkg_atomic_replacement,
                                    ::core::sync::atomic::Ordering::SeqCst,
                                    ::core::sync::atomic::Ordering::SeqCst)
                            {
                                ::core::result::Result::Ok(value) => (value, true),
                                ::core::result::Result::Err(value) => (value, false),
                            }
                        });
                        return;
                    }
                    self.unsupported_intrinsics.insert(name);
                }
            }
        }
        if let Expr::Path(path) = expr {
            if path.qself.is_none() && path.path.segments.len() == 1 {
                let name = path.path.segments[0].ident.to_string();
                if self.streams.contains(&name) {
                    let getter = format_ident!("gamepkg_native_{name}");
                    *expr = parse_quote!(#getter());
                    return;
                }
                if self.stream_functions.contains(&name) {
                    path.path.segments[0].ident = format_ident!("gamepkg_native_{name}");
                    return;
                }
                if self.tls.contains_key(&name) {
                    if let Some(used) = &mut self.locals {
                        used.insert(name.clone());
                        let pointer = format_ident!("__gamepkg_tls_{name}");
                        *expr = parse_quote!((*#pointer));
                    } else {
                        let getter = format_ident!("gamepkg_tls_{name}");
                        *expr = parse_quote!((*#getter()));
                    }
                    return;
                }
            }
            // These strict-provenance APIs were renamed before stabilization.
            if path.path.segments.len() >= 3
                && path.path.segments.iter().rev().nth(1).unwrap().ident == "ptr"
                && matches!(
                    path.path.segments[0].ident.to_string().as_str(),
                    "core" | "std"
                )
            {
                let last = path.path.segments.last_mut().unwrap();
                let name = match last.ident.to_string().as_str() {
                    "from_exposed_addr" => Some("with_exposed_provenance"),
                    "from_exposed_addr_mut" => Some("with_exposed_provenance_mut"),
                    _ => None,
                };
                if let Some(name) = name {
                    last.ident = format_ident!("{name}");
                }
            }
        }
        if let Expr::MethodCall(call) = expr {
            if call.method == "expose_addr" && call.args.is_empty() {
                call.method = format_ident!("expose_provenance");
            }
        }
    }
}

fn normalize(source: &str) -> Result<(String, serde_json::Value)> {
    ensure!(
        !source.contains("__gamepkg_tls_")
            && !source.contains("__gamepkg_atomic_")
            && !source.contains("gamepkg_native_"),
        "reserved generated local prefix already present"
    );
    let mut file = syn::parse_file(source).context("invalid transpiled Rust")?;
    let mut change = Normalize::default();
    let mut attributes = Vec::new();
    for attr in std::mem::take(&mut file.attrs) {
        if attr.path().is_ident("feature") {
            let features = attr.parse_args_with(
                syn::punctuated::Punctuated::<syn::Ident, syn::Token![,]>::parse_terminated,
            )?;
            for feature in features {
                ensure!(
                    matches!(
                        feature.to_string().as_str(),
                        "thread_local"
                            | "strict_provenance"
                            | "raw_ref_op"
                            | "core_intrinsics"
                            | "extern_types"
                    ),
                    "unreviewed Rust feature: {feature}"
                );
            }
        } else {
            attributes.push(attr);
        }
    }
    file.attrs = attributes;
    let mut opaque_items = Vec::<Item>::new();
    let mut opaque_names = Vec::new();
    for item in &mut file.items {
        if let Item::ForeignMod(block) = item {
            let mut kept = Vec::new();
            for item in std::mem::take(&mut block.items) {
                if let ForeignItem::Type(opaque) = item {
                    ensure!(
                        opaque.attrs.is_empty()
                            && opaque.generics.params.is_empty()
                            && opaque.generics.where_clause.is_none(),
                        "unreviewed opaque type declaration"
                    );
                    let name = opaque.ident;
                    let visibility = opaque.vis;
                    opaque_names.push(name.to_string());
                    // A C incomplete type is only used through pointers. This
                    // marker supplies a stable Rust name, never a value layout.
                    opaque_items.push(
                        parse_quote!(#[repr(C)] #visibility struct #name { _private: [u8; 0] }),
                    );
                } else {
                    kept.push(item);
                }
            }
            block.items = kept;
            for item in &mut block.items {
                if let ForeignItem::Static(global) = item {
                    let name = global.ident.to_string();
                    if matches!(name.as_str(), "stdin" | "stdout" | "stderr") {
                        ensure!(
                            global.attrs.is_empty(),
                            "unreviewed native stream attributes"
                        );
                        ensure!(
                            global.ty.to_token_stream().to_string() == "* mut FILE",
                            "unreviewed native stream type"
                        );
                        ensure!(
                            change.streams.insert(name.clone()),
                            "duplicate native stream"
                        );
                        let getter = format_ident!("gamepkg_native_{name}");
                        *item = parse_quote!(fn #getter() -> *mut FILE;);
                        continue;
                    }
                    if global
                        .attrs
                        .iter()
                        .any(|a| a.path().is_ident("thread_local"))
                    {
                        ensure!(global.attrs.len() == 1, "unreviewed TLS attributes");
                        let name = global.ident.to_string();
                        let ty = &global.ty;
                        ensure!(
                            change
                                .tls
                                .insert(name.clone(), ty.to_token_stream().to_string())
                                .is_none(),
                            "duplicate TLS declaration"
                        );
                        let getter = format_ident!("gamepkg_tls_{name}");
                        *item = parse_quote!(fn #getter() -> *mut #ty;);
                    }
                }
            }
        }
    }
    file.items.extend(opaque_items);
    for item in &mut file.items {
        if let Item::Const(value) = item {
            if value.ident == "INT64_MIN" {
                let expected: Expr = parse_quote!(
                    -9223372036854775807 as ::core::ffi::c_long - 1 as ::core::ffi::c_long
                );
                ensure!(
                    value.ty.to_token_stream().to_string() == ":: core :: ffi :: c_long"
                        && value.expr.to_token_stream().to_string()
                            == expected.to_token_stream().to_string(),
                    "unreviewed INT64_MIN definition"
                );
                // C2Rust emitted the generation machine's LP64 macro type.
                // This is a fixed-width C constant, independent of host long.
                value.ty = Box::new(parse_quote!(i64));
                value.expr = Box::new(parse_quote!(i64::MIN));
                *change
                    .fixed_constants
                    .entry("INT64_MIN".into())
                    .or_default() += 1;
            }
        }
    }
    if !change.streams.is_empty() {
        for item in &mut file.items {
            if let Item::ForeignMod(block) = item {
                for item in &mut block.items {
                    if let ForeignItem::Fn(function) = item {
                        let name = function.sig.ident.to_string();
                        if matches!(name.as_str(), "fprintf" | "fflush") {
                            ensure!(
                                function.attrs.is_empty(),
                                "unreviewed native stdio function attributes"
                            );
                            change.stream_functions.insert(name.clone());
                            function.sig.ident = format_ident!("gamepkg_native_{name}");
                        }
                    }
                }
            }
        }
        let aliases = file
            .items
            .iter()
            .filter(|item| {
                matches!(item, Item::Type(alias)
            if alias.ident == "FILE" && alias.ty.to_token_stream().to_string() == "_IO_FILE")
            })
            .count();
        ensure!(aliases == 1, "unreviewed C stream layout alias");
        let mut replaced = 0;
        for item in &mut file.items {
            if matches!(item, Item::Struct(s) if s.ident == "_IO_FILE") {
                *item = parse_quote!(
                    #[repr(C)]
                    pub struct _IO_FILE {
                        _private: [u8; 0],
                    }
                );
                replaced += 1;
            }
        }
        ensure!(
            replaced == 1,
            "missing or duplicate native FILE declaration"
        );
    }
    let mut exports = Vec::new();
    for item in &file.items {
        if let Item::Fn(function) = item {
            if function
                .attrs
                .iter()
                .any(|a| a.path().is_ident("no_mangle"))
            {
                exports.push(function.sig.ident.to_string());
            }
        }
    }
    change.visit_file_mut(&mut file);
    ensure!(
        !change.invalid_file_value,
        "native FILE must remain an opaque pointer"
    );
    ensure!(
        change.unsupported_intrinsics.is_empty(),
        "unreviewed intrinsic calls: {:?}",
        change.unsupported_intrinsics
    );
    let result = prettyplease::unparse(&file);
    // Parsing the result again catches malformed transformations immediately.
    syn::parse_file(&result)?;
    Ok((
        result,
        serde_json::json!({"thread_locals": change.tls, "exports": exports,
            "atomic_calls": change.atomic_calls, "opaque_types": opaque_names,
            "stdio_streams": change.streams, "stdio_functions": change.stream_functions,
            "fixed_width_constants": change.fixed_constants}),
    ))
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    ensure!(
        args.len() == 3,
        "usage: gamepkg-takedawn-rust-port INPUT.rs OUTPUT.rs REPORT.json"
    );
    let input = Path::new(&args[0]);
    let output = Path::new(&args[1]);
    let report = Path::new(&args[2]);
    ensure!(
        input != output && !output.try_exists()? && !report.try_exists()?,
        "outputs must be new files"
    );
    let source = fs::read_to_string(input)?;
    ensure!(
        source.len() <= 64 * 1024 * 1024,
        "transpiled unit exceeds 64 MiB"
    );
    let (rust, metadata) = normalize(&source)?;
    fs::write(output, rust)?;
    fs::write(report, serde_json::to_vec_pretty(&metadata)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shares_live_native_tls_including_array_addresses() {
        let (rust, report) = normalize(
            r#"
            #![feature(thread_local, raw_ref_op, strict_provenance)]
            extern "C" {
                #[thread_local] static mut g_eax: u32;
                #[thread_local] static mut g_fp_stack: [f64; 8];
                static mut ordinary: u32;
            }
            #[no_mangle] pub unsafe extern "C" fn translated() {
                g_eax = g_eax.wrapping_add(ordinary);
                let p = &raw mut g_fp_stack;
                (*p)[0] = 1.5;
                let q = ::core::ptr::from_exposed_addr_mut::<u8>(123);
                let _ = q.expose_addr();
            }
        "#,
        )
        .unwrap();
        assert!(!rust.contains("#![feature"));
        assert!(!rust.contains("#[thread_local]"));
        assert!(rust.contains("fn gamepkg_tls_g_fp_stack() -> *mut [f64; 8]"));
        assert!(rust.contains("let __gamepkg_tls_g_eax = unsafe { gamepkg_tls_g_eax() }"));
        assert!(rust.contains("(*__gamepkg_tls_g_eax).wrapping_add(ordinary)"));
        assert!(rust.contains("&raw mut (*__gamepkg_tls_g_fp_stack)"));
        assert!(rust.contains("with_exposed_provenance_mut::<u8>(123)"));
        assert!(rust.contains("q.expose_provenance()"));
        assert_eq!(report["exports"], serde_json::json!(["translated"]));
        assert_eq!(report["thread_locals"].as_object().unwrap().len(), 2);
    }
    #[test]
    fn rejects_features_and_attributes_without_a_reviewed_conversion() {
        assert!(normalize("#![feature(c_variadic)] fn f() {} ").is_err());
        assert!(normalize("fn f() { let __gamepkg_tls_g = 0; }").is_err());
        assert!(normalize(
            "extern \"C\" { #[thread_local] #[link_name=\"other\"] static mut g: u32; }"
        )
        .is_err());
    }
    #[test]
    fn preserves_atomic_results_and_ordering_with_stable_apis() {
        let (rust, report) = normalize(r#"
            #![feature(core_intrinsics)]
            #[no_mangle] pub unsafe extern "C" fn add(p: *mut u32, n: u32) -> u32 {
                ::core::intrinsics::atomic_xadd_seqcst(p, n)
            }
            #[no_mangle] pub unsafe extern "C" fn exchange(p: *mut u32, a: u32, b: u32) -> (u32, bool) {
                ::core::intrinsics::atomic_cxchg_seqcst_seqcst(p, a, b)
            }
        "#).unwrap();
        assert!(!rust.contains("core::intrinsics"));
        assert!(rust.contains(".fetch_add("));
        assert!(rust.contains(".compare_exchange("));
        assert!(rust.contains("Ok(value) => (value, true)"));
        assert!(rust.contains("Err(value) => (value, false)"));
        assert_eq!(report["atomic_calls"]["atomic_xadd_seqcst"], 1);
        assert_eq!(report["atomic_calls"]["atomic_cxchg_seqcst_seqcst"], 1);
        assert!(
            normalize("unsafe fn f() { ::core::intrinsics::atomic_xchg_seqcst(0, 1); }").is_err()
        );
    }
    #[test]
    fn opaque_c_declarations_remain_distinct_pointer_types() {
        let (rust, report) = normalize(
            r#"
            #![feature(extern_types)]
            extern "C" { pub type Opaque; fn inspect(p: *const Opaque) -> u32; }
            #[no_mangle] pub unsafe extern "C" fn call(p: *const Opaque) -> u32 { inspect(p) }
        "#,
        )
        .unwrap();
        assert!(rust.contains("struct Opaque"));
        assert!(rust.contains("_private: [u8; 0]"));
        assert!(rust.contains("p: *const Opaque"));
        assert!(!rust.contains("pub type Opaque"));
        assert_eq!(report["opaque_types"], serde_json::json!(["Opaque"]));
    }
    #[test]
    fn native_streams_use_host_getters_and_no_foreign_file_layout() {
        let source = r#"
            #[repr(C)] pub struct _IO_FILE { pub linux_only: i64 }
            pub type FILE = _IO_FILE;
            extern "C" {
                static mut stderr: *mut FILE;
                fn fflush(stream: *mut FILE) -> i32;
            }
            #[no_mangle] pub unsafe extern "C" fn flush() -> i32 { fflush(stderr) }
        "#;
        let (rust, report) = normalize(source).unwrap();
        assert!(rust.contains("fn gamepkg_native_stderr() -> *mut FILE"));
        assert!(rust.contains("gamepkg_native_fflush(gamepkg_native_stderr())"));
        assert!(!rust.contains("static mut stderr"));
        assert!(!rust.contains("linux_only"));
        assert_eq!(report["stdio_streams"], serde_json::json!(["stderr"]));
        assert_eq!(report["stdio_functions"], serde_json::json!(["fflush"]));
        for extra in [
            "unsafe fn value(stream: FILE) {}",
            "fn size() { let _ = core::mem::size_of::<FILE>(); }",
            "unsafe fn array(stream: *mut [FILE; 2]) {}",
            "fn create() { let _ = _IO_FILE { linux_only: 0 }; }",
            "type Other = FILE;",
        ] {
            assert!(normalize(&format!("{source}\n{extra}")).is_err(), "{extra}");
        }
        assert!(normalize(&source.replace("*mut FILE;", "*mut u8;")).is_err());
        assert!(
            normalize(&source.replace("pub type FILE = _IO_FILE;", "pub type FILE = u8;")).is_err()
        );
        assert!(normalize(&source.replace(
            "static mut stderr:",
            "#[link_name=\"other\"] static mut stderr:"
        ))
        .is_err());
    }
    #[test]
    fn fixed_width_minimum_does_not_inherit_the_generation_hosts_long_width() {
        let source = "pub const INT64_MIN: ::core::ffi::c_long = -9223372036854775807 as ::core::ffi::c_long - 1 as ::core::ffi::c_long;";
        let (rust, report) = normalize(source).unwrap();
        assert!(rust.contains("pub const INT64_MIN: i64 = i64::MIN"));
        assert_eq!(report["fixed_width_constants"]["INT64_MIN"], 1);
        assert!(normalize(&source.replace("9223372036854775807", "9223372036854775806")).is_err());
        assert!(normalize("pub const INT64_MIN: u32 = 0;").is_err());
    }
}
