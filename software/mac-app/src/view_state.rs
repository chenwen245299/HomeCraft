//! Publish resolved, immutable window snapshots without doing aggregation on the UI thread.
use crate::backend::{Shared, State};
use eframe::egui;
use std::{
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

pub struct Cache {
    published: Arc<Mutex<Arc<State>>>,
    changed: mpsc::SyncSender<()>,
}
impl Cache {
    pub fn start(state: Shared, repaint: egui::Context) -> Self {
        Self::with_projector(state, repaint, crate::cloud::presentation)
    }
    fn with_projector(
        state: Shared,
        repaint: egui::Context,
        mut project: impl FnMut(State) -> State + Send + 'static,
    ) -> Self {
        let (changed, changes) = mpsc::sync_channel(1);
        let published = Arc::new(Mutex::new(Arc::new(State {
            usage_resolved: true,
            ..Default::default()
        })));
        let output = published.clone();
        std::thread::spawn(move || {
            let mut day = crate::home::model::day();
            loop {
                match changes.recv_timeout(Duration::from_secs(1)) {
                    Ok(()) => {}
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        let current = crate::home::model::day();
                        if current == day {
                            continue;
                        }
                        day = current;
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
                // Coalesce progress events, then release the backend lock before resolving.
                while changes.try_recv().is_ok() {}
                let Some(raw) = state.lock().ok().map(|s| s.clone()) else {
                    continue;
                };
                let next = Arc::new(project(raw));
                let previous = output
                    .lock()
                    .ok()
                    .map(|mut slot| std::mem::replace(&mut *slot, next));
                // Dropping an old catalog must not hold the window's snapshot lock either.
                drop(previous);
                repaint.request_repaint();
            }
        });
        let cache = Self { published, changed };
        cache.invalidate();
        cache
    }
    pub fn notifier(&self) -> mpsc::SyncSender<()> {
        self.changed.clone()
    }
    pub fn invalidate(&self) {
        let _ = self.changed.try_send(());
    }
    pub fn snapshot(&self) -> Arc<State> {
        self.published
            .lock()
            .map(|s| Arc::clone(&s))
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn wait(cache: &Cache, predicate: impl Fn(&State) -> bool) -> Arc<State> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let view = cache.snapshot();
            if predicate(&view) {
                return view;
            }
            assert!(Instant::now() < deadline, "snapshot did not update");
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    #[test]
    fn snapshots_reuse_resolved_values_without_waiting_for_the_backend_lock() {
        let raw = Arc::new(Mutex::new(State {
            readings: vec![crate::harness::test_reading("1111111111111111", 100)],
            ..Default::default()
        }));
        let cache = Cache::start(raw.clone(), egui::Context::default());
        let first = wait(&cache, |s| !s.readings.is_empty());
        assert!(first.usage_resolved);
        assert_eq!(first.readings[0].packet.d.unwrap()[1], 100);
        let backend = raw.lock().unwrap();
        assert!(!backend.usage_resolved);
        for _ in 0..100 {
            assert!(Arc::ptr_eq(&first, &cache.snapshot()));
        }
    }
    #[test]
    fn a_slow_projection_keeps_the_previous_view_and_coalesces_the_latest_update() {
        let raw = Arc::new(Mutex::new(State {
            status: "initial".into(),
            ..Default::default()
        }));
        let (started, ready) = mpsc::channel();
        let (release, blocked) = mpsc::channel();
        let mut pause = false;
        let cache = Cache::with_projector(raw.clone(), egui::Context::default(), move |s| {
            if pause {
                started.send(()).unwrap();
                blocked.recv().unwrap();
            }
            pause = true;
            crate::cloud::presentation(s)
        });
        wait(&cache, |s| s.status == "initial");
        raw.lock().unwrap().status = "first update".into();
        cache.invalidate();
        ready.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(cache.snapshot().status, "initial");
        // The projector has already released this lock while it is blocked.
        raw.try_lock().unwrap().status = "latest update".into();
        for _ in 0..100 {
            cache.invalidate();
        }
        release.send(()).unwrap();
        ready.recv_timeout(Duration::from_secs(5)).unwrap();
        release.send(()).unwrap();
        wait(&cache, |s| s.status == "latest update");
    }
    #[test]
    fn backend_events_publish_new_values_without_echoing_the_resolved_usage() {
        let raw = Arc::new(Mutex::new(State::default()));
        let context = egui::Context::default();
        let cache = Cache::start(raw.clone(), context.clone());
        let events = crate::backend::Events {
            state: raw.clone(),
            view_changed: cache.notifier(),
        };
        events.update(|s| {
            s.readings = vec![crate::harness::test_reading("1111111111111111", 100)];
        });
        let first = wait(&cache, |s| !s.readings.is_empty());
        events.update(|s| {
            s.readings = vec![crate::harness::test_reading("1111111111111111", 200)];
        });
        let next = wait(&cache, |s| {
            s.readings
                .first()
                .and_then(|r| r.packet.d)
                .is_some_and(|d| d[1] == 200)
        });
        assert_eq!(first.readings[0].packet.d.unwrap()[1], 100);
        assert_eq!(next.readings[0].packet.d.unwrap()[1], 200);
        let original = raw.lock().unwrap();
        assert!(!original.usage_resolved);
        assert!(original.readings[0].tokens.devices.is_empty());
    }
}
