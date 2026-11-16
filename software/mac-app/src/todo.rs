//! HomeCraft todos. The hub owns the list; displays receive open items per Eisenhower quadrant.
//! iCloud Drive does the syncing: each Mac writes only its own partition file in the chosen
//! folder and reads the others from disk. Edits merge by id with the newest change winning
//! (ties broken by origin), and deletions stay as tombstones for 30 days so they propagate.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub const QUADRANTS: [&str; 4] = ["重要且紧急", "重要不紧急", "紧急不重要", "不重要不紧急"];
pub const HINTS: [&str; 4] = ["马上就做", "安排时间做", "尽快处理或交给别人", "有空再说"];
pub const DISPLAY_ITEMS: usize = 5;
pub const NOTES_CHARS: usize = 500;
const MAX_ITEMS: usize = 2000;
/// Room for every item carrying full details (2000 × ~2.4 KB worst case).
const MAX_FILE: u64 = 8 * 1024 * 1024;
const MAX_PARTITIONS: usize = 16;
const TOMBSTONE_MS: i64 = 30 * 86_400_000;
/// Shared folder for todos, beside AI用量 and 温湿度. Earlier builds used HomeCraft-Todos-v1,
/// which is still read (a Mac that has not updated keeps syncing) but never written.
pub const DIR: &str = "待办";
const LEGACY_DIR: &str = "HomeCraft-Todos-v1";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Todo {
    pub id: String,
    pub title: String,
    /// Free-form details shown only on the Mac. Left out when empty, so items without
    /// details serialize exactly as before this field existed.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub notes: String,
    pub quadrant: u8,
    /// UTC+8 calendar day as YYYYMMDD.
    #[serde(default)]
    pub date: Option<u32>,
    /// Minutes after midnight, for the day timeline.
    #[serde(default)]
    pub time: Option<u16>,
    #[serde(default)]
    pub minutes: Option<u16>,
    #[serde(default)]
    pub done: bool,
    #[serde(default)]
    pub deleted: bool,
    /// Last change, milliseconds since the Unix epoch.
    pub updated: i64,
    pub origin: String,
}
impl Todo {
    pub fn new(title: &str, quadrant: u8) -> Self {
        Self {
            id: format!("{:016x}", rand::random::<u64>()),
            title: title.trim().into(),
            notes: String::new(),
            quadrant,
            date: None,
            time: None,
            minutes: None,
            done: false,
            deleted: false,
            updated: 0,
            origin: String::new(),
        }
    }
    pub fn valid(&self) -> bool {
        self.id.len() == 16
            && self.id.bytes().all(|c| c.is_ascii_hexdigit())
            && valid_title(&self.title)
            && valid_notes(&self.notes)
            && self.quadrant < 4
            && self.date.is_none_or(valid_day)
            && self.time.is_none_or(|t| t < 1440)
            && self.minutes.is_none_or(|m| (5..=720).contains(&m))
            && self.updated >= 0
            && self.origin.len() <= 64
            && self.origin.bytes().all(|c| c.is_ascii_alphanumeric())
    }
    fn newer_than(&self, other: &Self) -> bool {
        (self.updated, &self.origin) > (other.updated, &other.origin)
    }
    pub fn open(&self) -> bool {
        !self.done && !self.deleted
    }
}
pub fn valid_title(s: &str) -> bool {
    let n = s.trim().chars().count();
    (1..=60).contains(&n) && !s.chars().any(char::is_control)
}
pub fn valid_notes(s: &str) -> bool {
    s.chars().count() <= NOTES_CHARS && !s.chars().any(|c| c.is_control() && c != '\n' && c != '\t')
}
/// Normalizes pasted details: Unix line breaks, no other control characters, trimmed and capped.
pub fn tidy_notes(s: &str) -> String {
    let text: String = s
        .replace("\r\n", "\n")
        .chars()
        .map(|c| if c == '\r' { '\n' } else { c })
        .filter(|&c| !c.is_control() || c == '\n' || c == '\t')
        .take(NOTES_CHARS)
        .collect();
    text.trim().to_owned()
}
pub fn valid_day(d: u32) -> bool {
    (20200101..=21001231).contains(&d)
        && chrono::NaiveDate::parse_from_str(&d.to_string(), "%Y%m%d").is_ok()
}
pub fn today() -> u32 {
    crate::home::model::day() as u32
}
fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}
/// Union by id; the newer edit wins. Returns true when `into` changed.
pub fn merge(into: &mut Vec<Todo>, other: impl IntoIterator<Item = Todo>) -> bool {
    let mut changed = false;
    for item in other {
        if !item.valid() {
            continue;
        }
        match into.iter_mut().find(|t| t.id == item.id) {
            Some(old) if item.newer_than(old) => {
                *old = item;
                changed = true;
            }
            Some(_) => {}
            None => {
                into.push(item);
                changed = true;
            }
        }
    }
    changed
}
/// Drops old tombstones and caps the list; returns true when something was removed.
pub fn prune(items: &mut Vec<Todo>, now: i64) -> bool {
    let before = items.len();
    items.retain(|t| !t.deleted || now - t.updated < TOMBSTONE_MS);
    if items.len() > MAX_ITEMS {
        items.sort_by_key(|t| std::cmp::Reverse(t.updated));
        items.truncate(MAX_ITEMS);
    }
    items.len() != before
}
/// Open items in display order: overdue and today first, then undated, then upcoming.
pub fn display_order(items: &[Todo], today: u32) -> Vec<&Todo> {
    let mut open: Vec<&Todo> = items.iter().filter(|t| t.open()).collect();
    open.sort_by_key(|t| {
        let bucket = match t.date {
            Some(d) if d <= today => 0,
            None => 1,
            Some(_) => 2,
        };
        (
            bucket,
            t.date.unwrap_or(0),
            t.time.unwrap_or(u16::MAX),
            std::cmp::Reverse(t.updated),
        )
    });
    open
}
/// Display text: today's timed items lead with their time; at most 48 UTF-8 bytes.
pub fn label(t: &Todo, today: u32) -> String {
    let text = match (t.date, t.time) {
        (Some(d), Some(m)) if d == today => {
            format!("{:02}:{:02} {}", m / 60, m % 60, t.title.trim())
        }
        _ => t.title.trim().to_owned(),
    };
    let mut out = String::new();
    for ch in text.chars() {
        if out.len() + ch.len_utf8() > 48 {
            break;
        }
        out.push(ch);
    }
    out
}
/// One message per quadrant (todo:1): up to five open items plus the quadrant's open count.
pub fn wire(items: &[Todo], today: u32) -> Result<Vec<Vec<u8>>> {
    let order = display_order(items, today);
    (0..4u8)
        .map(|q| {
            let open: Vec<&&Todo> = order.iter().filter(|t| t.quadrant == q).collect();
            let labels: Vec<String> = open
                .iter()
                .take(DISPLAY_ITEMS)
                .map(|t| label(t, today))
                .collect();
            let bytes = serde_json::to_vec(
                &json!({"v":1,"op":"todo","q":q,"items":labels,"n":open.len().min(999)}),
            )?;
            ensure!(bytes.len() <= 480, "待办摘要过长");
            Ok(bytes)
        })
        .collect()
}
#[derive(Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Partition {
    version: u8,
    origin: String,
    items: Vec<Todo>,
}
pub enum Command {
    Save(Todo),
    Toggle(String),
    Delete(String),
}
fn load(path: &Path) -> Vec<Todo> {
    let mut items = vec![];
    if let Ok(p) = crate::cloud::read_json::<Partition>(path, MAX_FILE)
        && p.version == 1
    {
        merge(&mut items, p.items);
    }
    items
}
fn encode(origin: &str, items: &[Todo]) -> Vec<u8> {
    serde_json::to_vec(&Partition {
        version: 1,
        origin: origin.into(),
        items: items.to_vec(),
    })
    .unwrap_or_default()
}
/// Cheap change detector for the iCloud folder: names, sizes and modification times.
fn fingerprint(dir: &Path) -> Vec<(String, u64, u128)> {
    let mut rows: Vec<_> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let m = e.metadata().ok()?;
            let at = m
                .modified()
                .ok()?
                .duration_since(UNIX_EPOCH)
                .ok()?
                .as_nanos();
            Some((name, m.len(), at))
        })
        .collect();
    rows.sort();
    rows
}
/// Reads every partition in the folder (iCloud placeholders download on coordinated read).
fn read_partitions(dir: &Path) -> Vec<Todo> {
    let mut items = vec![];
    let names: Vec<String> = fingerprint(dir)
        .into_iter()
        .map(|(name, _, _)| {
            name.strip_prefix('.')
                .and_then(|s| s.strip_suffix(".icloud"))
                .unwrap_or(&name)
                .to_owned()
        })
        .filter(|name| name.starts_with("todos-") && name.ends_with(".json"))
        .take(MAX_PARTITIONS)
        .collect();
    for name in names {
        let path = dir.join(&name);
        if let Ok(p) = epaper_macos::coordinated(&path, false, |p| {
            crate::cloud::read_json::<Partition>(p, MAX_FILE)
                .map_err(|_| std::io::Error::other("待办同步文档无效"))
        }) && p.version == 1
        {
            merge(&mut items, p.items);
        }
    }
    items
}
pub fn spawn(
    root: PathBuf,
    events: crate::backend::Events,
) -> tokio::sync::mpsc::UnboundedSender<Command> {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(async move {
        let origin = crate::harness::Origin::local(&root)
            .map(|o| {
                o.id.chars()
                    .filter(char::is_ascii_alphanumeric)
                    .take(32)
                    .collect::<String>()
            })
            .unwrap_or_default();
        let local = root.join(".todos.json");
        let mut items = load(&local);
        let publish = |items: &[Todo], note: &str| {
            let visible: Vec<Todo> = items.iter().filter(|t| !t.deleted).cloned().collect();
            let note = note.to_owned();
            events.update(|s| {
                s.todos = visible;
                s.todo_note = note;
            });
        };
        publish(&items, "");
        let mut seen = None;
        let mut written = None::<Vec<u8>>;
        let mut timer = tokio::time::interval(Duration::from_secs(3));
        loop {
            let mut changed = false;
            tokio::select! {
                command = rx.recv() => {
                    let Some(command) = command else { break };
                    let next = match command {
                        Command::Save(t) => Some(t),
                        Command::Toggle(id) => items.iter().find(|t| t.id == id).cloned().map(|mut t| {
                            t.done = !t.done;
                            t
                        }),
                        Command::Delete(id) => items.iter().find(|t| t.id == id).cloned().map(|mut t| {
                            t.deleted = true;
                            t
                        }),
                    };
                    if let Some(mut t) = next {
                        // Strictly newer than the stored copy, so this local edit wins the merge.
                        let previous = items.iter().find(|o| o.id == t.id).map_or(0, |o| o.updated + 1);
                        t.updated = now_ms().max(previous);
                        t.origin = origin.clone();
                        changed = merge(&mut items, [t]);
                    }
                }
                _ = timer.tick() => {}
            }
            // Pick up partitions other Macs wrote; iCloud Drive syncs the folder by itself.
            let folder = events
                .state
                .lock()
                .ok()
                .and_then(|s| s.cloud.folder.clone());
            let folder = folder.filter(|f| epaper_macos::is_icloud_directory(f));
            let dir = folder
                .as_ref()
                .map(|f| f.join(DIR))
                .filter(|d| d.is_dir() || fs::create_dir(d).is_ok());
            let legacy = folder.as_ref().map(|f| f.join(LEGACY_DIR)).filter(|d| {
                fs::symlink_metadata(d).is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink())
            });
            let mut note = String::new();
            if let Some(dir) = &dir {
                let mut print = fingerprint(dir);
                if let Some(old) = &legacy {
                    print.extend(
                        fingerprint(old)
                            .into_iter()
                            .map(|(n, len, at)| (format!("{LEGACY_DIR}/{n}"), len, at)),
                    );
                }
                if seen.as_ref() != Some(&print) {
                    let remote = tokio::task::block_in_place(|| {
                        let mut remote = read_partitions(dir);
                        if let Some(old) = &legacy {
                            remote.extend(read_partitions(old));
                        }
                        remote
                    });
                    changed |= merge(&mut items, remote);
                    seen = Some(print);
                }
                note = "iCloud Drive".into();
            }
            changed |= prune(&mut items, now_ms());
            if changed {
                let bytes = encode(&origin, &items);
                let _ = crate::cloud::atomic(&local, &bytes);
                publish(&items, &note);
            }
            if let Some(dir) = &dir {
                let bytes = encode(&origin, &items);
                if written.as_ref() != Some(&bytes) && !origin.is_empty() {
                    let path = dir.join(format!("todos-{origin}.json"));
                    if epaper_macos::coordinated(&path, true, |p| crate::cloud::atomic(p, &bytes))
                        .is_ok()
                    {
                        written = Some(bytes);
                        seen = None;
                    }
                }
            }
        }
    });
    tx
}
#[cfg(test)]
mod tests {
    use super::*;
    fn item(id: u64, title: &str, q: u8, updated: i64, origin: &str) -> Todo {
        Todo {
            id: format!("{id:016x}"),
            title: title.into(),
            notes: String::new(),
            quadrant: q,
            date: None,
            time: None,
            minutes: None,
            done: false,
            deleted: false,
            updated,
            origin: origin.into(),
        }
    }
    #[test]
    fn details_stay_on_the_mac_and_old_partitions_still_load() {
        // Written before details existed; still read, and empty details are not written back.
        let old = r#"{"version":1,"origin":"a","items":[{"id":"0000000000000001","title":"买牛奶","quadrant":0,"updated":1,"origin":"a"}]}"#;
        let p: Partition = serde_json::from_str(old).unwrap();
        assert!(p.items[0].notes.is_empty());
        assert!(
            !String::from_utf8(encode("a", &p.items))
                .unwrap()
                .contains("notes")
        );
        let mut t = item(2, "看牙医", 1, 1, "a");
        t.notes = tidy_notes("  带医保卡\r\n地址：中山路 12 号\u{7}  ");
        assert_eq!(t.notes, "带医保卡\n地址：中山路 12 号");
        assert!(t.valid());
        let back: Partition = serde_json::from_slice(&encode("a", &[t.clone()])).unwrap();
        assert_eq!(back.items[0], t);
        // Displays get the title only.
        let first: serde_json::Value =
            serde_json::from_slice(&wire(&[t.clone()], 20261027).unwrap()[1]).unwrap();
        assert_eq!(first["items"][0], "看牙医");
        assert_eq!(tidy_notes(&"字".repeat(600)).chars().count(), NOTES_CHARS);
        t.notes = "字".repeat(NOTES_CHARS + 1);
        assert!(!t.valid());
        t.notes = "响铃\u{7}".into();
        assert!(!t.valid());
    }
    #[test]
    fn newest_edit_wins_and_tombstones_propagate_then_expire() {
        let mut mine = vec![item(1, "买牛奶", 0, 10, "a")];
        let mut theirs = mine.clone();
        theirs[0].title = "买豆浆".into();
        theirs[0].updated = 20;
        theirs[0].origin = "b".into();
        assert!(merge(&mut mine, theirs.clone()));
        assert_eq!(mine[0].title, "买豆浆");
        assert!(!merge(&mut mine, [item(1, "旧", 0, 15, "c")]));
        let mut gone = mine[0].clone();
        gone.deleted = true;
        gone.updated = 30;
        assert!(merge(&mut mine, [gone]));
        assert!(!mine[0].open());
        assert!(!prune(&mut mine, 31));
        assert!(prune(&mut mine, 30 + TOMBSTONE_MS));
        assert!(mine.is_empty());
        // Same timestamp from two Macs resolves the same way on both.
        let (x, y) = (item(2, "甲", 0, 5, "a"), item(2, "乙", 0, 5, "b"));
        let (mut one, mut two) = (vec![x.clone()], vec![y.clone()]);
        merge(&mut one, [y]);
        merge(&mut two, [x]);
        assert_eq!(one, two);
    }
    #[test]
    fn displays_get_overdue_and_today_first_with_time_and_byte_limits() {
        let today = 20261027;
        let mut a = item(1, "周报", 0, 1, "a");
        a.date = Some(20261030);
        let mut b = item(2, "站会", 0, 2, "a");
        b.date = Some(today);
        b.time = Some(9 * 60 + 30);
        let mut c = item(3, "逾期的事", 0, 3, "a");
        c.date = Some(20261020);
        let d = item(4, "随手记", 0, 4, "a");
        let mut e = item(5, "已完成", 0, 5, "a");
        e.done = true;
        let all = vec![a, b, c, d, e];
        let order: Vec<&str> = display_order(&all, today)
            .iter()
            .map(|t| t.title.as_str())
            .collect();
        assert_eq!(order, ["逾期的事", "站会", "随手记", "周报"]);
        assert_eq!(label(&all[1], today), "09:30 站会");
        let long = item(6, &"长".repeat(40), 1, 1, "a");
        assert!(label(&long, today).len() <= 48);
        let messages = wire(&all, today).unwrap();
        assert_eq!(messages.len(), 4);
        let first: serde_json::Value = serde_json::from_slice(&messages[0]).unwrap();
        assert_eq!(first["n"], 4);
        assert_eq!(first["items"][0], "逾期的事");
        assert!(messages.iter().all(|m| m.len() <= 480));
    }
    #[test]
    fn partitions_from_other_macs_are_read_from_disk_and_changes_are_noticed() {
        let dir = std::env::temp_dir().join(format!("hc-todos-{:016x}", rand::random::<u64>()));
        fs::create_dir_all(&dir).unwrap();
        let write = |name: &str, items: Vec<Todo>| {
            fs::write(dir.join(name), encode(name, &items)).unwrap();
        };
        write("todos-maca.json", vec![item(1, "买牛奶", 0, 10, "maca")]);
        let before = fingerprint(&dir);
        let mut newer = item(1, "买豆浆", 0, 20, "macb");
        newer.date = Some(20261027);
        write(
            "todos-macb.json",
            vec![newer, item(2, "倒垃圾", 3, 5, "macb")],
        );
        // Unrelated files and not-yet-downloaded placeholders are ignored.
        fs::write(dir.join("notes.json"), b"{}").unwrap();
        fs::write(dir.join(".todos-macc.json.icloud"), b"").unwrap();
        assert_ne!(fingerprint(&dir), before);
        let mut merged = read_partitions(&dir);
        merged.sort_by_key(|t| t.id.clone());
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].title, "买豆浆");
        assert_eq!(merged[1].title, "倒垃圾");
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn invalid_items_are_ignored() {
        let mut list = vec![];
        let mut bad = item(1, "", 0, 1, "a");
        assert!(!merge(&mut list, [bad.clone()]));
        bad.title = "ok".into();
        bad.quadrant = 4;
        assert!(!merge(&mut list, [bad.clone()]));
        bad.quadrant = 0;
        bad.date = Some(20261332);
        assert!(!merge(&mut list, [bad]));
    }
}
