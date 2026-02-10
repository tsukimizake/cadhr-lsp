use tower_lsp::lsp_types::*;
use tree_sitter::Tree;

pub fn hover_info(tree: &Tree, source: &str, position: Position) -> Option<Hover> {
    let point = tree_sitter::Point {
        row: position.line as usize,
        column: position.character as usize,
    };

    let node = tree
        .root_node()
        .descendant_for_point_range(point, point)?;

    // Walk up to find the atom node (functor name)
    let atom_node = if node.kind() == "unquoted_atom" {
        Some(node)
    } else if node.kind() == "atom" {
        node.child(0) // unquoted_atom inside atom
    } else {
        None
    };

    let atom_node = atom_node?;
    let name = atom_node.utf8_text(source.as_bytes()).ok()?;

    let doc = functor_doc(name)?;

    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: doc.to_string(),
        }),
        range: Some(Range {
            start: Position {
                line: atom_node.start_position().row as u32,
                character: atom_node.start_position().column as u32,
            },
            end: Position {
                line: atom_node.end_position().row as u32,
                character: atom_node.end_position().column as u32,
            },
        }),
    })
}

fn functor_doc(name: &str) -> Option<&'static str> {
    match name {
        "cube" => Some("**cube(X, Y, Z)**\n\nAxis-aligned box with dimensions X, Y, Z."),
        "sphere" => Some("**sphere(R)** / **sphere(R, Segments)**\n\nSphere with radius R. Default 32 segments."),
        "cylinder" => Some("**cylinder(R, H)** / **cylinder(R, H, Segments)**\n\nCylinder with radius R and height H. Default 32 segments."),
        "tetrahedron" => Some("**tetrahedron**\n\nRegular tetrahedron."),
        "union" => Some("**union(A, B)**\n\nBoolean union of shapes A and B. Also available as `A + B`."),
        "difference" => Some("**difference(A, B)**\n\nBoolean difference: A minus B. Also available as `A - B`."),
        "intersection" => Some("**intersection(A, B)**\n\nBoolean intersection of shapes A and B. Also available as `A * B`."),
        "translate" => Some("**translate(Shape, X, Y, Z)**\n\nMove shape by (X, Y, Z)."),
        "scale" => Some("**scale(Shape, X, Y, Z)**\n\nScale shape by factors (X, Y, Z)."),
        "rotate" => Some("**rotate(Shape, X, Y, Z)**\n\nRotate shape by (X, Y, Z) degrees."),
        _ => None,
    }
}
