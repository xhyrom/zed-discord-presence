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
use std::sync::Arc;
use std::time::Duration;

use configuration::Configuration;
use discord::Discord;
use git::get_repository_and_remote;
use tokio::sync::{Mutex, MutexGuard};
use tokio::task::JoinHandle;
use tokio::time;
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
    discord: Arc<Mutex<Discord>>,
    workspace_file_name: Arc<Mutex<String>>,
    git_remote_url: Arc<Mutex<Option<String>>>,
    config: Arc<Mutex<Configuration>>,
    idle_timeout: Arc<Mutex<Option<JoinHandle<()>>>>,
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
            discord: Arc::new(Mutex::new(Discord::new())),
            workspace_file_name: Arc::new(Mutex::new(String::new())),
            git_remote_url: Arc::new(Mutex::new(None)),
            config: Arc::new(Mutex::new(Configuration::new())),
            idle_timeout: Arc::new(Mutex::new(None)),
        }
    }

    async fn on_change(&self, doc: Document) {
        self.reset_idle_timeout().await;

        let (state, details, large_image, large_text, small_image, small_text, git_integration) =
            self.get_config_values(Some(&doc)).await;

        self.get_discord()
            .await
            .change_activity(
                state,
                details,
                large_image,
                large_text,
                small_image,
                small_text,
                if git_integration {
                    self.get_git_remote_url().await
                } else {
                    None
                },
            )
            .await;
    }

    async fn reset_idle_timeout(&self) {
        let mut idle_timeout = self.idle_timeout.lock().await;

        if let Some(handle) = idle_timeout.take() {
            handle.abort();
        }

        let discord_clone = Arc::clone(&self.discord);
        let config_clone = Arc::clone(&self.config);
        let git_remote_url_clone = Arc::clone(&self.git_remote_url);

        let timeout_duration = {
            let config_guard = config_clone.lock().await;
            Duration::from_secs(config_guard.idle.timeout)
        };

        let handle = tokio::spawn(async move {
            time::sleep(timeout_duration).await;

            let config_guard = config_clone.lock().await;
            let placeholders = Placeholders::new(None, &config_guard, "");

            let discord_guard = discord_clone.lock().await;

            if config_guard.idle.action == configuration::IdleAction::ClearActivity {
                discord_guard.clear_activity().await;
                return;
            }

            let (state, details, large_image, large_text, small_image, small_text) =
                Backend::process_fields(
                    &placeholders,
                    &config_guard.idle.state,
                    &config_guard.idle.details,
                    &config_guard.idle.large_image,
                    &config_guard.idle.large_text,
                    &config_guard.idle.small_image,
                    &config_guard.idle.small_text,
                );

            discord_guard
                .change_activity(
                    state,
                    details,
                    large_image,
                    large_text,
                    small_image,
                    small_text,
                    if config_guard.git_integration {
                        let git_remote_url_guard = git_remote_url_clone.lock().await;
                        git_remote_url_guard.clone()
                    } else {
                        None
                    },
                )
                .await;
        });

        *idle_timeout = Some(handle);
    }

    async fn get_workspace_file_name(&self) -> MutexGuard<'_, String> {
        return self.workspace_file_name.lock().await;
    }

    async fn get_git_remote_url(&self) -> Option<String> {
        let guard = self.git_remote_url.lock().await;

        guard.clone()
    }

    async fn get_config(&self) -> MutexGuard<Configuration> {
        return self.config.lock().await;
    }

    async fn get_discord(&self) -> MutexGuard<Discord> {
        return self.discord.lock().await;
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

    #[allow(clippy::type_complexity)]
    fn process_fields(
        placeholders: &Placeholders,
        state: &Option<String>,
        details: &Option<String>,
        large_image: &Option<String>,
        large_text: &Option<String>,
        small_image: &Option<String>,
        small_text: &Option<String>,
    ) -> (
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) {
        let state = state.as_ref().map(|s| placeholders.replace(s));
        let details = details.as_ref().map(|d| placeholders.replace(d));
        let large_image = large_image.as_ref().map(|img| placeholders.replace(img));
        let large_text = large_text.as_ref().map(|text| placeholders.replace(text));
        let small_image = small_image.as_ref().map(|img| placeholders.replace(img));
        let small_text = small_text.as_ref().map(|text| placeholders.replace(text));

        (
            state,
            details,
            large_image,
            large_text,
            small_image,
            small_text,
        )
    }

    #[allow(clippy::type_complexity)]
    async fn get_config_values(
        &self,
        doc: Option<&Document>,
    ) -> (
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        bool,
    ) {
        let config = self.get_config().await;
        let workspace = self.get_workspace_file_name().await;
        let placeholders = Placeholders::new(doc, &config, workspace.deref());

        let (state, details, large_image, large_text, small_image, small_text) =
            Self::process_fields(
                &placeholders,
                &config.state,
                &config.details,
                &config.large_image,
                &config.large_text,
                &config.small_image,
                &config.small_text,
            );

        (
            state,
            details,
            large_image,
            large_text,
            small_image,
            small_text,
            config.git_integration,
        )
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        // Set workspace name
        let workspace_path = Self::resolve_workspace_path(&params);

        self.workspace_file_name.lock().await.push_str(
            workspace_path
                .file_name()
                .expect("Failed to get workspace file name")
                .to_str()
                .expect("Failed to convert workspace file name &OsStr to &str"),
        );

        let mut git_remote_url = self.git_remote_url.lock().await;
        *git_remote_url = get_repository_and_remote(workspace_path.to_str().unwrap());

        let mut config = self.config.lock().await;
        config.set(params.initialization_options);

        let mut discord = self.get_discord().await;
        discord.create_client(config.application_id.to_string());

        if config.rules.suitable(
            workspace_path
                .to_str()
                .expect("Failed to transform workspace path to str"),
        ) {
            // Connect discord client
            discord.connect().await;
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
        self.get_discord().await.kill().await;

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
