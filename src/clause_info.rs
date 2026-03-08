use std::path::{Path, PathBuf};

use tower_lsp::lsp_types::{Position, Range};
use tree_sitter::{Node, Tree};

pub struct ClauseInfo {
    pub head_name: String,
    pub head_text: String,
    pub arity: usize,
    pub doc: Option<String>,
    pub head_range: Range,
}

pub struct UseInfo {
    pub module_path: String,
    pub expose: Vec<String>,
    pub string_range: Range,
}

pub fn collect_use_directives(tree: &Tree, source: &str) -> Vec<UseInfo> {
    let root = tree.root_node();
    let mut result = Vec::new();
    for i in 0..root.child_count() {
        let child = root.child(i).unwrap();
        if child.kind() == "use_directive" {
            if let Some(info) = extract_use_info(child, source) {
                result.push(info);
            }
        }
    }
    result
}

fn extract_use_info(node: Node, source: &str) -> Option<UseInfo> {
    let mut module_path = None;
    let mut string_range = None;
    let mut expose = Vec::new();

    for i in 0..node.child_count() {
        let child = node.child(i).unwrap();
        match child.kind() {
            "string_literal" => {
                let text = &source[child.byte_range()];
                module_path = Some(text.trim_matches('"').to_string());
                string_range = Some(ts_range_to_lsp(child));
            }
            "use_expose" => {
                for j in 0..child.child_count() {
                    let ec = child.child(j).unwrap();
                    if ec.kind() == "atom" {
                        expose.push(atom_text(ec, source));
                    }
                }
            }
            _ => {}
        }
    }

    Some(UseInfo {
        module_path: module_path?,
        expose,
        string_range: string_range?,
    })
}

pub fn resolve_module_file(module_path: &str, current_file: &Path) -> Option<PathBuf> {
    let dir = current_file.parent()?;
    let trimmed = module_path.trim_end_matches('/');
    let candidate = dir.join(trimmed).join("db.cadhr");
    candidate.is_file().then_some(candidate)
}

pub fn collect_clauses(tree: &Tree, source: &str) -> Vec<ClauseInfo> {
    let root = tree.root_node();
    if root.kind() != "source_file" {
        return Vec::new();
    }

    let mut result = Vec::new();
    let mut pending_comments: Vec<String> = Vec::new();
    let mut last_comment_end_row: Option<usize> = None;

    for i in 0..root.child_count() {
        let child = root.child(i).unwrap();
        match child.kind() {
            "line_comment" | "block_comment" => {
                let comment_start_row = child.start_position().row;
                if let Some(prev_end) = last_comment_end_row {
                    if comment_start_row > prev_end + 1 {
                        pending_comments.clear();
                    }
                }
                let text = &source[child.byte_range()];
                pending_comments.push(strip_comment(text));
                last_comment_end_row = Some(child.end_position().row);
            }
            "clause" => {
                let clause_start_row = child.start_position().row;
                let doc = if !pending_comments.is_empty() {
                    if let Some(end_row) = last_comment_end_row {
                        if clause_start_row <= end_row + 1 {
                            Some(pending_comments.join("\n"))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };
                pending_comments.clear();
                last_comment_end_row = None;

                if let Some(info) = extract_clause_info(child, source, doc) {
                    result.push(info);
                }
            }
            _ => {
                pending_comments.clear();
                last_comment_end_row = None;
            }
        }
    }

    result
}

fn extract_clause_info(clause_node: Node, source: &str, doc: Option<String>) -> Option<ClauseInfo> {
    let inner = (0..clause_node.child_count()).find_map(|i| {
        let c = clause_node.child(i).unwrap();
        if c.kind() == "fact" || c.kind() == "rule" {
            Some(c)
        } else {
            None
        }
    })?;

    let head_term = (0..inner.child_count()).find_map(|i| {
        let c = inner.child(i).unwrap();
        if c.is_named() && c.kind() == "term" {
            Some(c)
        } else {
            None
        }
    })?;

    let (head_name, arity) = resolve_head_name_arity(head_term, source)?;
    let head_text = source[head_term.byte_range()].to_string();

    Some(ClauseInfo {
        head_name,
        head_text,
        arity,
        doc,
        head_range: ts_range_to_lsp(head_term),
    })
}

fn qualified_atom_text(node: Node, source: &str) -> String {
    let mut parts = Vec::new();
    for i in 0..node.child_count() {
        let c = node.child(i).unwrap();
        if c.kind() == "atom" {
            parts.push(atom_text(c, source));
        }
    }
    parts.join("::")
}

fn resolve_head_name_arity(node: Node, source: &str) -> Option<(String, usize)> {
    match node.kind() {
        "struct" => {
            let name_node = (0..node.child_count()).find_map(|i| {
                let c = node.child(i).unwrap();
                if c.kind() == "qualified_atom"
                    || c.kind() == "atom"
                    || c.kind() == "unquoted_atom"
                    || c.kind() == "quoted_atom"
                {
                    Some(c)
                } else {
                    None
                }
            })?;
            let name = if name_node.kind() == "qualified_atom" {
                qualified_atom_text(name_node, source)
            } else {
                atom_text(name_node, source)
            };
            let arity = (0..node.child_count())
                .filter(|&i| node.child(i).unwrap().kind() == "term")
                .count();
            Some((name, arity))
        }
        "qualified_atom" => Some((qualified_atom_text(node, source), 0)),
        "atom" | "unquoted_atom" | "quoted_atom" => {
            Some((atom_text(node, source), 0))
        }
        "term" | "pipe_expr" | "add_expr" | "mul_expr" | "primary_term" => {
            for i in 0..node.child_count() {
                let child = node.child(i).unwrap();
                if child.is_named() {
                    return resolve_head_name_arity(child, source);
                }
            }
            None
        }
        _ => None,
    }
}

pub fn atom_text(node: Node, source: &str) -> String {
    let text = &source[node.byte_range()];
    if node.kind() == "quoted_atom" {
        text.trim_matches('\'').to_string()
    } else if node.kind() == "atom" {
        if let Some(inner) = node.child(0) {
            let inner_text = &source[inner.byte_range()];
            if inner.kind() == "quoted_atom" {
                inner_text.trim_matches('\'').to_string()
            } else {
                inner_text.to_string()
            }
        } else {
            text.to_string()
        }
    } else {
        text.to_string()
    }
}

pub fn strip_comment(text: &str) -> String {
    if let Some(rest) = text.strip_prefix('%') {
        rest.trim().to_string()
    } else if text.starts_with("/*") && text.ends_with("*/") {
        text[2..text.len() - 2].trim().to_string()
    } else {
        text.to_string()
    }
}

pub fn find_all_atom_occurrences(tree: &Tree, source: &str, name: &str) -> Vec<Range> {
    let mut results = Vec::new();
    find_atoms_recursive(tree.root_node(), source, name, &mut results);
    results
}

fn find_atoms_recursive(node: Node, source: &str, name: &str, results: &mut Vec<Range>) {
    match node.kind() {
        "unquoted_atom" => {
            let text = &source[node.byte_range()];
            if text == name {
                results.push(ts_range_to_lsp(node));
            }
        }
        "quoted_atom" => {
            let text = &source[node.byte_range()];
            if text.trim_matches('\'') == name {
                results.push(ts_range_to_lsp(node));
            }
        }
        "atom" => {
            if let Some(inner) = node.child(0) {
                find_atoms_recursive(inner, source, name, results);
            }
        }
        _ => {
            for i in 0..node.child_count() {
                if let Some(child) = node.child(i) {
                    find_atoms_recursive(child, source, name, results);
                }
            }
        }
    }
}

fn ts_range_to_lsp(node: Node) -> Range {
    Range {
        start: Position {
            line: node.start_position().row as u32,
            character: node.start_position().column as u32,
        },
        end: Position {
            line: node.end_position().row as u32,
            character: node.end_position().column as u32,
        },
    }
}
