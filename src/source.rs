use crate::archive::Package;
use anyhow::{bail, Result};
use syn::visit::Visit;
struct Scanner {
    rejected: bool,
}
fn reserved(name: &str) -> bool {
    matches!(
        name.strip_prefix("r#").unwrap_or(name),
        "include"
            | "include_bytes"
            | "include_str"
            | "unsafe"
            | "extern"
            | "fs"
            | "net"
            | "process"
            | "env"
            | "libc"
    )
}
fn forbidden_tokens(tokens: proc_macro2::TokenStream, macro_context: bool) -> bool {
    let mut pending = vec![tokens];
    while let Some(tokens) = pending.pop() {
        for token in tokens {
            match token {
                proc_macro2::TokenTree::Group(group) => pending.push(group.stream()),
                proc_macro2::TokenTree::Ident(name)
                    if reserved(&name.to_string()) || (macro_context && name == "mod") =>
                {
                    return true
                }
                proc_macro2::TokenTree::Punct(punct) if macro_context && punct.as_char() == '#' => {
                    return true
                }
                _ => {}
            }
        }
    }
    false
}
impl<'ast> Visit<'ast> for Scanner {
    fn visit_ident(&mut self, node: &'ast syn::Ident) {
        if reserved(&node.to_string()) {
            self.rejected = true;
        }
    }
    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        self.rejected |= forbidden_tokens(node.tokens.clone(), true);
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
        if !node.attrs.is_empty() {
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
struct Modules<'a> {
    base: String,
    entries: &'a std::collections::BTreeMap<String, u64>,
    rejected: bool,
}
impl<'ast> Visit<'ast> for Modules<'_> {
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        let name = node.ident.to_string();
        let name = name.strip_prefix("r#").unwrap_or(&name);
        let base = format!("{}/{name}", self.base);
        if node.content.is_none() {
            let file = self.entries.contains_key(&format!("{base}.rs"));
            let directory = self.entries.contains_key(&format!("{base}/mod.rs"));
            self.rejected |= file == directory;
        } else {
            let previous = std::mem::replace(&mut self.base, base);
            syn::visit::visit_item_mod(self, node);
            self.base = previous;
        }
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
    let name = package
        .get("name")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("Cargo package name must be a string"))?;
    if name.is_empty()
        || name.len() > 64
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
        || !name.as_bytes()[0].is_ascii_alphabetic() && name.as_bytes()[0] != b'_'
    {
        bail!("Cargo package name must be a Rust-compatible ASCII name of at most 64 bytes");
    }
    if package
        .get("edition")
        .is_some_and(|edition| !matches!(edition.as_str(), Some("2015" | "2018" | "2021" | "2024")))
    {
        bail!("unsupported Rust edition; expected 2015, 2018, 2021, or 2024");
    }
    crate::allowlist::approved(&cargo, &pkg.read("source/Cargo.lock")?)?;
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
        if !name.starts_with("source/src/")
            || !name.ends_with(".rs")
            || name.ends_with("/build.rs")
            || name.len() > 1024
            || name.split('/').count() > 32
        {
            bail!("source profile permits only Rust source files under src/");
        }
        let data = pkg.read(&name)?;
        let text = std::str::from_utf8(&data)?;
        let tokens = text
            .parse::<proc_macro2::TokenStream>()
            .map_err(|error| anyhow::anyhow!("invalid source tokens: {error}"))?;
        if forbidden_tokens(tokens, false) {
            bail!("source uses a forbidden construct");
        }
        let ast = syn::parse_file(text)?;
        let mut scanner = Scanner { rejected: false };
        scanner.visit_file(&ast);
        let parent = name.rsplit_once('/').unwrap().0;
        let file = name.rsplit_once('/').unwrap().1;
        let base = if name == "source/src/lib.rs" || file == "mod.rs" {
            parent.to_owned()
        } else {
            format!("{parent}/{}", file.strip_suffix(".rs").unwrap())
        };
        let mut modules = Modules {
            base,
            entries: &pkg.entries,
            rejected: false,
        };
        modules.visit_file(&ast);
        scanner.rejected |= modules.rejected;
        if scanner.rejected {
            bail!("source uses a forbidden construct");
        }
    }
    Ok(())
}
