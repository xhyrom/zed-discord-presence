use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use discord_rich_presence::{
    activity::{self, Assets, Timestamps},
    DiscordIpc, DiscordIpcClient,
};

#[derive(Debug)]
struct Discord {
    client: Mutex<DiscordIpcClient>,
    start_timestamp: Duration,
}

#[derive(Debug)]
struct Backend {
    discord: Discord,
    client: Client,
    root_uri: Mutex<PathBuf>,
}

struct Document {
    path: PathBuf,
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
        let discord_client = DiscordIpcClient::new("1263505205522337886")
            .expect("Failed to initialize Discord Ipc Client");
        let start_timestamp = SystemTime::now();
        let since_epoch = start_timestamp
            .duration_since(UNIX_EPOCH)
            .expect("Failed to get duration since UNIX_EPOCH");

        Self {
            client,
            discord: Discord {
                client: Mutex::new(discord_client),
                start_timestamp: since_epoch,
            },
            root_uri: Mutex::new(PathBuf::new()),
        }
    }

    fn start_client(&self) {
        let mut client = self.get_discord_client();
        let result = client.connect();
        result.unwrap();
    }

    async fn on_change(&self, doc: Document) {
        self.client
            .log_message(
                MessageType::LOG,
                format!(
                    "Changing to {} in {}",
                    doc.get_filename(),
                    self.get_workspace()
                ),
            )
            .await;

        let mut client = self.get_discord_client();
        let timestamp: i64 = self.discord.start_timestamp.as_millis() as i64;

        let _ = client.set_activity(
            activity::Activity::new()
                .assets(Assets::new().large_image("logo"))
                .state(format!("Working on {}", doc.get_filename()).as_str())
                .details(format!("In {}", self.get_workspace()).as_str())
                .timestamps(Timestamps::new().start(timestamp)),
        );
    }

    fn get_discord_client(&self) -> MutexGuard<DiscordIpcClient> {
        return self
            .discord
            .client
            .lock()
            .unwrap_or_else(|e| e.into_inner());
    }

    fn get_workspace(&self) -> String {
        return String::from(
            self.root_uri
                .lock()
                .unwrap()
                .file_name()
                .unwrap()
                .to_str()
                .unwrap(),
        );
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        self.start_client();
        self.root_uri
            .lock()
            .unwrap()
            .push(params.root_uri.unwrap().path());

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
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend::new(client));

    Server::new(stdin, stdout, socket).serve(service).await;
}
