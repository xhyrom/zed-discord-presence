use std::{
    sync::{Mutex, MutexGuard},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use discord_rich_presence::{
    activity::{self, Assets, Timestamps},
    DiscordIpc, DiscordIpcClient,
};

#[derive(Debug)]
pub struct Discord {
    client: Mutex<DiscordIpcClient>,
    start_timestamp: Duration,
}

impl Discord {
    pub fn new() -> Self {
        let discord_client = DiscordIpcClient::new("1263505205522337886")
            .expect("Failed to initialize Discord Ipc Client");
        let start_timestamp = SystemTime::now();
        let since_epoch = start_timestamp
            .duration_since(UNIX_EPOCH)
            .expect("Failed to get duration since UNIX_EPOCH");

        Self {
            client: Mutex::new(discord_client),
            start_timestamp: since_epoch,
        }
    }

    pub fn connect(&self) {
        let mut client = self.get_client();
        let result = client.connect();
        result.unwrap();
    }

    pub fn change_file(&self, filename: &str, workspace: &str) {
        self.change_activity(
            format!("Working on {}", filename),
            format!("In {}", workspace),
        )
    }

    pub fn get_client(&self) -> MutexGuard<DiscordIpcClient> {
        return self.client.lock().expect("Failed to lock discord client");
    }

    fn change_activity(&self, state: String, details: String) {
        let mut client = self.get_client();
        let timestamp: i64 = self.start_timestamp.as_millis() as i64;

        client
            .set_activity(
                activity::Activity::new()
                    .assets(Assets::new().large_image("logo"))
                    .state(state.as_str())
                    .details(details.as_str())
                    .timestamps(Timestamps::new().start(timestamp)),
            )
            .expect(
                format!(
                    "Failed to set activity with state {} and details {}",
                    state, details
                )
                .as_str(),
            );
    }
}
