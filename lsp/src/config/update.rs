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

use serde_json::Value;

pub trait UpdateFromJson {
    fn update_from_json(&mut self, json: &Value) -> Result<()>;
}

macro_rules! update_optional_string_field {
    ($target:expr, $json:expr, $field:ident, $key:expr) => {
        if let Some(value) = $json.get($key) {
            $target.$field = if value.is_null() {
                None
            } else {
                value.as_str().map(String::from)
            };
        }
    };
}

pub(crate) use update_optional_string_field;

use crate::error::Result;
