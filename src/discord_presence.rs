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

use std::fs;
use zed_extension_api::{self as zed};

struct DiscordPresenceExtension {
    cached_binary_path: Option<String>,
}

#[cfg(unix)]
fn create_symlink(src: &str, dst: &str) -> std::io::Result<()> {
    std::os::unix::fs::symlink(src, dst)
}

#[cfg(windows)]
fn create_symlink(src: &str, dst: &str) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(src, dst)
}

#[cfg(not(any(unix, windows)))]
fn create_symlink(_src: &str, _dst: &str) -> std::io::Result<()> {
    fs::soft_link(_src, _dst)
}

#[allow(clippy::match_wildcard_for_single_variants)]
impl DiscordPresenceExtension {
    fn fallback(&mut self) -> zed::Result<String> {
        if let Some(path) = &self.cached_binary_path {
            if fs::metadata(path).is_ok_and(|stat| stat.is_file()) {
                return Ok(path.clone());
            }
        }

        let binary_path: String = "discord-presence-lsp".to_string();

        if !fs::metadata(&binary_path).is_ok_and(|stat| stat.is_file()) {
            return Err("failed to find fallback language server binary".to_string());
        }

        self.cached_binary_path = Some(binary_path.clone());
        Ok(binary_path)
    }

    fn language_server_binary_path(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<String> {
        if let Some(path) = worktree.which("discord-presence-lsp") {
            return Ok(path);
        }

        if let Some(path) = &self.cached_binary_path {
            if fs::metadata(path).is_ok_and(|stat| stat.is_file()) {
                return Ok(path.clone());
            }
        }

        zed::set_language_server_installation_status(
            language_server_id,
            &zed_extension_api::LanguageServerInstallationStatus::CheckingForUpdate,
        );

        let release = zed::latest_github_release(
            "xhyrom/zed-discord-presence",
            zed::GithubReleaseOptions {
                require_assets: true,
                pre_release: false,
            },
        )?;

        let (platform, arch) = zed::current_platform();
        let asset_name = format!(
            "discord-presence-lsp-{arch}-{os}.{extension}",
            arch = match arch {
                zed::Architecture::Aarch64 => "aarch64",
                zed::Architecture::X8664 => "x86_64",
                _ => return Err(format!("unsupported architecture: {arch:?}")),
            },
            os = match platform {
                zed::Os::Mac => "apple-darwin",
                zed::Os::Linux => "unknown-linux-gnu",
                zed::Os::Windows => "pc-windows-msvc",
            },
            extension = match platform {
                zed::Os::Mac | zed::Os::Linux => "tar.gz",
                zed::Os::Windows => "zip",
            }
        );

        let asset = release
            .assets
            .iter()
            .find(|asset| asset.name == asset_name)
            .ok_or_else(|| format!("no asset found matching {asset_name:?}"))?;

        let version_dir = format!("discord-presence-lsp-{}", release.version);
        let asset_name = asset_name
            .split('.')
            .next()
            .expect("failed to split asset name");

        let binary_path: String = match platform {
            zed::Os::Windows => format!("{version_dir}/{asset_name}/discord-presence-lsp.exe"),
            _ => format!("{version_dir}/{asset_name}/discord-presence-lsp"),
        };

        if !fs::metadata(&binary_path).is_ok_and(|stat| stat.is_file()) {
            zed::set_language_server_installation_status(
                language_server_id,
                &zed::LanguageServerInstallationStatus::Downloading,
            );

            zed::download_file(
                &asset.download_url,
                &version_dir,
                match platform {
                    zed::Os::Mac | zed::Os::Linux => zed::DownloadedFileType::GzipTar,
                    zed::Os::Windows => zed::DownloadedFileType::Zip,
                },
            )
            .map_err(|e| format!("failed to download file: {e}"))?;

            zed::make_file_executable(&binary_path).expect("failed to make file executable");

            let entries =
                fs::read_dir(".").map_err(|e| format!("failed to list working directory {e}"))?;

            for entry in entries {
                let entry = entry.map_err(|e| format!("failed to load directory entry {e}"))?;
                if entry.file_name().to_str() != Some(&version_dir) {
                    fs::remove_dir_all(entry.path()).ok();
                }
            }

            let _ = fs::remove_file("discord-presence-lsp");
        }

        if !fs::metadata("discord-presence-lsp").is_ok_and(|stat| stat.is_file()) {
            create_symlink(&binary_path, "discord-presence-lsp")
                .map_err(|e| format!("failed to create symlink: {e}"))?;
        }

        self.cached_binary_path = Some(binary_path.clone());
        Ok(binary_path)
    }
}

impl zed::Extension for DiscordPresenceExtension {
    fn new() -> Self {
        Self {
            cached_binary_path: None,
        }
    }

    fn language_server_command(
        &mut self,
        language_server_id: &zed_extension_api::LanguageServerId,
        worktree: &zed_extension_api::Worktree,
    ) -> zed_extension_api::Result<zed_extension_api::Command> {
        let mut path = self.language_server_binary_path(language_server_id, worktree);
        if let Err(orig_e) = path {
            path = self
                .fallback()
                .map_err(|e| format!("failed to get language server binary path: {e}, {orig_e}"));
        }

        Ok(zed::Command {
            command: path.map_err(|e| format!("failed to get language server binary path: {e}"))?,
            args: vec![],
            env: vec![],
        })
    }
}

zed::register_extension!(DiscordPresenceExtension);
