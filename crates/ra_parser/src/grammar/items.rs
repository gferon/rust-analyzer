mod consts;
mod nominal;
mod traits;
mod use_item;

pub(crate) use self::{
    expressions::{match_arm_list, named_field_list},
    nominal::{enum_variant_list, named_field_def_list},
    traits::{impl_item_list, trait_item_list},
    use_item::use_tree_list,
};
use super::*;

// test mod_contents
// fn foo() {}
// macro_rules! foo {}
// foo::bar!();
// super::baz! {}
// struct S;
pub(super) fn mod_contents(p: &mut Parser, stop_on_r_curly: bool) {
    attributes::inner_attributes(p);
    while !p.at(EOF) && !(stop_on_r_curly && p.at(R_CURLY)) {
        item_or_macro(p, stop_on_r_curly, ItemFlavor::Mod)
    }
}

pub(super) enum ItemFlavor {
    Mod,
    Trait,
}

pub(super) const ITEM_RECOVERY_SET: TokenSet = token_set![
    FN_KW, STRUCT_KW, ENUM_KW, IMPL_KW, TRAIT_KW, CONST_KW, STATIC_KW, LET_KW, MOD_KW, PUB_KW,
    CRATE_KW
];

pub(super) fn item_or_macro(p: &mut Parser, stop_on_r_curly: bool, flavor: ItemFlavor) {
    let m = p.start();
    attributes::outer_attributes(p);
    let m = match maybe_item(p, m, flavor) {
        Ok(()) => return,
        Err(m) => m,
    };
    if paths::is_path_start(p) {
        match macro_call(p) {
            BlockLike::Block => (),
            BlockLike::NotBlock => {
                p.expect(SEMI);
            }
        }
        m.complete(p, MACRO_CALL);
    } else {
        m.abandon(p);
        if p.at(L_CURLY) {
            error_block(p, "expected an item");
        } else if p.at(R_CURLY) && !stop_on_r_curly {
            let e = p.start();
            p.error("unmatched `}`");
            p.bump();
            e.complete(p, ERROR);
        } else if !p.at(EOF) && !p.at(R_CURLY) {
            p.err_and_bump("expected an item");
        } else {
            p.error("expected an item");
        }
    }
}

pub(super) fn maybe_item(p: &mut Parser, m: Marker, flavor: ItemFlavor) -> Result<(), Marker> {
    // test_err pub_expr
    // fn foo() { pub 92; }
    let has_visibility = opt_visibility(p);

    let m = match items_without_modifiers(p, m) {
        Ok(()) => return Ok(()),
        Err(m) => m,
    };

    let mut has_mods = false;

    // modifiers
    has_mods |= p.eat(CONST_KW);

    // test_err unsafe_block_in_mod
    // fn foo(){} unsafe { } fn bar(){}
    if p.at(UNSAFE_KW) && p.nth(1) != L_CURLY {
        p.eat(UNSAFE_KW);
        has_mods = true;
    }

    // test_err async_without_semicolon
    // fn foo() { let _ = async {} }
    if p.at(ASYNC_KW) && p.nth(1) != L_CURLY && p.nth(1) != MOVE_KW && p.nth(1) != PIPE {
        p.eat(ASYNC_KW);
        has_mods = true;
    }

    if p.at(EXTERN_KW) {
        has_mods = true;
        abi(p);
    }
    if p.at(IDENT) && p.at_contextual_kw("auto") && p.nth(1) == TRAIT_KW {
        p.bump_remap(AUTO_KW);
        has_mods = true;
    }
    if p.at(IDENT) && p.at_contextual_kw("default") && p.nth(1) == IMPL_KW {
        p.bump_remap(DEFAULT_KW);
        has_mods = true;
    }

    // items
    match p.current() {
        // test async_fn
        // async fn foo() {}

        // test extern_fn
        // extern fn foo() {}

        // test const_fn
        // const fn foo() {}

        // test const_unsafe_fn
        // const unsafe fn foo() {}

        // test unsafe_extern_fn
        // unsafe extern "C" fn foo() {}

        // test unsafe_fn
        // unsafe fn foo() {}

        // test combined_fns
        // unsafe async fn foo() {}
        // const unsafe fn bar() {}

        // test_err wrong_order_fns
        // async unsafe fn foo() {}
        // unsafe const fn bar() {}
        FN_KW => {
            fn_def(p, flavor);
            m.complete(p, FN_DEF);
        }

        // test unsafe_trait
        // unsafe trait T {}

        // test auto_trait
        // auto trait T {}

        // test unsafe_auto_trait
        // unsafe auto trait T {}
        TRAIT_KW => {
            traits::trait_def(p);
            m.complete(p, TRAIT_DEF);
        }

        // test unsafe_impl
        // unsafe impl Foo {}

        // test default_impl
        // default impl Foo {}

        // test unsafe_default_impl
        // unsafe default impl Foo {}
        IMPL_KW => {
            traits::impl_block(p);
            m.complete(p, IMPL_BLOCK);
        }
        _ => {
            if !has_visibility && !has_mods {
                return Err(m);
            } else {
                if has_mods {
                    p.error("expected fn, trait or impl");
                } else {
                    p.error("expected an item");
                }
                m.complete(p, ERROR);
            }
        }
    }
    Ok(())
}

fn items_without_modifiers(p: &mut Parser, m: Marker) -> Result<(), Marker> {
    let la = p.nth(1);
    match p.current() {
        // test extern_crate
        // extern crate foo;
        EXTERN_KW if la == CRATE_KW => extern_crate_item(p, m),
        TYPE_KW => type_def(p, m),
        MOD_KW => mod_item(p, m),
        STRUCT_KW => {
            // test struct_items
            // struct Foo;
            // struct Foo {}
            // struct Foo();
            // struct Foo(String, usize);
            // struct Foo {
            //     a: i32,
            //     b: f32,
            // }
            nominal::struct_def(p, m, STRUCT_KW);
        }
        IDENT if p.at_contextual_kw("union") && p.nth(1) == IDENT => {
            // test union_items
            // union Foo {}
            // union Foo {
            //     a: i32,
            //     b: f32,
            // }
            nominal::struct_def(p, m, UNION_KW);
        }
        ENUM_KW => nominal::enum_def(p, m),
        USE_KW => use_item::use_item(p, m),
        CONST_KW if (la == IDENT || la == MUT_KW) => consts::const_def(p, m),
        STATIC_KW => consts::static_def(p, m),
        // test extern_block
        // extern {}
        EXTERN_KW
            if la == L_CURLY || ((la == STRING || la == RAW_STRING) && p.nth(2) == L_CURLY) =>
        {
            abi(p);
            extern_item_list(p);
            m.complete(p, EXTERN_BLOCK);
        }
        _ => return Err(m),
    };
    if p.at(SEMI) {
        p.err_and_bump(
            "expected item, found `;`\n\
             consider removing this semicolon",
        );
    }
    Ok(())
}

fn extern_crate_item(p: &mut Parser, m: Marker) {
    assert!(p.at(EXTERN_KW));
    p.bump();
    assert!(p.at(CRATE_KW));
    p.bump();
    name_ref(p);
    opt_alias(p);
    p.expect(SEMI);
    m.complete(p, EXTERN_CRATE_ITEM);
}

pub(crate) fn extern_item_list(p: &mut Parser) {
    assert!(p.at(L_CURLY));
    let m = p.start();
    p.bump();
    mod_contents(p, true);
    p.expect(R_CURLY);
    m.complete(p, EXTERN_ITEM_LIST);
}

fn fn_def(p: &mut Parser, flavor: ItemFlavor) {
    assert!(p.at(FN_KW));
    p.bump();

    name_r(p, ITEM_RECOVERY_SET);
    // test function_type_params
    // fn foo<T: Clone + Copy>(){}
    type_params::opt_type_param_list(p);

    if p.at(L_PAREN) {
        match flavor {
            ItemFlavor::Mod => params::param_list(p),
            ItemFlavor::Trait => params::param_list_opt_patterns(p),
        }
    } else {
        p.error("expected function arguments");
    }
    // test function_ret_type
    // fn foo() {}
    // fn bar() -> () {}
    opt_fn_ret_type(p);

    // test function_where_clause
    // fn foo<T>() where T: Copy {}
    type_params::opt_where_clause(p);

    // test fn_decl
    // trait T { fn foo(); }
    if p.at(SEMI) {
        p.bump();
    } else {
        expressions::block(p)
    }
}

// test type_item
// type Foo = Bar;
fn type_def(p: &mut Parser, m: Marker) {
    assert!(p.at(TYPE_KW));
    p.bump();

    name(p);

    // test type_item_type_params
    // type Result<T> = ();
    type_params::opt_type_param_list(p);

    if p.at(COLON) {
        type_params::bounds(p);
    }

    // test type_item_where_clause
    // type Foo where Foo: Copy = ();
    type_params::opt_where_clause(p);

    if p.eat(EQ) {
        types::type_(p);
    }
    p.expect(SEMI);
    m.complete(p, TYPE_ALIAS_DEF);
}

pub(crate) fn mod_item(p: &mut Parser, m: Marker) {
    assert!(p.at(MOD_KW));
    p.bump();

    name(p);
    if p.at(L_CURLY) {
        mod_item_list(p);
    } else if !p.eat(SEMI) {
        p.error("expected `;` or `{`");
    }
    m.complete(p, MODULE);
}

pub(crate) fn mod_item_list(p: &mut Parser) {
    assert!(p.at(L_CURLY));
    let m = p.start();
    p.bump();
    mod_contents(p, true);
    p.expect(R_CURLY);
    m.complete(p, ITEM_LIST);
}

fn macro_call(p: &mut Parser) -> BlockLike {
    assert!(paths::is_path_start(p));
    paths::use_path(p);
    macro_call_after_excl(p)
}

pub(super) fn macro_call_after_excl(p: &mut Parser) -> BlockLike {
    p.expect(EXCL);
    if p.at(IDENT) {
        name(p);
    }
    match p.current() {
        L_CURLY => {
            token_tree(p);
            BlockLike::Block
        }
        L_PAREN | L_BRACK => {
            token_tree(p);
            BlockLike::NotBlock
        }
        _ => {
            p.error("expected `{`, `[`, `(`");
            BlockLike::NotBlock
        }
    }
}

pub(crate) fn token_tree(p: &mut Parser) {
    let closing_paren_kind = match p.current() {
        L_CURLY => R_CURLY,
        L_PAREN => R_PAREN,
        L_BRACK => R_BRACK,
        _ => unreachable!(),
    };
    let m = p.start();
    p.bump();
    while !p.at(EOF) && !p.at(closing_paren_kind) {
        match p.current() {
            L_CURLY | L_PAREN | L_BRACK => token_tree(p),
            R_CURLY => {
                p.error("unmatched `}`");
                m.complete(p, TOKEN_TREE);
                return;
            }
            R_PAREN | R_BRACK => p.err_and_bump("unmatched brace"),
            _ => p.bump_raw(),
        }
    }
    p.expect(closing_paren_kind);
    m.complete(p, TOKEN_TREE);
}
