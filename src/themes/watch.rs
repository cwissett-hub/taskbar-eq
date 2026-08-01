use notify::{RecursiveMode, Watcher as _};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct Watcher {
    dirty: Arc<AtomicBool>,
    _inner: Option<notify::RecommendedWatcher>,
}

impl Watcher {
    /// Watches the themes directory. Creates it if absent so the user has an
    /// obvious place to drop files, and so the watch has something to attach to.
    pub fn new() -> Self {
        Self::for_dir(crate::config::Config::dir().join("themes"))
    }

    /// Watches an explicit directory.
    ///
    /// Split out from `new` purely so the failure path is testable. The original test
    /// claimed to cover "constructing must never panic even if the watch cannot be
    /// established" but called `new()` against the real, watchable AppData path - so it
    /// could not have failed even if the fallback were broken. A test that cannot fail
    /// is worse than no test, because it reads as coverage.
    pub fn for_dir(dir: std::path::PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&dir);

        let dirty = Arc::new(AtomicBool::new(false));
        let flag = dirty.clone();

        let inner = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(ev) = res {
                if ev.paths.iter().any(|p| p.extension().map(|e| e == "toml").unwrap_or(false)) {
                    flag.store(true, Ordering::Relaxed);
                }
            }
        })
        .and_then(|mut w| {
            w.watch(&dir, RecursiveMode::NonRecursive)?;
            Ok(w)
        })
        .ok();

        if inner.is_none() {
            eprintln!("themes: hot reload unavailable; edits need a restart");
        }
        Watcher { dirty, _inner: inner }
    }

    /// True once per batch of changes, then resets.
    pub fn changed(&self) -> bool {
        self.dirty.swap(false, Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_clean() {
        let w = Watcher::new();
        assert!(!w.changed(), "a fresh watcher has no pending changes");
    }

    #[test]
    fn changed_resets_after_reading() {
        let w = Watcher::new();
        w.dirty.store(true, Ordering::Relaxed);
        assert!(w.changed(), "first read sees the change");
        assert!(!w.changed(), "second read must be clean - one reload per batch");
    }

    #[test]
    fn survives_an_unwatchable_directory() {
        // Genuinely exercises the fallback: a path on a drive letter that does not
        // exist cannot be created and cannot be watched, so `recommended_watcher(..)
        // .and_then(watch).ok()` must yield None. Construction must still succeed, and
        // `changed()` must simply never fire, so the app runs with hot reload disabled
        // rather than dying.
        let bogus = std::path::PathBuf::from(r"Q:\definitely-no-such-drive\themes");
        let w = Watcher::for_dir(bogus);
        assert!(
            w._inner.is_none(),
            "an unwatchable path must leave the watcher inert, not half-initialised"
        );
        assert!(!w.changed(), "an inert watcher must never report changes");
        assert!(!w.changed(), "and must stay quiet on repeat calls");
    }
}
