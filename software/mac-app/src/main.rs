mod account;
mod argus;
mod backend;
mod breakdown;
mod cloud;
mod codex_cli;
mod config;
mod control;
mod device;
mod device_pages;
mod envlog;
mod feedback;
mod harness;
mod home;
mod hub;
mod poem;
mod preview;
mod refresh;
mod registry;
mod security;
mod todo;
mod ui;
mod update;
mod usage;
mod view_state;
mod weather;

use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    if std::env::args().any(|a| a == "--ui-control") {
        struct GpuLog;
        impl log::Log for GpuLog {
            fn enabled(&self, m: &log::Metadata) -> bool {
                m.target().starts_with("egui_wgpu") && m.level() <= log::Level::Warn
            }
            fn log(&self, r: &log::Record) {
                if self.enabled(r.metadata()) {
                    eprintln!("GPU: {}", r.args());
                }
            }
            fn flush(&self) {}
        }
        static LOGGER: GpuLog = GpuLog;
        let _ = log::set_logger(&LOGGER);
        log::set_max_level(log::LevelFilter::Warn);
    }
    let args: Vec<String> = std::env::args().collect();
    if let Some(index) = args.iter().position(|a| a == "--verify-update-files") {
        let manifest = args
            .get(index + 1)
            .ok_or_else(|| anyhow::anyhow!("需要更新清单路径"))?;
        let archive = args
            .get(index + 2)
            .ok_or_else(|| anyhow::anyhow!("需要更新 ZIP 路径"))?;
        return update::verify_release_files(
            std::path::Path::new(manifest),
            std::path::Path::new(archive),
        );
    }
    if let Some(index) = args.iter().position(|a| a == "--render-ui-preview") {
        let directory = args
            .get(index + 1)
            .ok_or_else(|| anyhow::anyhow!("需要预览输出目录"))?;
        return tokio::runtime::Runtime::new()?
            .block_on(preview::render(std::path::Path::new(directory)));
    }
    if let Some(i) = args.iter().position(|a| a == "--use-data-directory") {
        let path = args
            .get(i + 1)
            .ok_or_else(|| anyhow::anyhow!("请选择已有配置目录"))?;
        config::use_data_directory(std::path::Path::new(path))?;
        println!("已保留本机原配置与设备绑定。");
        return Ok(());
    }
    let root = config::project_root()?;
    if let Some(i) = args.iter().position(|x| x == "--read-weather") {
        let mut settings = weather::Settings::default();
        if let Some(city) = args.get(i + 1).filter(|a| !a.starts_with("--")) {
            settings.city = city.clone();
        }
        let snapshot =
            tokio::runtime::Runtime::new()?.block_on(weather::fetch(&root, &settings))?;
        println!("{}", serde_json::to_string(&snapshot)?);
        return Ok(());
    }
    if args.iter().any(|x| x == "--read-harness") {
        let reading = tokio::runtime::Runtime::new()?.block_on(harness::collect(&root));
        println!("{}", serde_json::to_string(&reading)?);
        return Ok(());
    }
    if args.iter().any(|x| x == "--diagnose-codex") {
        let rt = tokio::runtime::Runtime::new()?;
        let report = rt.block_on(codex_cli::diagnose(&root, None));
        println!("{}", serde_json::to_string(&report)?);
        return Ok(());
    }
    if args.iter().any(|x| x == "--claude-statusline") {
        return usage::statusline(&root);
    }
    if args.iter().any(|x| x == "--install-claude-statusline") {
        return usage::install_statusline();
    }
    if args.iter().any(|x| x == "--read-usage") {
        let rt = tokio::runtime::Runtime::new()?;
        let packets = rt.block_on(usage::collect(&root));
        println!("{}", serde_json::to_string(&packets)?);
        return Ok(());
    }
    if args.iter().any(|x| x == "--refresh-codex") {
        let rt = tokio::runtime::Runtime::new()?;
        let reading = rt.block_on(usage::collect_codex(&root, true));
        println!("{}", serde_json::to_string(&reading)?);
        return Ok(());
    }
    if args.iter().any(|x| x == "--smoke") {
        return tokio::runtime::Runtime::new()?.block_on(backend::smoke(&root));
    }
    let control = args
        .iter()
        .position(|a| a == "--ui-control")
        .map(|i| {
            args.get(i + 1)
                .map(PathBuf::from)
                .ok_or_else(|| anyhow::anyhow!("--ui-control 需要目录"))
        })
        .transpose()?;
    ui::run(
        root,
        ui::Options {
            background: args.iter().any(|x| x == "--login-item"),
            auto_connect: args.iter().any(|x| x == "--connect"),
            preview: args.iter().any(|x| x == "--ui-preview"),
            control,
        },
    )
}
