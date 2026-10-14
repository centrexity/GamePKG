//! Enforced guest data rules; games always compile for the same wasm32 ABI.
use anyhow::{bail, Result};
use syn::visit::Visit;

fn byte_conversion(name: &str) -> bool {
    matches!(
        name.strip_prefix("r#").unwrap_or(name),
        "to_le_bytes" | "to_be_bytes" | "from_le_bytes" | "from_be_bytes"
    )
}
fn forbidden(name: &str) -> bool {
    matches!(
        name.strip_prefix("r#").unwrap_or(name),
        "to_ne_bytes"
            | "from_ne_bytes"
            | "NativeEndian"
            | "isize"
            | "size_of"
            | "size_of_val"
            | "align_of"
            | "align_of_val"
            | "offset_of"
            | "transmute"
            | "transmute_copy"
            | "as_ptr"
            | "as_mut_ptr"
            | "ptr"
            | "NonNull"
            | "AtomicPtr"
            | "MaybeUninit"
            | "into_raw"
            | "into_raw_with_allocator"
            | "from_raw"
            | "from_raw_in"
            | "as_raw"
            | "expose_provenance"
            | "with_exposed_provenance"
            | "without_provenance"
            | "from_raw_parts"
            | "from_raw_parts_mut"
            | "into_raw_parts"
            | "read_unaligned"
            | "write_unaligned"
            | "read_volatile"
            | "write_volatile"
            | "target_endian"
            | "target_pointer_width"
            | "target_arch"
            | "target_os"
            | "arch"
    )
}
fn primitive(name: &str) -> bool {
    matches!(
        name.strip_prefix("r#").unwrap_or(name),
        "u8" | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "f32"
            | "f64"
            | "usize"
            | "isize"
    )
}
#[derive(Default)]
struct Policy {
    reason: Option<&'static str>,
}
impl<'ast> Visit<'ast> for Policy {
    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        if primitive(&item.ident.to_string()) {
            self.reason = Some("primitive type names cannot be shadowed");
        }
        syn::visit::visit_item_mod(self, item);
    }
    fn visit_item_struct(&mut self, item: &'ast syn::ItemStruct) {
        if primitive(&item.ident.to_string()) {
            self.reason = Some("primitive type names cannot be shadowed");
        }
        syn::visit::visit_item_struct(self, item);
    }
    fn visit_item_enum(&mut self, item: &'ast syn::ItemEnum) {
        if primitive(&item.ident.to_string()) {
            self.reason = Some("primitive type names cannot be shadowed");
        }
        syn::visit::visit_item_enum(self, item);
    }
    fn visit_item_union(&mut self, item: &'ast syn::ItemUnion) {
        if primitive(&item.ident.to_string()) {
            self.reason = Some("primitive type names cannot be shadowed");
        }
        syn::visit::visit_item_union(self, item);
    }
    fn visit_ident(&mut self, ident: &'ast syn::Ident) {
        if forbidden(&ident.to_string()) {
            self.reason = Some("native byte order, pointer access, layout inspection, or platform-dependent types are forbidden");
        }
    }
    fn visit_type_ptr(&mut self, _: &'ast syn::TypePtr) {
        self.reason = Some("raw pointer types are forbidden");
    }
    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        if byte_conversion(&call.method.to_string()) {
            self.reason = Some("byte conversion must name a fixed-width primitive, such as u32::to_le_bytes(value)");
        }
        syn::visit::visit_expr_method_call(self, call);
    }
    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = &*call.func {
            if path
                .path
                .segments
                .last()
                .is_some_and(|s| byte_conversion(&s.ident.to_string()))
            {
                let names: Vec<_> = path
                    .path
                    .segments
                    .iter()
                    .map(|s| s.ident.to_string())
                    .collect();
                let primitive = names.get(names.len().saturating_sub(2)).map(String::as_str);
                let fixed = matches!(
                    primitive,
                    Some(
                        "u8" | "u16"
                            | "u32"
                            | "u64"
                            | "u128"
                            | "i8"
                            | "i16"
                            | "i32"
                            | "i64"
                            | "i128"
                            | "f32"
                            | "f64"
                    )
                );
                let canonical = names.len() == 2
                    || (names.len() == 4
                        && matches!(names[0].as_str(), "core" | "std")
                        && names[1] == "primitive");
                if !fixed
                    || !canonical
                    || path.qself.is_some()
                    || path
                        .path
                        .segments
                        .iter()
                        .any(|s| !matches!(s.arguments, syn::PathArguments::None))
                {
                    self.reason = Some("serialized numbers require an explicit fixed-width primitive and byte order");
                }
            }
        }
        syn::visit::visit_expr_call(self, call);
    }
}
pub(crate) fn validate(ast: &syn::File) -> Result<()> {
    let mut policy = Policy::default();
    policy.visit_file(ast);
    if let Some(reason) = policy.reason {
        bail!("source violates portable data policy: {reason}");
    }
    Ok(())
}
