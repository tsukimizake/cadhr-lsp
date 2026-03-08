use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};

use crate::clause_info::{
    collect_clauses, collect_use_directives, find_all_atom_occurrences, resolve_module_file,
};
use crate::completion::{builtin_completion_items, module_completion_items, user_defined_completion_items};
use crate::diagnostics::compute_diagnostics;
use crate::formatting;
use crate::hover::hover_info;

pub struct CadhrBackend {
    client: Client,
    documents: RwLock<HashMap<Url, String>>,
    parser: Mutex<tree_sitter::Parser>,
}

impl CadhrBackend {
    pub fn new(client: Client) -> Self {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(tree_sitter_cadhr_lang::language())
            .expect("Failed to load cadhr-lang grammar");
        Self {
            client,
            documents: RwLock::new(HashMap::new()),
            parser: Mutex::new(parser),
        }
    }

    fn parse(&self, text: &str) -> Option<tree_sitter::Tree> {
        let mut parser = self.parser.lock().unwrap();
        parser.parse(text, None)
    }

    async fn publish_diagnostics(&self, uri: Url, text: &str) {
        let diagnostics = match self.parse(text) {
            Some(tree) => compute_diagnostics(&tree, text),
            None => vec![],
        };
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for CadhrBackend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec!["(".to_string()]),
                    ..Default::default()
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                document_formatting_provider: Some(OneOf::Left(true)),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "cadhr-lsp initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri;
        let text = params.text_document.text;
        self.documents
            .write()
            .await
            .insert(uri.clone(), text.clone());
        self.publish_diagnostics(uri, &text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        if let Some(change) = params.content_changes.into_iter().last() {
            let text = change.text;
            self.documents
                .write()
                .await
                .insert(uri.clone(), text.clone());
            self.publish_diagnostics(uri, &text).await;
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let uri = params.text_document.uri;
        let docs = self.documents.read().await;
        if let Some(text) = docs.get(&uri) {
            let text = text.clone();
            drop(docs);
            self.publish_diagnostics(uri, &text).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.documents.write().await.remove(&uri);
        self.client.publish_diagnostics(uri, vec![], None).await;
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let mut items = builtin_completion_items();

        let uri = params.text_document_position.text_document.uri;
        let docs = self.documents.read().await;
        if let Some(text) = docs.get(&uri) {
            if let Some(tree) = self.parse(text) {
                let clauses = collect_clauses(&tree, text);
                items.extend(user_defined_completion_items(&clauses, &items));

                let current_file = Path::new(uri.path());
                let use_directives = collect_use_directives(&tree, text);
                items.extend(module_completion_items(&use_directives, current_file));
            }
        }

        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let docs = self.documents.read().await;
        let result = docs.get(&uri).and_then(|text| {
            let tree = self.parse(text)?;
            let (name, atom_range) = atom_at(&tree, text, pos)?;
            let clauses = collect_clauses(&tree, text);
            hover_info(&clauses, &name, atom_range)
        });
        Ok(result)
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;
        let docs = self.documents.read().await;
        let result = docs.get(&uri).and_then(|text| {
            let tree = self.parse(text)?;

            // #use directive: jump to the referenced file
            if let Some(response) = use_directive_goto(&tree, text, pos, &uri) {
                return Some(response);
            }

            let (name, _) = atom_at(&tree, text, pos)?;

            // qualified name (e.g. ive_reararck::main): jump into module file
            if let Some((module_prefix, local_name)) = name.split_once("::") {
                let use_directives = collect_use_directives(&tree, text);
                let current_file = Path::new(uri.path());
                let ud = use_directives.iter().find(|ud| {
                    module_name_from_use_path(&ud.module_path) == module_prefix
                })?;
                let target = resolve_module_file(&ud.module_path, current_file)?;
                let target_source = std::fs::read_to_string(&target).ok()?;
                let target_tree = self.parse(&target_source)?;
                let target_clauses = collect_clauses(&target_tree, &target_source);
                let target_uri = Url::from_file_path(&target).ok()?;
                let ci = target_clauses.iter().find(|c| c.head_name == local_name)?;
                return Some(GotoDefinitionResponse::Scalar(Location {
                    uri: target_uri,
                    range: ci.head_range,
                }));
            }

            let clauses = collect_clauses(&tree, text);
            clauses
                .iter()
                .find(|c| c.head_name == name)
                .map(|ci| {
                    GotoDefinitionResponse::Scalar(Location {
                        uri: uri.clone(),
                        range: ci.head_range,
                    })
                })
        });
        Ok(result)
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;
        let docs = self.documents.read().await;
        let result = docs.get(&uri).and_then(|text| {
            let tree = self.parse(text)?;
            let (name, _) = atom_at(&tree, text, pos)?;
            let locs: Vec<Location> = find_all_atom_occurrences(&tree, text, &name)
                .into_iter()
                .map(|r| Location {
                    uri: uri.clone(),
                    range: r,
                })
                .collect();
            if locs.is_empty() { None } else { Some(locs) }
        });
        Ok(result)
    }

    async fn formatting(
        &self,
        params: DocumentFormattingParams,
    ) -> Result<Option<Vec<TextEdit>>> {
        let uri = params.text_document.uri;
        let docs = self.documents.read().await;
        let result = docs.get(&uri).and_then(|text| {
            let tree = self.parse(text)?;
            let edits = formatting::format_document(&tree, text);
            if edits.is_empty() {
                None
            } else {
                Some(edits)
            }
        });
        Ok(result)
    }
}

fn atom_at(
    tree: &tree_sitter::Tree,
    source: &str,
    pos: Position,
) -> Option<(String, Range)> {
    let point = tree_sitter::Point {
        row: pos.line as usize,
        column: pos.character as usize,
    };
    let leaf = tree
        .root_node()
        .descendant_for_point_range(point, point)?;

    let atom_node = match leaf.kind() {
        "unquoted_atom" => leaf,
        "atom" => leaf.child(0)?,
        _ => return None,
    };

    // qualified_atom (e.g. module::name) の場合は全体を返す
    let target = if let Some(parent) = atom_node.parent() {
        if parent.kind() == "atom" {
            parent.parent().filter(|gp| gp.kind() == "qualified_atom").unwrap_or(parent)
        } else if parent.kind() == "qualified_atom" {
            parent
        } else {
            atom_node
        }
    } else {
        atom_node
    };

    let name = target.utf8_text(source.as_bytes()).ok()?;
    let range = Range {
        start: Position {
            line: target.start_position().row as u32,
            character: target.start_position().column as u32,
        },
        end: Position {
            line: target.end_position().row as u32,
            character: target.end_position().column as u32,
        },
    };
    Some((name.to_string(), range))
}

fn use_directive_goto(
    tree: &tree_sitter::Tree,
    source: &str,
    pos: Position,
    uri: &Url,
) -> Option<GotoDefinitionResponse> {
    let point = tree_sitter::Point {
        row: pos.line as usize,
        column: pos.character as usize,
    };
    let node = tree.root_node().descendant_for_point_range(point, point)?;

    // Walk up to find if we're inside a use_directive
    let mut current = node;
    loop {
        if current.kind() == "use_directive" {
            break;
        }
        current = current.parent()?;
    }

    let use_directives = collect_use_directives(tree, source);
    let current_file = Path::new(uri.path());

    for ud in &use_directives {
        // Check if this use_directive covers our position
        if pos.line >= ud.string_range.start.line
            && pos.line <= ud.string_range.end.line
        {
            let target = resolve_module_file(&ud.module_path, current_file)?;
            let target_uri = Url::from_file_path(&target).ok()?;
            return Some(GotoDefinitionResponse::Scalar(Location {
                uri: target_uri,
                range: Range {
                    start: Position { line: 0, character: 0 },
                    end: Position { line: 0, character: 0 },
                },
            }));
        }
    }
    None
}

fn module_name_from_use_path(path: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    std::path::Path::new(trimmed)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}
