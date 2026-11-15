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

mod presence_service;
mod workspace_service;

pub use presence_service::PresenceService;
pub use workspace_service::WorkspaceService;

use crate::{config::Configuration, discord::Discord, document::Document};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::Mutex;

#[derive(Debug)]
pub struct AppState {
    pub discord: Arc<Mutex<Discord>>,
    pub config: Arc<Mutex<Configuration>>,
    pub workspace: Arc<Mutex<WorkspaceService>>,
    pub git_remote_url: Arc<Mutex<Option<String>>>,
    pub git_branch: Arc<Mutex<Option<String>>>,
    pub last_document: Arc<Mutex<Option<Document>>>,
    pub shutting_down: Arc<AtomicBool>,
}

impl AppState {
    pub fn new() -> Self {
        let shutting_down = Arc::new(AtomicBool::new(false));

        Self {
            discord: Arc::new(Mutex::new(Discord::new(Arc::clone(&shutting_down)))),
            config: Arc::new(Mutex::new(Configuration::default())),
            workspace: Arc::new(Mutex::new(WorkspaceService::new())),
            git_remote_url: Arc::new(Mutex::new(None)),
            git_branch: Arc::new(Mutex::new(None)),
            last_document: Arc::new(Mutex::new(None)),
            shutting_down,
        }
    }

    pub fn is_shutting_down(&self) -> bool {
        self.shutting_down.load(Ordering::SeqCst)
    }

    pub fn mark_shutting_down(&self) -> bool {
        !self.shutting_down.swap(true, Ordering::SeqCst)
    }
}
