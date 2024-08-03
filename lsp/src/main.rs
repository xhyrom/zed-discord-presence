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

use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use configuration::Configuration;
use discord::Discord;
use git::get_repository_and_remote;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

mod configuration;
mod discord;
mod git;

#[derive(Debug)]
struct Document {
    path: PathBuf,
}

#[derive(Debug)]
struct Backend {
    discord: discord::Discord,
    client: Client,
    workspace_file_name: Mutex<String>,
    git_remote_url: Mutex<Option<String>>,
    config: Mutex<Configuration>,
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
            git_remote_url: Mutex::new(None),
            config: Mutex::new(Configuration::new()),
        }
    }

    async fn on_change(&self, doc: Document) {
        let config = self.get_config();

        let state = config.state.replace("{filename}", &doc.get_filename());
        let details = config
            .details
            .replace("{workspace}", &self.get_workspace_file_name());

        self.discord.change_activity(
            state,
            details,
            if config.git_integration {
                self.get_git_remote_url()
            } else {
                None
            },
        );
    }

    fn get_workspace_file_name(&self) -> MutexGuard<'_, String> {
        return self
            .workspace_file_name
            .lock()
            .expect("Failed to lock workspace file name");
    }

    fn get_git_remote_url(&self) -> Option<String> {
        let guard = self
            .git_remote_url
            .lock()
            .expect("Failed to lock git remote url");

        guard.clone()
    }

    fn get_config(&self) -> MutexGuard<Configuration> {
        return self.config.lock().expect("Failed to lock config");
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

        let mut git_remote_url = self.git_remote_url.lock().unwrap();
        *git_remote_url = get_repository_and_remote(workspace_path.to_str().unwrap());

        let mut config = self.config.lock().unwrap();
        config.set(params.initialization_options);

        Ok(InitializeResult {
            server_info: Some(ServerInfo {
                name: env!("CARGO_PKG_NAME").into(),
                version: Some(env!("CARGO_PKG_VERSION").into()),
            }),
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::INCREMENTAL,
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
        self.discord.kill();

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

    let (service, socket) = LspService::new(Backend::new);

    Server::new(stdin, stdout, socket).serve(service).await;
}
