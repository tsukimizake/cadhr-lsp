use std::collections::HashMap;
use std::path::Path;

use tower_lsp::lsp_types::*;

use crate::clause_info::{ClauseInfo, UseInfo, collect_clauses, resolve_module_file};

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
            "hull",
            "hull(A, B)",
            "CSG: convex hull of A and B",
            "hull($1)",
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
        builtin(
            "sketchXY",
            "sketchXY([p(X,Y), ...])",
            "2D sketch on XY plane",
            "sketchXY([$1])",
        ),
        builtin(
            "sketchYZ",
            "sketchYZ([p(Y,Z), ...])",
            "2D sketch on YZ plane",
            "sketchYZ([$1])",
        ),
        builtin(
            "sketchXZ",
            "sketchXZ([p(X,Z), ...])",
            "2D sketch on XZ plane",
            "sketchXZ([$1])",
        ),
        builtin(
            "circle",
            "circle(R) / circle(R, Segments)",
            "2D profile: circle",
            "circle($1)",
        ),
        builtin(
            "path",
            "path(Start, [line_to(..), bezier_to(..), ...])",
            "2D profile: path with line and Bézier segments",
            "path(p($1), [$2])",
        ),
        builtin(
            "line_to",
            "line_to(p(X,Y))",
            "Path segment: straight line to point",
            "line_to(p($1))",
        ),
        builtin(
            "bezier_to",
            "bezier_to(CP, End) / bezier_to(CP1, CP2, End)",
            "Path segment: quadratic or cubic Bézier curve",
            "bezier_to(p($1), p($2))",
        ),
        builtin(
            "linear_extrude",
            "linear_extrude(Profile, Height)",
            "Extrude a 2D profile along Z axis",
            "linear_extrude($1)",
        ),
        builtin(
            "complex_extrude",
            "complex_extrude(Profile, Height, Twist, ScaleX, ScaleY)",
            "Extrude with twist and scaling",
            "complex_extrude($1)",
        ),
        builtin(
            "revolve",
            "revolve(Profile, Degrees) / revolve(Profile, Degrees, Segments)",
            "Revolve a 2D profile around Y axis",
            "revolve($1)",
        ),
        builtin(
            "polyhedron",
            "polyhedron(Points, Faces)",
            "Polyhedron from vertex list and face index lists",
            "polyhedron([$1], [$2])",
        ),
        builtin(
            "stl",
            "stl(\"path/to/file.stl\")",
            "Import mesh from STL file",
            "stl(\"$1\")",
        ),
        builtin(
            "color",
            "color(Shape, R, G, B)",
            "Set preview color (RGB: 0.0–1.0)",
            "color($1)",
        ),
        builtin(
            "control",
            "control(X, Y, Z) / control(X, Y, Z, Name)",
            "Draggable control point in the viewport",
            "control($1)",
        ),
        builtin(
            "bom",
            "bom(\"Name\", [prop(Value), ...])",
            "Bill of materials entry",
            "bom(\"$1\", [$2])",
        ),
        CompletionItem {
            label: "#use".to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("#use(\"module\") / #use(\"module\", expose([...]))".to_string()),
            documentation: Some(Documentation::String(
                "Import definitions from another .cadhr file".to_string(),
            )),
            insert_text: Some("#use(\"$1\").".to_string()),
            insert_text_format: Some(InsertTextFormat::SNIPPET),
            ..Default::default()
        },
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

pub fn module_completion_items(
    use_directives: &[UseInfo],
    current_file: &Path,
) -> Vec<CompletionItem> {
    let mut items = Vec::new();

    for ud in use_directives {
        let module_file = match resolve_module_file(&ud.module_path, current_file) {
            Some(f) => f,
            None => continue,
        };
        let source = match std::fs::read_to_string(&module_file) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let mut parser = tree_sitter::Parser::new();
        if parser
            .set_language(tree_sitter_cadhr_lang::language())
            .is_err()
        {
            continue;
        }
        let tree = match parser.parse(&source, None) {
            Some(t) => t,
            None => continue,
        };
        let clauses = collect_clauses(&tree, &source);
        let module_name = Path::new(&ud.module_path)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();

        for ci in &clauses {
            let qualified_name = format!("{}::{}", module_name, ci.head_name);
            let snippet = if ci.arity == 0 {
                qualified_name.clone()
            } else {
                format!("{}($1)", qualified_name)
            };
            items.push(CompletionItem {
                label: qualified_name,
                kind: Some(if ci.arity == 0 {
                    CompletionItemKind::CONSTANT
                } else {
                    CompletionItemKind::FUNCTION
                }),
                detail: Some(ci.head_text.clone()),
                documentation: ci.doc.clone().map(Documentation::String),
                insert_text: Some(snippet),
                insert_text_format: Some(InsertTextFormat::SNIPPET),
                ..Default::default()
            });

            // expose されたfunctorは非修飾版も追加
            if ud.expose.contains(&ci.head_name) {
                let snippet = if ci.arity == 0 {
                    ci.head_name.clone()
                } else {
                    format!("{}($1)", ci.head_name)
                };
                items.push(CompletionItem {
                    label: ci.head_name.clone(),
                    kind: Some(if ci.arity == 0 {
                        CompletionItemKind::CONSTANT
                    } else {
                        CompletionItemKind::FUNCTION
                    }),
                    detail: Some(format!("{} (from {})", ci.head_text, module_name)),
                    documentation: ci.doc.clone().map(Documentation::String),
                    insert_text: Some(snippet),
                    insert_text_format: Some(InsertTextFormat::SNIPPET),
                    ..Default::default()
                });
            }
        }
    }

    items
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
