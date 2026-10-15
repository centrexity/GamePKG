use crate::archive::Package;
use anyhow::{bail, Result};
use syn::visit::Visit;
// Bound syntax complexity with a non-recursive lexer before either token-tree
// construction or AST parsing. Literals/comments are single tokens, so large
// asset literals cannot impersonate delimiter/operator nesting.
fn syntax_budget(text: &str) -> Result<()> {
    use rustc_lexer::TokenKind as K;
    struct Scope {
        close: Option<K>,
        current: usize,
        maximum: usize,
        declaration: bool,
    }
    let mut scopes = vec![Scope {
        close: None,
        current: 0,
        maximum: 0,
        declaration: false,
    }];
    let mut count = 0;
    let mut offset = 0;
    let mut declaration = false;
    for token in rustc_lexer::tokenize(text) {
        let spelling = &text[offset..offset + token.len];
        offset += token.len;
        if matches!(
            token.kind,
            K::Whitespace | K::LineComment | K::BlockComment { .. }
        ) {
            continue;
        }
        count += 1;
        if count > 131072 {
            bail!("Rust source exceeds token budget");
        }
        if token.kind == K::Ident
            && matches!(
                spelling,
                "fn" | "mod" | "impl" | "struct" | "enum" | "trait"
            )
        {
            declaration = true;
        }
        scopes.last_mut().unwrap().current += 1;
        if scopes.iter().map(|scope| scope.current).sum::<usize>() > 1024 {
            bail!("Rust source exceeds expression complexity budget");
        }
        let close = match token.kind {
            K::OpenParen => Some(K::CloseParen),
            K::OpenBrace => Some(K::CloseBrace),
            K::OpenBracket => Some(K::CloseBracket),
            _ => None,
        };
        if let Some(close) = close {
            if scopes.len() > 64 {
                bail!("Rust source exceeds syntax nesting budget");
            }
            let item = token.kind == K::OpenBrace && declaration;
            if token.kind == K::OpenBrace {
                declaration = false;
            }
            scopes.push(Scope {
                close: Some(close),
                current: 0,
                maximum: 0,
                declaration: item,
            });
        } else if matches!(token.kind, K::CloseParen | K::CloseBrace | K::CloseBracket) {
            if scopes.len() == 1 {
                bail!("unmatched Rust delimiter");
            }
            let child = scopes.pop().unwrap();
            if child.close != Some(token.kind) {
                bail!("mismatched Rust delimiter");
            }
            let parent = scopes.last_mut().unwrap();
            parent.current += child.current.max(child.maximum);
            if scopes.iter().map(|scope| scope.current).sum::<usize>() > 1024 {
                bail!("Rust source exceeds expression complexity budget");
            }
            if child.declaration {
                let parent = scopes.last_mut().unwrap();
                parent.maximum = parent.maximum.max(parent.current);
                parent.current = 0;
            }
        } else if token.kind == K::Semi {
            let current = scopes.last_mut().unwrap();
            current.maximum = current.maximum.max(current.current);
            current.current = 0;
        }
    }
    if scopes.len() != 1 {
        bail!("unterminated Rust delimiter");
    }
    Ok(())
}

struct Scanner {
    rejected: bool,
    bindings: std::collections::BTreeSet<String>,
    functions: std::collections::BTreeSet<String>,
    constants: std::collections::BTreeSet<String>,
}
fn reserved(name: &str) -> bool {
    if name.trim_start_matches("r#").starts_with("__gamepkg_") {
        return true;
    }
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
    fn visit_macro(&mut self, _node: &'ast syn::Macro) {
        self.rejected = true;
    }
    fn visit_use_rename(&mut self, _: &'ast syn::UseRename) {
        self.rejected = true;
    }
    fn visit_use_glob(&mut self, _: &'ast syn::UseGlob) {
        self.rejected = true;
    }
    fn visit_impl_item_type(&mut self, _: &'ast syn::ImplItemType) {
        self.rejected = true;
    }
    fn visit_item_type(&mut self, _: &'ast syn::ItemType) {
        self.rejected = true;
    }
    fn visit_item_trait(&mut self, _: &'ast syn::ItemTrait) {
        self.rejected = true;
    }
    fn visit_expr_closure(&mut self, _: &'ast syn::ExprClosure) {
        self.rejected = true;
    }
    fn visit_type_bare_fn(&mut self, _: &'ast syn::TypeBareFn) {
        self.rejected = true;
    }
    fn visit_type_trait_object(&mut self, _: &'ast syn::TypeTraitObject) {
        self.rejected = true;
    }
    fn visit_type_impl_trait(&mut self, _: &'ast syn::TypeImplTrait) {
        self.rejected = true;
    }
    fn visit_trait_bound(&mut self, _: &'ast syn::TraitBound) {
        self.rejected = true;
    }
    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        // A call target must be a literal Rust path, never a computed value.
        if let syn::Expr::Path(path) = &*node.func {
            if path.path.segments.len() == 1
                && self
                    .bindings
                    .contains(&path.path.segments[0].ident.to_string())
            {
                self.rejected = true;
            }
            self.visit_path(&path.path);
        } else {
            self.rejected = true;
        }
        for argument in &node.args {
            self.visit_expr(argument);
        }
    }
    fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
        // Qualified paths may be called directly, but may not be passed around
        // as callbacks. Reject local function items as values as well.
        if (node.path.segments.len() > 1
            && !node.path.segments.last().is_some_and(|s| {
                let name = s.ident.to_string();
                name.chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
            }))
            || node
                .path
                .segments
                .last()
                .is_some_and(|s| self.functions.contains(&s.ident.to_string()))
        {
            self.rejected = true;
        }
        if node.path.segments.len() == 1 {
            let name = node.path.segments[0].ident.to_string();
            if !self.bindings.contains(&name)
                && !self.constants.contains(&name)
                && !matches!(name.as_str(), "None" | "self")
            {
                self.rejected = true;
            }
        }
        syn::visit::visit_expr_path(self, node);
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
#[derive(Default)]
struct Names {
    bindings: std::collections::BTreeSet<String>,
    functions: std::collections::BTreeSet<String>,
    constants: std::collections::BTreeSet<String>,
}
impl<'ast> Visit<'ast> for Names {
    fn visit_item_const(&mut self, node: &'ast syn::ItemConst) {
        self.constants.insert(node.ident.to_string());
        syn::visit::visit_item_const(self, node);
    }
    fn visit_item_static(&mut self, node: &'ast syn::ItemStatic) {
        self.constants.insert(node.ident.to_string());
        syn::visit::visit_item_static(self, node);
    }
    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.functions.insert(node.sig.ident.to_string());
        syn::visit::visit_impl_item_fn(self, node);
    }
    fn visit_variant(&mut self, node: &'ast syn::Variant) {
        if matches!(node.fields, syn::Fields::Unnamed(_)) {
            self.functions.insert(node.ident.to_string());
        }
        syn::visit::visit_variant(self, node);
    }
    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        if matches!(node.fields, syn::Fields::Unnamed(_)) {
            self.functions.insert(node.ident.to_string());
        }
        syn::visit::visit_item_struct(self, node);
    }

    fn visit_use_name(&mut self, node: &'ast syn::UseName) {
        let name = node.ident.to_string();
        if name.chars().any(|c| c.is_ascii_lowercase()) {
            self.functions.insert(name);
        }
    }

    fn visit_pat_ident(&mut self, node: &'ast syn::PatIdent) {
        self.bindings.insert(node.ident.to_string());
        syn::visit::visit_pat_ident(self, node);
    }
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.functions.insert(node.sig.ident.to_string());
        syn::visit::visit_item_fn(self, node);
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
    let mut parsed = Vec::new();
    let mut global_names = Names::default();
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
        syntax_budget(text)?;
        let tokens = text
            .parse::<proc_macro2::TokenStream>()
            .map_err(|error| anyhow::anyhow!("invalid source tokens: {error}"))?;
        if forbidden_tokens(tokens, false) {
            bail!("source uses a forbidden construct");
        }
        let ast = syn::parse_file(text)?;
        crate::portability::validate(&ast)?;
        global_names.visit_file(&ast);
        parsed.push((name, ast));
    }
    for (name, ast) in parsed {
        let mut names = Names::default();
        names.visit_file(&ast);
        let mut scanner = Scanner {
            rejected: false,
            bindings: names.bindings,
            functions: global_names.functions.clone(),
            constants: global_names.constants.clone(),
        };
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

#[cfg(test)]
mod restricted_calls_tests {
    use super::*;
    fn rejected(text: &str) -> bool {
        let ast = syn::parse_file(text).unwrap();
        let mut names = Names::default();
        names.visit_file(&ast);
        let mut scanner = Scanner {
            rejected: false,
            bindings: names.bindings,
            functions: names.functions,
            constants: names.constants,
        };
        scanner.visit_file(&ast);
        scanner.rejected
    }
    #[test]
    fn rejects_aliases_macros_and_callbacks() {
        for text in [
            "use gamepkg as api;",
            "fn tick() { __gamepkg_rom_count(); }",
            "fn tick() { r#__gamepkg_rom_count(); }",
            "fn __gamepkg_asset_read() {}",
            "fn tick() { let mapped = Some(1).map(Some); }",
            "struct Value; impl Value { pub fn UPPER(x:u32)->u32 { x } } fn tick() { Some(1).map(Value::UPPER); }",
            "enum Value { UPPER(u32) } fn tick() { Some(1).map(Value::UPPER); }",
            "struct UPPER(u32); fn tick() { Some(1).map(UPPER); }",

            "use gamepkg::*;",
            "use gamepkg::save_write; fn tick() { gamepkg::callback(save_write); }",
            "use gamepkg::save_write as write;",
            "type Callback = fn();",
            "fn tick() { assert!(true); }",
            "macro_rules! hidden { () => {} }",
            "fn tick() { let f = gamepkg::save_write; f(b\"x\"); }",
            "fn tick(f: fn()) { f(); }",
            "fn tick() { (gamepkg::save_write)(b\"x\"); }",
            "fn tick() { let f = || 1; f(); }",
            "fn helper() {} fn tick() { let f = helper; }",
            "fn tick(f: &dyn Fn()) { f(); }",
            "fn tick() { gamepkg::callback(gamepkg::save_write); }",
        ] {
            assert!(rejected(text), "accepted: {text}");
        }
    }
    #[test]
    fn accepts_direct_functions_and_api_calls() {
        assert!(!rejected("fn helper(x: u32) -> u32 { x + 1 } pub fn game_tick(input: u32, state: u64) -> u64 { let _ = gamepkg::draw_clear(0); helper(input) as u64 + state }"));
    }
}

/// Parse the exact private build tree, including host bindings and pinned dependencies.
/// Guest files have already passed `scan`; trusted generated/dependency code has
/// a different policy, but must still parse before any compiler is launched.
pub(crate) fn parse_build_tree(root: &std::path::Path) -> Result<usize> {
    let mut pending = vec![(root.to_path_buf(), 0usize)];
    let mut files = 0;
    let mut total_bytes = 0u64;
    while let Some((directory, depth)) = pending.pop() {
        if depth > 32 {
            bail!("build source tree exceeds depth limit");
        }
        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_symlink() {
                bail!("build source tree contains a symbolic link");
            }
            if kind.is_dir() {
                pending.push((entry.path(), depth + 1));
            } else if entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "rs")
            {
                if !kind.is_file() {
                    bail!("build source is not a regular file");
                }
                files += 1;
                if files > 4096 {
                    bail!("too many Rust source files");
                }
                let bytes = crate::files::read_regular(
                    &entry.path(),
                    crate::archive::MAX_FILE + 1024 * 1024,
                    false,
                )?;
                total_bytes += bytes.len() as u64;
                if total_bytes > crate::archive::MAX_TOTAL + 1024 * 1024 {
                    bail!("staged Rust sources exceed package and host-binding budgets");
                }
                let text = std::str::from_utf8(&bytes)?;
                syn::parse_file(text).map_err(|error| {
                    anyhow::anyhow!(
                        "invalid staged Rust source {}: {error}",
                        entry.path().display()
                    )
                })?;
            }
        }
    }
    if files == 0 {
        bail!("build source tree contains no Rust code");
    }
    Ok(files)
}

#[cfg(test)]
mod staged_tree_tests {
    use super::*;
    #[test]
    fn parses_unused_files_and_dependencies_before_compilation() {
        let tree = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tree.path().join("vendor/dep/src")).unwrap();
        std::fs::write(tree.path().join("lib.rs"), "pub fn game_tick() {}").unwrap();
        let dependency = tree.path().join("vendor/dep/src/lib.rs");
        std::fs::write(&dependency, "pub fn incomplete(").unwrap();
        assert!(parse_build_tree(tree.path())
            .unwrap_err()
            .to_string()
            .contains("invalid staged Rust"));
        std::fs::write(&dependency, "pub fn valid() {}").unwrap();
        assert_eq!(parse_build_tree(tree.path()).unwrap(), 2);
        std::fs::write(tree.path().join("unused.rs"), [255u8]).unwrap();
        assert!(parse_build_tree(tree.path()).is_err());
    }
    #[test]
    fn generated_bindings_and_pinned_dependency_parse() {
        for text in [
            include_str!("guest_api.rs.txt"),
            include_str!("component_guest.rs.txt"),
            include_str!("../vendor/gamepkg-byteorder-0.1.0/src/lib.rs"),
        ] {
            syn::parse_file(text).unwrap();
        }
    }
}

#[cfg(test)]
mod syntax_budget_tests {
    use super::*;
    #[test]
    fn excessive_operators_and_nesting_are_rejected_without_ast_parsing() {
        assert!(syntax_budget(&format!(
            "fn x() {{ let value = {}false; }}",
            "!".repeat(10000)
        ))
        .is_err());
        assert!(syntax_budget(&format!(
            "fn x() {{ {}0{}; }}",
            "(".repeat(1000),
            ")".repeat(1000)
        ))
        .is_err());
        assert!(syntax_budget(&format!("fn x() {{ {}0; }}", "1 + ".repeat(2000))).is_err());
    }
    #[test]
    fn punctuation_in_strings_and_comments_is_not_syntax_nesting() {
        let text = format!(
            "fn x() {{ let text = \"{}\"; /* {} */ }}",
            "!([".repeat(10000),
            ")!]".repeat(10000)
        );
        syn::parse_file(&text).unwrap();
        syntax_budget(&text).unwrap();
        let raw = r####"fn x() { let text = r###"! ([ /* ) ] */"###; let byte = b'('; }"####;
        syn::parse_file(raw).unwrap();
        syntax_budget(raw).unwrap();
    }
}
