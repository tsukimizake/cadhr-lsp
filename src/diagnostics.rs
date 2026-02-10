use tower_lsp::lsp_types::*;
use tree_sitter::{Node, Tree};

pub fn compute_diagnostics(tree: &Tree, source: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    collect_errors(tree.root_node(), source, &mut diagnostics);
    diagnostics
}

fn collect_errors(node: Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    if node.is_error() {
        let text = node
            .utf8_text(source.as_bytes())
            .unwrap_or("")
            .chars()
            .take(30)
            .collect::<String>();
        diagnostics.push(Diagnostic {
            range: node_range(node),
            severity: Some(DiagnosticSeverity::ERROR),
            source: Some("cadhr-lsp".to_string()),
            message: format!("Syntax error: unexpected `{}`", text),
            ..Default::default()
        });
    } else if node.is_missing() {
        diagnostics.push(Diagnostic {
            range: node_range(node),
            severity: Some(DiagnosticSeverity::ERROR),
            source: Some("cadhr-lsp".to_string()),
            message: format!("Syntax error: missing `{}`", node.kind()),
            ..Default::default()
        });
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_errors(child, source, diagnostics);
    }
}

fn node_range(node: Node) -> Range {
    let start = node.start_position();
    let end = node.end_position();
    Range {
        start: Position {
            line: start.row as u32,
            character: start.column as u32,
        },
        end: Position {
            line: end.row as u32,
            character: end.column as u32,
        },
    }
}
