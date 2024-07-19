use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use discord::Discord;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

mod discord;

#[derive(Debug)]
struct Document {
    path: PathBuf,
}

#[derive(Debug)]
struct Backend {
    discord: discord::Discord,
    client: Client,
    workspace_file_name: Mutex<String>,
}

impl Document {
    fn new(url: Url) -> Self {
        let url_path = url.path();
        let path = Path::new(url_path);

        Self {
            path: path.to_owned(),
        }
    }

    fn get_filename(&self) -> &str {
        return self.path.file_name().unwrap().to_str().unwrap();
    }
}

impl Backend {
    fn new(client: Client) -> Self {
        Self {
            client,
            discord: Discord::new(),
            workspace_file_name: Mutex::new(String::new()),
        }
    }

    async fn on_change(&self, doc: Document) {
        self.client
            .log_message(
                MessageType::LOG,
                format!(
                    "Changing to {} in {}",
                    doc.get_filename(),
                    self.get_workspace_file_name()
                ),
            )
            .await;

        self.discord
            .change_file(doc.get_filename(), self.get_workspace_file_name().as_str())
    }

    fn get_workspace_file_name(&self) -> MutexGuard<'_, String> {
        return self
            .workspace_file_name
            .lock()
            .expect("Failed to lock workspace file name");
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        // Connect discord client
        self.discord.connect();

        // Set workspace name
        let root_uri = params.root_uri.expect("Failed to get root uri");
        let workspace_path = Path::new(root_uri.path());
        self.workspace_file_name
            .lock()
            .expect("Failed to lock workspace file name")
            .push_str(
                workspace_path
                    .file_name()
                    .expect("Failed to get workspace file name")
                    .to_str()
                    .expect("Failed to convert workspace file name &OsStr to &str"),
            );

        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: env!("CARGO_PKG_NAME").into(),
                version: Some(env!("CARGO_PKG_VERSION").into()),
            }),
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::NONE,
                )),
                ..Default::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(
                MessageType::INFO,
                "Discord Presence LSP server intiailized!",
            )
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.on_change(Document::new(params.text_document.uri))
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        self.on_change(Document::new(params.text_document.uri))
            .await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        self.on_change(Document::new(params.text_document.uri))
            .await;
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        self.on_change(Document::new(
            params.text_document_position_params.text_document.uri,
        ))
        .await;

        return Ok(None);
    }

    async fn folding_range(&self, params: FoldingRangeParams) -> Result<Option<Vec<FoldingRange>>> {
        self.on_change(Document::new(params.text_document.uri))
            .await;

        return Ok(Some(vec![]));
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        self.on_change(Document::new(params.text_document.uri))
            .await;

        return Ok(None);
    }

    async fn semantic_tokens_full_delta(
        &self,
        params: SemanticTokensDeltaParams,
    ) -> Result<Option<SemanticTokensFullDeltaResult>> {
        self.on_change(Document::new(params.text_document.uri))
            .await;

        return Ok(None);
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(Backend::new);

    Server::new(stdin, stdout, socket).serve(service).await;
}
