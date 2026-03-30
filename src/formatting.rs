use pretty::{Arena, DocAllocator, DocBuilder};
use tower_lsp::lsp_types::{Position, Range, TextEdit};
use tree_sitter::Node;

const WIDTH: usize = 80;
const INDENT: isize = 4;

type D<'a> = DocBuilder<'a, Arena<'a>>;

pub fn format_document(tree: &tree_sitter::Tree, source: &str) -> Vec<TextEdit> {
    let root = tree.root_node();
    if root.has_error() {
        return vec![];
    }

    let formatted = format_source_file(root, source.as_bytes());
    if formatted == source {
        return vec![];
    }

    let end = root.end_position();
    vec![TextEdit {
        range: Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: end.row as u32,
                character: end.column as u32,
            },
        },
        new_text: formatted,
    }]
}

fn is_comment(node: &Node) -> bool {
    matches!(node.kind(), "line_comment" | "block_comment")
}

fn node_text<'a>(node: &Node, src: &'a [u8]) -> &'a str {
    node.utf8_text(src).unwrap_or("")
}

fn format_source_file(node: Node, src: &[u8]) -> String {
    let arena = Arena::new();
    let doc = source_file_doc(&arena, node, src);
    let mut output = String::new();
    doc.render_fmt(WIDTH, &mut output).unwrap();
    output
}

// ============================================================
// Top-level structure
// ============================================================

fn source_file_doc<'a>(arena: &'a Arena<'a>, node: Node, src: &[u8]) -> D<'a> {
    let mut parts: Vec<D<'a>> = Vec::new();
    let mut cursor = node.walk();
    let mut prev_was_clause = false;

    for child in node.children(&mut cursor) {
        let (doc, is_clause) = match child.kind() {
            "clause" => (clause_doc(arena, child, src), true),
            "use_directive" => (arena.text(node_text(&child, src).trim_end().to_string()), false),
            _ if is_comment(&child) => {
                (arena.text(node_text(&child, src).trim_end().to_string()), false)
            }
            _ => continue,
        };

        if !parts.is_empty() {
            let sep = if prev_was_clause {
                arena.hardline().append(arena.hardline())
            } else {
                arena.hardline()
            };
            parts.push(sep);
        }
        parts.push(doc);
        prev_was_clause = is_clause;
    }

    if parts.is_empty() {
        arena.nil()
    } else {
        arena.concat(parts).append(arena.hardline())
    }
}

fn clause_doc<'a>(arena: &'a Arena<'a>, node: Node, src: &[u8]) -> D<'a> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "fact" => return fact_doc(arena, child, src),
            "rule" => return rule_doc(arena, child, src),
            _ => {}
        }
    }
    arena.text(node_text(&node, src).to_string())
}

fn fact_doc<'a>(arena: &'a Arena<'a>, node: Node, src: &[u8]) -> D<'a> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "term" {
            return node_doc(arena, child, src).append(arena.text("."));
        }
    }
    arena.text(node_text(&node, src).to_string())
}

fn rule_doc<'a>(arena: &'a Arena<'a>, node: Node, src: &[u8]) -> D<'a> {
    let mut head = arena.nil();
    let mut goals = arena.nil();
    let mut comments_before_goals: Vec<D<'a>> = Vec::new();
    let mut seen_neck = false;
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        match child.kind() {
            "term" => head = node_doc(arena, child, src),
            "goals" => goals = goals_doc(arena, child, src),
            ":-" => seen_neck = true,
            _ if is_comment(&child) && seen_neck => {
                comments_before_goals
                    .push(arena.text(node_text(&child, src).trim_end().to_string()));
            }
            _ => {}
        }
    }

    let indent_str = arena.text(" ".repeat(INDENT as usize));
    let mut result = head.append(arena.text(" :-")).append(arena.hardline());
    for c in comments_before_goals {
        result = result.append(indent_str.clone()).append(c).append(arena.hardline());
    }
    result.append(goals).append(arena.text("."))
}

// ============================================================
// Goals (always multiline, 4-space indent, blank line preservation)
// ============================================================

fn goals_doc<'a>(arena: &'a Arena<'a>, node: Node, src: &[u8]) -> D<'a> {
    let mut items: Vec<(D<'a>, bool, usize)> = Vec::new(); // (doc, is_goal, source_row)
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        match child.kind() {
            "goal" | "term" => {
                items.push((goal_doc(arena, child, src), true, child.start_position().row));
            }
            _ if is_comment(&child) => {
                items.push((
                    arena.text(node_text(&child, src).trim_end().to_string()),
                    false,
                    child.start_position().row,
                ));
            }
            _ => {}
        }
    }

    let indent_str = arena.text(" ".repeat(INDENT as usize));
    let goal_count = items.iter().filter(|(_, is_goal, _)| *is_goal).count();
    let mut parts: Vec<D<'a>> = Vec::new();
    let mut goal_idx = 0;
    let mut prev_end_row: Option<usize> = None;

    for (doc, is_goal, start_row) in &items {
        if let Some(prev) = prev_end_row {
            let blank_lines = start_row.saturating_sub(prev).saturating_sub(1);
            for _ in 0..blank_lines {
                parts.push(arena.hardline());
            }
        }
        if *is_goal {
            goal_idx += 1;
            let line = if goal_idx < goal_count {
                indent_str.clone().append(doc.clone()).append(arena.text(","))
            } else {
                indent_str.clone().append(doc.clone())
            };
            parts.push(line);
            prev_end_row = Some(start_row + node_line_count(&doc, arena));
        } else {
            parts.push(indent_str.clone().append(doc.clone()));
            prev_end_row = Some(*start_row);
        }
        parts.push(arena.hardline());
    }

    // Remove trailing hardline (the rule_doc adds the final ".")
    if !parts.is_empty() {
        parts.pop();
    }

    arena.concat(parts)
}

fn node_line_count(_doc: &D<'_>, _arena: &Arena<'_>) -> usize {
    0 // goals are typically single-line; multiline handled by source row tracking
}

fn goal_doc<'a>(arena: &'a Arena<'a>, node: Node, src: &[u8]) -> D<'a> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.is_named() && !is_comment(&child) {
            return match child.kind() {
                "eq_constraint" => eq_constraint_doc(arena, child, src),
                _ => node_doc(arena, child, src),
            };
        }
    }
    node_doc(arena, node, src)
}

fn eq_constraint_doc<'a>(arena: &'a Arena<'a>, node: Node, src: &[u8]) -> D<'a> {
    let mut terms: Vec<D<'a>> = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "term" {
            terms.push(node_doc(arena, child, src));
        }
    }
    if terms.len() == 2 {
        terms.remove(0)
            .append(arena.text(" = "))
            .append(terms.remove(0))
    } else {
        arena.text(node_text(&node, src).to_string())
    }
}

// ============================================================
// General node dispatch
// ============================================================

fn node_doc<'a>(arena: &'a Arena<'a>, node: Node, src: &[u8]) -> D<'a> {
    match node.kind() {
        "term" | "primary_term" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.is_named() && !is_comment(&child) {
                    return node_doc(arena, child, src);
                }
            }
            arena.text(node_text(&node, src).to_string())
        }
        "pipe_expr" | "add_expr" | "mul_expr" => infix_doc(arena, node, src),
        "struct" => struct_doc(arena, node, src),
        "list" => list_doc(arena, node, src),
        "paren_expr" => paren_doc(arena, node, src),
        "annotated_var" => annotated_var_doc(arena, node, src),
        "qualified_atom" => qualified_atom_doc(arena, node, src),
        _ => arena.text(node_text(&node, src).to_string()),
    }
}

// ============================================================
// Infix expressions (pipe_expr, add_expr, mul_expr)
// ============================================================

enum InfixPart<'a> {
    Operand(D<'a>),
    Operator(String),
    Comment(String),
}

fn infix_doc<'a>(arena: &'a Arena<'a>, node: Node, src: &[u8]) -> D<'a> {
    let mut parts: Vec<InfixPart<'a>> = Vec::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if is_comment(&child) {
            parts.push(InfixPart::Comment(
                node_text(&child, src).trim_end().to_string(),
            ));
        } else if child.is_named() {
            parts.push(InfixPart::Operand(node_doc(arena, child, src)));
        } else {
            let text = node_text(&child, src).trim();
            if !text.is_empty() {
                parts.push(InfixPart::Operator(text.to_string()));
            }
        }
    }

    let mut result = arena.nil();
    let mut need_space = false;
    let mut after_newline = false;

    for part in parts {
        match part {
            InfixPart::Operand(doc) => {
                if need_space {
                    result = result.append(arena.text(" "));
                }
                result = result.append(doc);
                need_space = false;
                after_newline = false;
            }
            InfixPart::Operator(op) => {
                if after_newline {
                    result = result.append(arena.text(op));
                } else {
                    result = result.append(arena.text(format!(" {}", op)));
                }
                need_space = true;
                after_newline = false;
            }
            InfixPart::Comment(text) => {
                result = result
                    .append(arena.hardline())
                    .append(arena.text(text))
                    .append(arena.hardline());
                need_space = false;
                after_newline = true;
            }
        }
    }

    result
}

// ============================================================
// Struct: f(a, b, c) or multiline with group()
// ============================================================

fn struct_doc<'a>(arena: &'a Arena<'a>, node: Node, src: &[u8]) -> D<'a> {
    let mut name = arena.nil();
    let mut items = Vec::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        match child.kind() {
            "atom" | "qualified_atom" => name = node_doc(arena, child, src),
            "term" => items.push(SeqItem::Value(node_doc(arena, child, src))),
            _ if is_comment(&child) => {
                items.push(SeqItem::Comment(
                    node_text(&child, src).trim_end().to_string(),
                ));
            }
            _ => {}
        }
    }

    name.append(bracketed(arena, "(", ")", &items))
}

// ============================================================
// List: [a, b, c] or multiline with group()
// ============================================================

fn list_doc<'a>(arena: &'a Arena<'a>, node: Node, src: &[u8]) -> D<'a> {
    let mut items = Vec::new();
    let mut tail: Option<D<'a>> = None;
    let mut seen_pipe = false;
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        match child.kind() {
            "term" => {
                if seen_pipe {
                    tail = Some(node_doc(arena, child, src));
                } else {
                    items.push(SeqItem::Value(node_doc(arena, child, src)));
                }
            }
            "|" => seen_pipe = true,
            _ if is_comment(&child) => {
                items.push(SeqItem::Comment(
                    node_text(&child, src).trim_end().to_string(),
                ));
            }
            _ => {}
        }
    }

    if let Some(tail_doc) = tail {
        // List with tail: [items | tail]
        let body = join_seq_items(arena, &items);
        let tail_part = arena.line().append(arena.text("| ")).append(tail_doc);
        arena
            .text("[")
            .append(arena.line_().append(body).append(tail_part).nest(INDENT))
            .append(arena.line_())
            .append(arena.text("]"))
            .group()
    } else {
        bracketed(arena, "[", "]", &items)
    }
}

// ============================================================
// Paren expression
// ============================================================

fn paren_doc<'a>(arena: &'a Arena<'a>, node: Node, src: &[u8]) -> D<'a> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "term" {
            return arena
                .text("(")
                .append(node_doc(arena, child, src))
                .append(arena.text(")"));
        }
    }
    arena.text(node_text(&node, src).to_string())
}

// ============================================================
// Annotated var: 0 < X @ 5 < 10, X @ 5, 0 < X, etc.
// ============================================================

fn annotated_var_doc<'a>(arena: &'a Arena<'a>, node: Node, src: &[u8]) -> D<'a> {
    let mut parts: Vec<D<'a>> = Vec::new();
    let mut cursor = node.walk();
    let mut prev_was_at = false;

    for child in node.children(&mut cursor) {
        if is_comment(&child) {
            continue;
        }
        match child.kind() {
            "variable" | "number" | "comp_op" => {
                if !parts.is_empty() && !prev_was_at {
                    parts.push(arena.text(" "));
                }
                parts.push(arena.text(node_text(&child, src).to_string()));
                prev_was_at = false;
            }
            "@" => {
                parts.push(arena.text("@"));
                prev_was_at = true;
            }
            _ => {}
        }
    }

    arena.concat(parts)
}

// ============================================================
// Qualified atom: module::name
// ============================================================

fn qualified_atom_doc<'a>(arena: &'a Arena<'a>, node: Node, src: &[u8]) -> D<'a> {
    let mut atoms = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "atom" {
            atoms.push(node_text(&child, src).to_string());
        }
    }
    arena.text(atoms.join("::"))
}

// ============================================================
// Shared: bracketed comma-separated items with group()
// ============================================================

enum SeqItem<'a> {
    Value(D<'a>),
    Comment(String),
}

fn bracketed<'a>(
    arena: &'a Arena<'a>,
    open: &str,
    close: &str,
    items: &[SeqItem<'a>],
) -> D<'a> {
    let value_count = items.iter().filter(|i| matches!(i, SeqItem::Value(_))).count();
    if value_count == 0 {
        return arena.text(format!("{}{}", open, close));
    }

    let body = join_seq_items(arena, items);

    arena
        .text(open.to_string())
        .append(arena.line_().append(body).nest(INDENT))
        .append(arena.line_())
        .append(arena.text(close.to_string()))
        .group()
}

fn join_seq_items<'a>(arena: &'a Arena<'a>, items: &[SeqItem<'a>]) -> D<'a> {
    let value_count = items
        .iter()
        .filter(|i| matches!(i, SeqItem::Value(_)))
        .count();
    let mut parts: Vec<D<'a>> = Vec::new();
    let mut value_idx = 0;
    let mut after_comment = false;

    for item in items {
        match item {
            SeqItem::Value(doc) => {
                value_idx += 1;
                if after_comment {
                    // comment already provided the line break
                } else if value_idx > 1 {
                    parts.push(arena.text(",").append(arena.line()));
                }
                parts.push(doc.clone());
                if value_idx < value_count {
                    after_comment = false;
                }
            }
            SeqItem::Comment(text) => {
                if value_idx > 0 {
                    parts.push(arena.text(","));
                }
                parts.push(
                    arena
                        .hardline()
                        .append(arena.text(text.clone()))
                        .append(arena.hardline()),
                );
                after_comment = true;
            }
        }
    }

    arena.concat(parts)
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> tree_sitter::Tree {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(tree_sitter_cadhr_lang::language())
            .unwrap();
        parser.parse(source, None).unwrap()
    }

    fn format(source: &str) -> String {
        let tree = parse(source);
        format_source_file(tree.root_node(), source.as_bytes())
    }

    #[test]
    fn test_simple_fact() {
        assert_eq!(format("cube(1,2,3)."), "cube(1, 2, 3).\n");
    }

    #[test]
    fn test_atom_fact() {
        assert_eq!(format("tetrahedron."), "tetrahedron.\n");
    }

    #[test]
    fn test_simple_rule() {
        assert_eq!(
            format("foo(X):-bar(X),baz(X)."),
            "foo(X) :-\n    bar(X),\n    baz(X).\n"
        );
    }

    #[test]
    fn test_single_goal_rule() {
        assert_eq!(format("foo(X):-bar(X)."), "foo(X) :-\n    bar(X).\n");
    }

    #[test]
    fn test_addition_operator() {
        assert_eq!(
            format("cube(1,1,1)+sphere(2)."),
            "cube(1, 1, 1) + sphere(2).\n"
        );
    }

    #[test]
    fn test_subtraction_operator() {
        assert_eq!(
            format("cube(5,5,5)-sphere(3)."),
            "cube(5, 5, 5) - sphere(3).\n"
        );
    }

    #[test]
    fn test_pipe_operator() {
        assert_eq!(
            format("circle(5)|>linear_extrude(_,10)."),
            "circle(5) |> linear_extrude(_, 10).\n"
        );
    }

    #[test]
    fn test_list() {
        assert_eq!(format("[1,2,3]."), "[1, 2, 3].\n");
    }

    #[test]
    fn test_empty_list() {
        assert_eq!(format("[]."), "[].\n");
    }

    #[test]
    fn test_list_with_tail() {
        assert_eq!(format("[H|T]."), "[H | T].\n");
    }

    #[test]
    fn test_list_with_elements_and_tail() {
        assert_eq!(format("[A,B|T]."), "[A, B | T].\n");
    }

    #[test]
    fn test_multiple_clauses_separated_by_blank_line() {
        assert_eq!(format("foo(1).\nbar(2)."), "foo(1).\n\nbar(2).\n");
    }

    #[test]
    fn test_comment_before_clause() {
        assert_eq!(format("% comment\nfoo(1)."), "% comment\nfoo(1).\n");
    }

    #[test]
    fn test_comment_between_clauses() {
        assert_eq!(
            format("foo(1).\n% comment\nbar(2)."),
            "foo(1).\n\n% comment\nbar(2).\n"
        );
    }

    #[test]
    fn test_already_formatted() {
        let src = "cube(1, 2, 3).\n";
        let tree = parse(src);
        let edits = format_document(&tree, src);
        assert!(edits.is_empty());
    }

    #[test]
    fn test_no_format_on_error() {
        let tree = parse("foo(");
        let edits = format_document(&tree, "foo(");
        assert!(edits.is_empty());
    }

    #[test]
    fn test_default_var() {
        assert_eq!(format("X@5."), "X@5.\n");
    }

    #[test]
    fn test_range_var() {
        assert_eq!(format("0<X<10."), "0 < X < 10.\n");
    }

    #[test]
    fn test_paren_expr() {
        assert_eq!(format("(cube(1,2,3))."), "(cube(1, 2, 3)).\n");
    }

    #[test]
    fn test_nested_struct() {
        assert_eq!(
            format("translate(cube(1,2,3),5,0,0)."),
            "translate(cube(1, 2, 3), 5, 0, 0).\n"
        );
    }

    #[test]
    fn test_complex_pipe_chain() {
        assert_eq!(
            format("circle(5)|>linear_extrude(_,10)|>translate(_,0,0,5)."),
            "circle(5) |> linear_extrude(_, 10) |> translate(_, 0, 0, 5).\n"
        );
    }

    #[test]
    fn test_rule_with_comment_in_goals() {
        assert_eq!(
            format("foo(X):-\n% step\nbar(X),baz(X)."),
            "foo(X) :-\n    % step\n    bar(X),\n    baz(X).\n"
        );
    }

    #[test]
    fn test_sketch_xy() {
        assert_eq!(
            format("sketchXY([p(0,0),p(10,0),p(10,10)])."),
            "sketchXY([p(0, 0), p(10, 0), p(10, 10)]).\n"
        );
    }

    #[test]
    fn test_list_with_comment() {
        let input = "[a,\n% comment\nb].";
        let expected = "[\n    a,\n    % comment\n    b\n].\n";
        assert_eq!(format(input), expected);
    }

    #[test]
    fn test_struct_with_multiline_list_arg() {
        let input = "path(p(0,0),[line_to(p(15,0)),\n% surface\nbezier_to(p(5,5),p(10,10))]).";
        let result = format(input);
        assert!(result.contains("% surface"), "comment must be preserved");
        assert!(result.contains("path("), "struct name must be present");
        assert!(
            result.contains("line_to(p(15, 0))"),
            "list elements formatted"
        );
    }

    #[test]
    fn test_eq_constraint() {
        assert_eq!(
            format("foo(X):-X=5,bar(X)."),
            "foo(X) :-\n    X = 5,\n    bar(X).\n"
        );
    }

    #[test]
    fn test_eq_constraint_complex() {
        assert_eq!(
            format("cut(SLIT,W,H):-X=(W-SLIT)/2,sketchXY([p(X,0)])."),
            "cut(SLIT, W, H) :-\n    X = (W - SLIT) / 2,\n    sketchXY([p(X, 0)]).\n"
        );
    }

    #[test]
    fn test_blank_line_between_goals() {
        assert_eq!(
            format("p :-\n    a,\n\n    b."),
            "p :-\n    a,\n\n    b.\n"
        );
    }

    #[test]
    fn test_comment_in_infix_expr() {
        assert_eq!(
            format("a + b\n% comment\n+ c."),
            "a + b\n% comment\n+ c.\n"
        );
    }

    #[test]
    fn test_blade_cut_example() {
        let input = "\
blade_cut :-
    path(p(0, 0), [
      line_to(p(15, 0)),
      line_to(p(15, 20)),
      % 表面
      bezier_to(p(X1, Y1), p(X2, Y2)),
      line_to(p(0, 40)),
      % 裏スキ
      bezier_to(p(3, 30), p(2, 10))]),
    control(X1@16, Y1@34, 0),
    control(X2@8, Y2@36, 0).";
        let result = format(input);
        assert!(result.contains("% 表面"), "表面 comment preserved");
        assert!(result.contains("% 裏スキ"), "裏スキ comment preserved");
        assert!(result.contains("control(X1@16, Y1@34, 0)"));
    }
}
