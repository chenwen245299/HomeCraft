use super::{
    Home, Intent, secondary,
    theme::{self, Tokens, bold},
};
use crate::update::Status;
use eframe::egui::{self, RichText};

impl Home {
    pub(super) fn update_settings(
        &mut self,
        ui: &mut egui::Ui,
        t: Tokens,
        intents: &mut Vec<Intent>,
    ) {
        ui.label(
            RichText::new(format!(
                "HomeCraft {} · build {}",
                env!("CARGO_PKG_VERSION"),
                crate::update::current_build()
            ))
            .font(bold(17.))
            .color(t.ink),
        );
        ui.label(RichText::new(self.updates.summary()).color(t.secondary));
        if let Status::Failed(message) = &self.updates {
            egui::CollapsingHeader::new("查看详情").show(ui, |ui| {
                ui.label(RichText::new(message).color(t.error));
            });
        }
        if let Some(release) = self.updates.release() {
            ui.add_space(10.);
            ui.label(
                RichText::new(format!(
                    "新版本 {} · build {}",
                    release.version, release.build
                ))
                .font(bold(19.))
                .color(t.ink),
            );
            theme::surface(t).show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("update-notes")
                    .max_height(210.)
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        for line in release.notes.lines() {
                            if line.trim().is_empty() {
                                ui.add_space(5.);
                            } else if line.starts_with('#') {
                                ui.label(
                                    RichText::new(line.trim_start_matches('#').trim())
                                        .font(bold(14.))
                                        .color(t.ink),
                                );
                            } else {
                                ui.label(
                                    RichText::new(line.replacen("- ", "• ", 1))
                                        .size(13.)
                                        .color(t.secondary),
                                );
                            }
                        }
                    });
            });
        }
        if let Status::Downloading { done, total, .. } = &self.updates {
            ui.add_space(8.);
            ui.add(
                egui::ProgressBar::new(*done as f32 / (*total).max(1) as f32)
                    .fill(t.accent)
                    .text(format!(
                        "{:.1} / {:.1} MB",
                        *done as f64 / 1e6,
                        *total as f64 / 1e6
                    )),
            );
        }
        if matches!(self.updates, Status::Ready(_)) {
            ui.label(
                RichText::new("下载和校验已完成。可立即重启安装，或在退出 HomeCraft 时自动安装。")
                    .size(12.)
                    .color(t.secondary),
            );
        }
        ui.add_space(8.);
        ui.horizontal_wrapped(|ui| {
            let check = ui.add_enabled(
                !self.updates.busy() && !matches!(self.updates, Status::Ready(_)),
                theme::secondary_button("检查更新", t),
            );
            self.target("update.check", check.rect);
            if check.clicked() {
                intents.push(Intent::CheckUpdates);
            }
            if matches!(self.updates, Status::Available(_)) {
                let download = ui.add(theme::primary_button("下载更新", t));
                self.target("update.download", download.rect);
                if download.clicked() {
                    intents.push(Intent::DownloadUpdate);
                }
            }
            if matches!(self.updates, Status::Ready(_)) {
                let install = ui.add(theme::primary_button("安装并重启", t));
                self.target("update.install", install.rect);
                if install.clicked() {
                    intents.push(Intent::InstallUpdate);
                }
            }
            if self.updates.release().is_some() {
                let later = secondary(ui, "跳过本次", t);
                self.target("update.later", later.rect);
                if later.clicked() {
                    intents.push(Intent::LaterUpdate);
                }
            }
            let website = secondary(ui, "发布页面", t);
            self.target("update.releases", website.rect);
            if website.clicked() {
                intents.push(Intent::OpenReleases);
            }
        });
        theme::section(ui, "更新设置", t);
        let auto = ui.checkbox(&mut self.prefs.updates.auto_check, "自动检查更新");
        self.target("update.auto_check", auto.rect);
        let download = ui.checkbox(&mut self.prefs.updates.auto_download, "发现新版后自动下载");
        self.target("update.auto_download", download.rect);
        ui.label(
            RichText::new(
                "启动后及每 6 小时检查一次；只更新 App，保留设置和设备绑定。硬件固件单独升级。",
            )
            .size(12.)
            .color(t.secondary),
        );
        let save = ui.add_enabled(
            self.preferences_changed() && self.prefs.validate(),
            theme::primary_button("保存更新设置", t),
        );
        self.target("update.save", save.rect);
        if save.clicked() {
            intents.push(Intent::SavePreferences);
        }
    }
}
