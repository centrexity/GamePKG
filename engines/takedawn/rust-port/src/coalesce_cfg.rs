//! Join unique straight-line C2Rust steps without deleting any original arm.
//!
//! The loop must contain only its dispatch match. Keeping every original arm
//! and every state assignment lets Rust check the same exhaustive enum and
//! preserves side effects; each copied arm keeps its own lexical scope. This
//! is preparation work, never a player-side instruction interpreter.
use std::collections::{BTreeMap, BTreeSet};
use syn::{parse_quote, visit_mut::VisitMut, Expr, ExprPath, ItemFn, Pat, Stmt};

fn local(path: &ExprPath) -> Option<String> {
    (path.qself.is_none() && path.path.leading_colon.is_none() && path.path.segments.len() == 1)
        .then(|| path.path.segments[0].ident.to_string())
}
fn variant(path: &ExprPath) -> Option<(String, String)> {
    if path.qself.is_some()
        || path.path.leading_colon.is_some()
        || path.path.segments.len() != 2
        || path
            .path
            .segments
            .iter()
            .any(|s| !matches!(s.arguments, syn::PathArguments::None))
    {
        return None;
    }
    let mut segments = path.path.segments.iter();
    let name = segments.next()?.ident.to_string();
    let state = segments.next()?.ident.to_string();
    (name == "C2Rust_Block"
        || name
            .strip_prefix("C2Rust_Block_")
            .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())))
    .then_some((name, state))
}
#[derive(Default)]
struct Uses {
    paths: BTreeMap<String, usize>,
    allowed: BTreeMap<String, usize>,
    bindings: BTreeMap<String, usize>,
    variants: BTreeMap<(String, String), usize>,
    enums: BTreeMap<String, usize>,
    enum_states: BTreeMap<String, BTreeSet<String>>,
    type_names: BTreeMap<String, usize>,
    opaque: bool,
    initializers: BTreeMap<String, Option<(String, String)>>,
    uninitialized: BTreeSet<String>,
    writes: BTreeMap<String, Vec<Option<(String, String)>>>,
    dispatches: BTreeMap<String, usize>,
    loop_escape: bool,
}
impl Uses {
    fn note_allowed(&mut self, expr: &Expr) {
        if let Expr::Path(path) = expr {
            if let Some(name) = local(path) {
                *self.allowed.entry(name).or_default() += 1;
            }
        }
    }
    fn safe(&self, name: &str) -> bool {
        !self.opaque
            && (name == "c2rust_current_block"
                || name
                    .strip_prefix("c2rust_current_block_")
                    .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())))
            && self.bindings.get(name) == Some(&1)
            && self.paths.get(name) == self.allowed.get(name)
    }
}
impl VisitMut for Uses {
    fn visit_local_mut(&mut self, node: &mut syn::Local) {
        let pat = match &node.pat {
            Pat::Type(p) => p.pat.as_ref(),
            p => p,
        };
        if let Pat::Ident(p) = pat {
            if node.init.is_none() && node.attrs.is_empty() {
                self.uninitialized.insert(p.ident.to_string());
            }
            let value = node.init.as_ref().and_then(|i| {
                if !node.attrs.is_empty() || i.diverge.is_some() {
                    return None;
                }
                match i.expr.as_ref() {
                    Expr::Path(p) if p.attrs.is_empty() => variant(p),
                    _ => None,
                }
            });
            self.initializers.insert(p.ident.to_string(), value);
        }
        syn::visit_mut::visit_local_mut(self, node);
    }
    fn visit_item_mut(&mut self, item: &mut syn::Item) {
        let name = match item {
            syn::Item::Enum(i) => Some(&i.ident),
            syn::Item::Struct(i) => Some(&i.ident),
            syn::Item::Union(i) => Some(&i.ident),
            syn::Item::Type(i) => Some(&i.ident),
            syn::Item::Mod(i) => Some(&i.ident),
            syn::Item::Trait(i) => Some(&i.ident),
            syn::Item::TraitAlias(i) => Some(&i.ident),
            syn::Item::Use(_) | syn::Item::ExternCrate(_) => {
                self.opaque = true;
                None
            }
            _ => None,
        };
        if let Some(name) = name {
            *self.type_names.entry(name.to_string()).or_default() += 1;
        }
        syn::visit_mut::visit_item_mut(self, item);
    }
    fn visit_type_param_mut(&mut self, _: &mut syn::TypeParam) {
        self.opaque = true;
    }

    fn visit_item_enum_mut(&mut self, item: &mut syn::ItemEnum) {
        if item.attrs.is_empty()
            && item.generics.params.is_empty()
            && !item.variants.is_empty()
            && item
                .variants
                .iter()
                .all(|v| v.attrs.is_empty() && matches!(v.fields, syn::Fields::Unit))
        {
            *self.enums.entry(item.ident.to_string()).or_default() += 1;
            self.enum_states.insert(
                item.ident.to_string(),
                item.variants.iter().map(|v| v.ident.to_string()).collect(),
            );
        }
        syn::visit_mut::visit_item_enum_mut(self, item);
    }
    fn visit_pat_mut(&mut self, pat: &mut Pat) {
        // syn represents a variant pattern with ExprPath too; it is not a constructor.
        if matches!(pat, Pat::Path(_)) {
            return;
        }
        syn::visit_mut::visit_pat_mut(self, pat);
    }
    fn visit_expr_mut(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Assign(a) => {
                self.note_allowed(&a.left);
                if let Expr::Path(left) = a.left.as_ref() {
                    if let Some(name) = local(left) {
                        let value = match a.right.as_ref() {
                            Expr::Path(p)
                                if a.attrs.is_empty()
                                    && left.attrs.is_empty()
                                    && p.attrs.is_empty() =>
                            {
                                variant(p)
                            }
                            _ => None,
                        };
                        self.writes.entry(name).or_default().push(value);
                    }
                }
            }
            Expr::Match(m) => {
                self.note_allowed(&m.expr);
                if let Expr::Path(p) = m.expr.as_ref() {
                    if let Some(name) = local(p) {
                        *self.dispatches.entry(name).or_default() += 1;
                    }
                }
            }
            Expr::Break(_) | Expr::Continue(_) => self.loop_escape = true,
            // A closure can capture the state without an explicit reference.
            Expr::Closure(_) | Expr::Async(_) | Expr::Await(_) | Expr::Yield(_) => {
                self.opaque = true
            }
            _ => {}
        }
        syn::visit_mut::visit_expr_mut(self, expr);
    }
    fn visit_expr_path_mut(&mut self, path: &mut ExprPath) {
        if let Some(name) = local(path) {
            *self.paths.entry(name).or_default() += 1;
        }
        if let Some(key) = variant(path) {
            *self.variants.entry(key).or_default() += 1;
        }
        syn::visit_mut::visit_expr_path_mut(self, path);
    }
    fn visit_pat_ident_mut(&mut self, pat: &mut syn::PatIdent) {
        *self.bindings.entry(pat.ident.to_string()).or_default() += 1;
        syn::visit_mut::visit_pat_ident_mut(self, pat);
    }
    fn visit_macro_mut(&mut self, _: &mut syn::Macro) {
        self.opaque = true;
    }
}
#[derive(Default)]
pub struct Coalesce {
    pub chains: usize,
    pub joined_steps: usize,
    pub compacted_arms: usize,
    compact: bool,
}
impl Coalesce {
    /// Additional, opt-in preparation experiment. No runtime decoding is added.
    pub fn compact() -> Self {
        Self {
            compact: true,
            ..Self::default()
        }
    }
}
struct Loops<'a> {
    uses: &'a Uses,
    total: &'a mut Coalesce,
}
impl VisitMut for Loops<'_> {
    fn visit_expr_loop_mut(&mut self, node: &mut syn::ExprLoop) {
        syn::visit_mut::visit_expr_loop_mut(self, node);
        // Never skip work before/after a dispatch, including an interrupt poll.
        let [Stmt::Expr(Expr::Match(m), _)] = node.body.stmts.as_mut_slice() else {
            return;
        };
        let Expr::Path(state) = m.expr.as_ref() else {
            return;
        };
        let Some(state) = local(state) else {
            return;
        };
        if !self.uses.safe(&state) || !m.attrs.is_empty() {
            return;
        }
        let mut enum_name = None;
        let mut indices = BTreeMap::new();
        for (i, arm) in m.arms.iter().enumerate() {
            if arm.guard.is_some() || !arm.attrs.is_empty() {
                return;
            }
            let Pat::Path(path) = &arm.pat else {
                return;
            };
            let Some((name, label)) = variant(path) else {
                return;
            };
            if enum_name.as_ref().is_some_and(|n| n != &name) || indices.insert(label, i).is_some()
            {
                return;
            }
            enum_name = Some(name);
        }
        let Some(name) = enum_name else {
            return;
        };
        // Associated constants may alias; only an actual local unit enum has distinct states.
        if self.uses.enums.get(&name) != Some(&1)
            || self.uses.type_names.get(&name) != Some(&1)
            || !indices.keys().all(|label| {
                self.uses
                    .enum_states
                    .get(&name)
                    .is_some_and(|states| states.contains(label))
            })
        {
            return;
        }
        let mut edges = BTreeMap::new();
        for (label, &i) in &indices {
            let Expr::Block(block) = m.arms[i].body.as_ref() else {
                continue;
            };
            let Some(Stmt::Expr(Expr::Assign(a), _)) = block.block.stmts.last() else {
                continue;
            };
            if !block.attrs.is_empty() || !a.attrs.is_empty() {
                continue;
            }
            let Expr::Path(left) = a.left.as_ref() else {
                continue;
            };
            let Expr::Path(right) = a.right.as_ref() else {
                continue;
            };
            let Some((target_enum, target)) = variant(right) else {
                continue;
            };
            if left.attrs.is_empty()
                && right.attrs.is_empty()
                && local(left).as_deref() == Some(&state)
                && target_enum == name
                && target != *label
                && indices.contains_key(&target)
                && self.uses.variants.get(&(name.clone(), target.clone())) == Some(&1)
            {
                edges.insert(label.clone(), target);
            }
        }
        let targets: BTreeSet<_> = edges.values().cloned().collect();
        // A target may be removed from the dispatch only when ALL values ever
        // assigned to this state (including its initial value) are literal
        // variants of this enum. Constructor counts alone cannot prove that:
        // an arbitrary call/transmute could otherwise enter any original arm.
        // Reject even nested break/continue conservatively: a continue inside
        // a copied intermediate step could return to dispatch at that step.
        let compact = self.total.compact
            && !self.uses.loop_escape
            && self.uses.dispatches.get(&state) == Some(&1)
            // C2Rust commonly declares the local without an initializer and
            // assigns the entry variant next. Rust's definite-initialization
            // check still applies; every later assignment is checked below.
            && (self.uses.uninitialized.contains(&state)
                || self.uses.initializers.get(&state).is_some_and(|v| {
                    v.as_ref()
                        .is_some_and(|(e, s)| e == &name && indices.contains_key(s))
                }))
            && self.uses.writes.get(&state).is_some_and(|writes| {
                writes.iter().all(|v| {
                    v.as_ref()
                        .is_some_and(|(e, s)| e == &name && indices.contains_key(s))
                })
            });
        let mut removed = BTreeSet::new();
        let original = m.arms.clone();
        for (head, &i) in &indices {
            if targets.contains(head) {
                continue;
            }
            let mut chain = vec![head.clone()];
            let mut seen = BTreeSet::from([head.clone()]);
            while let Some(next) = edges.get(chain.last().unwrap()) {
                // Unreachable closed cycles have no head; never unroll a cycle.
                if !seen.insert(next.clone()) {
                    break;
                }
                chain.push(next.clone());
            }
            if chain.len() < 2 {
                continue;
            }
            let mut joined: syn::Block = parse_quote!({});
            for label in &chain {
                // Distinct expression blocks preserve locals and Drop order.
                let body = original[indices[label]].body.as_ref().clone();
                joined
                    .stmts
                    .push(Stmt::Expr(body, Some(Default::default())));
            }
            let replacement: Expr = parse_quote!({ #joined });
            m.arms[i].body = Box::new(replacement);
            if compact {
                removed.extend(chain.iter().skip(1).cloned());
            }
            self.total.chains += 1;
            self.total.joined_steps += chain.len() - 1;
        }
        if !removed.is_empty() {
            m.arms.retain(|arm| {
                let Pat::Path(p) = &arm.pat else { return true };
                !variant(p).is_some_and(|(_, label)| removed.contains(&label))
            });
            let enum_ident = quote::format_ident!("{}", name);
            let paths: Vec<syn::Path> = removed
                .iter()
                .map(|label| {
                    let label = quote::format_ident!("{}", label);
                    parse_quote!(#enum_ident::#label)
                })
                .collect();
            // Keep enum values and all intermediate assignments unchanged.
            // A defensive abort, never unchecked UB, catches a broken proof.
            m.arms.push(parse_quote!(#(#paths)|* => {
                unreachable!("GamePKG compacted dispatch state reached")
            }));
            self.total.compacted_arms += removed.len();
        }
    }
}
impl VisitMut for Coalesce {
    fn visit_item_fn_mut(&mut self, function: &mut ItemFn) {
        let mut probe = function.clone();
        let mut uses = Uses::default();
        uses.visit_item_fn_mut(&mut probe);
        Loops {
            uses: &uses,
            total: self,
        }
        .visit_block_mut(&mut function.block);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn run(body: &str) -> (String, Coalesce) {
        let mut f: syn::File = syn::parse_str(body).unwrap();
        let mut pass = Coalesce::default();
        pass.visit_file_mut(&mut f);
        let out = prettyplease::unparse(&f);
        syn::parse_file(&out).unwrap();
        (out, pass)
    }
    const GRAPH: &str = r#"
fn f(p: *mut u32) {
 enum C2Rust_Block_0 {S_0,S_1,S_2,S_3}
 let mut c2rust_current_block_0 = C2Rust_Block_0::S_0;
 loop { match c2rust_current_block_0 {
 C2Rust_Block_0::S_0 => {let same=1; unsafe{p.write_volatile(same)}; c2rust_current_block_0=C2Rust_Block_0::S_1;}
 C2Rust_Block_0::S_1 => {let same=2; unsafe{p.write_volatile(same)}; c2rust_current_block_0=C2Rust_Block_0::S_2;}
 C2Rust_Block_0::S_2 => {if unsafe{p.read_volatile()}==2 {c2rust_current_block_0=C2Rust_Block_0::S_3;}else{c2rust_current_block_0=C2Rust_Block_0::S_0;}}
 C2Rust_Block_0::S_3 => return,
 } }
}"#;
    fn compact(body: &str) -> (String, Coalesce) {
        let mut f: syn::File = syn::parse_str(body).unwrap();
        let mut pass = Coalesce::compact();
        pass.visit_file_mut(&mut f);
        let out = prettyplease::unparse(&f);
        syn::parse_file(&out).unwrap();
        (out, pass)
    }
    #[test]
    fn compacts_only_dispatch_targets_and_keeps_assignments_scopes_and_enum() {
        let (out, pass) = compact(GRAPH);
        assert_eq!(pass.compacted_arms, 2);
        assert_eq!(out.matches("=>").count(), 3);
        assert_eq!(out.matches("let same = 2;").count(), 1);
        assert!(out.contains("unreachable!"));
        assert!(out.contains("c2rust_current_block_0 = C2Rust_Block_0::S_1;"));
        assert!(out.contains("c2rust_current_block_0 = C2Rust_Block_0::S_2;"));
        assert!(out.contains("S_1 | C2Rust_Block_0::S_2"));
        let (_, again) = compact(&out);
        assert_eq!(again.compacted_arms, 0);
    }
    #[test]
    fn compaction_refuses_arbitrary_state_values_even_without_literal_constructor() {
        for value in ["make_state()", "unsafe { std::mem::zeroed() }", "other"] {
            let source = GRAPH.replace("= C2Rust_Block_0::S_0;", &format!("= {value};"));
            assert_eq!(compact(&source).1.compacted_arms, 0);
            let source = GRAPH.replace(
                "c2rust_current_block_0=C2Rust_Block_0::S_3;",
                &format!("c2rust_current_block_0={value};"),
            );
            assert_eq!(compact(&source).1.compacted_arms, 0);
        }
    }
    #[test]
    fn compacts_c2rust_separate_declaration_and_entry_assignment() {
        let source = GRAPH.replace(
            "let mut c2rust_current_block_0 = C2Rust_Block_0::S_0;",
            "let mut c2rust_current_block_0: C2Rust_Block_0; c2rust_current_block_0 = C2Rust_Block_0::S_0;",
        );
        assert_eq!(compact(&source).1.compacted_arms, 2);
    }
    #[test]
    fn compaction_keeps_an_intermediate_state_used_as_an_entry() {
        let source = GRAPH.replace(
            "let mut c2rust_current_block_0 = C2Rust_Block_0::S_0;",
            "let mut c2rust_current_block_0 = C2Rust_Block_0::S_1;",
        );
        let (out, pass) = compact(&source);
        assert_eq!(pass.compacted_arms, 1); // S_2 only; S_1 is an entry.
        assert!(out.contains("C2Rust_Block_0::S_1 =>"));
    }
    #[test]
    fn compaction_refuses_continue_and_break_in_intermediate_steps() {
        for exit in ["continue", "break", "continue 'outer", "break 'outer"] {
            let source = GRAPH.replace(
                "let same=2;",
                &format!("if unsafe{{p.read_volatile()}}==0 {{ {exit}; }} let same=2;"),
            );
            let (_, pass) = compact(&source);
            assert!(pass.joined_steps > 0); // Original retained-arm pass is safe.
            assert_eq!(pass.compacted_arms, 0);
        }
    }
    #[test]
    fn compaction_refuses_a_second_state_observer_and_cfg_writes() {
        let source = GRAPH.replace(
            "let same=2;",
            "match c2rust_current_block_0 {_=>{}}; let same=2;",
        );
        assert_eq!(compact(&source).1.compacted_arms, 0);
        let source = GRAPH.replace(
            "c2rust_current_block_0=C2Rust_Block_0::S_3;",
            "#[cfg(unix)] c2rust_current_block_0=C2Rust_Block_0::S_3;",
        );
        assert_eq!(compact(&source).1.compacted_arms, 0);
    }
    #[test]
    fn joins_steps_keeps_arms_side_effects_and_scopes() {
        let (out, pass) = run(GRAPH);
        assert_eq!(pass.chains, 1);
        assert_eq!(pass.joined_steps, 2);
        assert_eq!(out.matches("=>").count(), 4);
        assert_eq!(out.matches("let same = 1;").count(), 1);
        assert_eq!(out.matches("let same = 2;").count(), 2);
        assert!(out.contains("write_volatile"));
        let (_, again) = run(&out);
        assert_eq!(again.joined_steps, 0);
    }
    #[test]
    fn accepts_unsuffixed_dispatch_and_named_return_state() {
        let source = GRAPH
            .replace("C2Rust_Block_0", "C2Rust_Block")
            .replace("c2rust_current_block_0", "c2rust_current_block")
            .replace("S_3", "done");
        let (_, pass) = run(&source);
        assert_eq!(pass.joined_steps, 2);
    }
    #[test]
    fn does_not_unroll_unreachable_closed_cycles() {
        let (_, pass) = run(r#"fn f(){enum C2Rust_Block {start,a,b}
            let mut c2rust_current_block=C2Rust_Block::start;
            loop {match c2rust_current_block {
                C2Rust_Block::start=>return,
                C2Rust_Block::a=>{c2rust_current_block=C2Rust_Block::b;},
                C2Rust_Block::b=>{c2rust_current_block=C2Rust_Block::a;},
            }}}
        "#);
        assert_eq!(pass.joined_steps, 0);
    }
    #[test]
    fn refuses_aliased_associated_constants_instead_of_enum_states() {
        let source=GRAPH.replace("enum C2Rust_Block_0 {S_0,S_1,S_2,S_3}",
            "struct C2Rust_Block_0; impl C2Rust_Block_0 {const S_0:bool=false; const S_1:bool=true; const S_2:bool=false; const S_3:bool=true;}");
        let (_, pass) = run(&source);
        assert_eq!(pass.joined_steps, 0);
    }
    #[test]
    fn does_not_inline_a_conditionally_compiled_transition() {
        let source = GRAPH.replace(
            "c2rust_current_block_0=C2Rust_Block_0::S_1;",
            r#"#[cfg(target_os="windows")] c2rust_current_block_0=C2Rust_Block_0::S_1;"#,
        );
        let (_, pass) = run(&source);
        assert_eq!(pass.joined_steps, 1); // Only the unconditional S_1 -> S_2 edge.
    }
    #[test]
    fn leaves_iteration_hooks_in_place() {
        let (_, pass) = run(&GRAPH.replace("loop { match", "loop { tick(); match"));
        assert_eq!(pass.joined_steps, 0);
    }
    #[test]
    fn refuses_escaped_state_even_before_loop() {
        let (_, pass) = run(&GRAPH.replace(
            "loop {",
            "let escaped=&raw mut c2rust_current_block_0; loop {",
        ));
        assert_eq!(pass.joined_steps, 0);
    }
    #[test]
    fn refuses_shadowed_state() {
        let (_, pass) = run(&GRAPH.replace(
            "let same=1;",
            "let mut c2rust_current_block_0=C2Rust_Block_0::S_0; let same=1;",
        ));
        assert_eq!(pass.joined_steps, 0);
    }
    #[test]
    fn refuses_macro_and_closure_capture() {
        for extra in [
            "opaque!();",
            "let capture=|| { c2rust_current_block_0=C2Rust_Block_0::S_0; };",
        ] {
            let (_, pass) = run(&GRAPH.replace("loop {", &format!("{extra} loop {{")));
            assert_eq!(pass.joined_steps, 0);
        }
    }
    #[test]
    fn keeps_guards() {
        let (_, pass) = run(&GRAPH.replace(
            "C2Rust_Block_0::S_1 =>",
            "C2Rust_Block_0::S_1 if condition() =>",
        ));
        assert_eq!(pass.joined_steps, 0);
    }
}
