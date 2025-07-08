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
use std::sync::Arc;

use document::Document;
use git::get_repository_and_remote;
use service::{AppState, PresenceService};
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tracing::{debug, error, info, instrument};

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
    presence_service: PresenceService,
    app_state: Arc<AppState>,
}

impl Backend {
    fn new(client: Client) -> Self {
        let app_state = Arc::new(AppState::new());
        let presence_service = PresenceService::new(Arc::clone(&app_state));

        info!("Backend initialized");

        Self {
            client,
            presence_service,
            app_state,
        }
    }

    async fn on_change(&self, doc: Document) {
        debug!("Document changed");

        if let Err(e) = self.presence_service.update_presence(Some(doc)).await {
            error!("Failed to update presence: {}", e);
        } else {
            debug!("Presence updated successfully");
        }
    }

    fn resolve_workspace_path(params: &InitializeParams) -> PathBuf {
        if let Some(folders) = &params.workspace_folders {
            if let Some(first_folder) = folders.first() {
                let path = Path::new(first_folder.uri.path()).to_owned();
                debug!("Using workspace folder: {}", path.display());
                return path;
            }
        }

        let root_uri = params.root_uri.as_ref().expect(
            "Failed to get workspace path - neither workspace_folders nor root_uri is present",
        );

        let path = Path::new(root_uri.path()).to_owned();
        debug!("Using root URI: {}", path.display());
        path
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        info!("Initializing Discord Presence LSP");

        // Set workspace
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

        // Set git remote URL
        {
            let mut git_remote_url = self.app_state.git_remote_url.lock().await;
            let remote_url = get_repository_and_remote(workspace_path.to_str().unwrap_or(""));

            if let Some(ref url) = remote_url {
                info!("Git remote URL found: {}", url);
            } else {
                debug!("No git remote URL found");
            }

            *git_remote_url = remote_url;
        }

        // Update config
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

            // Check if workspace is suitable
            if !config.rules.suitable(workspace_path.to_str().unwrap_or("")) {
                info!("Workspace not suitable according to rules, exiting");
                exit(0);
            }
        }

        // Initialize Discord
        {
            let config = self.app_state.config.lock().await;
            if let Err(e) = self
                .presence_service
                .initialize_discord(&config.application_id)
                .await
            {
                error!("Failed to initialize Discord: {}", e);
                return Err(tower_lsp::jsonrpc::Error::internal_error());
            }
            info!("Discord client initialized and connected");
        }

        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: env!("CARGO_PKG_NAME").into(),
                version: Some(env!("CARGO_PKG_VERSION").into()),
            }),
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::INCREMENTAL,
                )),
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
        self.on_change(Document::new(params.text_document.uri))
            .await;
    }

    #[instrument(skip(self, params))]
    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        debug!("Document changed: {}", params.text_document.uri);
        self.on_change(Document::new(params.text_document.uri))
            .await;
    }

    #[instrument(skip(self, params))]
    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        debug!("Document saved: {}", params.text_document.uri);
        self.on_change(Document::new(params.text_document.uri))
            .await;
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
