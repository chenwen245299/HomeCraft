//! Opt-in local test channel, following TexCraft's screenshot/inspect workflow.
//! Private Unix socket; no TCP listener, shell execution, credentials or arbitrary file reads.
use anyhow::{Result, ensure};
use eframe::egui;
use serde_json::Value;
use std::{
    fs,
    io::{BufRead, BufReader, Read, Write},
    os::unix::{fs::PermissionsExt, net::UnixListener},
    path::{Path, PathBuf},
    sync::mpsc,
    time::Duration,
};
pub struct Request {
    pub value: Value,
    pub reply: mpsc::Sender<Value>,
}
pub struct Control {
    pub rx: mpsc::Receiver<Request>,
    pub directory: PathBuf,
}
impl Control {
    pub fn start(directory: &Path, ctx: egui::Context) -> Result<Self> {
        ensure!(directory.is_absolute(), "控制通道目录必须是绝对路径");
        fs::create_dir_all(directory)?;
        fs::set_permissions(directory, fs::Permissions::from_mode(0o700))?;
        let directory = fs::canonicalize(directory)?;
        let path = directory.join("control.sock");
        // Existing sockets are not replaced: no way to hijack a running app's control channel.
        let listener = UnixListener::bind(path)?;
        let (tx, rx) = mpsc::channel::<Request>();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
                let mut input = String::new();
                if BufReader::new((&mut stream).take(16385))
                    .read_line(&mut input)
                    .is_err()
                    || input.len() > 16384
                {
                    continue;
                }
                let Ok(value) = serde_json::from_str::<Value>(&input) else {
                    continue;
                };
                let (reply, result) = mpsc::channel();
                if tx.send(Request { value, reply }).is_err() {
                    break;
                }
                ctx.request_repaint();
                if let Ok(answer) = result.recv_timeout(Duration::from_secs(20))
                    && let Ok(mut bytes) = serde_json::to_vec(&answer)
                {
                    bytes.push(b'\n');
                    let _ = stream.write_all(&bytes);
                }
            }
        });
        Ok(Self { rx, directory })
    }
    pub fn image_path(&self, name: &str) -> Result<PathBuf> {
        ensure!(
            !name.is_empty()
                && name.len() <= 120
                && !name.contains('/')
                && !name.contains('\\')
                && name.ends_with(".png"),
            "截图只允许控制目录中的PNG文件名"
        );
        let path = self.directory.join(name);
        ensure!(!path.exists(), "截图已存在，请使用新名称");
        Ok(path)
    }
}
impl Drop for Control {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.directory.join("control.sock"));
    }
}
