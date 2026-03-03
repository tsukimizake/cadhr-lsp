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
        "hull" => Some(
            "**hull(A, B)**\n\nConvex hull of shapes A and B.",
        ),
        "translate" => Some("**translate(Shape, X, Y, Z)**\n\nMove shape by (X, Y, Z)."),
        "scale" => Some("**scale(Shape, X, Y, Z)**\n\nScale shape by factors (X, Y, Z)."),
        "rotate" => Some("**rotate(Shape, X, Y, Z)**\n\nRotate shape by (X, Y, Z) degrees."),
        "polygon" => Some("**polygon([p(X,Y), ...])**\n\n2D polygon profile from a list of points."),
        "circle" => Some("**circle(R)** / **circle(R, Segments)**\n\n2D circle profile with radius R. Default 32 segments."),
        "path" => Some("**path(Start, [Segments...])**\n\n2D path profile built from `line_to` and `bezier_to` segments.\n\nExample:\n```\npath(p(0,0), [\n  line_to(p(10,0)),\n  bezier_to(p(15,5), p(10,10)),\n  bezier_to(p(5,15), p(0,10), p(0,0))\n])\n```"),
        "line_to" => Some("**line_to(p(X,Y))**\n\nPath segment: straight line to the given point."),
        "bezier_to" => Some("**bezier_to(CP, End)** / **bezier_to(CP1, CP2, End)**\n\nPath segment: quadratic (1 control point) or cubic (2 control points) Bézier curve."),
        "linear_extrude" => Some("**linear_extrude(Profile, Height)**\n\nExtrude a 2D profile (polygon, circle, path) along the Z axis."),
        "complex_extrude" => Some("**complex_extrude(Profile, Height, Twist, ScaleX, ScaleY)**\n\nExtrude a 2D profile along the Z axis with twist (degrees) and top-face scaling."),
        "revolve" => Some("**revolve(Profile, Degrees)** / **revolve(Profile, Degrees, Segments)**\n\nRevolve a 2D profile around the Y axis. Default 32 segments."),
        "polyhedron" => Some("**polyhedron(Points, Faces)**\n\nConstruct a polyhedron from a list of 3D points and face index lists."),
        "stl" => Some("**stl(\"path/to/file.stl\")**\n\nImport a mesh from an STL file."),
        "control" => Some("**control(X, Y, Z)** / **control(X, Y, Z, Name)**\n\nDraggable control point in the viewport. Variables are bound to the drag position."),
        _ => None,
    }
}
