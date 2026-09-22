//! Own the native LevelDB LOCK file for the full database handle lifetime.
use std::{
    collections::HashSet,
    fs::{File, OpenOptions},
    io,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

fn held_paths() -> &'static Mutex<HashSet<PathBuf>> {
    static HELD: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
    HELD.get_or_init(|| Mutex::new(HashSet::new()))
}

pub struct DatabaseLock {
    file: Option<File>,
    path: PathBuf,
}

impl DatabaseLock {
    pub fn acquire(directory: &Path) -> io::Result<Self> {
        let path = directory.canonicalize()?.join("LOCK");
        let mut held = held_paths()
            .lock()
            .map_err(|_| io::Error::other("Database lock registry poisoned"))?;
        if held.contains(&path) {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "World is already open",
            ));
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            // Deny sharing with both VoxelPopuli and the native Bedrock client.
            options.share_mode(0);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW);
        }
        let file = options.open(&path)?;
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            // LevelDB uses POSIX record locks, not flock(). Zero length locks to EOF.
            // SAFETY: zero is valid for flock's integer fields; fcntl borrows a
            // live descriptor and a correctly sized, initialized structure.
            let mut lock: libc::flock = unsafe { std::mem::zeroed() };
            lock.l_type = libc::F_WRLCK as _;
            lock.l_whence = libc::SEEK_SET as _;
            if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETLK, &lock) } == -1 {
                return Err(io::Error::last_os_error());
            }
        }
        #[cfg(not(any(windows, unix)))]
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Native world locking is unavailable on this platform",
        ));
        held.insert(path.clone());
        Ok(Self {
            file: Some(file),
            path,
        })
    }
}

impl Drop for DatabaseLock {
    fn drop(&mut self) {
        let mut held = held_paths()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        // POSIX record locks release when the descriptor closes. Keep the registry
        // entry until then so another thread cannot slip between the two operations.
        drop(self.file.take());
        held.remove(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_lock_excludes_other_handles_and_releases_on_drop() {
        let root = std::env::temp_dir().join(format!(
            "voxel-db-lock-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        std::fs::create_dir(&root).unwrap();
        let first = DatabaseLock::acquire(&root).unwrap();
        assert!(DatabaseLock::acquire(&root).is_err());
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "bedrock::lock::tests::database_lock_child",
                "--ignored",
            ])
            .env("VOXELPOPULI_LOCK_TEST", &root)
            .status()
            .unwrap();
        assert!(status.success());
        drop(first);
        drop(DatabaseLock::acquire(&root).unwrap());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "launched by the database lock parent test"]
    fn database_lock_child() {
        let Some(root) = std::env::var_os("VOXELPOPULI_LOCK_TEST") else {
            return;
        };
        assert!(DatabaseLock::acquire(Path::new(&root)).is_err());
    }
}
