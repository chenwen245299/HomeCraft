//! Public GitHub release updates. Collection/BLE never wait for update I/O.
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

pub fn releases_url() -> String {
    let repository = serde_json::from_str::<serde_json::Value>(include_str!("../../release.json"))
        .ok()
        .and_then(|v| v["repository"].as_str().map(str::to_owned))
        .unwrap_or_default();
    format!("https://github.com/{repository}/releases")
}
fn manifest_url() -> String {
    format!("{}/latest/download/latest.json", releases_url())
}
const MAX_ARCHIVE: u64 = 256 * 1024 * 1024;
const MAX_UNPACKED: u64 = 512 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub auto_check: bool,
    pub auto_download: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_check: true,
            auto_download: true,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Asset {
    pub url: String,
    pub sha256: String,
    pub size: u64,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Manifest {
    pub schema: u8,
    pub version: String,
    pub build: u64,
    pub tag: String,
    pub minimum_macos: String,
    pub notes: String,
    pub release_url: String,
    pub platforms: BTreeMap<String, Asset>,
}
#[derive(Clone, Debug)]
pub struct Release {
    pub version: String,
    pub build: u64,
    pub notes: String,
    pub asset: Asset,
}
#[derive(Clone, Debug)]
pub struct Staged {
    pub release: Release,
    directory: Arc<StageDir>,
}
impl Staged {
    fn app(&self) -> PathBuf {
        self.directory.path.join("unpacked/HomeCraft.app")
    }
}
#[derive(Debug)]
struct StageDir {
    path: PathBuf,
    keep: AtomicBool,
}
impl StageDir {
    fn new() -> Result<Arc<Self>> {
        let path =
            std::env::temp_dir().join(format!("homecraft-update-{:016x}", rand::random::<u64>()));
        fs::DirBuilder::new().mode(0o700).create(&path)?;
        Ok(Arc::new(Self {
            path,
            keep: AtomicBool::new(false),
        }))
    }
}
impl Drop for StageDir {
    fn drop(&mut self) {
        if !self.keep.load(Ordering::SeqCst) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

#[derive(Clone, Debug, Default)]
pub enum Status {
    #[default]
    Idle,
    Checking,
    Current,
    Available(Release),
    Downloading {
        release: Release,
        done: u64,
        total: u64,
    },
    Ready(Staged),
    Failed(String),
}
impl Status {
    pub fn busy(&self) -> bool {
        matches!(self, Self::Checking | Self::Downloading { .. })
    }
    pub fn release(&self) -> Option<&Release> {
        match self {
            Self::Available(r) | Self::Downloading { release: r, .. } => Some(r),
            Self::Ready(s) => Some(&s.release),
            _ => None,
        }
    }
    pub fn summary(&self) -> &str {
        match self {
            Self::Idle => "查看版本与更新设置",
            Self::Checking => "正在检查更新…",
            Self::Current => "已是最新版本",
            Self::Available(_) => "发现新版本",
            Self::Downloading { .. } => "正在下载更新…",
            Self::Ready(_) => "更新已就绪",
            Self::Failed(_) => "暂未完成，可重试",
        }
    }
}

pub fn current_build() -> u64 {
    serde_json::from_str::<serde_json::Value>(include_str!("../../release.json"))
        .ok()
        .and_then(|v| v["build"].as_u64())
        .unwrap_or(0)
}

pub fn newer(manifest: Manifest, current: &str, build: u64, os: &str) -> Result<Option<Release>> {
    let base = releases_url();
    ensure!(manifest.schema == 1, "更新清单版本暂不支持");
    let version = semver::Version::parse(&manifest.version).context("更新版本无效")?;
    let current = semver::Version::parse(current)?;
    // GitHub's latest endpoint excludes prereleases; also refuse one in a malformed feed.
    if !version.pre.is_empty() {
        return Ok(None);
    }
    ensure!(
        manifest.build > 0
            && manifest.build <= 2_147_483_647
            && manifest.tag == format!("v{version}"),
        "更新版本与标签不匹配"
    );
    ensure!(
        manifest.notes.len() <= 60_000
            && manifest.release_url == format!("{base}/tag/{}", manifest.tag),
        "更新来源无效"
    );
    let os_pair = |s: &str| -> Option<(u64, u64)> {
        let mut p = s.split('.');
        Some((
            p.next()?.parse().ok()?,
            p.next().unwrap_or("0").parse().ok()?,
        ))
    };
    ensure!(
        os_pair(os)
            .zip(os_pair(&manifest.minimum_macos))
            .is_some_and(|(have, need)| have >= need),
        "新版需要更高版本的 macOS"
    );
    let asset = manifest
        .platforms
        .get("macos-arm64")
        .context("此版本没有 Apple 芯片 Mac 更新包")?;
    let expected = format!(
        "{base}/download/{}/HomeCraft-{}-build{}-macos-arm64.zip",
        manifest.tag, version, manifest.build
    );
    ensure!(
        asset.url == expected
            && asset.sha256.len() == 64
            && asset.sha256.bytes().all(|b| b.is_ascii_hexdigit())
            && (1..=MAX_ARCHIVE).contains(&asset.size),
        "更新包来源或校验信息无效"
    );
    if (version, manifest.build) <= (current, build) {
        return Ok(None);
    }
    Ok(Some(Release {
        version: manifest.version,
        build: manifest.build,
        notes: manifest.notes,
        asset: asset.clone(),
    }))
}

fn curl() -> Command {
    let mut c = Command::new("/usr/bin/curl");
    c.args([
        "--fail",
        "--silent",
        "--show-error",
        "--location",
        "--proto",
        "=https",
        "--proto-redir",
        "=https",
        "--connect-timeout",
        "15",
    ]);
    c.stdin(Stdio::null());
    c
}
fn check_release() -> Result<Option<Release>> {
    let out = curl()
        .args([
            "--max-time",
            "30",
            "--max-filesize",
            "262144",
            &manifest_url(),
        ])
        .output()?;
    if !out.status.success() {
        if String::from_utf8_lossy(&out.stderr).contains("404") {
            bail!("暂未发布可用更新，请稍后再试");
        }
        bail!("无法连接更新服务，请稍后重试");
    }
    ensure!(out.stdout.len() <= 262144, "更新清单过大");
    let os = Command::new("/usr/bin/sw_vers")
        .arg("-productVersion")
        .output()?;
    ensure!(os.status.success(), "无法核对系统版本");
    newer(
        serde_json::from_slice(&out.stdout)?,
        env!("CARGO_PKG_VERSION"),
        current_build(),
        String::from_utf8_lossy(&os.stdout).trim(),
    )
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buf = [0; 65536];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn verify_archive(path: &Path, asset: &Asset) -> Result<()> {
    ensure!(
        fs::metadata(path)?.len() == asset.size
            && sha256_file(path)?.eq_ignore_ascii_case(&asset.sha256),
        "更新包大小或 SHA-256 校验失败"
    );
    validate_zip(path)
}
pub fn validate_zip(path: &Path) -> Result<()> {
    let mut archive = zip::ZipArchive::new(fs::File::open(path)?)?;
    ensure!(archive.len() <= 2000, "更新包文件过多");
    let mut total = 0u64;
    let mut names = BTreeSet::new();
    for i in 0..archive.len() {
        let file = archive.by_index_raw(i)?;
        let name = file.name()?;
        ensure!(
            name.len() <= 1024
                && !name.contains('\\')
                && !name.chars().any(char::is_control)
                && name.starts_with("HomeCraft.app/")
                && file.enclosed_name().is_some()
                && !name.split('/').any(|p| matches!(p, ".." | "."))
                && !file.is_symlink()
                && !file.encrypted(),
            "更新包包含无效路径或符号链接"
        );
        ensure!(names.insert(name.to_string()), "更新包包含重复文件");
        // unix_mode() sanitizes permission bits in zip 9; ditto reads the original archive.
        ensure!(
            (file.external_attributes() >> 16) & 0o7000 == 0,
            "更新包包含特殊权限"
        );
        ensure!(
            file.unix_mode()
                .is_none_or(|m| m & 0o7000 == 0 && matches!(m & 0o170000, 0 | 0o100000 | 0o040000)),
            "更新包包含特殊文件"
        );
        total = total.checked_add(file.size()).context("更新包大小无效")?;
        ensure!(total <= MAX_UNPACKED, "更新包解压后过大");
        if name == "HomeCraft.app/Contents/Info.plist" {
            ensure!(file.size() <= 65536, "App 信息文件过大");
        }
    }
    ensure!(
        names.contains("HomeCraft.app/Contents/Info.plist")
            && names.contains("HomeCraft.app/Contents/MacOS/HomeCraft"),
        "更新包不含完整的 HomeCraft App"
    );
    Ok(())
}

fn bundle_info(app: &Path) -> Result<serde_json::Value> {
    let out = Command::new("/usr/bin/plutil")
        .args(["-convert", "json", "-o", "-"])
        .arg(app.join("Contents/Info.plist"))
        .output()?;
    ensure!(
        out.status.success() && out.stdout.len() <= 65536,
        "无法读取更新 App 信息"
    );
    Ok(serde_json::from_slice(&out.stdout)?)
}
fn validate_bundle(app: &Path, release: &Release) -> Result<()> {
    let info = bundle_info(app)?;
    ensure!(
        info["CFBundleIdentifier"] == "local.epaper.bluetooth"
            && info["CFBundleExecutable"] == "HomeCraft"
            && info["CFBundleShortVersionString"] == release.version
            && info["CFBundleVersion"]
                .as_str()
                .and_then(|s| s.parse::<u64>().ok())
                == Some(release.build),
        "更新 App 的身份或版本不匹配"
    );
    ensure!(
        fs::metadata(app.join("Contents/MacOS/HomeCraft"))?
            .permissions()
            .mode()
            & 0o111
            != 0,
        "更新程序不可执行"
    );
    let signature = Command::new("/usr/bin/codesign")
        .args(["--verify", "--deep", "--strict"])
        .arg(app)
        .output()?;
    ensure!(signature.status.success(), "更新 App 签名校验失败");
    let arch = Command::new("/usr/bin/lipo")
        .arg("-archs")
        .arg(app.join("Contents/MacOS/HomeCraft"))
        .output()?;
    ensure!(
        arch.status.success() && String::from_utf8_lossy(&arch.stdout).trim() == "arm64",
        "更新 App 架构不匹配"
    );
    Ok(())
}

fn download(release: Release, cancel: &AtomicBool, send: &impl Fn(Event)) -> Result<Staged> {
    ensure!(!cancel.load(Ordering::SeqCst), "下载已取消");
    let directory = StageDir::new()?;
    let archive = directory.path.join("update.zip");
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&archive)?;
    struct DownloadChild(std::process::Child);
    impl Drop for DownloadChild {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let mut child = DownloadChild(
        curl()
            .args([
                "--max-time",
                "600",
                "--max-filesize",
                &release.asset.size.to_string(),
                "--output",
            ])
            .arg(&archive)
            .arg(&release.asset.url)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?,
    );
    loop {
        let size = fs::metadata(&archive)?.len();
        if cancel.load(Ordering::SeqCst) || size > release.asset.size {
            let _ = child.0.kill();
            let _ = child.0.wait();
            bail!("下载已取消或大小超出预期");
        }
        if let Some(status) = child.0.try_wait()? {
            ensure!(status.success(), "更新下载失败，请重试");
            break;
        }
        send(Event::Progress(size, release.asset.size));
        std::thread::sleep(Duration::from_millis(150));
    }
    ensure!(!cancel.load(Ordering::SeqCst), "下载已取消");
    let staged = unpack_archive(directory, release, &archive)?;
    ensure!(!cancel.load(Ordering::SeqCst), "下载已取消");
    Ok(staged)
}

fn unpack_archive(directory: Arc<StageDir>, release: Release, archive: &Path) -> Result<Staged> {
    verify_archive(archive, &release.asset)?;
    let unpacked = directory.path.join("unpacked");
    fs::create_dir(&unpacked)?;
    ensure!(
        Command::new("/usr/bin/ditto")
            .args(["-x", "-k"])
            .arg(archive)
            .arg(&unpacked)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?
            .success(),
        "更新解压失败"
    );
    let staged = Staged { release, directory };
    validate_bundle(&staged.app(), &staged.release)?;
    Ok(staged)
}

/// Exercise the real update reader against local release files, without installing or networking.
pub fn verify_release_files(manifest: &Path, archive: &Path) -> Result<()> {
    let mut bytes = Vec::new();
    fs::File::open(manifest)?
        .take(262145)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= 262144, "更新清单过大");
    let os = Command::new("/usr/bin/sw_vers")
        .arg("-productVersion")
        .output()?;
    ensure!(os.status.success(), "无法核对系统版本");
    let release = newer(
        serde_json::from_slice(&bytes)?,
        "0.0.0",
        0,
        String::from_utf8_lossy(&os.stdout).trim(),
    )?
    .context("本地校验需要稳定版本清单")?;
    let staged = unpack_archive(StageDir::new()?, release, archive)?;
    println!(
        "更新包验证通过：{} build {}",
        staged.release.version, staged.release.build
    );
    Ok(())
}

pub fn install_location(executable: &Path, user: &Path) -> Option<PathBuf> {
    let target = executable.parent()?.parent()?.parent()?;
    (executable.ends_with("HomeCraft.app/Contents/MacOS/HomeCraft")
        && (target == Path::new("/Applications/HomeCraft.app")
            || target == user.join("Applications/HomeCraft.app")))
    .then(|| target.to_owned())
}
fn install(staged: &Staged, relaunch: bool) -> Result<()> {
    let user = PathBuf::from(std::env::var_os("HOME").context("找不到用户目录")?);
    let target = install_location(&std::env::current_exe()?, &user)
        .context("请将 HomeCraft 放入“应用程序”后使用 App 内安装，也可从发布页下载 DMG")?;
    ensure!(
        !fs::symlink_metadata(&target)?.file_type().is_symlink(),
        "应用位置不能是符号链接"
    );
    validate_bundle(&staged.app(), &staged.release)?;
    let parent = target.parent().context("安装位置无效")?;
    let probe = parent.join(format!(".homecraft-write-{:016x}", rand::random::<u64>()));
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&probe)
        .context("安装位置不可写，请下载 DMG 手动替换")?;
    fs::remove_file(probe)?;
    let helper = staged.directory.path.join("install.sh");
    fs::write(&helper, include_str!("../../tools/install_update.sh"))?;
    Command::new("/bin/sh")
        .arg(&helper)
        .arg(std::process::id().to_string())
        .arg(&target)
        .arg(staged.app())
        .arg(&staged.release.version)
        .arg(staged.release.build.to_string())
        .arg(if relaunch { "1" } else { "0" })
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("无法启动更新安装程序")?;
    staged.directory.keep.store(true, Ordering::SeqCst);
    Ok(())
}

enum Event {
    Checked(Option<Release>),
    Progress(u64, u64),
    Ready(Staged),
    Failed(String),
}
struct Job {
    rx: mpsc::Receiver<Event>,
    cancel: Arc<AtomicBool>,
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
}
fn job(
    wake: Arc<dyn Fn() + Send + Sync>,
    work: impl FnOnce(&AtomicBool, &dyn Fn(Event)) -> Result<Event> + Send + 'static,
) -> Job {
    let (tx, rx) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    let failure = tx.clone();
    if std::thread::Builder::new()
        .name("homecraft-update".into())
        .spawn(move || {
            let send = |event| {
                let _ = tx.send(event);
                wake();
            };
            let result = work(&flag, &send).unwrap_or_else(|e| Event::Failed(e.to_string()));
            send(result);
        })
        .is_err()
    {
        let _ = failure.send(Event::Failed("无法启动更新任务".into()));
    }
    Job { rx, cancel }
}

pub struct Updater {
    pub status: Status,
    pub show: bool,
    pub settings: Settings,
    enabled: bool,
    started: Instant,
    last_check: Option<Instant>,
    job: Option<Job>,
    automatic: bool,
    wake: Arc<dyn Fn() + Send + Sync>,
}
impl Updater {
    pub fn new(settings: Settings, enabled: bool, wake: Arc<dyn Fn() + Send + Sync>) -> Self {
        Self {
            status: Status::Idle,
            show: false,
            settings,
            enabled,
            started: Instant::now(),
            last_check: None,
            job: None,
            automatic: false,
            wake,
        }
    }
    pub fn configure(&mut self, settings: Settings) {
        if self.automatic && (!settings.auto_check || !settings.auto_download) {
            self.later();
        }
        self.settings = settings;
    }
    pub fn check(&mut self, interactive: bool) {
        if !self.enabled || self.status.busy() || matches!(self.status, Status::Ready(_)) {
            return;
        }
        self.show |= interactive;
        self.last_check = Some(Instant::now());
        self.automatic = !interactive;
        self.status = Status::Checking;
        self.job = Some(job(self.wake.clone(), |_, _| {
            check_release().map(Event::Checked)
        }));
    }
    pub fn download(&mut self) {
        let Status::Available(release) = &self.status else {
            return;
        };
        let release = release.clone();
        self.status = Status::Downloading {
            release: release.clone(),
            done: 0,
            total: release.asset.size,
        };
        self.job = Some(job(self.wake.clone(), move |cancel, send| {
            download(release, cancel, &send).map(Event::Ready)
        }));
    }
    pub fn later(&mut self) {
        self.job = None;
        self.status = Status::Idle;
        self.show = false;
        self.last_check = Some(Instant::now());
    }
    pub fn install(&mut self, relaunch: bool) -> Result<()> {
        if let Status::Ready(staged) = &self.status {
            install(staged, relaunch)?;
            self.status = Status::Idle;
        }
        Ok(())
    }
    pub fn tick(&mut self) {
        let events: Vec<_> = self
            .job
            .as_ref()
            .map(|j| j.rx.try_iter().collect())
            .unwrap_or_default();
        for event in events {
            match event {
                Event::Checked(found) => {
                    self.job = None;
                    match found {
                        Some(release) => {
                            self.status = Status::Available(release);
                            self.show = true;
                            if self.automatic && self.settings.auto_download {
                                self.download();
                            }
                        }
                        None => self.status = Status::Current,
                    }
                }
                Event::Progress(done, total) => {
                    if let Status::Downloading {
                        done: d, total: t, ..
                    } = &mut self.status
                    {
                        *d = done;
                        *t = total;
                    }
                }
                Event::Ready(staged) => {
                    self.job = None;
                    self.status = Status::Ready(staged);
                    self.show = true;
                }
                Event::Failed(message) => {
                    self.job = None;
                    self.status = Status::Failed(message);
                }
            }
        }
        if self.enabled
            && self.settings.auto_check
            && !self.status.busy()
            && !matches!(self.status, Status::Available(_) | Status::Ready(_))
            && self.started.elapsed() >= Duration::from_secs(3)
            && self
                .last_check
                .is_none_or(|at| at.elapsed() >= Duration::from_secs(6 * 3600))
        {
            self.check(false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn manifest() -> Manifest {
        let base = releases_url();
        serde_json::from_value(serde_json::json!({
            "schema":1,"version":"0.3.2","build":50,"tag":"v0.3.2","minimum_macos":"12.0",
            "notes":"更新说明","release_url":format!("{base}/tag/v0.3.2"),
            "platforms":{"macos-arm64":{"url":format!("{base}/download/v0.3.2/HomeCraft-0.3.2-build50-macos-arm64.zip"),"sha256":"a".repeat(64),"size":1024}}
        })).unwrap()
    }
    fn archive(entries: &[(&str, Option<u32>)]) -> (Arc<StageDir>, PathBuf) {
        let dir = StageDir::new().unwrap();
        let path = dir.path.join("test.zip");
        let mut zip = zip::ZipWriter::new(fs::File::create(&path).unwrap());
        for (name, mode) in entries {
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored)
                .unix_permissions(mode.unwrap_or(0o644));
            if mode == &Some(0o120777) {
                zip.add_symlink(*name, "elsewhere", options).unwrap();
            } else {
                zip.start_file(*name, options).unwrap();
                zip.write_all(b"synthetic fixture").unwrap();
            }
        }
        zip.finish().unwrap();
        (dir, path)
    }
    #[test]
    fn version_build_downgrade_and_platform_selection() {
        assert!(newer(manifest(), "0.3.1", 49, "12.0").unwrap().is_some());
        assert!(newer(manifest(), "0.3.2", 49, "26.0").unwrap().is_some());
        assert!(newer(manifest(), "0.3.2", 50, "26.0").unwrap().is_none());
        assert!(newer(manifest(), "0.4.0", 1, "26.0").unwrap().is_none());
        let mut m = manifest();
        m.version = "0.4.0-beta.1".into();
        assert!(newer(m, "0.3.1", 49, "26.0").unwrap().is_none());
        let mut m = manifest();
        m.platforms.clear();
        assert!(newer(m, "0.3.1", 49, "26.0").is_err());
    }
    #[test]
    fn feed_rejects_foreign_url_bad_hash_size_tag_and_newer_os() {
        for change in 0..7 {
            let mut m = manifest();
            let a = m.platforms.get_mut("macos-arm64").unwrap();
            match change {
                0 => a.url = "http://example.test/update.zip".into(),
                1 => a.sha256 = "bad".into(),
                2 => a.size = 0,
                3 => a.size = MAX_ARCHIVE + 1,
                4 => m.tag = "v9.0.0".into(),
                5 => m.release_url = "https://example.test".into(),
                _ => m.minimum_macos = "27.0".into(),
            }
            assert!(newer(m, "0.3.1", 49, "26.0").is_err(), "case {change}");
        }
    }
    #[test]
    fn checks_size_hash_and_required_archive_files_before_extracting() {
        let (_dir, path) = archive(&[
            ("HomeCraft.app/Contents/Info.plist", None),
            ("HomeCraft.app/Contents/MacOS/HomeCraft", None),
        ]);
        let mut a = Asset {
            url: String::new(),
            sha256: sha256_file(&path).unwrap(),
            size: fs::metadata(&path).unwrap().len(),
        };
        verify_archive(&path, &a).unwrap();
        a.size += 1;
        assert!(verify_archive(&path, &a).is_err());
        a.size -= 1;
        a.sha256 = "b".repeat(64);
        assert!(verify_archive(&path, &a).is_err());
        let (_dir, path) = archive(&[("HomeCraft.app/Contents/Info.plist", None)]);
        assert!(validate_zip(&path).is_err());
    }
    #[test]
    fn archive_cannot_escape_or_install_symlinks() {
        for name in [
            "../outside",
            "/absolute",
            "HomeCraft.app/../../outside",
            "Other.app/file",
            "HomeCraft.app/Contents/../elsewhere",
            "HomeCraft.app/Contents\\file",
        ] {
            let (_dir, path) = archive(&[(name, None)]);
            assert!(validate_zip(&path).is_err(), "{name}");
        }
        let (_dir, path) = archive(&[("HomeCraft.app/Contents/link", Some(0o120777))]);
        assert!(validate_zip(&path).is_err());
    }
    #[test]
    fn archive_cannot_preserve_privileged_permission_bits() {
        let (_dir, path) = archive(&[
            ("HomeCraft.app/Contents/Info.plist", None),
            ("HomeCraft.app/Contents/MacOS/HomeCraft", None),
        ]);
        let mut bytes = fs::read(&path).unwrap();
        let start = bytes.windows(4).position(|w| w == b"PK\x01\x02").unwrap();
        bytes[start + 38..start + 42].copy_from_slice(&(0o104755u32 << 16).to_le_bytes());
        fs::write(&path, bytes).unwrap();
        assert!(validate_zip(&path).is_err());
    }
    #[test]
    fn private_download_directory_is_leased_and_cleaned_after_last_view() {
        let dir = StageDir::new().unwrap();
        let path = dir.path.clone();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o700
        );
        let view = dir.clone();
        drop(dir);
        assert!(path.exists());
        drop(view);
        assert!(!path.exists());
    }
    #[test]
    fn only_known_installed_app_locations_can_be_replaced() {
        let user = Path::new("/Users/test");
        assert!(
            install_location(
                Path::new("/Applications/HomeCraft.app/Contents/MacOS/HomeCraft"),
                user
            )
            .is_some()
        );
        assert!(
            install_location(
                Path::new("/Users/test/Applications/HomeCraft.app/Contents/MacOS/HomeCraft"),
                user
            )
            .is_some()
        );
        for path in [
            "/tmp/HomeCraft.app/Contents/MacOS/HomeCraft",
            "/Applications/Other.app/Contents/MacOS/HomeCraft",
            "/repo/software/dist/HomeCraft.app/Contents/MacOS/HomeCraft",
        ] {
            assert!(install_location(Path::new(path), user).is_none());
        }
    }
    #[test]
    fn preview_and_disabled_auto_checks_never_start_network_work() {
        let mut u = Updater::new(Settings::default(), false, Arc::new(|| {}));
        u.started = Instant::now() - Duration::from_secs(10);
        u.check(true);
        u.tick();
        assert!(u.job.is_none());
        assert!(matches!(u.status, Status::Idle));
        let mut u = Updater::new(
            Settings {
                auto_check: false,
                auto_download: true,
            },
            true,
            Arc::new(|| {}),
        );
        u.started = Instant::now() - Duration::from_secs(10);
        u.tick();
        assert!(u.job.is_none());
    }
    #[test]
    fn cancelling_a_download_does_not_contact_the_server() {
        let release = newer(manifest(), "0.3.1", 49, "26.0").unwrap().unwrap();
        assert!(download(release, &AtomicBool::new(true), &|_| {}).is_err());
    }
    #[test]
    fn legacy_preferences_get_update_defaults_and_source_matches_release_metadata() {
        let prefs: crate::home::model::Preferences = serde_json::from_str("{}").unwrap();
        assert_eq!(prefs.updates, Settings::default());
        let meta: serde_json::Value =
            serde_json::from_str(include_str!("../../release.json")).unwrap();
        assert_eq!(meta["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(Some(current_build()), meta["build"].as_u64());
        assert_eq!(
            releases_url(),
            format!(
                "https://github.com/{}/releases",
                meta["repository"].as_str().unwrap()
            )
        );
    }
}
