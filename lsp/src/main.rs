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

use std::ffi::OsStr;
use std::fmt::Debug;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::sync::{Mutex, MutexGuard};

use configuration::Configuration;
use discord::Discord;
use git::get_repository_and_remote;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use util::Placeholders;

mod configuration;
mod discord;
mod git;
mod languages;
mod util;

#[derive(Debug)]
struct Document {
    path: PathBuf,
}

#[derive(Debug)]
struct Backend {
    client: Client,
    discord: Mutex<Discord>,
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

    fn get_filename(&self) -> String {
        let filename = self.path.file_name().unwrap().to_str().unwrap();
        let filename = urlencoding::decode(filename).unwrap();

        filename.to_string()
    }

    fn get_extension(&self) -> &str {
        self.path
            .extension()
            .unwrap_or(OsStr::new(""))
            .to_str()
            .unwrap()
    }
}

impl Backend {
    fn new(client: Client) -> Self {
        Self {
            client,
            discord: Mutex::new(Discord::new()),
            workspace_file_name: Mutex::new(String::new()),
            git_remote_url: Mutex::new(None),
            config: Mutex::new(Configuration::new()),
        }
    }

    async fn on_change(&self, doc: Document) {
        let config = self.get_config();
        let workspace = self.get_workspace_file_name();
        let placeholders = Placeholders::new(&doc, &config, workspace.deref());

        let state = config
            .state
            .as_ref()
            .map(|state| placeholders.replace(state));
        let details = config
            .details
            .as_ref()
            .map(|details| placeholders.replace(details));

        let large_image = config
            .large_image
            .as_ref()
            .map(|img| placeholders.replace(img));
        let large_text = config
            .large_text
            .as_ref()
            .map(|text| placeholders.replace(text));
        let small_image = config
            .small_image
            .as_ref()
            .map(|img| placeholders.replace(img));
        let small_text = config
            .small_text
            .as_ref()
            .map(|text| placeholders.replace(text));

        self.get_discord().change_activity(
            state,
            details,
            large_image,
            large_text,
            small_image,
            small_text,
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

    fn get_discord(&self) -> MutexGuard<Discord> {
        return self.discord.lock().expect("Failed to lock discord");
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
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

        let mut discord = self.get_discord();
        discord.create_client(config.application_id.to_string());

        if config.rules.suitable(
            workspace_path
                .to_str()
                .expect("Failed to transform workspace path to str"),
        ) {
            // Connect discord client
            discord.connect();
        } else {
            // Exit LSP
            exit(0);
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
        self.get_discord().kill();

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
