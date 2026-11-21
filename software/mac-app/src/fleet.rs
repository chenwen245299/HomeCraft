//! One independent authenticated session per remembered display. Account queries stay in the hub.
use super::*;
use tokio::{sync::oneshot, task::JoinHandle};
#[derive(Clone, Default)]
pub struct Status {
    pub id: String,
    pub name: String,
    pub model: String,
    pub connected: bool,
    pub connecting: bool,
    pub paused: bool,
    pub synced: Option<i64>,
    pub notice: String,
    pub usage_notice: Option<String>,
    /// Latest reported temperature, humidity and sample time.
    pub climate: Option<(f32, f32, i64)>,
}
struct Actor {
    cancel: oneshot::Sender<()>,
    task: JoinHandle<()>,
}
pub(super) struct Manager {
    root: PathBuf,
    actors: HashMap<String, Actor>,
    paused: std::collections::HashSet<String>,
    enabled: bool,
    last: Instant,
    form: Option<String>,
    registry_error: bool,
}
impl Manager {
    pub(super) fn new(enabled: bool, root: PathBuf) -> Self {
        Self {
            root,
            actors: HashMap::new(),
            paused: Default::default(),
            enabled,
            last: Instant::now() - Duration::from_secs(10),
            form: None,
            registry_error: false,
        }
    }
    pub(super) async fn stop(&mut self, id: &str) {
        if let Some(actor) = self.actors.remove(id) {
            let _ = actor.cancel.send(());
            let mut task = actor.task;
            if tokio::time::timeout(Duration::from_secs(6), &mut task)
                .await
                .is_err()
            {
                task.abort();
                let _ = task.await;
            }
        }
    }
    pub(super) fn resume(&mut self, id: &str) {
        self.paused.remove(id);
    }
    pub(super) fn pause(&mut self, id: &str) {
        self.paused.insert(id.into());
    }
    pub(super) fn form_target(&mut self, target: Option<String>) {
        self.form = target;
    }
    /// A background session is owned per device: never for the controlled one, a paused one,
    /// one whose connection form is open, or one that already runs.
    fn should_spawn(&self, id: &str, primary: Option<&str>) -> bool {
        primary != Some(id)
            && self.form.as_deref() != Some(id)
            && !self.paused.contains(id)
            && !self.actors.contains_key(id)
    }
    pub(super) fn attach(&mut self, binding: crate::registry::Binding, link: Link, events: Events) {
        self.spawn(binding, Some(link), events);
    }
    fn spawn(&mut self, binding: crate::registry::Binding, link: Option<Link>, events: Events) {
        let id = binding.id.clone();
        let (cancel, rx) = oneshot::channel();
        let task = tokio::spawn(run(binding, link, events, rx, self.root.clone()));
        self.actors.insert(id, Actor { cancel, task });
    }
    pub(super) async fn reconcile(&mut self, root: &Path, primary: Option<&str>, events: &Events) {
        if !self.enabled || self.last.elapsed() < Duration::from_secs(5) {
            return;
        }
        self.last = Instant::now();
        let rows = match crate::registry::list(root) {
            Ok(r) => r,
            Err(_) => {
                if !self.registry_error {
                    events.feedback("设备目录暂不可用，现有连接保持运行", false, true);
                    self.registry_error = true;
                }
                return;
            }
        };
        self.registry_error = false;
        for row in rows {
            if self.should_spawn(&row.id, primary) {
                self.spawn(row, None, events.clone());
            }
        }
    }
    pub(super) async fn shutdown(&mut self) {
        let ids: Vec<_> = self.actors.keys().cloned().collect();
        for id in ids {
            self.stop(&id).await;
        }
    }
}
async fn run(
    binding: crate::registry::Binding,
    mut link: Option<Link>,
    events: Events,
    mut cancel: oneshot::Receiver<()>,
    root: PathBuf,
) {
    let id = binding.id.clone();
    let name = binding.name.clone();
    let mut peripheral = None;
    let mut backoff = 3;
    let mut retry = Instant::now();
    let mut last_sync = None;
    let mut last_ping = Instant::now();
    let mut last_climate: Option<Instant> = None;
    let status = Status {
        id: id.clone(),
        name: name.clone(),
        model: binding.model.clone(),
        connected: link.is_some(),
        notice: "等待连接".into(),
        ..Default::default()
    };
    events.update(|s| {
        s.fleet.insert(id.clone(), status);
    });
    loop {
        tokio::select! {biased;_=&mut cancel=>break,_=tokio::time::sleep(Duration::from_secs(1))=>{}}
        if link.is_none() && Instant::now() >= retry {
            events.update(|s| {
                if let Some(row) = s.fleet.get_mut(&id) {
                    row.connecting = true;
                    row.notice = "正在验证设备身份".into();
                }
            });
            let connect = async {
                let adapter = super::adapter().await?;
                let wanted = Uuid::parse_str(&id)?;
                let fresh = adapter
                    .retrieve_peripherals(RetrievePeripheralsOptions {
                        identifiers: Some(vec![wanted.into()]),
                        services: None,
                    })
                    .await?
                    .into_iter()
                    .find(|p| p.id().to_string() == id)
                    .ok_or_else(|| anyhow::anyhow!("设备暂不可用"))?;
                peripheral = Some(fresh.clone());
                Link::connect(fresh, &binding.directory, &name).await
            };
            let result = tokio::select! {biased;_=&mut cancel=>break,result=connect=>result};
            match result {
                Ok(l) => {
                    link = Some(l);
                    backoff = 3;
                    last_sync = None;
                    last_climate = None;
                    events.update(|s| {
                        if let Some(row) = s.fleet.get_mut(&id) {
                            row.connected = true;
                            row.connecting = false;
                            row.notice = "已安全连接".into();
                        }
                    });
                }
                Err(_) => {
                    retry = Instant::now() + Duration::from_secs(backoff);
                    backoff = (backoff * 2).min(60);
                    events.update(|s| {
                        if let Some(row) = s.fleet.get_mut(&id) {
                            row.connected = false;
                            row.connecting = false;
                            row.notice = "等待设备重连".into();
                        }
                    });
                }
            }
        }
        if let Some(l) = link.as_mut() {
            // Same view as the window: local readings, gaps filled from other Macs via iCloud.
            let (readings, harness, synced, weather, todos, poem, totals, places) =
                if let Ok(s) = events.state.lock() {
                    (
                        crate::hub::readings(&s),
                        s.harness_devices.clone(),
                        s.last_read,
                        s.weather.clone(),
                        s.todos.clone(),
                        s.poem.clone(),
                        crate::home::model::totals_wire(&s, crate::home::model::day()),
                        crate::home::model::places_wire(&s, &id),
                    )
                } else {
                    continue;
                };
            let work = async {
                // Versions include each provider timestamp, so a manual single-provider refresh also fans out.
                let revision =
                    serde_json::to_vec(&readings.iter().map(|r| &r.packet).collect::<Vec<_>>())?;
                if !readings.is_empty() && last_sync.as_ref() != Some(&revision) {
                    let usage_notice = l.usage_notice().map(str::to_owned);
                    for r in &readings {
                        let effective = crate::harness::effective(r, &harness);
                        l.send_reading(&effective).await?;
                    }
                    last_sync = Some(revision);
                    last_ping = Instant::now();
                    events.update(|s| {
                        if let Some(row) = s.fleet.get_mut(&id) {
                            row.synced = synced.or(Some(usage::now()));
                            row.usage_notice = usage_notice.clone();
                            row.notice =
                                usage_notice.clone().unwrap_or_else(|| "已安全连接".into());
                        }
                    });
                }
                l.sync_weather(&weather).await?;
                l.sync_extras(&todos, poem.as_ref()).await?;
                l.sync_totals(totals).await?;
                l.sync_places(places).await?;
                if l.device_supported
                    && last_climate.is_none_or(|t| {
                        t.elapsed() >= Duration::from_secs(crate::device::TELEMETRY_SECONDS)
                    })
                {
                    last_climate = Some(Instant::now());
                    let t = l.telemetry().await?;
                    if let (Some(temperature), Some(humidity)) = (t.temperature, t.humidity) {
                        events.update(|s| {
                            if let Some(row) = s.fleet.get_mut(&id) {
                                row.climate = Some((temperature, humidity, t.sampled_at));
                            }
                            if crate::envlog::record(
                                &mut s.env_log,
                                &id,
                                t.sampled_at,
                                temperature,
                                humidity,
                            ) {
                                crate::envlog::save(&root, &s.env_log);
                            }
                        });
                    }
                }
                if last_ping.elapsed() >= Duration::from_secs(30) {
                    l.send(b"?").await?;
                    last_ping = Instant::now();
                }
                Ok::<(), anyhow::Error>(())
            };
            let result = tokio::select! {biased;_=&mut cancel=>break,result=work=>result};
            if result.is_err() {
                if let Some(l) = link.take() {
                    l.disconnect().await;
                }
                retry = Instant::now() + Duration::from_secs(3);
                last_sync = None;
                events.update(|s| {
                    if let Some(row) = s.fleet.get_mut(&id) {
                        row.connected = false;
                        row.notice = "同步中断，等待重连".into();
                    }
                });
            }
        }
    }
    if let Some(l) = link.take() {
        l.disconnect().await;
    } else if let Some(p) = peripheral {
        let _ = p.disconnect().await;
    }
    events.update(|s| {
        if let Some(row) = s.fleet.get_mut(&id) {
            row.connected = false;
            row.connecting = false;
            row.notice = "已暂停".into();
            row.paused = true;
        }
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    fn idle(manager: &mut Manager, id: &str) {
        let (cancel, rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            let _ = rx.await;
        });
        manager.actors.insert(id.into(), Actor { cancel, task });
    }
    #[tokio::test]
    async fn pausing_one_display_leaves_the_other_session_running() {
        let mut m = Manager::new(true, std::env::temp_dir());
        idle(&mut m, "a");
        idle(&mut m, "b");
        m.pause("a");
        m.stop("a").await;
        tokio::task::yield_now().await;
        assert!(!m.actors.contains_key("a"));
        assert!(!m.actors["b"].task.is_finished());
        // A paused display stays stopped; a running one is never duplicated.
        assert!(!m.should_spawn("a", None));
        assert!(!m.should_spawn("b", None));
        m.resume("a");
        assert!(m.should_spawn("a", None));
        // The controlled display and an open connection form own their device exclusively.
        assert!(!m.should_spawn("a", Some("a")));
        m.form_target(Some("a".into()));
        assert!(!m.should_spawn("a", None));
        m.form_target(None);
        assert!(m.should_spawn("a", Some("b")));
        m.shutdown().await;
        assert!(m.actors.is_empty());
    }
}
