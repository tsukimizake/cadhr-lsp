use tower_lsp::lsp_types::*;

use crate::clause_info::ClauseInfo;

pub fn hover_info(clauses: &[ClauseInfo], name: &str, atom_range: Range) -> Option<Hover> {
    let doc = functor_doc(name)
        .map(|s| s.to_string())
        .or_else(|| user_defined_doc(clauses, name));
    let doc = doc?;

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: doc,
        }),
        range: Some(atom_range),
    })
}

fn user_defined_doc(clauses: &[ClauseInfo], name: &str) -> Option<String> {
    let ci = clauses.iter().find(|c| c.head_name == name)?;

    let mut parts = Vec::new();
    parts.push(format!("**{}**", ci.head_text));
    if let Some(ref d) = ci.doc {
        parts.push(String::new());
        parts.push(d.clone());
    }
    Some(parts.join("\n"))
}

fn functor_doc(name: &str) -> Option<&'static str> {
    match name {
        "cube" => Some("**cube(X, Y, Z)**\n\nAxis-aligned box with dimensions X, Y, Z."),
        "sphere" => Some(
            "**sphere(R)** / **sphere(R, Segments)**\n\nSphere with radius R. Default 32 segments.",
        ),
        "cylinder" => Some(
            "**cylinder(R, H)** / **cylinder(R, H, Segments)**\n\nCylinder with radius R and height H. Default 32 segments.",
        ),
        "tetrahedron" => Some("**tetrahedron**\n\nRegular tetrahedron."),
        "union" => {
            Some("**union(A, B)**\n\nBoolean union of shapes A and B. Also available as `A + B`.")
        }
        "difference" => Some(
            "**difference(A, B)**\n\nBoolean difference: A minus B. Also available as `A - B`.",
        ),
        "intersection" => Some(
            "**intersection(A, B)**\n\nBoolean intersection of shapes A and B. Also available as `A * B`.",
        ),
        "translate" => Some("**translate(Shape, X, Y, Z)**\n\nMove shape by (X, Y, Z)."),
        "scale" => Some("**scale(Shape, X, Y, Z)**\n\nScale shape by factors (X, Y, Z)."),
        "rotate" => Some("**rotate(Shape, X, Y, Z)**\n\nRotate shape by (X, Y, Z) degrees."),
        _ => None,
    }
}
