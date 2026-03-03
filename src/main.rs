mod backend;
mod clause_info;
mod completion;
mod diagnostics;
mod formatting;
mod hover;

use backend::CadhrBackend;
use tower_lsp::{LspService, Server};

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(CadhrBackend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
