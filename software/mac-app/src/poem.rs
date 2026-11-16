//! Bundled public-domain quatrains (verified texts with their sources), read from the app bundle. One per UTC+8 day, or the
//! user's next pick; displays advertising poem:1 receive the title, author and two verse lines.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::OnceLock;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Poem {
    pub id: u32,
    pub title: String,
    pub dynasty: String,
    pub author: String,
    pub lines: Vec<String>,
    #[serde(default)]
    pub source: String,
}
impl Poem {
    fn valid(&self) -> bool {
        let short = |s: &str, max: usize| {
            !s.is_empty() && s.len() <= max && !s.chars().any(char::is_control)
        };
        short(&self.title, 36)
            && short(&self.author, 18)
            && self.dynasty.len() <= 9
            && self.byline().len() <= 24
            && (1..=4).contains(&self.lines.len())
            && self.lines.iter().all(|l| short(l, 60))
    }
    pub fn byline(&self) -> String {
        if self.dynasty.is_empty() {
            self.author.clone()
        } else {
            format!("{} · {}", self.dynasty, self.author)
        }
    }
    pub fn wire(&self) -> Result<Vec<u8>> {
        let bytes = serde_json::to_vec(
            &json!({"v":1,"op":"poem","title":self.title,"author":self.byline(),"lines":self.lines}),
        )?;
        ensure!(bytes.len() <= 480, "古诗过长");
        Ok(bytes)
    }
}
#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub enabled: bool,
    /// Added to the day index when the user asks for another poem.
    pub offset: i64,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            offset: 0,
        }
    }
}
/// The bundled list: Contents/Resources/poems.json in the app, or software/assets in a checkout.
fn source() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let bundled = exe.parent()?.parent()?.join("Resources/poems.json");
    if bundled.is_file() {
        return Some(bundled);
    }
    exe.ancestors()
        .map(|p| p.join("software/assets/poems/poems.json"))
        .find(|p| p.is_file())
        .or_else(|| {
            let dev =
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets/poems/poems.json");
            dev.is_file().then_some(dev)
        })
}
pub fn all() -> &'static [Poem] {
    static POEMS: OnceLock<Vec<Poem>> = OnceLock::new();
    POEMS.get_or_init(|| {
        source()
            .and_then(|path| crate::cloud::read_json::<Vec<Poem>>(&path, 512 * 1024).ok())
            .unwrap_or_default()
            .into_iter()
            .filter(Poem::valid)
            .collect()
    })
}
/// Days since 1970-01-01 in UTC+8.
pub fn day_index(now: i64) -> i64 {
    (now + 8 * 3600).div_euclid(86_400)
}
pub fn pick(now: i64, settings: &Settings) -> Option<&'static Poem> {
    let poems = all();
    (!poems.is_empty() && settings.enabled)
        .then(|| &poems[(day_index(now) + settings.offset).rem_euclid(poems.len() as i64) as usize])
}
/// Keeps the hub's current poem in state; reads only preferences and the bundled list.
pub fn spawn(
    root: std::path::PathBuf,
    events: crate::backend::Events,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let settings = crate::config::read_private::<crate::home::model::Preferences>(
                &root.join(".home-ui.json"),
            )
            .ok()
            .filter(|p| p.validate())
            .unwrap_or_default()
            .poem;
            let current = pick(crate::usage::now(), &settings).cloned();
            events.update(|s| {
                if s.poem != current {
                    s.poem = current;
                }
            });
            tokio::time::sleep(std::time::Duration::from_secs(20)).await;
        }
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_poems_are_valid_quatrains_that_fit_the_display() {
        let poems = all();
        assert_eq!(poems.len(), 100);
        for p in poems {
            assert_eq!(p.lines.len(), 2, "{}", p.title);
            for text in [p.title.as_str(), p.author.as_str()]
                .into_iter()
                .chain(p.lines.iter().map(String::as_str))
            {
                assert!(text.chars().all(|c| c.is_ascii() || is_gb2312(c)), "{text}");
            }
            assert!(p.wire().is_ok());
        }
    }
    #[test]
    fn one_poem_per_day_and_next_moves_forward() {
        let now = 1792987200;
        let first = pick(now, &Settings::default()).unwrap();
        assert_eq!(pick(now + 3600, &Settings::default()), Some(first));
        assert_ne!(pick(now + 86_400, &Settings::default()), Some(first));
        let next = Settings {
            offset: 1,
            ..Settings::default()
        };
        assert_eq!(pick(now, &next), pick(now + 86_400, &Settings::default()));
        let off = Settings {
            enabled: false,
            ..Settings::default()
        };
        assert!(pick(now, &off).is_none());
    }
    /// GB2312 hanzi live in the CJK block; punctuation used by the bundle is listed explicitly.
    fn is_gb2312(c: char) -> bool {
        ('\u{4e00}'..='\u{9fa5}').contains(&c) || "，。？！；：·“”《》、—…（）".contains(c)
    }
}
