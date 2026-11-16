//! Resolve the user's installed CLI independently of Finder's reduced PATH.
//! Never install a CLI, read login tokens, or execute a user-entered path through a shell.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsString,
    io::Read,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Stdio,
    sync::OnceLock,
    time::{Duration, Instant},
};
use tokio::{io::AsyncReadExt, process::Command};

#[derive(Clone, Default, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub codex_cli: String,
    pub codex_node: String,
}
#[derive(Clone, Default, Serialize)]
pub struct Diagnostic {
    pub executable: String,
    pub node: String,
    pub version: String,
    pub source: String,
    pub message: String,
    pub ok: bool,
}
pub struct Launch {
    pub executable: PathBuf,
    node: Option<PathBuf>,
    script: bool,
    path: OsString,
    source: &'static str,
}
impl Launch {
    pub fn command(&self) -> Command {
        let mut command = if self.script {
            let mut c = Command::new(self.node.as_ref().unwrap_or(&self.executable));
            c.arg(&self.executable);
            c
        } else {
            Command::new(&self.executable)
        };
        command.env("PATH", &self.path).kill_on_drop(true);
        command
    }
}
#[derive(Clone, Default)]
struct ShellPaths {
    cli: Option<PathBuf>,
    node: Option<PathBuf>,
    path: OsString,
}
struct Search {
    home: PathBuf,
    path: OsString,
    extra: Vec<PathBuf>,
    override_cli: Option<OsString>,
    override_node: Option<OsString>,
}
fn executable(path: &Path) -> bool {
    path.is_absolute()
        && std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}
fn expand(text: &str, home: &Path) -> PathBuf {
    text.strip_prefix("~/")
        .map(|tail| home.join(tail))
        .unwrap_or_else(|| PathBuf::from(text))
}
pub fn valid_setting(text: &str) -> bool {
    text.is_empty()
        || (text.len() <= 4096
            && !text.chars().any(char::is_control)
            && (Path::new(text).is_absolute() || text.starts_with("~/")))
}
pub fn cache_key(root: &Path) -> [OsString; 4] {
    let settings: Settings =
        crate::config::read_private(&root.join(".home-ui.json")).unwrap_or_default();
    [
        settings.codex_cli.into(),
        settings.codex_node.into(),
        std::env::var_os("EPAPER_CODEX").unwrap_or_default(),
        std::env::var_os("EPAPER_NODE").unwrap_or_default(),
    ]
}
fn node_script(path: &Path) -> bool {
    let mut bytes = Vec::new();
    if let Ok(file) = std::fs::File::open(path) {
        let _ = file.take(256).read_to_end(&mut bytes);
    }
    let line = bytes.split(|b| *b == b'\n').next().unwrap_or(&[]);
    line.starts_with(b"#!")
        && line
            .split(|b| b.is_ascii_whitespace() || *b == b'/')
            .any(|word| word == b"node")
}
fn in_path(path: &std::ffi::OsStr, name: &str) -> Option<PathBuf> {
    std::env::split_paths(path)
        .filter(|d| d.is_absolute()) // Never search cwd / writable project folders implicitly.
        .map(|d| d.join(name))
        .find(|p| executable(p))
}
fn managers(home: &Path) -> Vec<PathBuf> {
    let mut bins = Vec::new();
    for parent in [
        home.join(".nvm/versions/node"),
        home.join(".local/share/fnm/node-versions"),
        home.join("Library/Application Support/fnm/node-versions"),
        home.join("Library/pnpm/nodejs"),
    ] {
        let mut versions = std::fs::read_dir(parent)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .take(128)
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect::<Vec<_>>();
        versions.sort_by_cached_key(|p| {
            p.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .split(|c: char| !c.is_ascii_digit())
                .filter_map(|part| part.parse::<u32>().ok())
                .collect::<Vec<_>>()
        });
        for version in versions.into_iter().rev() {
            bins.push(version.join("bin"));
            bins.push(version.join("installation/bin"));
        }
    }
    bins
}
fn select(settings: &Settings, search: &Search, shell: &ShellPaths) -> Result<Launch> {
    ensure!(
        valid_setting(&settings.codex_cli) && valid_setting(&settings.codex_node),
        "CLI路径请填写绝对路径或 ~/ 开头的路径，不要填写整条命令。"
    );
    let mut dirs = vec![
        search.home.join(".local/bin"),
        search.home.join(".npm-global/bin"),
        search.home.join(".npm/bin"),
        search.home.join("Library/pnpm"),
        search.home.join(".volta/bin"),
        search.home.join(".asdf/shims"),
        search.home.join(".local/share/mise/shims"),
        search.home.join(".bun/bin"),
    ];
    dirs.extend(managers(&search.home));
    dirs.extend(search.extra.iter().cloned());
    let (cli, source) = if let Some(path) = search.override_cli.as_ref().filter(|p| !p.is_empty()) {
        (
            expand(&path.to_string_lossy(), &search.home),
            "EPAPER_CODEX",
        )
    } else if !settings.codex_cli.is_empty() {
        (expand(&settings.codex_cli, &search.home), "手动选择")
    } else if let Some(path) = in_path(&search.path, "codex") {
        (path, "应用 PATH")
    } else if let Some(path) = shell.cli.as_ref().filter(|p| executable(p)) {
        (path.clone(), "终端登录环境")
    } else if let Some(path) = in_path(&shell.path, "codex") {
        (path, "终端 PATH")
    } else {
        (
            dirs.iter()
                .map(|d| d.join("codex"))
                .find(|p| executable(p))
                .context("未找到 Codex CLI；请在设置 → Codex CLI 中重新检测或选择可执行文件。")?,
            "常用安装目录",
        )
    };
    ensure!(
        executable(&cli),
        "所选 Codex CLI 不存在或不可执行；请重新选择文件。"
    );
    let script = node_script(&cli);
    let node = if let Some(path) = search.override_node.as_ref().filter(|p| !p.is_empty()) {
        let p = expand(&path.to_string_lossy(), &search.home);
        ensure!(executable(&p), "EPAPER_NODE 指定的文件不可执行。");
        Some(p)
    } else if !settings.codex_node.is_empty() {
        let p = expand(&settings.codex_node, &search.home);
        ensure!(executable(&p), "所选 Node.js 不存在或不可执行。");
        Some(p)
    } else {
        cli.parent()
            .map(|p| p.join("node"))
            .filter(|p| executable(p))
            .or_else(|| in_path(&search.path, "node"))
            .or_else(|| shell.node.clone().filter(|p| executable(p)))
            .or_else(|| in_path(&shell.path, "node"))
            .or_else(|| dirs.iter().map(|d| d.join("node")).find(|p| executable(p)))
    };
    ensure!(
        !script || node.is_some(),
        "已找到 Codex 的 npm 启动脚本，但未找到 Node.js；请在设置中选择对应的 node 文件。"
    );
    let mut paths = Vec::new();
    if let Some(parent) = node.as_ref().and_then(|n| n.parent()) {
        paths.push(parent.to_path_buf());
    }
    if let Some(parent) = cli.parent() {
        paths.push(parent.to_path_buf());
    }
    paths.extend(std::env::split_paths(&search.path));
    paths.extend(std::env::split_paths(&shell.path));
    paths.extend(dirs);
    let mut seen = std::collections::HashSet::new();
    paths.retain(|p| p.is_absolute() && seen.insert(p.clone()));
    Ok(Launch {
        executable: cli,
        node,
        script,
        path: std::env::join_paths(paths)?,
        source,
    })
}
fn parse_shell(bytes: &[u8]) -> Option<ShellPaths> {
    let fields = bytes.split(|b| *b == 0).collect::<Vec<_>>();
    let start = fields.iter().position(|f| *f == b"HOMECRAFT_CLI_BEGIN")?;
    let values = fields.get(start + 1..start + 5)?;
    if values[3] != b"HOMECRAFT_CLI_END" {
        return None;
    }
    let text = |value: &[u8]| {
        String::from_utf8_lossy(value)
            .trim_end_matches(['\r', '\n'])
            .to_string()
    };
    let cli = PathBuf::from(text(values[0]));
    let node = PathBuf::from(text(values[1]));
    Some(ShellPaths {
        cli: executable(&cli).then_some(cli),
        node: executable(&node).then_some(node),
        path: OsString::from(text(values[2])),
    })
}
async fn shell_paths(home: &Path, force: bool) -> ShellPaths {
    type Cache = Option<(Instant, PathBuf, ShellPaths)>;
    static CACHE: OnceLock<tokio::sync::Mutex<Cache>> = OnceLock::new();
    let mut cached = CACHE
        .get_or_init(|| tokio::sync::Mutex::new(None))
        .lock()
        .await;
    if !force
        && let Some((at, key, paths)) = &*cached
        && key == home
        && at.elapsed() < Duration::from_secs(600)
    {
        return paths.clone();
    }
    // Static command only; no path, credentials or GUI text is interpolated into the shell.
    // Interactive login startup loads nvm/fnm/pnpm initialization just like the user's terminal.
    let shell = std::env::var_os("SHELL")
        .map(PathBuf::from)
        .filter(|p| {
            executable(p) && matches!(p.file_name().and_then(|s| s.to_str()), Some("zsh" | "bash"))
        })
        .unwrap_or_else(|| PathBuf::from("/bin/zsh"));
    let script = r#"printf '\0%s\0%s\0%s\0%s\0%s\0' HOMECRAFT_CLI_BEGIN "$(command -v codex)" "$(command -v node)" "$PATH" HOMECRAFT_CLI_END"#;
    let result: Result<ShellPaths> = async {
        let mut child = Command::new(shell)
            .args(["-l", "-i", "-c", script])
            .current_dir(home)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()?;
        let mut output = Vec::new();
        let stdout = child.stdout.take().context("终端路径检测未响应")?;
        stdout.take(65537).read_to_end(&mut output).await?;
        ensure!(output.len() <= 65536, "终端启动输出过长");
        let _ = child.wait().await?;
        parse_shell(&output).context("终端路径检测未返回有效结果")
    }
    .await;
    // Bound the entire login-shell operation, not just process spawning.
    let paths = result.unwrap_or_default();
    *cached = Some((Instant::now(), home.to_path_buf(), paths.clone()));
    paths
}
pub async fn resolve(
    root: &Path,
    override_settings: Option<Settings>,
    force: bool,
) -> Result<Launch> {
    let settings = override_settings.unwrap_or_else(|| {
        crate::config::read_private(&root.join(".home-ui.json")).unwrap_or_default()
    });
    let home = PathBuf::from(std::env::var_os("HOME").context("找不到用户目录")?);
    let mut extra = vec![
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
        PathBuf::from("/usr/bin"),
        PathBuf::from("/bin"),
        PathBuf::from("/usr/sbin"),
        PathBuf::from("/sbin"),
    ];
    if let Some(dir) = std::env::var_os("CODEX_INSTALL_DIR") {
        extra.insert(0, PathBuf::from(dir));
    }
    let search = Search {
        home: home.clone(),
        path: std::env::var_os("PATH").unwrap_or_default(),
        extra,
        override_cli: std::env::var_os("EPAPER_CODEX"),
        override_node: std::env::var_os("EPAPER_NODE"),
    };
    // Executing the login shell is a fallback. Most launches resolve from PATH or an
    // already-selected file; only the first missing/Node-dependent lookup needs it.
    let empty = ShellPaths::default();
    if !force && let Ok(launch) = select(&settings, &search, &empty) {
        return Ok(launch);
    }
    let shell = tokio::time::timeout(Duration::from_secs(4), shell_paths(&home, force))
        .await
        .unwrap_or_default();
    select(&settings, &search, &shell)
}
pub async fn diagnose(root: &Path, settings: Option<Settings>) -> Diagnostic {
    let launch = match resolve(root, settings, true).await {
        Ok(launch) => launch,
        Err(e) => {
            return Diagnostic {
                message: e.to_string(),
                ..Default::default()
            };
        }
    };
    let mut report = Diagnostic {
        executable: launch.executable.to_string_lossy().into(),
        node: launch
            .node
            .as_ref()
            .map(|p| p.to_string_lossy().into())
            .unwrap_or_default(),
        source: launch.source.into(),
        ..Default::default()
    };
    let result: Result<String> = tokio::time::timeout(Duration::from_secs(6), async {
        let mut child = launch
            .command()
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("CLI文件已找到，但无法启动")?;
        let mut bytes = Vec::new();
        child
            .stdout
            .take()
            .context("CLI无版本输出")?
            .take(4097)
            .read_to_end(&mut bytes)
            .await?;
        ensure!(
            bytes.len() <= 4096 && child.wait().await?.success(),
            "CLI文件已找到，但运行失败；请检查对应Node.js或文件权限"
        );
        let line = String::from_utf8_lossy(&bytes)
            .lines()
            .find(|line| line.starts_with("codex-cli "))
            .map(str::to_owned)
            .context("所选程序未返回官方Codex CLI版本信息")?;
        Ok(line)
    })
    .await
    .unwrap_or_else(|_| {
        Err(anyhow::anyhow!(
            "CLI文件已找到，但启动超时；请检查对应的Node.js。"
        ))
    });
    match result {
        Ok(version) => {
            report.version = version;
            report.ok = true;
            report.message = "已找到并成功启动 Codex CLI；此检测不查询账户或调用模型。".into();
        }
        Err(e) => report.message = e.to_string(),
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root =
                std::env::temp_dir().join(format!("homecraft-cli-{:016x}", rand::random::<u64>()));
            std::fs::create_dir_all(&root).unwrap();
            Self(root)
        }
        fn file(&self, name: &str, content: &str) -> PathBuf {
            let p = self.0.join(name);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, content).unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o700)).unwrap();
            p
        }
        fn search(&self) -> Search {
            Search {
                home: self.0.clone(),
                path: OsString::from("/usr/bin:/bin"),
                extra: vec![],
                override_cli: None,
                override_node: None,
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn finder_path_finds_standalone_install_without_homebrew() {
        let f = Fixture::new();
        let cli = f.file(".local/bin/codex", "#!/bin/sh\nexit 0\n");
        let launch = select(&Settings::default(), &f.search(), &ShellPaths::default()).unwrap();
        assert_eq!(launch.executable, cli);
        assert!(!launch.script);
    }
    #[test]
    fn finder_path_finds_nvm_cli_and_matching_node_through_symlink() {
        let f = Fixture::new();
        let script = f.file(
            ".nvm/versions/node/v24.1.0/lib/node_modules/@openai/codex/bin/codex.js",
            "#!/usr/bin/env node\n// fixture\n",
        );
        let node = f.file(".nvm/versions/node/v24.1.0/bin/node", "#!/bin/sh\nexit 0\n");
        let cli = node.parent().unwrap().join("codex");
        std::os::unix::fs::symlink(script, &cli).unwrap();
        let launch = select(&Settings::default(), &f.search(), &ShellPaths::default()).unwrap();
        assert_eq!(launch.executable, cli);
        assert_eq!(launch.node.as_ref(), Some(&node));
        assert!(launch.script);
        let command = launch.command();
        assert_eq!(command.as_std().get_program(), node.as_os_str());
        assert_eq!(command.as_std().get_args().next(), Some(cli.as_os_str()));
    }
    #[test]
    fn pnpm_global_launcher_is_found_from_gui() {
        let f = Fixture::new();
        let cli = f.file("Library/pnpm/codex", "#!/bin/sh\nexit 0\n");
        assert_eq!(
            select(&Settings::default(), &f.search(), &ShellPaths::default())
                .unwrap()
                .executable,
            cli
        );
    }
    #[test]
    fn explicit_path_with_spaces_is_one_executable_not_shell_text() {
        let f = Fixture::new();
        let cli = f.file("My tools/codex $(literal)", "#!/bin/sh\nexit 0\n");
        let s = Settings {
            codex_cli: cli.to_string_lossy().into(),
            ..Default::default()
        };
        let launch = select(&s, &f.search(), &ShellPaths::default()).unwrap();
        assert_eq!(launch.command().as_std().get_program(), cli.as_os_str());
        assert_eq!(launch.command().as_std().get_args().count(), 0);
        assert!(!valid_setting("codex --some-argument"));
    }
    #[test]
    fn missing_node_is_distinct_from_missing_cli() {
        let f = Fixture::new();
        let cli = f.file("cli/codex", "#!/usr/bin/env node\n");
        let s = Settings {
            codex_cli: cli.to_string_lossy().into(),
            ..Default::default()
        };
        let error = select(&s, &f.search(), &ShellPaths::default())
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains("Node.js"));
        assert!(!error.contains("未找到 Codex CLI"));
    }
    #[test]
    fn invalid_selected_file_does_not_silently_launch_another_installation() {
        let f = Fixture::new();
        f.file(".local/bin/codex", "#!/bin/sh\nexit 0\n");
        let s = Settings {
            codex_cli: f.0.join("missing").to_string_lossy().into(),
            ..Default::default()
        };
        assert!(select(&s, &f.search(), &ShellPaths::default()).is_err());
    }
    #[test]
    fn parse_login_output_ignores_startup_chatter_and_keeps_path_spaces() {
        let f = Fixture::new();
        let cli = f.file("CLI tools/codex", "#!/bin/sh\n");
        let text = format!(
            "fixture startup message\n\0HOMECRAFT_CLI_BEGIN\0{}\0\0/usr/bin:/bin\0HOMECRAFT_CLI_END\0",
            cli.display()
        );
        let probe = parse_shell(text.as_bytes()).unwrap();
        assert_eq!(probe.cli, Some(cli));
        assert_eq!(probe.path, OsString::from("/usr/bin:/bin"));
        assert!(parse_shell(b"plain unrelated output").is_none());
    }
    #[test]
    fn manual_node_override_and_tilde_paths_work() {
        let f = Fixture::new();
        let cli = f.file("tools/codex", "#!/usr/bin/env node\n");
        let node = f.file("runtime/node", "#!/bin/sh\nexit 0\n");
        let s = Settings {
            codex_cli: "~/tools/codex".into(),
            codex_node: "~/runtime/node".into(),
        };
        let launch = select(&s, &f.search(), &ShellPaths::default()).unwrap();
        assert_eq!(launch.executable, cli);
        assert_eq!(launch.node, Some(node));
    }
}
