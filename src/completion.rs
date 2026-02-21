use std::collections::HashMap;

use tower_lsp::lsp_types::*;

use crate::clause_info::ClauseInfo;

pub fn builtin_completion_items() -> Vec<CompletionItem> {
    vec![
        builtin(
            "cube",
            "cube(X, Y, Z)",
            "Primitive: axis-aligned box",
            "cube($1)",
        ),
        builtin(
            "sphere",
            "sphere(R) / sphere(R, Segments)",
            "Primitive: sphere",
            "sphere($1)",
        ),
        builtin(
            "cylinder",
            "cylinder(R, H) / cylinder(R, H, Segments)",
            "Primitive: cylinder",
            "cylinder($1)",
        ),
        builtin(
            "tetrahedron",
            "tetrahedron",
            "Primitive: regular tetrahedron",
            "tetrahedron",
        ),
        builtin(
            "union",
            "union(A, B)",
            "CSG: boolean union (A + B)",
            "union($1)",
        ),
        builtin(
            "difference",
            "difference(A, B)",
            "CSG: boolean difference (A - B)",
            "difference($1)",
        ),
        builtin(
            "intersection",
            "intersection(A, B)",
            "CSG: boolean intersection (A * B)",
            "intersection($1)",
        ),
        builtin(
            "translate",
            "translate(Shape, X, Y, Z)",
            "Transform: move shape",
            "translate($1)",
        ),
        builtin(
            "scale",
            "scale(Shape, X, Y, Z)",
            "Transform: scale shape",
            "scale($1)",
        ),
        builtin(
            "rotate",
            "rotate(Shape, X, Y, Z)",
            "Transform: rotate shape (degrees)",
            "rotate($1)",
        ),
    ]
}

pub fn user_defined_completion_items(
    clauses: &[ClauseInfo],
    builtins: &[CompletionItem],
) -> Vec<CompletionItem> {
    let builtin_names: std::collections::HashSet<&str> =
        builtins.iter().map(|item| item.label.as_str()).collect();

    // (name, arity) -> (detail, doc) — keep first occurrence
    let mut heads: HashMap<(String, usize), (String, Option<String>)> = HashMap::new();

    for ci in clauses {
        if builtin_names.contains(ci.head_name.as_str()) {
            continue;
        }
        let detail = if ci.arity == 0 {
            ci.head_name.clone()
        } else {
            let args: Vec<String> = (0..ci.arity)
                .map(|i| String::from(('A' as u8 + i as u8) as char))
                .collect();
            format!("{}({})", ci.head_name, args.join(", "))
        };
        heads
            .entry((ci.head_name.clone(), ci.arity))
            .or_insert((detail, ci.doc.clone()));
    }

    heads
        .into_iter()
        .map(|((name, arity), (detail, doc))| {
            let snippet = if arity == 0 {
                name.clone()
            } else {
                format!("{}($1)", name)
            };
            CompletionItem {
                label: name,
                kind: Some(if arity == 0 {
                    CompletionItemKind::CONSTANT
                } else {
                    CompletionItemKind::FUNCTION
                }),
                detail: Some(detail),
                documentation: doc.map(Documentation::String),
                insert_text: Some(snippet),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                ..Default::default()
            }
        })
        .collect()
}

fn builtin(label: &str, detail: &str, doc: &str, snippet: &str) -> CompletionItem {
    CompletionItem {
        label: label.to_string(),
        kind: Some(CompletionItemKind::FUNCTION),
        detail: Some(detail.to_string()),
        documentation: Some(Documentation::String(doc.to_string())),
        insert_text: Some(snippet.to_string()),
        insert_text_format: Some(InsertTextFormat::SNIPPET),
        ..Default::default()
    }
}
