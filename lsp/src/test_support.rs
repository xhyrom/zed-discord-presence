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

//! Helpers shared by unit tests.

use tracing::span;
use tracing::subscriber::Interest;
use tracing::{Event, Metadata, Subscriber};

/// Registered as a dispatcher but never installed on any thread, so it never
/// receives events. Its only job is to answer "sometimes" for every callsite,
/// which makes tracing-core ask the emitting thread's subscriber per event.
struct AskEveryThread;

impl Subscriber for AskEveryThread {
    fn register_callsite(&self, _: &'static Metadata<'static>) -> Interest {
        Interest::sometimes()
    }

    fn enabled(&self, _: &Metadata<'_>) -> bool {
        false
    }

    fn new_span(&self, _: &span::Attributes<'_>) -> span::Id {
        span::Id::from_u64(1)
    }

    fn record(&self, _: &span::Id, _: &span::Record<'_>) {}

    fn record_follows_from(&self, _: &span::Id, _: &span::Id) {}

    fn event(&self, _: &Event<'_>) {}

    fn enter(&self, _: &span::Id) {}

    fn exit(&self, _: &span::Id) {}
}

/// Call before installing a thread-local log capture with `set_default`.
///
/// tracing-core caches each callsite's interest globally on its first hit.
/// While only one dispatcher is registered, it takes that interest from the
/// hitting thread's subscriber alone, so a parallel test without a subscriber
/// caches `Interest::never` and a capture on another thread silently misses
/// the event. Keeping a second dispatcher registered for the whole test run
/// means interest is combined across dispatchers and is never `never`.
pub(crate) fn keep_callsites_interested() {
    static KEEPER: std::sync::OnceLock<tracing::Dispatch> = std::sync::OnceLock::new();
    KEEPER.get_or_init(|| tracing::Dispatch::new(AskEveryThread));
}
