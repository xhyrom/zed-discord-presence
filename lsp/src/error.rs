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

use std::{fmt, string::FromUtf8Error};

#[derive(Debug)]
pub enum PresenceError {
    Discord(String),
    Config(String),
    Io(std::io::Error),
    JsonParse(serde_json::Error),
    UrlDecode(FromUtf8Error),
}

impl fmt::Display for PresenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PresenceError::Discord(msg) => write!(f, "Discord error: {msg}"),
            PresenceError::Config(msg) => write!(f, "Config error: {msg}"),
            PresenceError::Io(err) => write!(f, "IO error: {err}"),
            PresenceError::JsonParse(err) => write!(f, "JSON parse error: {err}"),
            PresenceError::UrlDecode(err) => write!(f, "URL decode error: {err}"),
        }
    }
}

impl std::error::Error for PresenceError {}

impl From<std::io::Error> for PresenceError {
    fn from(err: std::io::Error) -> Self {
        PresenceError::Io(err)
    }
}

impl From<serde_json::Error> for PresenceError {
    fn from(err: serde_json::Error) -> Self {
        PresenceError::JsonParse(err)
    }
}

impl From<FromUtf8Error> for PresenceError {
    fn from(err: FromUtf8Error) -> Self {
        PresenceError::UrlDecode(err)
    }
}

pub type Result<T> = std::result::Result<T, PresenceError>;
