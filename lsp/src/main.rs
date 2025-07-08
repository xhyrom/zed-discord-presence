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

mod activity;
mod config;
mod discord;
mod document;
mod error;
mod git;
mod idle;
mod languages;
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

        Self {
            client,
            presence_service,
            app_state,
        }
    }

    async fn on_change(&self, doc: Document) {
        if let Err(e) = self.presence_service.update_presence(Some(doc)).await {
            eprintln!("Failed to update presence: {}", e);
        }
    }

    fn resolve_workspace_path(params: &InitializeParams) -> PathBuf {
        if let Some(folders) = &params.workspace_folders {
            if let Some(first_folder) = folders.first() {
                return Path::new(first_folder.uri.path()).to_owned();
            }
        }

        let root_uri = params.root_uri.as_ref().expect(
            "Failed to get workspace path - neither workspace_folders nor root_uri is present",
        );

        Path::new(root_uri.path()).to_owned()
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        // Set workspace
        let workspace_path = Self::resolve_workspace_path(&params);

        {
            let mut workspace = self.app_state.workspace.lock().await;
            if let Err(e) = workspace.set_workspace(&workspace_path) {
                eprintln!("Failed to set workspace: {}", e);
                return Err(tower_lsp::jsonrpc::Error::internal_error());
            }
        }

        // Set git remote URL
        {
            let mut git_remote_url = self.app_state.git_remote_url.lock().await;
            *git_remote_url = get_repository_and_remote(workspace_path.to_str().unwrap_or(""));
        }

        // Update config
        {
            let mut config = self.app_state.config.lock().await;
            if let Err(e) = config.update(params.initialization_options) {
                eprintln!("Failed to update config: {}", e);
                return Err(tower_lsp::jsonrpc::Error::internal_error());
            }

            // Check if workspace is suitable
            if !config.rules.suitable(workspace_path.to_str().unwrap_or("")) {
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
                eprintln!("Failed to initialize Discord: {}", e);
                return Err(tower_lsp::jsonrpc::Error::internal_error());
            }
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
        self.client
            .log_message(
                MessageType::INFO,
                "Discord Presence LSP server initialized!",
            )
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        if let Err(e) = self.presence_service.shutdown().await {
            eprintln!("Failed to shutdown presence service: {}", e);
        }
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
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(Backend::new);

    Server::new(stdin, stdout, socket).serve(service).await;
}
