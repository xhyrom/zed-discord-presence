/*
 * This file is part of discord-presence. Extension for Zed that adds support for Discord Rich Presence using LSP.
 *
 * Copyright (c) 2024 Steinhübl
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>
 */

use std::path::{Path, PathBuf};
use std::process::exit;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use config::PresenceConfig;
use document::Document;
use git::get_repository_and_remote;
use service::{AppState, PresenceService};
use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    DidChangeTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
    DocumentHighlight, DocumentHighlightParams, InitializeParams, InitializeResult,
    InitializedParams, MessageType, SaveOptions, ServerCapabilities, ServerInfo,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions,
    TextDocumentSyncSaveOptions, WorkspaceServerCapabilities,
};
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tracing::{debug, error, info, instrument, warn};

mod activity;
mod config;
mod discord;
mod document;
mod error;
mod git;
mod idle;
mod languages;
mod logger;
mod service;
mod util;

#[derive(Debug)]
struct Backend {
    client: Client,
    presence_service: Arc<PresenceService>,
    app_state: Arc<AppState>,
    active_doc_uri: Arc<Mutex<Option<String>>>,
}

impl Backend {
    fn new(client: Client) -> Self {
        let app_state = Arc::new(AppState::new());
        let config = PresenceConfig::default();
        let presence_service = Arc::new(PresenceService::new(Arc::clone(&app_state), config));

        info!("Backend initialized");

        Self {
            client,
            presence_service,
            app_state,
            active_doc_uri: Arc::new(Mutex::new(None)),
        }
    }

    async fn set_active_doc_uri(&self, uri: Option<String>) {
        *self.active_doc_uri.lock().await = uri;
    }

    async fn on_change(&self, uri: &tower_lsp::lsp_types::Url, line_number: Option<u32>) {
        debug!("Document changed");

        let doc = {
            let workspace = self.app_state.workspace.lock().await;
            let workspace_path = Path::new(workspace.path().unwrap_or(""));

            Document::new(uri, workspace_path, line_number)
        };

        if let Err(e) = self.presence_service.update_presence(Some(doc)).await {
            error!("Failed to update presence: {}", e);
        } else {
            debug!("Presence updated successfully");
        }
    }

    fn resolve_workspace_path(params: &InitializeParams) -> PathBuf {
        if let Some(folders) = &params.workspace_folders {
            if let Some(first_folder) = folders.first() {
                if let Ok(path) = first_folder.uri.to_file_path() {
                    debug!("Using workspace folder: {}", path.display());
                    return path;
                }
            }
        }

        let root_uri = params.root_uri.as_ref().expect(
            "Failed to get workspace path - neither workspace_folders nor root_uri is present",
        );

        if let Ok(path) = root_uri.to_file_path() {
            debug!("Using root URI: {}", path.display());
            return path;
        }

        panic!("Failed to resolve workspace path from URI")
    }

    async fn setup_git_info(&self, workspace_path: &Path) {
        let path_str = workspace_path.to_str().unwrap_or("");
        let clean_path = if cfg!(target_os = "windows") && path_str.starts_with('/') {
            &path_str[1..]
        } else {
            path_str
        };

        info!("Checking git repo at: {}", clean_path);

        let overrides = {
            let config = self.app_state.config.lock().await;
            config.git_host_overrides.clone()
        };

        let remote_url = get_repository_and_remote(clean_path, &overrides);
        if let Some(ref url) = remote_url {
            info!("Git remote URL found: {}", url);
        } else {
            debug!("No git remote URL found at path: {}", clean_path);
        }
        *self.app_state.git_remote_url.lock().await = remote_url;

        let branch = git::get_current_branch(clean_path);
        if let Some(ref b) = branch {
            info!("Git branch: {}", b);
        } else {
            debug!("No git branch found at path: {}", clean_path);
        }
        *self.app_state.git_branch.lock().await = branch;
    }

    fn start_file_polling_loop(&self) {
        let active_doc_uri = Arc::clone(&self.active_doc_uri);
        let app_state = Arc::clone(&self.app_state);
        let file_monitor = self.presence_service.file_monitor().clone();
        let presence_service = Arc::clone(&self.presence_service);
        let shutting_down = Arc::clone(&self.app_state.shutting_down);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(100));
            loop {
                interval.tick().await;
                if shutting_down.load(Ordering::SeqCst) {
                    debug!("Polling loop shutting down");
                    break;
                }

                if let Ok(uri_lock) = active_doc_uri.try_lock() {
                    if let Some(current_uri) = uri_lock.clone() {
                        drop(uri_lock);

                        if file_monitor
                            .check_active_file(Some(current_uri.clone()))
                            .await
                            .is_some()
                        {
                            debug!("Active file changed (polling detected): {}", current_uri);

                            if let Ok(url) = tower_lsp::lsp_types::Url::parse(&current_uri) {
                                let workspace_path = {
                                    let workspace = app_state.workspace.lock().await;
                                    workspace.path().map(ToString::to_string)
                                };

                                if let Some(path_str) = workspace_path {
                                    let doc = Document::new(&url, Path::new(&path_str), None);
                                    if let Err(e) =
                                        presence_service.update_presence(Some(doc)).await
                                    {
                                        warn!("Failed to update presence on file switch: {}", e);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        info!("Initializing Discord Presence LSP");

        let workspace_path = Self::resolve_workspace_path(&params);
        info!("Workspace path: {}", workspace_path.display());

        {
            let mut workspace = self.app_state.workspace.lock().await;
            if let Err(e) = workspace.set_workspace(&workspace_path) {
                error!("Failed to set workspace: {}", e);
                return Err(tower_lsp::jsonrpc::Error::internal_error());
            }
            info!("Workspace set to: {}", workspace.name());
        }

        {
            let mut config = self.app_state.config.lock().await;
            if let Err(e) = config.update(params.initialization_options) {
                error!("Failed to update config: {}", e);
                return Err(tower_lsp::jsonrpc::Error::internal_error());
            }
            debug!(
                "Configuration updated: application_id={}, git_integration={}",
                config.application_id, config.git_integration
            );
            if !config.rules.suitable(workspace_path.to_str().unwrap_or("")) {
                info!("Workspace not suitable according to rules, exiting");
                exit(0);
            }
        }

        self.setup_git_info(&workspace_path).await;

        {
            let config = self.app_state.config.lock().await;
            match self
                .presence_service
                .initialize_discord(&config.application_id)
                .await
            {
                Ok(()) => info!("Discord client initialized and connected"),
                Err(e) => warn!(
                    "Discord connection failed during init, will retry on activity: {}",
                    e
                ),
            }
        }

        self.start_file_polling_loop();

        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: env!("CARGO_PKG_NAME").into(),
                version: Some(env!("CARGO_PKG_VERSION").into()),
            }),
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Options(
                    TextDocumentSyncOptions {
                        open_close: Some(true),
                        change: Some(TextDocumentSyncKind::INCREMENTAL),
                        save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                            include_text: Some(false),
                        })),
                        ..Default::default()
                    },
                )),
                document_highlight_provider: Some(tower_lsp::lsp_types::OneOf::Left(true)),
                workspace: Some(WorkspaceServerCapabilities {
                    file_operations: None,
                    workspace_folders: None,
                }),
                ..Default::default()
            },
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        info!("Discord Presence LSP server fully initialized and ready");

        self.client
            .log_message(
                MessageType::INFO,
                "Discord Presence LSP server initialized!",
            )
            .await;
    }

    #[instrument(skip(self))]
    async fn shutdown(&self) -> Result<()> {
        info!("Shutting down Discord Presence LSP");

        if let Err(e) = self.presence_service.shutdown().await {
            error!("Failed to shutdown presence service: {}", e);
        } else {
            info!("Presence service shutdown successfully");
        }

        Ok(())
    }

    #[instrument(skip(self, params))]
    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        debug!("Document opened: {}", params.text_document.uri);
        let uri = params.text_document.uri.to_string();
        self.set_active_doc_uri(Some(uri)).await;
        self.on_change(&params.text_document.uri, None).await;
    }

    #[instrument(skip(self, params))]
    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        debug!("Document changed: {}", params.text_document.uri);

        let uri = params.text_document.uri.to_string();
        self.set_active_doc_uri(Some(uri)).await;

        let line_number = params
            .content_changes
            .last()
            .and_then(|change| change.range.as_ref())
            .map(|range| range.start.line);

        self.on_change(&params.text_document.uri, line_number).await;
    }

    #[instrument(skip(self, params))]
    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        debug!("Document saved: {}", params.text_document.uri);
        let uri = params.text_document.uri.to_string();
        self.set_active_doc_uri(Some(uri)).await;
        self.on_change(&params.text_document.uri, None).await;
    }

    #[instrument(skip(self, params))]
    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> Result<Option<Vec<DocumentHighlight>>> {
        debug!(
            "Document highlight requested: {}",
            params.text_document_position_params.text_document.uri
        );
        let pos = params.text_document_position_params;
        self.on_change(&pos.text_document.uri, Some(pos.position.line))
            .await;

        Ok(None)
    }
}

#[tokio::main]
async fn main() {
    logger::init_logger();

    info!(
        "Starting Discord Presence LSP server v{}",
        env!("CARGO_PKG_VERSION")
    );

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(Backend::new);

    info!("LSP service created, starting server");
    Server::new(stdin, stdout, socket).serve(service).await;

    info!("Discord Presence LSP server stopped");
}
