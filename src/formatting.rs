use tower_lsp::lsp_types::{Position, Range, TextEdit};
use tree_sitter::Node;

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
    let mut result = String::new();
    let mut cursor = node.walk();
    let mut prev_was_clause = false;

    for child in node.children(&mut cursor) {
        if is_comment(&child) {
            if prev_was_clause {
                result.push_str("\n\n");
            } else if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(node_text(&child, src).trim_end());
            prev_was_clause = false;
        } else if child.kind() == "use_directive" {
            if prev_was_clause {
                result.push_str("\n\n");
            } else if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(node_text(&child, src).trim_end());
            prev_was_clause = false;
        } else if child.kind() == "clause" {
            if prev_was_clause {
                result.push_str("\n\n");
            } else if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(&format_clause(child, src));
            prev_was_clause = true;
        }
    }

    if !result.is_empty() {
        result.push('\n');
    }
    result
}

fn format_clause(node: Node, src: &[u8]) -> String {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "fact" => return format_fact(child, src),
            "rule" => return format_rule(child, src),
            _ => {}
        }
    }
    node_text(&node, src).to_string()
}

fn format_fact(node: Node, src: &[u8]) -> String {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "term" {
            return format!("{}.", format_term(child, src, 0));
        }
    }
    node_text(&node, src).to_string()
}

fn format_rule(node: Node, src: &[u8]) -> String {
    let mut head = String::new();
    let mut goals = String::new();
    let mut comments_before_goals = Vec::new();
    let mut seen_neck = false;
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        match child.kind() {
            "term" => head = format_term(child, src, 0),
            "goals" => goals = format_goals(child, src),
            "line_comment" | "block_comment" if seen_neck => {
                comments_before_goals.push(node_text(&child, src).trim_end().to_string());
            }
            ":-" => seen_neck = true,
            _ => {}
        }
    }

    let mut result = format!("{} :-\n", head);
    for c in comments_before_goals {
        result.push_str(&format!("    {}\n", c));
    }
    result.push_str(&goals);
    result.push('.');
    result
}

enum Item {
    Term(String),
    Comment(String),
}

fn format_goals(node: Node, src: &[u8]) -> String {
    let indent = 4;
    let indent_str = " ".repeat(indent);
    let mut items: Vec<(Item, usize)> = Vec::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if child.kind() == "term" {
            items.push((
                Item::Term(format_term(child, src, indent)),
                child.start_position().row,
            ));
        } else if is_comment(&child) {
            items.push((
                Item::Comment(node_text(&child, src).trim_end().to_string()),
                child.start_position().row,
            ));
        }
    }

    let term_count = items
        .iter()
        .filter(|(i, _)| matches!(i, Item::Term(_)))
        .count();
    let mut result = String::new();
    let mut term_idx = 0;
    let mut prev_end_row: Option<usize> = None;

    for (item, start_row) in &items {
        if let Some(prev) = prev_end_row {
            let blank_lines = start_row.saturating_sub(prev).saturating_sub(1);
            for _ in 0..blank_lines {
                result.push('\n');
            }
        }
        match item {
            Item::Term(t) => {
                term_idx += 1;
                let end_row = start_row + t.matches('\n').count();
                if term_idx < term_count {
                    result.push_str(&format!("{}{},\n", indent_str, t));
                } else {
                    result.push_str(&format!("{}{}", indent_str, t));
                }
                prev_end_row = Some(end_row);
            }
            Item::Comment(c) => {
                result.push_str(&format!("{}{}\n", indent_str, c));
                prev_end_row = Some(*start_row);
            }
        }
    }
    result
}

fn format_term(node: Node, src: &[u8], indent: usize) -> String {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.is_named() && !is_comment(&child) {
            return format_expr(child, src, indent);
        }
    }
    node_text(&node, src).to_string()
}

fn format_expr(node: Node, src: &[u8], indent: usize) -> String {
    match node.kind() {
        "term" => format_term(node, src, indent),
        "primary_term" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.is_named() && !is_comment(&child) {
                    return format_expr(child, src, indent);
                }
            }
            node_text(&node, src).to_string()
        }
        "pipe_expr" | "add_expr" | "mul_expr" => format_infix_expr(node, src, indent),
        "struct" => format_struct(node, src, indent),
        "list" => format_list(node, src, indent),
        "paren_expr" => format_paren_expr(node, src, indent),
        "default_var" => format_default_var(node, src),
        "range_var" => format_range_var(node, src),
        "qualified_atom" => format_qualified_atom(node, src),
        "atom" | "unquoted_atom" | "quoted_atom" | "number" | "variable" => {
            node_text(&node, src).to_string()
        }
        _ => node_text(&node, src).to_string(),
    }
}

enum InfixItem {
    Operand(String),
    Operator(String),
    Comment(String),
}

fn format_infix_expr(node: Node, src: &[u8], indent: usize) -> String {
    let mut items = Vec::new();
    let mut cursor = node.walk();
    let mut has_comments = false;

    for child in node.children(&mut cursor) {
        if is_comment(&child) {
            items.push(InfixItem::Comment(
                node_text(&child, src).trim_end().to_string(),
            ));
            has_comments = true;
        } else if child.is_named() {
            items.push(InfixItem::Operand(format_expr(child, src, indent)));
        } else {
            let text = node_text(&child, src).trim();
            if !text.is_empty() {
                items.push(InfixItem::Operator(text.to_string()));
            }
        }
    }

    if !has_comments {
        let mut result = String::new();
        let mut need_space = false;
        for item in &items {
            match item {
                InfixItem::Operand(s) => {
                    if need_space {
                        result.push(' ');
                    }
                    result.push_str(s);
                    need_space = false;
                }
                InfixItem::Operator(s) => {
                    result.push(' ');
                    result.push_str(s);
                    need_space = true;
                }
                InfixItem::Comment(_) => unreachable!(),
            }
        }
        return result;
    }

    let indent_str = " ".repeat(indent);
    let mut lines: Vec<String> = Vec::new();
    let mut current_line = String::new();
    let mut need_space = false;

    for item in &items {
        match item {
            InfixItem::Comment(c) => {
                if !current_line.is_empty() {
                    lines.push(current_line.clone());
                    current_line.clear();
                    need_space = false;
                }
                lines.push(c.clone());
            }
            InfixItem::Operand(s) => {
                if need_space {
                    current_line.push(' ');
                }
                current_line.push_str(s);
                need_space = false;
            }
            InfixItem::Operator(s) => {
                if !current_line.is_empty() {
                    current_line.push(' ');
                }
                current_line.push_str(s);
                need_space = true;
            }
        }
    }
    if !current_line.is_empty() {
        lines.push(current_line);
    }

    let mut result = String::new();
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            result.push('\n');
            result.push_str(&indent_str);
        }
        result.push_str(line);
    }
    result
}

fn format_qualified_atom(node: Node, src: &[u8]) -> String {
    let mut parts = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "atom" {
            parts.push(node_text(&child, src).to_string());
        }
    }
    parts.join("::")
}

fn format_struct(node: Node, src: &[u8], indent: usize) -> String {
    let mut name = String::new();
    let mut items: Vec<Item> = Vec::new();
    let mut cursor = node.walk();

    let inner_indent = indent + 4;

    for child in node.children(&mut cursor) {
        match child.kind() {
            "atom" | "qualified_atom" => name = format_expr(child, src, indent),
            "term" => items.push(Item::Term(format_term(child, src, inner_indent))),
            "line_comment" | "block_comment" => {
                items.push(Item::Comment(
                    node_text(&child, src).trim_end().to_string(),
                ));
            }
            _ => {}
        }
    }

    let has_comments = items.iter().any(|i| matches!(i, Item::Comment(_)));
    let any_multiline = items
        .iter()
        .any(|i| matches!(i, Item::Term(s) if s.contains('\n')));

    if has_comments || any_multiline {
        let inner = " ".repeat(inner_indent);
        let outer = " ".repeat(indent);
        let mut result = format!("{}(\n", name);
        let arg_count = items
            .iter()
            .filter(|i| matches!(i, Item::Term(_)))
            .count();
        let mut arg_idx = 0;
        for item in &items {
            match item {
                Item::Term(a) => {
                    arg_idx += 1;
                    if arg_idx < arg_count {
                        result.push_str(&format!("{}{},\n", inner, a));
                    } else {
                        result.push_str(&format!("{}{}\n", inner, a));
                    }
                }
                Item::Comment(c) => {
                    result.push_str(&format!("{}{}\n", inner, c));
                }
            }
        }
        result.push_str(&format!("{})", outer));
        result
    } else {
        let args: Vec<&str> = items
            .iter()
            .filter_map(|i| match i {
                Item::Term(s) => Some(s.as_str()),
                _ => None,
            })
            .collect();
        format!("{}({})", name, args.join(", "))
    }
}

fn format_list(node: Node, src: &[u8], indent: usize) -> String {
    let mut items: Vec<Item> = Vec::new();
    let mut tail = None;
    let mut seen_pipe = false;
    let mut cursor = node.walk();

    let inner_indent = indent + 4;

    for child in node.children(&mut cursor) {
        match child.kind() {
            "term" => {
                if seen_pipe {
                    tail = Some(format_term(child, src, inner_indent));
                } else {
                    items.push(Item::Term(format_term(child, src, inner_indent)));
                }
            }
            "|" => seen_pipe = true,
            "line_comment" | "block_comment" => {
                items.push(Item::Comment(
                    node_text(&child, src).trim_end().to_string(),
                ));
            }
            _ => {}
        }
    }

    let has_comments = items.iter().any(|i| matches!(i, Item::Comment(_)));
    let any_multiline = items
        .iter()
        .any(|i| matches!(i, Item::Term(s) if s.contains('\n')));

    if has_comments || any_multiline {
        let inner = " ".repeat(inner_indent);
        let outer = " ".repeat(indent);
        let mut result = String::from("[\n");
        let term_count = items
            .iter()
            .filter(|i| matches!(i, Item::Term(_)))
            .count();
        let mut term_idx = 0;
        for item in &items {
            match item {
                Item::Term(t) => {
                    term_idx += 1;
                    let has_more = term_idx < term_count || tail.is_some();
                    if has_more {
                        result.push_str(&format!("{}{},\n", inner, t));
                    } else {
                        result.push_str(&format!("{}{}\n", inner, t));
                    }
                }
                Item::Comment(c) => {
                    result.push_str(&format!("{}{}\n", inner, c));
                }
            }
        }
        if let Some(t) = &tail {
            result.push_str(&format!("{}| {}\n", inner, t));
        }
        result.push_str(&format!("{}]", outer));
        result
    } else {
        let terms: Vec<&str> = items
            .iter()
            .filter_map(|i| match i {
                Item::Term(s) => Some(s.as_str()),
                _ => None,
            })
            .collect();
        let mut result = String::from("[");
        result.push_str(&terms.join(", "));
        if let Some(t) = &tail {
            result.push_str(" | ");
            result.push_str(t);
        }
        result.push(']');
        result
    }
}

fn format_paren_expr(node: Node, src: &[u8], indent: usize) -> String {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "term" {
            return format!("({})", format_term(child, src, indent));
        }
    }
    node_text(&node, src).to_string()
}

fn format_default_var(node: Node, src: &[u8]) -> String {
    let mut var = String::new();
    let mut num = String::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        match child.kind() {
            "variable" => var = node_text(&child, src).to_string(),
            "number" => num = node_text(&child, src).to_string(),
            _ => {}
        }
    }
    format!("{}@{}", var, num)
}

fn format_range_var(node: Node, src: &[u8]) -> String {
    let mut result = String::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if is_comment(&child) {
            continue;
        }
        match child.kind() {
            "variable" | "number" | "comp_op" => {
                if !result.is_empty() {
                    result.push(' ');
                }
                result.push_str(node_text(&child, src));
            }
            _ => {}
        }
    }
    result
}

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
    fn test_comment_in_infix_expr() {
        assert_eq!(
            format("a + b\n% comment\n+ c."),
            "a + b\n% comment\n+ c.\n"
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
        // Comments in a list trigger multiline formatting
        let input = "[a,\n% comment\nb].";
        let expected = "[\n    a,\n    % comment\n    b\n].\n";
        assert_eq!(format(input), expected);
    }

    #[test]
    fn test_struct_with_multiline_list_arg() {
        // A struct arg that contains a multiline list triggers multiline struct
        let input = "path(p(0,0),[line_to(p(15,0)),\n% surface\nbezier_to(p(5,5),p(10,10))]).";
        let result = format(input);
        assert!(result.contains("% surface"), "comment must be preserved");
        assert!(result.contains("path("), "struct name must be present");
        assert!(result.contains("line_to(p(15, 0))"), "list elements formatted");
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
