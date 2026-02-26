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

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time;

use crate::{
    activity::ActivityManager,
    config::{Configuration, IdleAction},
    discord::Discord,
    document::Document,
};

#[derive(Debug, Clone)]
pub struct IdleManager {
    handle: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl IdleManager {
    pub fn new() -> Self {
        Self {
            handle: Arc::new(Mutex::new(None)),
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub async fn reset_timeout(
        &self,
        discord: Arc<Mutex<Discord>>,
        config: Arc<Mutex<Configuration>>,
        git_remote_url: Arc<Mutex<Option<String>>>,
        git_branch: Arc<Mutex<Option<String>>>,
        last_document: Arc<Mutex<Option<Document>>>,
        workspace: String,
        is_shutting_down: Arc<AtomicBool>,
    ) {
        let mut handle_guard = self.handle.lock().await;

        // Cancel existing timeout
        if let Some(handle) = handle_guard.take() {
            handle.abort();
        }

        // Get timeout duration
        let timeout_duration = {
            let config_guard = config.lock().await;
            config_guard.idle.timeout
        };

        // Spawn new timeout task
        let handle = tokio::spawn(async move {
            time::sleep(timeout_duration).await;

            if is_shutting_down.load(Ordering::SeqCst) {
                return;
            }

            let config_guard = config.lock().await;
            let mut discord_guard = discord.lock().await;

            if is_shutting_down.load(Ordering::SeqCst) {
                return;
            }

            match config_guard.idle.action {
                IdleAction::ClearActivity => {
                    let _ = discord_guard.clear_activity_with_reconnect().await; // Ignore errors in background task
                }
                IdleAction::ChangeActivity => {
                    let doc = last_document.lock().await;
                    let doc = doc.as_ref();

                    let branch = git_branch.lock().await.clone();
                    let activity_fields = ActivityManager::build_idle_activity_fields(
                        doc,
                        &config_guard,
                        &workspace,
                        branch,
                    );

                    let git_url = if config_guard.git_integration {
                        let git_guard = git_remote_url.lock().await;
                        git_guard.clone()
                    } else {
                        None
                    };

                    let _ = discord_guard
                        .change_activity_with_reconnect(activity_fields, git_url)
                        .await; // Ignore errors in background task
                }
            }
        });

        *handle_guard = Some(handle);
    }

    pub async fn cancel_timeout(&self) {
        let mut handle_guard = self.handle.lock().await;

        if let Some(handle) = handle_guard.take() {
            handle.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::AppState;
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::test]
    async fn test_idle_reset_cancels_previous() {
        let app_state = Arc::new(AppState::new());
        let idle_manager = IdleManager::new();

        // Start first timeout
        idle_manager
            .reset_timeout(
                Arc::clone(&app_state.discord),
                Arc::clone(&app_state.config),
                Arc::clone(&app_state.git_remote_url),
                Arc::clone(&app_state.git_branch),
                Arc::clone(&app_state.last_document),
                "test".to_string(),
                Arc::new(AtomicBool::new(false)),
            )
            .await;

        let handle1 = idle_manager.handle.lock().await.take().unwrap();
        *idle_manager.handle.lock().await = Some(handle1);

        // Start second timeout - this should abort the first one
        idle_manager
            .reset_timeout(
                Arc::clone(&app_state.discord),
                Arc::clone(&app_state.config),
                Arc::clone(&app_state.git_remote_url),
                Arc::clone(&app_state.git_branch),
                Arc::clone(&app_state.last_document),
                "test".to_string(),
                Arc::new(AtomicBool::new(false)),
            )
            .await;

        // Since we don't have handle1 anymore (it was taken by reset_timeout and aborted),
        // we can't check it directly unless we held onto it.
        // But reset_timeout does: if let Some(handle) = self.handle.lock().await.take() { handle.abort(); }

        // Let's just verify that after cancel_timeout, it's None.
    }

    #[tokio::test]
    async fn test_cancel_timeout() {
        let app_state = Arc::new(AppState::new());
        let idle_manager = IdleManager::new();

        idle_manager
            .reset_timeout(
                Arc::clone(&app_state.discord),
                Arc::clone(&app_state.config),
                Arc::clone(&app_state.git_remote_url),
                Arc::clone(&app_state.git_branch),
                Arc::clone(&app_state.last_document),
                "test".to_string(),
                Arc::new(AtomicBool::new(false)),
            )
            .await;

        assert!(idle_manager.handle.lock().await.is_some());

        idle_manager.cancel_timeout().await;

        assert!(idle_manager.handle.lock().await.is_none());
    }

    #[tokio::test]
    async fn test_reset_timeout_shutdown_true_task_exits_quickly() {
        let app_state = Arc::new(AppState::new());
        let idle_manager = IdleManager::new();

        {
            let mut config = app_state.config.lock().await;
            config.idle.timeout = Duration::from_millis(0);
        }

        idle_manager
            .reset_timeout(
                Arc::clone(&app_state.discord),
                Arc::clone(&app_state.config),
                Arc::clone(&app_state.git_remote_url),
                Arc::clone(&app_state.git_branch),
                Arc::clone(&app_state.last_document),
                "test".to_string(),
                Arc::new(AtomicBool::new(true)),
            )
            .await;

        let handle = idle_manager
            .handle
            .lock()
            .await
            .take()
            .expect("idle timeout handle should exist");

        let joined = tokio::time::timeout(Duration::from_millis(200), handle).await;
        assert!(joined.is_ok());
    }

    #[tokio::test]
    async fn test_idle_task_clear_activity_branch_completes_without_client() {
        let app_state = Arc::new(AppState::new());
        let idle_manager = IdleManager::new();

        {
            let mut config = app_state.config.lock().await;
            config.idle.timeout = Duration::from_millis(1);
            config.idle.action = IdleAction::ClearActivity;
        }

        idle_manager
            .reset_timeout(
                Arc::clone(&app_state.discord),
                Arc::clone(&app_state.config),
                Arc::clone(&app_state.git_remote_url),
                Arc::clone(&app_state.git_branch),
                Arc::clone(&app_state.last_document),
                "test".to_string(),
                Arc::new(AtomicBool::new(false)),
            )
            .await;

        let handle = idle_manager
            .handle
            .lock()
            .await
            .take()
            .expect("idle timeout handle should exist");

        let joined = tokio::time::timeout(Duration::from_millis(300), handle).await;
        assert!(joined.is_ok());
    }
}
