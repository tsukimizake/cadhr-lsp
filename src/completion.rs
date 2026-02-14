use tower_lsp::lsp_types::*;

pub fn completion_items() -> Vec<CompletionItem> {
    vec![
        builtin(
            "cube",
            "cube(X, Y, Z)",
            "Primitive: axis-aligned box",
            "cube(${1:x}, ${2:y}, ${3:z})",
        ),
        builtin(
            "sphere",
            "sphere(R) / sphere(R, Segments)",
            "Primitive: sphere",
            "sphere(${1:r})",
        ),
        builtin(
            "cylinder",
            "cylinder(R, H) / cylinder(R, H, Segments)",
            "Primitive: cylinder",
            "cylinder(${1:r}, ${2:h})",
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
            "union(${1:a}, ${2:b})",
        ),
        builtin(
            "difference",
            "difference(A, B)",
            "CSG: boolean difference (A - B)",
            "difference(${1:a}, ${2:b})",
        ),
        builtin(
            "intersection",
            "intersection(A, B)",
            "CSG: boolean intersection (A * B)",
            "intersection(${1:a}, ${2:b})",
        ),
        builtin(
            "translate",
            "translate(Shape, X, Y, Z)",
            "Transform: move shape",
            "translate(${1:shape}, ${2:x}, ${3:y}, ${4:z})",
        ),
        builtin(
            "scale",
            "scale(Shape, X, Y, Z)",
            "Transform: scale shape",
            "scale(${1:shape}, ${2:x}, ${3:y}, ${4:z})",
        ),
        builtin(
            "rotate",
            "rotate(Shape, X, Y, Z)",
            "Transform: rotate shape (degrees)",
            "rotate(${1:shape}, ${2:x}, ${3:y}, ${4:z})",
        ),
    ]
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
