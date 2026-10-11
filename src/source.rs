use crate::archive::Package;
use anyhow::{bail, Result};
use syn::visit::Visit;
struct Scanner {
    rejected: bool,
}
impl<'ast> Visit<'ast> for Scanner {
    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        if node.path.segments.iter().any(|s| {
            matches!(
                s.ident.to_string().as_str(),
                "include" | "include_bytes" | "include_str"
            )
        }) {
            self.rejected = true;
        }
        syn::visit::visit_macro(self, node);
    }
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        if node.content.is_none() || !node.attrs.is_empty() {
            self.rejected = true;
        }
        syn::visit::visit_item_mod(self, node);
    }
    fn visit_attribute(&mut self, _: &'ast syn::Attribute) {
        self.rejected = true;
    }
    fn visit_expr_unsafe(&mut self, _: &'ast syn::ExprUnsafe) {
        self.rejected = true;
    }
    fn visit_item_foreign_mod(&mut self, _: &'ast syn::ItemForeignMod) {
        self.rejected = true;
    }
    fn visit_path(&mut self, node: &'ast syn::Path) {
        if node.segments.iter().any(|s| {
            matches!(
                s.ident.to_string().as_str(),
                "fs" | "net" | "process" | "env" | "libc"
            )
        }) {
            self.rejected = true;
        }
        syn::visit::visit_path(self, node);
    }
}
pub fn scan(pkg: &mut Package) -> Result<()> {
    let cargo: toml::Value = toml::from_str(std::str::from_utf8(&pkg.read("source/Cargo.toml")?)?)?;
    let table = cargo
        .as_table()
        .ok_or_else(|| anyhow::anyhow!("invalid Cargo manifest"))?;
    for key in table.keys() {
        if !["package", "lib", "dependencies"].contains(&key.as_str()) {
            bail!("unsupported Cargo section: {key}");
        }
    }
    let package = cargo
        .get("package")
        .and_then(|p| p.as_table())
        .ok_or_else(|| anyhow::anyhow!("missing package"))?;
    for key in package.keys() {
        if !["name", "version", "edition", "build"].contains(&key.as_str()) {
            bail!("unsupported package field");
        }
    }
    if package.get("build") != Some(&toml::Value::Boolean(false)) {
        bail!("package.build must be false");
    }
    if let Some(deps) = cargo.get("dependencies") {
        if deps.as_table().is_none_or(|t| !t.is_empty()) {
            bail!("initial crate allowlist is empty");
        }
    }
    if let Some(lib) = cargo.get("lib") {
        let t = lib
            .as_table()
            .ok_or_else(|| anyhow::anyhow!("invalid lib section"))?;
        for (k, v) in t {
            if k != "path" || v.as_str() != Some("src/lib.rs") {
                bail!("unsupported library configuration");
            }
        }
    }
    for name in pkg
        .entries
        .keys()
        .filter(|n| n.starts_with("source/"))
        .cloned()
        .collect::<Vec<_>>()
    {
        if name == "source/Cargo.toml" || name == "source/Cargo.lock" {
            continue;
        }
        if name != "source/src/lib.rs" {
            bail!("initial source profile permits only src/lib.rs");
        }
        let data = pkg.read(&name)?;
        let ast = syn::parse_file(std::str::from_utf8(&data)?)?;
        let mut scanner = Scanner { rejected: false };
        scanner.visit_file(&ast);
        if scanner.rejected {
            bail!("source uses a forbidden construct");
        }
    }
    Ok(())
}
