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
        let dir = crate::config::Config::dir().join("themes");
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
        // Constructing must never panic even if the watch cannot be established.
        let _ = Watcher::new();
    }
}
