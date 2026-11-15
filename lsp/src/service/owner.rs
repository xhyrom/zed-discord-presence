//! Decides which language server instance owns the Discord presence.
//!
//! Zed starts one server per worktree, and each one has its own Discord IPC
//! connection. The instance that saw the most recent user event records
//! itself in a small file under the OS temp directory; the others stay quiet.
//! The record also says when the owner first sent after claiming, so the
//! previous owner can wait for it before clearing, and holds a session start
//! shared by all instances, so the elapsed timer survives handovers.
//! Every failure fails open: an IO error never blocks the presence.

use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::{Mutex as StdMutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::debug;

/// Contents of the ownership record.
///
/// Format: `<pid> <claim_ms> <sent_ms> <session_ms>`, in unix milliseconds;
/// `sent_ms` is 0 until the owner has sent and `session_ms` 0 if unknown.
/// Older versions wrote only `<pid> <claim_ms>`; missing fields read as `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OwnerRecord {
    pid: u32,
    claim_ms: u64,
    sent_ms: Option<u64>,
    session_ms: Option<u64>,
}

impl OwnerRecord {
    fn parse(text: &str) -> Option<Self> {
        let mut fields = text.split_whitespace();
        let pid = fields.next()?.parse().ok()?;
        let mut next_ms = || fields.next().and_then(|field| field.parse::<u64>().ok());

        let claim_ms = next_ms().unwrap_or(0);
        let sent_ms = next_ms();
        let session_ms = next_ms().filter(|&ms| ms > 0);

        Some(Self {
            pid,
            claim_ms,
            sent_ms,
            session_ms,
        })
    }

    fn format(&self) -> String {
        format!(
            "{} {} {} {}\n",
            self.pid,
            self.claim_ms,
            self.sent_ms.unwrap_or(0),
            self.session_ms.unwrap_or(0)
        )
    }

    /// Whether the owner has sent since it claimed. Records from older
    /// versions cannot tell, so they count as sent (fail open).
    fn owner_has_sent(&self) -> bool {
        self.sent_ms
            .is_none_or(|sent_ms| sent_ms > 0 && sent_ms >= self.claim_ms)
    }
}

/// Result of one ownership check, taken from a single read of the record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnershipCheck {
    pub is_owner: bool,
    /// Whether the current owner has already sent its presence. Always true
    /// when this process is the owner or the record cannot tell.
    pub owner_has_sent: bool,
}

#[derive(Debug)]
pub struct PresenceOwner {
    path: PathBuf,
    pid: u32,
    /// Session start last seen in a live record, or chosen by this process.
    session_ms: StdMutex<Option<u64>>,
}

impl PresenceOwner {
    #[must_use]
    pub fn new() -> Self {
        Self::with_path(default_path(), std::process::id())
    }

    fn with_path(path: PathBuf, pid: u32) -> Self {
        Self {
            path,
            pid,
            session_ms: StdMutex::new(None),
        }
    }

    /// Records this process as the owner unless the record already names it.
    /// Returns true when ownership was just acquired. Nothing is written while
    /// this process already owns the record, so `claim_ms` keeps the moment
    /// ownership was acquired.
    pub fn claim(&self) -> bool {
        let live = self.read_live_record();
        if live.is_some_and(|(record, _)| record.pid == self.pid) {
            return false;
        }

        // Reading a live record above already cached its session start.
        let now = unix_ms();
        let cached = self.cached_session();
        let session_ms = cached.unwrap_or(now);
        // Only used to log where the session start came from.
        let inherited = live.is_some_and(|(record, alive)| alive && record.session_ms.is_some());
        let record = OwnerRecord {
            pid: self.pid,
            claim_ms: now,
            sent_ms: Some(0),
            session_ms: Some(session_ms),
        };

        match self.write_record(&record) {
            Ok(()) => {
                self.cache_session(session_ms);
                debug!(
                    "Claimed presence ownership: pid={} claim_ms={} session_ms={} (source: {})",
                    self.pid,
                    now,
                    session_ms,
                    if inherited {
                        "live owner record"
                    } else if cached.is_some() {
                        "cached by this process"
                    } else {
                        "new session"
                    }
                );
                true
            }
            Err(e) => {
                debug!("Failed to claim presence ownership: {}", e);
                false
            }
        }
    }

    /// Returns whether this process may send presence updates: true if the
    /// record names this process, names a process that is no longer alive, or
    /// is missing or unreadable.
    #[must_use]
    pub fn is_owner(&self) -> bool {
        self.check().is_owner
    }

    #[must_use]
    pub fn check(&self) -> OwnershipCheck {
        match self.read_live_record() {
            None => OwnershipCheck {
                is_owner: true,
                owner_has_sent: true,
            },
            Some((record, alive)) => {
                let is_owner = record.pid == self.pid || !alive;
                OwnershipCheck {
                    is_owner,
                    owner_has_sent: is_owner || record.owner_has_sent(),
                }
            }
        }
    }

    /// Notes that this process, as the owner, has sent its presence. Only the
    /// first send after acquiring ownership is written: that is all a previous
    /// owner waits for before clearing its own activity.
    pub fn mark_sent(&self) {
        let Some((record, _)) = self.read_live_record() else {
            return;
        };
        if record.pid != self.pid || record.owner_has_sent() {
            return;
        }

        // Never earlier than the claim, even if the clock went backwards.
        let sent = OwnerRecord {
            sent_ms: Some(unix_ms().max(record.claim_ms)),
            ..record
        };
        if let Err(e) = self.write_record(&sent) {
            debug!("Failed to record the presence send: {}", e);
        }
    }

    /// Start of the elapsed timer (unix ms), shared by all instances: the
    /// session of a live record, else the last one this process saw, else now.
    pub fn session_start_ms(&self) -> u64 {
        // Refreshes the cached session from a live record, if any.
        let _ = self.read_live_record();
        *self
            .session_ms
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get_or_insert_with(unix_ms)
    }

    /// Reads the record and whether its owner is this process or still alive.
    /// The session of a live record is cached, so it outlives its owner.
    fn read_live_record(&self) -> Option<(OwnerRecord, bool)> {
        let record = OwnerRecord::parse(&fs::read_to_string(&self.path).ok()?)?;
        let alive = record.pid == self.pid || process_alive(record.pid);

        if let Some(session_ms) = record.session_ms.filter(|_| alive) {
            self.cache_session(session_ms);
        }
        Some((record, alive))
    }

    fn cached_session(&self) -> Option<u64> {
        *self
            .session_ms
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn cache_session(&self, session_ms: u64) {
        *self
            .session_ms
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(session_ms);
    }

    /// Writes the record atomically (temp file + rename), so readers never
    /// observe a partial record.
    fn write_record(&self, record: &OwnerRecord) -> io::Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }

        let tmp_path = self.path.with_extension(format!("{}.tmp", self.pid));
        fs::write(&tmp_path, record.format())?;
        fs::rename(&tmp_path, &self.path)
    }
}

impl Default for PresenceOwner {
    fn default() -> Self {
        Self::new()
    }
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

#[cfg(not(test))]
fn default_path() -> PathBuf {
    std::env::temp_dir()
        .join("discord-presence-lsp")
        .join("owner")
}

/// Tests get a fresh, never-written path, so they neither read nor change the
/// record of a real Zed session running on the same machine.
#[cfg(test)]
fn default_path() -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let id = NEXT.fetch_add(1, Ordering::SeqCst);

    std::env::temp_dir()
        .join("discord-presence-lsp-tests")
        .join(format!("{}-{id}", std::process::id()))
        .join("owner")
}

#[cfg(windows)]
fn process_alive(pid: u32) -> bool {
    use std::ffi::c_void;

    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const STILL_ACTIVE: u32 = 259;
    const ERROR_ACCESS_DENIED: i32 = 5;

    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(desired_access: u32, inherit_handle: i32, process_id: u32) -> *mut c_void;
        fn GetExitCodeProcess(process: *mut c_void, exit_code: *mut u32) -> i32;
        fn CloseHandle(handle: *mut c_void) -> i32;
    }

    // SAFETY: plain Win32 calls; the handle is checked for null, only used
    // while open, and closed exactly once.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            // Access denied means the process exists but is protected.
            return io::Error::last_os_error().raw_os_error() == Some(ERROR_ACCESS_DENIED);
        }

        let mut exit_code = 0u32;
        let queried = GetExitCodeProcess(handle, &raw mut exit_code);
        CloseHandle(handle);

        // If the query fails, report "not alive" so ownership fails open.
        queried != 0 && exit_code == STILL_ACTIVE
    }
}

#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    const EPERM: i32 = 1;

    extern "C" {
        fn kill(pid: i32, signal: i32) -> i32;
    }

    let Ok(pid) = i32::try_from(pid) else {
        return false;
    };

    // SAFETY: signal 0 sends nothing; it only checks that the process exists.
    if unsafe { kill(pid, 0) } == 0 {
        return true;
    }
    // Permission denied means the process exists but belongs to someone else.
    io::Error::last_os_error().raw_os_error() == Some(EPERM)
}

#[cfg(not(any(windows, unix)))]
fn process_alive(_pid: u32) -> bool {
    false
}

/// Test-only hooks so other modules can simulate other instances.
#[cfg(test)]
impl PresenceOwner {
    /// An owner for `pid` with a fresh, never-written record path.
    pub(crate) fn with_pid(pid: u32) -> Self {
        Self::with_path(default_path(), pid)
    }

    /// Overwrites the record in the old `<pid> <claim_ms>` format, as if an
    /// older version with `pid` had claimed it.
    pub(crate) fn set_record_owner(&self, pid: u32) {
        fs::create_dir_all(self.path.parent().unwrap()).unwrap();
        fs::write(&self.path, format!("{pid} 0\n")).unwrap();
    }

    /// Overwrites the record in the current format (no session start).
    pub(crate) fn set_record_fields(&self, pid: u32, claim_ms: u64, sent_ms: u64) {
        let record = OwnerRecord {
            pid,
            claim_ms,
            sent_ms: Some(sent_ms),
            session_ms: None,
        };
        fs::create_dir_all(self.path.parent().unwrap()).unwrap();
        fs::write(&self.path, record.format()).unwrap();
    }

    /// Deletes the record and its test directory.
    pub(crate) fn remove_record(&self) {
        let _ = fs::remove_dir_all(self.path.parent().unwrap());
    }
}

/// PID of a process that has already exited.
#[cfg(test)]
pub(crate) fn dead_test_pid() -> u32 {
    let mut command = if cfg!(windows) {
        let mut command = std::process::Command::new("cmd");
        command.args(["/C", "exit 0"]);
        command
    } else {
        std::process::Command::new("true")
    };
    let mut child = command.spawn().unwrap();
    let pid = child.id();
    child.wait().unwrap();
    pid
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A PID that is assumed not to belong to any live process.
    const OTHER_PID: u32 = u32::MAX - 7;

    fn test_owner(pid: u32) -> PresenceOwner {
        PresenceOwner::with_path(default_path(), pid)
    }

    fn write_raw(owner: &PresenceOwner, contents: &str) {
        fs::create_dir_all(owner.path.parent().unwrap()).unwrap();
        fs::write(&owner.path, contents).unwrap();
    }

    fn read_raw(owner: &PresenceOwner) -> String {
        fs::read_to_string(&owner.path).unwrap()
    }

    fn cleanup(owner: &PresenceOwner) {
        let _ = fs::remove_dir_all(owner.path.parent().unwrap());
    }

    #[test]
    fn test_current_process_is_alive() {
        assert!(process_alive(std::process::id()));
    }

    #[test]
    fn test_is_owner_when_record_is_missing() {
        let owner = test_owner(OTHER_PID);

        assert!(owner.is_owner());
    }

    #[test]
    fn test_is_not_owner_when_another_live_process_owns_the_record() {
        let owner = test_owner(OTHER_PID);
        // The test process itself is the live "other" owner here.
        write_raw(&owner, &format!("{} 0\n", std::process::id()));

        assert!(!owner.is_owner());
        cleanup(&owner);
    }

    #[test]
    fn test_is_owner_when_recorded_process_is_dead() {
        let owner = test_owner(OTHER_PID);
        write_raw(&owner, &format!("{} 0\n", dead_test_pid()));

        assert!(owner.is_owner());
        cleanup(&owner);
    }

    #[test]
    fn test_is_owner_when_record_is_garbage() {
        let owner = test_owner(OTHER_PID);
        write_raw(&owner, "not a pid");

        assert!(owner.is_owner());
        cleanup(&owner);
    }

    #[test]
    fn test_claim_reports_acquisition_only_once() {
        let owner = test_owner(OTHER_PID);

        assert!(owner.claim(), "First claim acquires ownership");
        assert!(!owner.claim(), "Already the owner: nothing acquired");
        cleanup(&owner);
    }

    #[test]
    fn test_claim_does_not_rewrite_while_still_owner() {
        let owner = test_owner(OTHER_PID);
        assert!(owner.claim());

        // Same PID with marker values: a rewrite would replace them.
        let marker = format!("{OTHER_PID} 1 0 7\n");
        write_raw(&owner, &marker);
        owner.claim();

        assert_eq!(read_raw(&owner), marker);
        cleanup(&owner);
    }

    #[test]
    fn test_claim_retakes_ownership_immediately() {
        let owner = test_owner(OTHER_PID);
        owner.claim();

        // Another live instance takes over right after our claim.
        write_raw(&owner, &format!("{} 0\n", std::process::id()));
        assert!(!owner.is_owner());

        assert!(
            owner.claim(),
            "Returning to an instance must reclaim at once"
        );
        assert!(owner.is_owner());
        cleanup(&owner);
    }

    #[test]
    fn test_owner_has_sent_only_after_mark_sent() {
        let path = default_path();
        let new_owner = PresenceOwner::with_path(path.clone(), std::process::id());
        let old_owner = PresenceOwner::with_path(path, OTHER_PID);

        assert!(new_owner.claim());
        let before = old_owner.check();
        assert!(!before.is_owner);
        assert!(!before.owner_has_sent, "Claimed but not sent yet");

        new_owner.mark_sent();

        assert!(old_owner.check().owner_has_sent);
        cleanup(&new_owner);
    }

    #[test]
    fn test_old_record_format_counts_as_sent() {
        let owner = test_owner(OTHER_PID);
        write_raw(&owner, &format!("{} 123\n", std::process::id()));

        let check = owner.check();

        assert!(!check.is_owner);
        assert!(check.owner_has_sent, "Old records cannot tell: fail open");
        cleanup(&owner);
    }

    #[test]
    fn test_instances_share_the_session_start() {
        let path = default_path();
        let first = PresenceOwner::with_path(path.clone(), std::process::id());
        let second = PresenceOwner::with_path(path, OTHER_PID);

        assert!(first.claim());
        std::thread::sleep(std::time::Duration::from_millis(5));
        // The first owner is alive, so the second one keeps its session.
        assert!(second.claim());

        assert_eq!(first.session_start_ms(), second.session_start_ms());
        cleanup(&first);
    }

    #[test]
    fn test_new_session_when_unknown_owner_is_dead() {
        let owner = test_owner(OTHER_PID);
        write_raw(&owner, &format!("{} 1 1 42\n", dead_test_pid()));

        assert!(owner.claim());

        assert_ne!(owner.session_start_ms(), 42);
        cleanup(&owner);
    }

    #[test]
    fn test_session_survives_the_owner_when_seen_before() {
        let owner = test_owner(OTHER_PID);
        // A live owner's session is cached on read...
        write_raw(&owner, &format!("{} 1 1 42\n", std::process::id()));
        assert!(!owner.is_owner());

        // ...and kept after that owner is gone.
        write_raw(&owner, &format!("{} 1 1 42\n", dead_test_pid()));
        assert!(owner.claim());

        assert_eq!(owner.session_start_ms(), 42);
        cleanup(&owner);
    }
}
