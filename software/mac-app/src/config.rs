use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    os::unix::{
        fs::{OpenOptionsExt, PermissionsExt},
        io::AsRawFd,
    },
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use zeroize::Zeroize;

pub fn project_root() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("EPAPER_PROJECT") {
        let path = PathBuf::from(p);
        return if path.join("software/project.json").is_file() {
            checkout_data_root(&path)
        } else {
            Ok(path)
        };
    }
    if let Some(root) = checkout_root(&std::env::current_exe()?) {
        return checkout_data_root(&root);
    }
    // A distributed .app can run without the developer's checkout. Do not copy any pairing
    // secret into the bundle; a new Mac binds its own device into this private directory.
    let home = std::env::var_os("HOME").ok_or_else(|| anyhow::anyhow!("找不到用户数据目录"))?;
    let support = support_root(Path::new(&home))?;
    selected_support_root(&support)
}

fn checkout_root(executable: &Path) -> Option<PathBuf> {
    executable.ancestors().skip(1).find_map(|path| {
        // A workspace may contain several boards. Never select software/ as the data root.
        (path.join("software/project.json").is_file()
            && path.join("software/mac-app/Cargo.toml").is_file())
        .then(|| path.to_owned())
    })
}

fn checkout_data_root(workspace: &Path) -> Result<PathBuf> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Project {
        schema_version: u8,
        hardware: PathBuf,
    }
    let selected: Project =
        serde_json::from_slice(&std::fs::read(workspace.join("software/project.json"))?)?;
    ensure!(
        selected.schema_version == 1
            && !selected.hardware.as_os_str().is_empty()
            && selected
                .hardware
                .components()
                .all(|c| matches!(c, std::path::Component::Normal(_))),
        "硬件目录设置无效。"
    );
    let board = workspace.join(selected.hardware).canonicalize()?;
    ensure!(
        board.starts_with(workspace.canonicalize()?) && board.join("board.json").is_file(),
        "请选择含 board.json 的独立硬件目录。"
    );
    let data = board.join(".local");
    std::fs::create_dir_all(&data)?;
    ensure!(
        !std::fs::symlink_metadata(&data)?.file_type().is_symlink(),
        "设备资料目录不能链接到其他板子。"
    );
    std::fs::set_permissions(&data, std::fs::Permissions::from_mode(0o700))?;
    Ok(data)
}

fn selected_support_root(support: &Path) -> Result<PathBuf> {
    let location = support.join(".data-location.json");
    if location.try_exists()? {
        let saved: DataLocation = read_private(&location)?;
        validate_data_location(&saved.path)?;
        return Ok(saved.path);
    }
    Ok(support.to_owned())
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DataLocation {
    path: PathBuf,
}

fn validate_data_location(path: &Path) -> Result<()> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::symlink_metadata(path)?;
    ensure!(
        path.is_absolute()
            && meta.is_dir()
            && !meta.file_type().is_symlink()
            && meta.uid() == unsafe { libc::geteuid() },
        "原配置目录不可用，请重新选择本机目录。"
    );
    pairing(path)?;
    Ok(())
}

/// Explicit per-Mac selection, never a build-machine path inside the distributed app.
pub fn use_data_directory(path: &Path) -> Result<()> {
    validate_data_location(path)?;
    let home = std::env::var_os("HOME").context("找不到用户数据目录")?;
    let support = support_root(Path::new(&home))?;
    write_private(
        &support.join(".data-location.json"),
        &DataLocation {
            path: path.to_owned(),
        },
    )
}

pub(crate) fn write_private<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let temp = path.with_file_name(format!(".homecraft-{:016x}.tmp", rand::random::<u64>()));
    let result = (|| {
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temp)?;
        let mut bytes = serde_json::to_vec(value)?;
        let result = f.write_all(&bytes);
        bytes.zeroize();
        result?;
        f.sync_all()?;
        std::fs::rename(&temp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temp);
    }
    result
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RememberedDevice {
    pub name: String,
    pub id: String,
}
impl RememberedDevice {
    fn valid(&self, paired: &Pairing) -> bool {
        self.name == paired.name && uuid::Uuid::parse_str(&self.id).is_ok()
    }
}
pub fn remembered_device(root: &Path, paired: &Pairing) -> Option<RememberedDevice> {
    let value: RememberedDevice = read_private(&root.join(".ble-device.json")).ok()?;
    value.valid(paired).then_some(value)
}
pub fn remember_device(root: &Path, name: &str, id: &str) -> Result<()> {
    let value = RememberedDevice {
        name: name.to_owned(),
        id: id.to_owned(),
    };
    ensure!(value.valid(&pairing(root)?), "设备标识与原绑定不一致。");
    write_private(&root.join(".ble-device.json"), &value)
}

fn support_root(home: &Path) -> Result<PathBuf> {
    let support = home.join("Library/Application Support");
    let legacy = support.join("EPaper Home");
    let current = support.join("HomeCraft");
    // Preserve a previous standalone installation without copying its credentials.
    let root = if !current.exists() && legacy.exists() {
        legacy
    } else {
        current
    };
    std::fs::create_dir_all(&root)?;
    let meta = std::fs::symlink_metadata(&root)?;
    ensure!(
        meta.is_dir() && !meta.file_type().is_symlink(),
        "应用数据目录不能是符号链接"
    );
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))?;
    Ok(root)
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Pairing {
    pub name: String,
    pub username: String,
    pub password: String,
    pub service_uuid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}
impl Pairing {
    fn validate(&self) -> Result<()> {
        ensure!(
            self.name.strip_prefix("EPaper_").is_some_and(
                |suffix| suffix.len() == 6 && suffix.bytes().all(|c| c.is_ascii_hexdigit())
            ) && self.username == "wifiprov"
                && self.password.len() == 32
                && self.password.bytes().all(|c| c.is_ascii_hexdigit())
                && self.service_uuid == crate::backend::SERVICE_UUID
                && self
                    .model
                    .as_deref()
                    .is_none_or(|m| ["3Inch7EPaper", "reTerminalE1002"].contains(&m)),
            "设备认证资料无效。"
        );
        Ok(())
    }
}
impl Drop for Pairing {
    fn drop(&mut self) {
        self.password.zeroize();
    }
}

pub fn read_private<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    let meta = file.metadata()?;
    ensure!(
        meta.is_file() && meta.permissions().mode() & 0o077 == 0,
        "私密配置需要权限 600。"
    );
    ensure!(meta.len() <= 32_768, "配置文件过长。");
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    let parsed = serde_json::from_slice(&bytes).context("配置文件格式错误。");
    bytes.zeroize();
    parsed
}

pub fn pairing(root: &Path) -> Result<Pairing> {
    let p: Pairing = read_private(&root.join(".ble-pairing.json"))
        .context("首次连接请在 HomeCraft 输入墨水屏上的配对码。")?;
    p.validate()?;
    Ok(p)
}

pub fn wireless_pairing(bytes: &[u8], advertised_name: &str) -> Result<Pairing> {
    let p: Pairing = serde_json::from_slice(bytes).context("设备认证回复格式无效。")?;
    p.validate()?;
    let suffix = advertised_name
        .strip_prefix("EPair_")
        .context("请选择配对中的设备。")?;
    ensure!(
        p.name == format!("EPaper_{suffix}"),
        "认证回复与所选设备不一致。"
    );
    Ok(p)
}

pub fn save_pairing(root: &Path, p: &Pairing) -> Result<()> {
    p.validate()?;
    let path = root.join(format!(".pairing-{:016x}.tmp", rand::random::<u64>()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)?;
        let mut bytes = serde_json::to_vec(p)?;
        let written = file.write_all(&bytes);
        bytes.zeroize();
        written?;
        file.sync_all()?;
        std::fs::rename(&path, root.join(".ble-pairing.json"))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(path);
    }
    result
}

pub struct Lock(File);
impl Lock {
    pub fn take(root: &Path, name: &str) -> Result<Self> {
        let mut f = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(root.join(name))?;
        f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        ensure!(
            unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0,
            "已有发送程序运行，请先停止旧程序。"
        );
        f.set_len(0)?;
        writeln!(f, "{}", std::process::id())?;
        Ok(Self(f))
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        unsafe {
            libc::flock(self.0.as_raw_fd(), libc::LOCK_UN);
        }
    }
}

pub fn pair_usb(root: &Path) -> Result<String> {
    let mut ports: Vec<_> = std::fs::read_dir("/dev")?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("cu.usbmodem")
        })
        .collect();
    ports.sort();
    ensure!(
        ports.len() == 1,
        "请通过 USB 连接一台 ESP32；检测到 {} 个 USB 串口。",
        ports.len()
    );
    let mut serial = OpenOptions::new()
        .read(true)
        .write(true)
        .custom_flags(libc::O_NOCTTY | libc::O_NONBLOCK)
        .open(&ports[0])?;
    let fd = serial.as_raw_fd();
    unsafe {
        let mut t: libc::termios = std::mem::zeroed();
        ensure!(libc::tcgetattr(fd, &mut t) == 0, "无法读取串口设置。");
        libc::cfmakeraw(&mut t);
        t.c_cflag |= libc::CLOCAL | libc::CREAD;
        t.c_cflag &= !libc::HUPCL;
        libc::cfsetspeed(&mut t, libc::B115200);
        ensure!(
            libc::tcsetattr(fd, libc::TCSANOW, &t) == 0,
            "无法设置串口。"
        );
        libc::tcflush(fd, libc::TCIFLUSH);
    }
    serial.write_all(b"\nEPAPER_PAIR\n")?;
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut buffer = Vec::new();
    let mut block = [0u8; 2048];
    while Instant::now() < deadline {
        match serial.read(&mut block) {
            Ok(n) => {
                buffer.extend_from_slice(&block[..n]);
                block.zeroize();
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => (),
            Err(_) => bail!("USB 读取失败。"),
        }
        while let Some(end) = buffer.iter().position(|b| *b == b'\n') {
            let mut line: Vec<_> = buffer.drain(..=end).collect();
            if let Some(raw) = line.strip_prefix(b"EPAPER_PAIR ") {
                let parsed = serde_json::from_slice::<Pairing>(raw);
                line.zeroize();
                buffer.zeroize();
                let p = parsed.context("配对回复无效。\u{200B}")?;
                ensure!(
                    p.name.starts_with("EPaper_")
                        && p.username == "wifiprov"
                        && p.password.len() == 32
                        && p.password.bytes().all(|c| c.is_ascii_hexdigit())
                        && p.service_uuid == crate::backend::SERVICE_UUID,
                    "设备配对资料不受支持。"
                );
                let mut f = OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .mode(0o600)
                    .custom_flags(libc::O_NOFOLLOW)
                    .open(root.join(".ble-pairing.json"))?;
                f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
                let mut bytes = serde_json::to_vec(&p)?;
                let result = f.write_all(&bytes);
                bytes.zeroize();
                result?;
                return Ok(p.name.clone());
            }
            if line.starts_with(b"EPAPER_PAIR_DENIED") {
                std::thread::sleep(Duration::from_secs(1));
                serial.write_all(b"EPAPER_PAIR\n")?;
            }
            line.zeroize();
        }
        if buffer.len() > 8192 {
            buffer.zeroize();
            buffer.clear();
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    buffer.zeroize();
    bail!("请在墨水屏网络页双击 BOOT 开启配对窗口，再点击 USB 绑定。");
}

#[cfg(test)]
mod home_storage_tests {
    use super::*;
    #[test]
    fn checkout_discovery_keeps_workspace_data_above_software_and_board_names() {
        let root =
            std::env::temp_dir().join(format!("homecraft-layout-{:016x}", rand::random::<u64>()));
        let app = root.join("software/dist/HomeCraft.app/Contents/MacOS/HomeCraft");
        std::fs::create_dir_all(app.parent().unwrap()).unwrap();
        std::fs::create_dir_all(root.join("software/mac-app")).unwrap();
        std::fs::create_dir_all(root.join("software/tools")).unwrap();
        std::fs::write(root.join("software/mac-app/Cargo.toml"), "").unwrap();
        std::fs::write(root.join("software/tools/ble_bridge.py"), "").unwrap();
        assert!(checkout_root(&app).is_none());
        std::fs::write(root.join("software/project.json"), "{}").unwrap();
        assert_eq!(checkout_root(&app), Some(root.clone()));
        let cli = root.join("software/mac-app/target/debug/epaper-ble");
        assert_eq!(checkout_root(&cli), Some(root.clone()));
        assert!(
            checkout_root(Path::new(
                "/Applications/HomeCraft.app/Contents/MacOS/HomeCraft"
            ))
            .is_none()
        );
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn hardware_profiles_keep_data_separate_and_reject_parent_paths() {
        let root =
            std::env::temp_dir().join(format!("homecraft-profiles-{:016x}", rand::random::<u64>()));
        std::fs::create_dir_all(root.join("software")).unwrap();
        let mut paths = Vec::new();
        for name in ["board-a", "board-b"] {
            std::fs::create_dir_all(root.join(name)).unwrap();
            std::fs::write(root.join(name).join("board.json"), "{}").unwrap();
            std::fs::write(
                root.join("software/project.json"),
                format!(r#"{{"schema_version":1,"hardware":"{name}"}}"#),
            )
            .unwrap();
            paths.push(checkout_data_root(&root).unwrap());
        }
        assert_ne!(paths[0], paths[1]);
        assert_eq!(
            paths[0],
            root.canonicalize().unwrap().join("board-a/.local")
        );
        for name in ["../board-a", "/tmp", ".", "software"] {
            std::fs::write(
                root.join("software/project.json"),
                serde_json::to_vec(&serde_json::json!({"schema_version":1,"hardware":name}))
                    .unwrap(),
            )
            .unwrap();
            assert!(checkout_data_root(&root).is_err());
        }
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn wireless_pairing_rejects_changed_identity_and_unexpected_secrets() {
        let fixture=br#"{"name":"EPaper_A1B2C3","username":"wifiprov","password":"00000000000000000000000000000000","service_uuid":"1775244d-6b43-439b-877c-060f2d9bed07"}"#;
        assert!(wireless_pairing(fixture, "EPair_A1B2C3").is_ok());
        assert!(wireless_pairing(fixture, "EPair_000000").is_err());
        assert!(wireless_pairing(fixture, "EPaper_A1B2C3").is_err());
        let mut value: serde_json::Value = serde_json::from_slice(fixture).unwrap();
        value["account_token"] = serde_json::json!("not-accepted");
        assert!(wireless_pairing(&serde_json::to_vec(&value).unwrap(), "EPair_A1B2C3").is_err());
    }
    #[test]
    fn two_macs_retain_their_binding_without_usb_or_rotation() {
        let fixture=br#"{"name":"EPaper_A1B2C3","username":"wifiprov","password":"00000000000000000000000000000000","service_uuid":"1775244d-6b43-439b-877c-060f2d9bed07"}"#;
        let p = wireless_pairing(fixture, "EPair_A1B2C3").unwrap();
        let root =
            std::env::temp_dir().join(format!("homecraft-two-macs-{:016x}", rand::random::<u64>()));
        for name in ["first", "second"] {
            let dir = root.join(name);
            std::fs::create_dir_all(&dir).unwrap();
            save_pairing(&dir, &p).unwrap();
        }
        for name in ["first", "second"] {
            let dir = root.join(name);
            let stored = pairing(&dir).unwrap();
            assert_eq!(stored.name, p.name);
            assert_eq!(stored.password, p.password);
            assert_eq!(
                std::fs::metadata(dir.join(".ble-pairing.json"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn standalone_data_directory_is_private() {
        let tmp = std::env::temp_dir().join(format!(
            "epaper-support-test-{:016x}",
            rand::random::<u64>()
        ));
        let root = support_root(&tmp).unwrap();
        assert_eq!(root, tmp.join("Library/Application Support/HomeCraft"));
        assert_eq!(
            std::fs::metadata(&root).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(support_root(&tmp).unwrap(), root);
        std::fs::remove_dir_all(tmp).unwrap();
    }
    #[test]
    fn explicit_local_directory_preserves_binding_and_shared_locks() {
        let tmp =
            std::env::temp_dir().join(format!("homecraft-location-{:016x}", rand::random::<u64>()));
        let source = tmp.join("original");
        std::fs::create_dir_all(&source).unwrap();
        let fixture=br#"{"name":"EPaper_A1B2C3","username":"wifiprov","password":"00000000000000000000000000000000","service_uuid":"1775244d-6b43-439b-877c-060f2d9bed07"}"#;
        let pairing = wireless_pairing(fixture, "EPair_A1B2C3").unwrap();
        save_pairing(&source, &pairing).unwrap();
        let support = support_root(&tmp).unwrap();
        write_private(
            &support.join(".data-location.json"),
            &DataLocation {
                path: source.clone(),
            },
        )
        .unwrap();
        let selected = selected_support_root(&support).unwrap();
        assert_eq!(selected, source);
        assert_eq!(
            super::pairing(&selected).unwrap().password,
            pairing.password
        );
        let _first = Lock::take(&source, ".ble-sender.lock").unwrap();
        assert!(Lock::take(&selected, ".ble-sender.lock").is_err());
        let alias = tmp.join("alias");
        std::os::unix::fs::symlink(&source, &alias).unwrap();
        write_private(
            &support.join(".data-location.json"),
            &DataLocation { path: alias },
        )
        .unwrap();
        assert!(selected_support_root(&support).is_err());
        std::fs::remove_dir_all(tmp).unwrap();
    }
    #[test]
    fn remembered_uuid_is_private_bound_to_existing_identity_and_contains_no_secret() {
        let tmp =
            std::env::temp_dir().join(format!("homecraft-remember-{:016x}", rand::random::<u64>()));
        std::fs::create_dir_all(&tmp).unwrap();
        let fixture=br#"{"name":"EPaper_A1B2C3","username":"wifiprov","password":"00000000000000000000000000000000","service_uuid":"1775244d-6b43-439b-877c-060f2d9bed07"}"#;
        let p = wireless_pairing(fixture, "EPair_A1B2C3").unwrap();
        save_pairing(&tmp, &p).unwrap();
        remember_device(&tmp, &p.name, "11111111-1111-1111-1111-111111111111").unwrap();
        assert!(remembered_device(&tmp, &p).is_some());
        let bytes = std::fs::read_to_string(tmp.join(".ble-device.json")).unwrap();
        assert!(!bytes.contains("password") && !bytes.contains(&p.password));
        assert!(
            remember_device(
                &tmp,
                "EPaper_000000",
                "11111111-1111-1111-1111-111111111111"
            )
            .is_err()
        );
        assert!(remember_device(&tmp, &p.name, "invalid").is_err());
        assert_eq!(
            std::fs::metadata(tmp.join(".ble-device.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        std::fs::remove_dir_all(tmp).unwrap();
    }
    #[test]
    fn standalone_legacy_directory_preserves_existing_binding() {
        let tmp = std::env::temp_dir().join(format!(
            "homecraft-legacy-test-{:016x}",
            rand::random::<u64>()
        ));
        let legacy = tmp.join("Library/Application Support/EPaper Home");
        std::fs::create_dir_all(&legacy).unwrap();
        assert_eq!(support_root(&tmp).unwrap(), legacy);
        std::fs::remove_dir_all(tmp).unwrap();
    }
}
