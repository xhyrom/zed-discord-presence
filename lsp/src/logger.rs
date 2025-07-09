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

use std::env;
use tracing::Level;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

pub fn init_logger() {
    let log_level = env::var("DISCORD_PRESENCE_LOG_LEVEL")
        .unwrap_or_else(|_| "info".to_string())
        .to_lowercase();

    let level = match log_level.as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => unreachable!(),
    };

    let filter = EnvFilter::from_default_env()
        .add_directive(format!("discord_presence_lsp={level}").parse().unwrap())
        .add_directive(format!("tower_lsp={level}").parse().unwrap()) // Reduce tower-lsp noise
        .add_directive(format!("discord_rich_presence={level}").parse().unwrap()); // Reduce discord lib noise

    let log_to_file = env::var("DISCORD_PRESENCE_LOG_TO_FILE")
        .map(|v| v.to_lowercase() == "true")
        .unwrap_or(false);

    if log_to_file {
        let log_dir = env::var("DISCORD_PRESENCE_LOG_DIR").unwrap_or_else(|_| {
            if cfg!(target_os = "windows") {
                format!(
                    "{}\\AppData\\Local\\discord-presence-lsp\\logs",
                    env::var("USERPROFILE").unwrap_or_else(|_| "C:\\".to_string())
                )
            } else {
                format!(
                    "{}/.local/share/discord-presence-lsp/logs",
                    env::var("HOME").unwrap_or_else(|_| "/tmp".to_string())
                )
            }
        });

        if let Err(e) = std::fs::create_dir_all(&log_dir) {
            eprintln!("Failed to create log directory {log_dir}: {e}");
            return;
        }

        let file_appender =
            RollingFileAppender::new(Rotation::DAILY, log_dir, "discord-presence-lsp.log");
        let file_layer = fmt::layer()
            .with_writer(file_appender)
            .with_ansi(false)
            .with_target(true)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_filter(filter);

        tracing_subscriber::registry().with(file_layer).init();
    } else {
        let stderr_layer = fmt::layer()
            .with_writer(std::io::stderr)
            .with_ansi(false)
            .with_target(false)
            .with_thread_ids(false)
            .with_thread_names(false)
            .with_filter(filter);

        tracing_subscriber::registry().with(stderr_layer).init();
    }
}
