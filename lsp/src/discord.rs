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

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex, MutexGuard as StdMutexGuard, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, MutexGuard};
use tokio::task::JoinHandle;
use tokio::time::Instant;

use discord_rich_presence::{
    activity::{Activity, Assets, Button, Timestamps},
    DiscordIpc, DiscordIpcClient,
};
use tracing::{debug, error, info, instrument, warn};

use crate::activity::ActivityFields;
use crate::{error::Result, util};

/// Maximum number of connection retries
const MAX_RETRIES: u32 = 5;
/// Initial delay between retries in milliseconds
const INITIAL_DELAY_MS: u64 = 500;
/// Maximum delay between retries in milliseconds
const MAX_DELAY_MS: u64 = 10_000;
/// Upper bound for opening the IPC pipe and completing the handshake.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// An identical activity is skipped while its last send is younger than this.
/// Once older, it is sent again to re-assert the presence (e.g. after a Discord
/// restart) and to surface a broken pipe through the failed write.
const REASSERT_INTERVAL: Duration = Duration::from_secs(20);
/// Minimum gap between two activity sends, matching the 2 s update throttle.
/// Discord documents 5 updates per 20 seconds for the Game SDK (the RPC docs
/// give no figure); identical activity is skipped, so only changing content
/// can reach one write per 2 s.
const MIN_SEND_INTERVAL: Duration = Duration::from_secs(2);

/// Activity fields plus the optional repository URL shown as a button.
type ActivityKey = (ActivityFields, Option<String>);

/// What `change_activity` should do with a requested activity.
#[derive(Debug, PartialEq, Eq)]
enum SendDecision {
    /// Send it now.
    Send,
    /// Identical to the last send, which is still recent: do nothing.
    Skip,
    /// Too soon after the previous send: stored as pending, and a flush must
    /// be scheduled at the given instant.
    Schedule(Instant),
    /// A flush is already scheduled: the activity replaced the pending one.
    Queued,
}

/// Deduplication and rate limiting state for activity sends.
///
/// Shared with the deferred flush task, so it lives behind a std mutex that is
/// only held briefly and never across an `.await`.
#[derive(Debug, Default)]
struct SendGate {
    last_activity: Option<ActivityKey>,
    last_sent_at: Option<Instant>,
    /// Latest activity waiting for the minimum send interval to pass.
    pending: Option<ActivityKey>,
    /// When the scheduled flush runs; `Some` while a flush task owns `pending`.
    flush_at: Option<Instant>,
    flush_task: Option<JoinHandle<()>>,
}

impl SendGate {
    fn decide(&mut self, activity: &ActivityKey, now: Instant) -> SendDecision {
        if self.flush_at.is_some() {
            self.pending = Some(activity.clone());
            return SendDecision::Queued;
        }

        let Some(sent_at) = self.last_sent_at else {
            return SendDecision::Send;
        };
        let elapsed = now.saturating_duration_since(sent_at);

        if elapsed < REASSERT_INTERVAL && self.last_activity.as_ref() == Some(activity) {
            return SendDecision::Skip;
        }

        if elapsed < MIN_SEND_INTERVAL {
            let flush_at = sent_at + MIN_SEND_INTERVAL;
            self.pending = Some(activity.clone());
            self.flush_at = Some(flush_at);
            return SendDecision::Schedule(flush_at);
        }

        SendDecision::Send
    }

    fn record_sent(&mut self, activity: ActivityKey, now: Instant) {
        self.last_activity = Some(activity);
        self.last_sent_at = Some(now);
    }

    /// Takes the pending activity for the running flush. Returns `None`, and
    /// ends the flush, if nothing is pending or it matches a recent send.
    fn take_pending(&mut self, now: Instant) -> Option<ActivityKey> {
        let recent = self
            .last_sent_at
            .is_some_and(|sent_at| now.saturating_duration_since(sent_at) < REASSERT_INTERVAL);
        let activity = self.pending.take();
        let duplicate = activity
            .as_ref()
            .is_some_and(|activity| recent && self.last_activity.as_ref() == Some(activity));

        if activity.is_none() || duplicate {
            self.flush_at = None;
            return None;
        }
        activity
    }

    /// Called by the flush task after a send: schedules another flush if an
    /// activity was queued meanwhile, otherwise ends the flush.
    fn next_flush(&mut self) -> Option<Instant> {
        self.flush_at = match (&self.pending, self.last_sent_at) {
            (Some(_), Some(sent_at)) => Some(sent_at + MIN_SEND_INTERVAL),
            _ => None,
        };
        self.flush_at
    }

    fn end_flush(&mut self) {
        self.flush_at = None;
    }

    /// Forgets the last activity so the next one is sent even if identical.
    /// The send time is kept, so the minimum interval still applies.
    fn forget_last_activity(&mut self) {
        self.last_activity = None;
    }

    /// Drops the pending activity and stops any scheduled flush.
    fn cancel_pending(&mut self) {
        self.pending = None;
        self.flush_at = None;
        if let Some(task) = self.flush_task.take() {
            task.abort();
        }
    }
}

/// Locks the send gate, recovering the state if a previous holder panicked.
fn lock_gate(gate: &StdMutex<SendGate>) -> StdMutexGuard<'_, SendGate> {
    gate.lock().unwrap_or_else(PoisonError::into_inner)
}

#[derive(Debug)]
pub struct Discord {
    client: Option<Arc<Mutex<DiscordIpcClient>>>,
    start_timestamp: Duration,
    connected: Arc<AtomicBool>,
    shutting_down: Arc<AtomicBool>,
    send_gate: Arc<StdMutex<SendGate>>,
}

impl Discord {
    pub fn new(shutting_down: Arc<AtomicBool>) -> Self {
        let start_timestamp = SystemTime::now();
        let since_epoch = start_timestamp
            .duration_since(UNIX_EPOCH)
            .expect("Failed to get duration since UNIX_EPOCH");

        Self {
            client: None,
            start_timestamp: since_epoch,
            connected: Arc::new(AtomicBool::new(false)),
            shutting_down,
            send_gate: Arc::default(),
        }
    }

    /// Returns whether the Discord IPC client is currently connected.
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    pub fn has_client(&self) -> bool {
        self.client.is_some()
    }

    fn is_shutting_down(&self) -> bool {
        self.shutting_down.load(Ordering::SeqCst)
    }

    fn shutdown_error(operation: &str) -> crate::error::PresenceError {
        crate::error::PresenceError::Discord(format!(
            "Cannot {operation} while shutdown is in progress"
        ))
    }

    #[instrument(skip(self))]
    pub fn create_client(&mut self, application_id: &str) -> Result<()> {
        info!(
            "Creating Discord IPC client with app ID: {}",
            application_id
        );

        let discord_client = DiscordIpcClient::new(application_id);
        self.client = Some(Arc::new(Mutex::new(discord_client)));

        debug!("Discord IPC client created successfully");
        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn connect(&self) -> Result<()> {
        if self.is_shutting_down() {
            return Err(Self::shutdown_error("connect to Discord IPC"));
        }

        debug!("Connecting to Discord IPC");

        let client = Arc::clone(self.client.as_ref().ok_or_else(|| {
            crate::error::PresenceError::Discord("Discord client not initialized".to_string())
        })?);

        // The crate's connect is blocking pipe IO (open + handshake read). Run
        // it on the blocking pool so it never stalls a runtime worker, and
        // bound it, including the wait for the client lock. On timeout the
        // blocking call may hold the client lock until it returns; later
        // attempts then time out too instead of hanging.
        let attempt = tokio::time::timeout(CONNECT_TIMEOUT, async move {
            let mut client = client.lock_owned().await;
            tokio::task::spawn_blocking(move || client.connect()).await
        })
        .await;

        let failure = match attempt {
            Ok(Ok(Ok(()))) => {
                self.connected.store(true, Ordering::SeqCst);
                info!("Successfully connected to Discord IPC");
                return Ok(());
            }
            Ok(Ok(Err(e))) => format!("Failed to connect to Discord IPC: {e}"),
            Ok(Err(e)) => format!("Discord IPC connect task failed: {e}"),
            Err(_) => format!("Timed out connecting to Discord IPC after {CONNECT_TIMEOUT:?}"),
        };

        self.connected.store(false, Ordering::SeqCst);
        error!("{}", failure);
        Err(crate::error::PresenceError::Discord(failure))
    }

    /// Connects to Discord IPC with exponential backoff retry.
    /// Will retry up to `MAX_RETRIES` times with increasing delays.
    #[instrument(skip(self))]
    pub async fn connect_with_retry(&self) -> Result<()> {
        if self.is_shutting_down() {
            return Err(Self::shutdown_error("connect to Discord IPC"));
        }

        if !self.has_client() {
            return Err(crate::error::PresenceError::Discord(
                "Discord client not initialized".to_string(),
            ));
        }

        let mut delay = Duration::from_millis(INITIAL_DELAY_MS);

        for attempt in 1..=MAX_RETRIES {
            match self.connect().await {
                Ok(()) => {
                    info!("Connected to Discord IPC on attempt {}", attempt);
                    return Ok(());
                }
                Err(e) => {
                    if attempt < MAX_RETRIES {
                        warn!(
                            "Connection attempt {}/{} failed: {}. Retrying in {:?}...",
                            attempt, MAX_RETRIES, e, delay
                        );
                        tokio::time::sleep(delay).await;
                        delay = (delay * 2).min(Duration::from_millis(MAX_DELAY_MS));
                    } else {
                        error!(
                            "Failed to connect to Discord after {} attempts: {}",
                            MAX_RETRIES, e
                        );
                        return Err(e);
                    }
                }
            }
        }

        Err(crate::error::PresenceError::Discord(
            "Failed to connect after all retries".into(),
        ))
    }

    /// Attempts to reconnect to Discord, closing any existing connection first.
    #[instrument(skip(self))]
    pub async fn reconnect(&self) -> Result<()> {
        if self.is_shutting_down() {
            return Err(Self::shutdown_error("reconnect to Discord IPC"));
        }

        if !self.has_client() {
            return Err(crate::error::PresenceError::Discord(
                "Discord client not initialized".to_string(),
            ));
        }

        info!("Attempting to reconnect to Discord IPC");
        self.connected.store(false, Ordering::SeqCst);

        // Try to close existing connection (ignore errors)
        let _ = self.kill().await;

        self.connect_with_retry().await
    }

    pub async fn kill(&self) -> Result<()> {
        debug!("Killing Discord IPC client");
        self.connected.store(false, Ordering::SeqCst);
        lock_gate(&self.send_gate).cancel_pending();

        if !self.has_client() {
            debug!("Skipping Discord IPC close because the client was not initialized");
            return Ok(());
        }

        let mut client = self.get_client().await?;
        client.close().map_err(|e| {
            crate::error::PresenceError::Discord(format!("Failed to close Discord connection: {e}"))
        })?;

        Ok(())
    }

    pub async fn get_client(&self) -> Result<MutexGuard<'_, DiscordIpcClient>> {
        let client = self.client.as_ref().ok_or_else(|| {
            crate::error::PresenceError::Discord("Discord client not initialized".to_string())
        })?;

        Ok(client.lock().await)
    }

    #[instrument(skip(self))]
    pub async fn clear_activity(&mut self) -> Result<()> {
        debug!("Clearing Discord activity");

        {
            // A deferred send must not overwrite the cleared presence.
            let mut gate = lock_gate(&self.send_gate);
            gate.cancel_pending();
            gate.forget_last_activity();
        }

        if !self.has_client() {
            debug!("Skipping Discord activity clear because the client was not initialized");
            return Ok(());
        }

        let mut client = self.get_client().await?;
        client.clear_activity().map_err(|e| {
            error!("Failed to clear activity: {}", e);
            // Mark as disconnected so next attempt will reconnect
            self.connected.store(false, Ordering::SeqCst);
            crate::error::PresenceError::Discord(format!("Failed to clear activity: {e}"))
        })?;

        info!("Discord activity cleared");
        Ok(())
    }

    #[instrument(skip(self, activity_fields), fields(
            state = activity_fields.state.as_deref().unwrap_or("None"),
            details = activity_fields.details.as_deref().unwrap_or("None")
    ))]
    pub async fn change_activity(
        &mut self,
        activity_fields: ActivityFields,
        git_remote_url: Option<String>,
    ) -> Result<()> {
        if self.is_shutting_down() {
            return Err(Self::shutdown_error("update Discord activity"));
        }

        let activity = (activity_fields, git_remote_url);
        let decision = lock_gate(&self.send_gate).decide(&activity, Instant::now());
        match decision {
            SendDecision::Send => {}
            SendDecision::Skip => {
                debug!("Activity unchanged and sent recently, skipping update");
                return Ok(());
            }
            SendDecision::Queued => {
                debug!("Activity queued behind an already scheduled send");
                return Ok(());
            }
            SendDecision::Schedule(flush_at) => {
                debug!("Activity deferred to respect the minimum send interval");
                self.schedule_flush(flush_at);
                return Ok(());
            }
        }

        // Only used to label the write in the debug log.
        let path = match lock_gate(&self.send_gate).last_activity.as_ref() {
            None => "first-on-connection",
            Some(last) if *last == activity => "re-assert",
            Some(_) => "immediate",
        };

        let mut client = self.get_client().await?;
        Self::send_activity(&mut client, &activity, self.start_timestamp, path).inspect_err(
            |_| {
                // Mark as disconnected so next attempt will reconnect
                self.connected.store(false, Ordering::SeqCst);
            },
        )?;
        drop(client);

        lock_gate(&self.send_gate).record_sent(activity, Instant::now());

        debug!("Discord activity updated successfully");
        Ok(())
    }

    /// Builds the activity payload and writes it to the Discord IPC socket.
    fn send_activity(
        client: &mut DiscordIpcClient,
        (activity_fields, git_remote_url): &ActivityKey,
        start_timestamp: Duration,
        path: &str,
    ) -> Result<()> {
        let timestamp: i64 = i64::try_from(start_timestamp.as_millis()).map_err(|e| {
            error!("Failed to convert timestamp: {}", e);
            crate::error::PresenceError::Discord(format!("Failed to convert timestamp: {e}"))
        })?;

        let activity = Activity::new()
            .timestamps(Timestamps::new().start(timestamp))
            .buttons(
                git_remote_url
                    .as_ref()
                    .map_or_else(Vec::new, |url| vec![Button::new("View Repository", url)]),
            );

        let activity =
            util::set_optional_field(activity, activity_fields.state.as_deref(), Activity::state);
        let activity = util::set_optional_field(
            activity,
            activity_fields.details.as_deref(),
            Activity::details,
        );

        let assets = Assets::new();
        let assets = util::set_optional_field(
            assets,
            activity_fields.large_image.as_deref(),
            Assets::large_image,
        );
        let assets = util::set_optional_field(
            assets,
            activity_fields.large_text.as_deref(),
            Assets::large_text,
        );
        let assets = util::set_optional_field(
            assets,
            activity_fields.small_image.as_deref(),
            Assets::small_image,
        );
        let assets = util::set_optional_field(
            assets,
            activity_fields.small_text.as_deref(),
            Assets::small_text,
        );

        let activity = activity.assets(assets);

        client.set_activity(activity).map_err(|e| {
            error!("Failed to set activity: {}", e);
            crate::error::PresenceError::Discord(format!("Failed to set activity: {e}"))
        })?;

        debug!("Activity sent: path={} start_ms={}", path, timestamp);
        Ok(())
    }

    /// Spawns the task that sends the pending activity once the minimum send
    /// interval has passed, repeating while new activities keep being queued.
    fn schedule_flush(&self, flush_at: Instant) {
        let gate = Arc::clone(&self.send_gate);
        let client = self.client.clone();
        let connected = Arc::clone(&self.connected);
        let shutting_down = Arc::clone(&self.shutting_down);
        let start_timestamp = self.start_timestamp;

        let task = tokio::spawn(async move {
            let mut flush_at = flush_at;
            loop {
                tokio::time::sleep_until(flush_at).await;

                if shutting_down.load(Ordering::SeqCst) {
                    lock_gate(&gate).end_flush();
                    return;
                }

                let Some(activity) = lock_gate(&gate).take_pending(Instant::now()) else {
                    return;
                };
                let Some(client) = client.as_ref() else {
                    lock_gate(&gate).end_flush();
                    return;
                };

                let mut client = client.lock().await;
                if let Err(e) =
                    Self::send_activity(&mut client, &activity, start_timestamp, "deferred")
                {
                    warn!(
                        "Deferred activity update failed, marking as disconnected: {}",
                        e
                    );
                    connected.store(false, Ordering::SeqCst);
                    let mut state = lock_gate(&gate);
                    state.forget_last_activity();
                    state.end_flush();
                    return;
                }
                drop(client);
                debug!("Deferred Discord activity sent");

                let next = {
                    let mut state = lock_gate(&gate);
                    state.record_sent(activity, Instant::now());
                    state.next_flush()
                };
                match next {
                    Some(at) => flush_at = at,
                    None => return,
                }
            }
        });

        lock_gate(&self.send_gate).flush_task = Some(task);
    }

    /// Changes activity with automatic reconnection on failure.
    /// If not connected, attempts to reconnect first.
    /// If activity update fails, marks connection as disconnected for future reconnection.
    pub async fn change_activity_with_reconnect(
        &mut self,
        activity_fields: ActivityFields,
        git_remote_url: Option<String>,
    ) -> Result<()> {
        if self.is_shutting_down() {
            lock_gate(&self.send_gate).forget_last_activity();
            return Err(Self::shutdown_error("update Discord activity"));
        }

        // If not connected, try to reconnect first
        if !self.is_connected() {
            warn!("Discord not connected, attempting reconnection...");
            lock_gate(&self.send_gate).forget_last_activity();
            if let Err(e) = self.reconnect().await {
                debug!("Reconnection failed: {}", e);
                return Err(e);
            }
            debug!("Reconnected; the next write is the first on this connection");
        }

        // Try to update activity
        match self.change_activity(activity_fields, git_remote_url).await {
            Ok(()) => Ok(()),
            Err(e) => {
                // Connection may have dropped, mark as disconnected
                warn!("Activity update failed, marking as disconnected: {}", e);
                self.connected.store(false, Ordering::SeqCst);
                lock_gate(&self.send_gate).forget_last_activity();
                Err(e)
            }
        }
    }

    /// Sets the connection flag without a real Discord client.
    #[cfg(test)]
    pub(crate) fn set_connected_for_test(&self, connected: bool) {
        self.connected.store(connected, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shutdown_signal(value: bool) -> Arc<AtomicBool> {
        Arc::new(AtomicBool::new(value))
    }

    fn sample_fields() -> ActivityFields {
        ActivityFields {
            state: Some("state".to_string()),
            details: Some("details".to_string()),
            large_image: None,
            large_text: None,
            small_image: None,
            small_text: None,
        }
    }

    #[test]
    fn test_discord_new_defaults() {
        let discord = Discord::new(shutdown_signal(false));
        assert!(!discord.is_connected());
        assert!(discord.client.is_none());
    }

    #[test]
    fn test_is_connected_default_false() {
        let discord = Discord::new(shutdown_signal(false));
        assert!(!discord.is_connected());
    }

    #[test]
    fn test_connected_state_can_be_changed() {
        let discord = Discord::new(shutdown_signal(false));
        assert!(!discord.is_connected());

        // Manually set connected to true
        discord.connected.store(true, Ordering::SeqCst);
        assert!(discord.is_connected());

        // Set back to false
        discord.connected.store(false, Ordering::SeqCst);
        assert!(!discord.is_connected());
    }

    #[test]
    fn test_retry_constants() {
        // Verify retry constants are reasonable
        assert_eq!(MAX_RETRIES, 5);
        assert_eq!(INITIAL_DELAY_MS, 500);
        assert_eq!(MAX_DELAY_MS, 10_000);

        // Verify exponential backoff calculation
        let mut delay = Duration::from_millis(INITIAL_DELAY_MS);
        let delays: Vec<u64> = (0..5)
            .map(|_| {
                let current = delay.as_millis() as u64;
                delay = (delay * 2).min(Duration::from_millis(MAX_DELAY_MS));
                current
            })
            .collect();

        assert_eq!(delays, vec![500, 1000, 2000, 4000, 8000]);
    }

    #[test]
    fn test_start_timestamp_is_set() {
        let discord = Discord::new(shutdown_signal(false));
        // Timestamp should be non-zero (set to current time)
        assert!(discord.start_timestamp.as_millis() > 0);
    }

    #[tokio::test]
    async fn test_connect_with_retry_fails_fast_during_shutdown() {
        let discord = Discord::new(shutdown_signal(true));

        let error = discord.connect_with_retry().await.unwrap_err();

        assert!(error.to_string().contains("shutdown is in progress"));
    }

    #[tokio::test]
    async fn test_clear_activity_without_client_is_noop() {
        let mut discord = Discord::new(shutdown_signal(false));

        assert!(discord.clear_activity().await.is_ok());
    }

    #[tokio::test]
    async fn test_change_activity_with_reconnect_fails_fast_during_shutdown() {
        let mut discord = Discord::new(shutdown_signal(true));

        let error = discord
            .change_activity_with_reconnect(sample_fields(), None)
            .await
            .unwrap_err();

        assert!(error.to_string().contains("shutdown is in progress"));
    }

    fn activity_with_state(state: &str) -> ActivityKey {
        let mut fields = sample_fields();
        fields.state = Some(state.to_string());
        (fields, None)
    }

    #[tokio::test(start_paused = true)]
    async fn test_identical_activity_is_skipped_before_reassert_interval() {
        let mut gate = SendGate::default();
        let coding = activity_with_state("coding");

        assert_eq!(gate.decide(&coding, Instant::now()), SendDecision::Send);
        gate.record_sent(coding.clone(), Instant::now());

        tokio::time::advance(REASSERT_INTERVAL.saturating_sub(Duration::from_secs(1))).await;

        assert_eq!(gate.decide(&coding, Instant::now()), SendDecision::Skip);
    }

    #[tokio::test(start_paused = true)]
    async fn test_identical_activity_is_resent_after_reassert_interval() {
        let mut gate = SendGate::default();
        let coding = activity_with_state("coding");

        gate.record_sent(coding.clone(), Instant::now());

        tokio::time::advance(REASSERT_INTERVAL + Duration::from_secs(1)).await;

        assert_eq!(gate.decide(&coding, Instant::now()), SendDecision::Send);
    }

    #[tokio::test(start_paused = true)]
    async fn test_activities_within_min_interval_are_not_sent_back_to_back() {
        let mut gate = SendGate::default();
        let first = activity_with_state("first");
        let second = activity_with_state("second");
        let third = activity_with_state("third");
        let start = Instant::now();

        assert_eq!(gate.decide(&first, start), SendDecision::Send);
        gate.record_sent(first, start);

        tokio::time::advance(Duration::from_secs(1)).await;

        assert_eq!(
            gate.decide(&second, Instant::now()),
            SendDecision::Schedule(start + MIN_SEND_INTERVAL)
        );
        assert_eq!(gate.decide(&third, Instant::now()), SendDecision::Queued);

        // When the flush runs, only the latest queued activity goes out.
        tokio::time::advance(MIN_SEND_INTERVAL.saturating_sub(Duration::from_secs(1))).await;

        assert_eq!(gate.take_pending(Instant::now()), Some(third));
    }

    #[tokio::test(start_paused = true)]
    async fn test_change_activity_defers_send_and_flush_drains_pending() {
        let mut discord = Discord::new(shutdown_signal(false));
        lock_gate(&discord.send_gate).record_sent(activity_with_state("first"), Instant::now());

        // There is no IPC client, so an immediate send would return an error:
        // `Ok` here means the activity was deferred instead of sent.
        let (fields, git_remote_url) = activity_with_state("second");
        assert!(discord
            .change_activity(fields, git_remote_url)
            .await
            .is_ok());
        {
            let gate = lock_gate(&discord.send_gate);
            assert_eq!(gate.pending, Some(activity_with_state("second")));
            assert!(gate.flush_at.is_some());
        }

        tokio::time::advance(MIN_SEND_INTERVAL).await;
        tokio::time::sleep(Duration::from_millis(1)).await;

        let gate = lock_gate(&discord.send_gate);
        assert!(
            gate.pending.is_none(),
            "The flush task should take the pending activity"
        );
        assert!(
            gate.flush_at.is_none(),
            "The flush should end after draining"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn test_clear_activity_cancels_pending_send() {
        let mut discord = Discord::new(shutdown_signal(false));
        lock_gate(&discord.send_gate).record_sent(activity_with_state("first"), Instant::now());

        let (fields, git_remote_url) = activity_with_state("second");
        assert!(discord
            .change_activity(fields, git_remote_url)
            .await
            .is_ok());
        assert!(discord.clear_activity().await.is_ok());

        let gate = lock_gate(&discord.send_gate);
        assert!(gate.pending.is_none());
        assert!(gate.flush_at.is_none());
        assert!(gate.last_activity.is_none());
    }

    #[tokio::test(start_paused = true)]
    async fn test_connect_times_out_instead_of_waiting_forever() {
        let mut discord = Discord::new(shutdown_signal(false));
        discord.create_client("0").unwrap();

        // Holding the client lock simulates a handshake that never returns.
        let stuck_handshake = discord.get_client().await.unwrap();

        let error = discord.connect().await.unwrap_err();

        assert!(error.to_string().contains("Timed out"));
        assert!(!discord.is_connected());
        drop(stuck_handshake);
    }
}
