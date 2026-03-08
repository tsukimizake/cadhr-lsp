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
        "sketchXY" => Some("**sketchXY([p(X,Y), ...])**\n\nXY平面上の2Dスケッチ。押し出し方向は+Z。"),
        "sketchYZ" => Some("**sketchYZ([p(Y,Z), ...])**\n\nYZ平面上の2Dスケッチ。押し出し方向は+X。"),
        "sketchXZ" => Some("**sketchXZ([p(X,Z), ...])**\n\nXZ平面上の2Dスケッチ。押し出し方向は+Y。"),
        "circle" => Some("**circle(R)** / **circle(R, Segments)**\n\n2D circle profile with radius R. Default 32 segments."),
        "path" => Some("**path(Start, [Segments...])**\n\n2D path profile built from `line_to` and `bezier_to` segments.\n\nExample:\n```\npath(p(0,0), [\n  line_to(p(10,0)),\n  bezier_to(p(15,5), p(10,10)),\n  bezier_to(p(5,15), p(0,10), p(0,0))\n])\n```"),
        "line_to" => Some("**line_to(p(X,Y))**\n\nPath segment: straight line to the given point."),
        "bezier_to" => Some("**bezier_to(CP, End)** / **bezier_to(CP1, CP2, End)**\n\nPath segment: quadratic (1 control point) or cubic (2 control points) Bézier curve."),
        "linear_extrude" => Some("**linear_extrude(Profile, Height)**\n\nExtrude a 2D profile (polygon, circle, path) along the Z axis."),
        "complex_extrude" => Some("**complex_extrude(Profile, Height, Twist, ScaleX, ScaleY)**\n\nExtrude a 2D profile along the Z axis with twist (degrees) and top-face scaling."),
        "revolve" => Some("**revolve(Profile, Degrees)** / **revolve(Profile, Degrees, Segments)**\n\nRevolve a 2D profile around the Y axis. Default 32 segments."),
        "sweep_extrude" => Some("**sweep_extrude(Profile, Path)**\n\nSweep a 2D profile along a path. The path is interpreted in the XZ plane, and the profile is oriented perpendicular to the path tangent.\n\nExample:\n```\nsketchXY([p(0,0), p(5,0), p(5,5), p(0,5)])\n  |> sweep_extrude(path(p(0,0), [line_to(p(0,20)), bezier_to(p(5,10), p(10,30))]))\n```"),
        "polyhedron" => Some("**polyhedron(Points, Faces)**\n\nConstruct a polyhedron from a list of 3D points and face index lists."),
        "stl" => Some("**stl(\"path/to/file.stl\")**\n\nImport a mesh from an STL file."),
        "control" => Some("**control(X, Y, Z)** / **control(X, Y, Z, Name)**\n\nDraggable control point in the viewport. Variables are bound to the drag position."),
        "bom" => Some("**bom(\"Name\", [prop(Value), ...])**\n\nBill of materials entry. Properties are functor(value) pairs in a list.\n\nExample:\n```\nbom(\"aluminum_extrusion\", [len(100), width(50)])\n```"),
        "#use" => Some("**#use(\"module\")**\n\nImport definitions from another `.cadhr` file.\n\n- `#use(\"bolts\").` — access as `bolts::xxx`\n- `#use(\"bolts\", expose([m5])).` — `m5` also available unqualified"),
        _ => None,
    }
}
