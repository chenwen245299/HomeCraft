//! Credential locations and remembered physical devices. Secrets stay in their board directory.
use crate::config::{self, Pairing};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub name: String,
    pub id: String,
    pub model: String,
    pub directory: PathBuf,
}
impl Binding {
    fn valid(&self) -> bool {
        valid_name(&self.name)
            && uuid::Uuid::parse_str(&self.id).is_ok()
            && ["3Inch7EPaper", "reTerminalE1002"].contains(&self.model.as_str())
            && self.directory.is_absolute()
    }
}
fn valid_name(s: &str) -> bool {
    s.strip_prefix("EPaper_")
        .is_some_and(|s| s.len() == 6 && s.bytes().all(|c| c.is_ascii_hexdigit()))
}
#[derive(Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Catalog {
    version: u8,
    devices: Vec<Binding>,
}
pub fn list(root: &Path) -> Result<Vec<Binding>> {
    let path = root.join(".ble-devices.json");
    let mut rows = if path.try_exists()? {
        let c: Catalog = config::read_private(&path)?;
        ensure!(c.version == 1 && c.devices.len() <= 8, "设备目录格式无效");
        c.devices
    } else {
        vec![]
    };
    for row in &rows {
        ensure!(row.valid(), "设备目录含无效标识");
        let p = config::pairing(&row.directory)?;
        ensure!(
            p.name == row.name && p.model.as_deref().unwrap_or("3Inch7EPaper") == row.model,
            "设备目录与绑定不匹配"
        );
    }
    let mut names = std::collections::HashSet::new();
    let mut ids = std::collections::HashSet::new();
    let mut dirs = std::collections::HashSet::new();
    for row in &rows {
        ensure!(
            names.insert(row.name.clone())
                && ids.insert(row.id.clone())
                && dirs.insert(row.directory.clone()),
            "设备目录存在重复身份"
        );
    }
    // Existing single-device files remain at their original path; no secret migration or rotation.
    if let Ok(p) = config::pairing(root)
        && !rows.iter().any(|r| r.name == p.name)
        && let Some(saved) = config::remembered_device(root, &p)
    {
        rows.push(Binding {
            name: p.name.clone(),
            id: saved.id,
            model: p.model.clone().unwrap_or_else(|| "3Inch7EPaper".into()),
            directory: root.canonicalize()?,
        });
    }
    ensure!(rows.len() <= 8, "最多保存8台设备");
    Ok(rows)
}
pub fn find(root: &Path, name: &str) -> Result<Binding> {
    list(root)?
        .into_iter()
        .find(|r| r.name == name)
        .context("这台设备尚未绑定，请输入它自己的屏幕码")
}
fn private_dir(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)?;
    let meta = std::fs::symlink_metadata(path)?;
    ensure!(
        meta.is_dir() && !meta.file_type().is_symlink(),
        "设备目录不能使用符号链接"
    );
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    Ok(())
}
fn new_directory(root: &Path, model: &str, name: &str) -> Result<PathBuf> {
    ensure!(
        valid_name(name) && ["3Inch7EPaper", "reTerminalE1002"].contains(&model),
        "设备型号或名称无效"
    );
    let parent = if let Some(board) = root.parent().filter(|p| p.join("board.json").is_file()) {
        let workspace = board.parent().context("工作区目录无效")?;
        let target = workspace.join(model);
        ensure!(target.join("board.json").is_file(), "对应硬件目录不存在");
        ensure!(
            target
                .canonicalize()?
                .starts_with(workspace.canonicalize()?),
            "硬件路径越界"
        );
        target.join(".local")
    } else {
        root.join("devices").join(model)
    };
    private_dir(&parent)?;
    let devices = parent.join("devices");
    private_dir(&devices)?;
    let directory = devices.join(name);
    private_dir(&directory)?;
    ensure!(
        directory
            .canonicalize()?
            .starts_with(parent.canonicalize()?),
        "设备资料路径越界"
    );
    Ok(directory.canonicalize()?)
}
pub fn store(root: &Path, p: &Pairing, id: &str) -> Result<Binding> {
    let model = p.model.as_deref().unwrap_or("3Inch7EPaper");
    let mut rows = list(root)?;
    ensure!(
        !rows.iter().any(|r| r.id == id && r.name != p.name),
        "蓝牙标识已属于另一台设备"
    );
    let old = rows.iter().find(|r| r.name == p.name);
    if let Some(old) = old {
        ensure!(
            old.id == id && old.model == model,
            "绑定身份发生变化，请检查所选设备"
        );
    }
    let directory = match old {
        Some(old) => old.directory.clone(),
        None => new_directory(root, model, &p.name)?,
    };
    ensure!(rows.len() < 8 || old.is_some(), "最多保存8台设备");
    // A new Mac enrollment must return the established long-term key, not silently rotate it.
    if directory.join(".ble-pairing.json").exists() {
        let previous = config::pairing(&directory)?;
        ensure!(
            previous.name == p.name && previous.password == p.password,
            "设备长期认证已变化，原绑定保留"
        );
    }
    config::save_pairing(&directory, p)?;
    config::remember_device(&directory, &p.name, id)?;
    let row = Binding {
        name: p.name.clone(),
        id: id.into(),
        model: model.into(),
        directory,
    };
    rows.retain(|r| r.name != row.name);
    rows.push(row.clone());
    config::write_private(
        &root.join(".ble-devices.json"),
        &Catalog {
            version: 1,
            devices: rows,
        },
    )?;
    Ok(row)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn binding(name: &str) -> Pairing {
        Pairing {
            name: name.into(),
            username: "wifiprov".into(),
            password: "1234567890abcdef1234567890abcdef".into(),
            service_uuid: crate::backend::SERVICE_UUID.into(),
            model: Some("reTerminalE1002".into()),
        }
    }
    #[test]
    fn independent_bindings_preserve_legacy_and_reject_identity_collision() {
        let root = std::env::temp_dir().join(format!("hc-registry-{:016x}", rand::random::<u64>()));
        private_dir(&root).unwrap();
        let a = store(
            &root,
            &binding("EPaper_AAAAAA"),
            "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
        )
        .unwrap();
        let b = store(
            &root,
            &binding("EPaper_BBBBBB"),
            "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb",
        )
        .unwrap();
        assert_ne!(a.directory, b.directory);
        assert!(!root.join(".ble-pairing.json").exists());
        assert_eq!(list(&root).unwrap().len(), 2);
        assert!(store(&root, &binding("EPaper_CCCCCC"), &a.id).is_err());
        let mut p = binding("EPaper_AAAAAA");
        p.password = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into();
        assert!(store(&root, &p, &a.id).is_err());
        assert_eq!(
            config::pairing(&a.directory).unwrap().password,
            binding("EPaper_AAAAAA").password
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn traversal_and_unknown_models_are_rejected() {
        assert!(!valid_name("EPaper_../../"));
        assert!(!valid_name("EPaper_12345G"));
        assert!(valid_name("EPaper_123ABC"));
    }
}
