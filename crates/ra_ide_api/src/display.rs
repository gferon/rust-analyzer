//! This module contains utilities for turning SyntaxNodes and HIR types
//! into types that may be used to render in a UI.

mod function_signature;
mod navigation_target;
mod structure;

use crate::db::RootDatabase;
use ra_syntax::{ast::{self, AstNode, TypeParamsOwner}, SyntaxKind::{ATTR, COMMENT}};

pub use navigation_target::NavigationTarget;
pub use structure::{StructureNode, file_structure};
pub use function_signature::FunctionSignature;

pub(crate) fn function_label(node: &ast::FnDef) -> String {
    FunctionSignature::from(node).to_string()
}

pub(crate) fn const_label(node: &ast::ConstDef) -> String {
    let label: String = node
        .syntax()
        .children_with_tokens()
        .filter(|child| !(child.kind() == COMMENT || child.kind() == ATTR))
        .map(|node| node.to_string())
        .collect();

    label.trim().to_owned()
}

pub(crate) fn type_label(node: &ast::TypeAliasDef) -> String {
    let label: String = node
        .syntax()
        .children_with_tokens()
        .filter(|child| !(child.kind() == COMMENT || child.kind() == ATTR))
        .map(|node| node.to_string())
        .collect();

    label.trim().to_owned()
}

pub(crate) fn generic_parameters<N: TypeParamsOwner>(node: &N) -> Vec<String> {
    let mut res = vec![];
    if let Some(type_params) = node.type_param_list() {
        res.extend(type_params.lifetime_params().map(|p| p.syntax().text().to_string()));
        res.extend(type_params.type_params().map(|p| p.syntax().text().to_string()));
    }
    res
}

pub(crate) fn where_predicates<N: TypeParamsOwner>(node: &N) -> Vec<String> {
    let mut res = vec![];
    if let Some(clause) = node.where_clause() {
        res.extend(clause.predicates().map(|p| p.syntax().text().to_string()));
    }
    res
}

pub(crate) fn rust_code_markup<CODE: AsRef<str>>(val: CODE) -> String {
    rust_code_markup_with_doc::<_, &str>(val, None)
}

pub(crate) fn rust_code_markup_with_doc<CODE, DOC>(val: CODE, doc: Option<DOC>) -> String
where
    CODE: AsRef<str>,
    DOC: AsRef<str>,
{
    if let Some(doc) = doc {
        format!("```rust\n{}\n```\n\n{}", val.as_ref(), doc.as_ref())
    } else {
        format!("```rust\n{}\n```", val.as_ref())
    }
}

// FIXME: this should not really use navigation target. Rather, approximately
// resolved symbol should return a `DefId`.
pub(crate) fn doc_text_for(db: &RootDatabase, nav: NavigationTarget) -> Option<String> {
    match (nav.description(db), nav.docs(db)) {
        (Some(desc), docs) => Some(rust_code_markup_with_doc(desc, docs)),
        (None, Some(docs)) => Some(docs),
        _ => None,
    }
}
