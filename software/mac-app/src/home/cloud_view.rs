//! Concise cloud controls and source-aware history; shares the normal glass theme.
use super::{
    Home, Intent, chart, model, secondary,
    theme::{self, Tokens, bold},
};
use crate::{backend::State, cloud};
use eframe::egui::{self, RichText};

fn at(time: i64) -> String {
    chrono::DateTime::from_timestamp(time, 0)
        .map(|t| {
            t.with_timezone(&chrono::Local)
                .format("%m/%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|| "—".into())
}
impl Home {
    pub(super) fn cloud_settings(
        &mut self,
        ui: &mut egui::Ui,
        state: &State,
        t: Tokens,
        intents: &mut Vec<Intent>,
    ) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("iCloud Drive").font(bold(17.)).color(t.ink));
            if state.cloud.busy {
                ui.add(egui::Spinner::new().size(14.));
            }
            let text = if state.cloud.busy {
                "同步中…"
            } else if state.cloud.message.is_empty() {
                "未启用"
            } else {
                &state.cloud.message
            };
            ui.add(
                egui::Label::new(RichText::new(text).size(12.).color(if state.cloud.error {
                    t.error
                } else {
                    t.secondary
                }))
                .truncate(),
            )
            .on_hover_text("上传下载由 macOS 完成，文件写入不代表另一台 Mac 已收到。");
        });
        let stamp = state.cloud.last.map(|last| format!("写入于 {}", at(last)));
        theme::action_row(
            ui,
            stamp.as_ref().map(|s| (s.as_str(), "最后一次文件交换时间")),
            t,
            |ui| {
                let choose = secondary(
                    ui,
                    if state.cloud.folder.is_some() {
                        "更换文件夹"
                    } else {
                        "选择同步文件夹"
                    },
                    t,
                );
                self.target("cloud.choose", choose.rect);
                let choose = choose.on_hover_text(
                    state
                        .cloud
                        .folder
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "选择 iCloud Drive 中的 HomeCraft 文件夹".into()),
                );
                if choose.clicked() {
                    intents.push(Intent::PickCloud);
                }
                if state.cloud.folder.is_some() {
                    let sync = ui.add_enabled(!state.cloud.busy, theme::primary_button("同步", t));
                    self.target("cloud.sync", sync.rect);
                    if sync.clicked() {
                        intents.push(Intent::Cloud(cloud::Command::Sync));
                    }
                    let pause = secondary(ui, "暂停", t);
                    self.target("cloud.pause", pause.rect);
                    if pause.clicked() {
                        intents.push(Intent::Cloud(cloud::Command::Configure(None)));
                    }
                }
            },
        );
        ui.add_space(14.);
        ui.label(RichText::new("汇总来源").font(bold(17.)).color(t.ink));
        ui.label(
            RichText::new("各台 Mac 需运行新版 HomeCraft，并选择同一个同步文件夹。")
                .size(12.)
                .color(t.secondary),
        );
        let mut latest = std::collections::BTreeMap::new();
        for s in &state.cloud_data.snapshots {
            for u in &s.usages {
                let entry = latest
                    .entry(s.origin.id.clone())
                    .or_insert((&s.origin, u.packet.t));
                entry.1 = entry.1.max(u.packet.t);
            }
        }
        for (id, (origin, time)) in latest {
            let stale = crate::usage::now() - time > 600;
            ui.label(
                RichText::new(format!(
                    "{}{} · {}{}",
                    origin.name,
                    if state.cloud.origin.as_ref() == Some(&id) {
                        "（本机）"
                    } else {
                        ""
                    },
                    at(time),
                    if stale {
                        " · 尚未收到近期数据"
                    } else {
                        ""
                    }
                ))
                .size(12.)
                .color(if stale { t.secondary } else { t.ink }),
            );
        }
        ui.add_space(14.);
        let accounts = state.cloud_data.accounts(&state.readings);
        ui.label(RichText::new("Codex 账号").font(bold(17.)).color(t.ink));
        if accounts.is_empty() {
            ui.label(RichText::new("刷新 Codex 后显示").color(t.secondary));
        }
        for s in accounts {
            let Some(a) = &s.account else {
                continue;
            };
            theme::surface(t).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(&a.label).font(bold(16.)).color(t.ink))
                        .on_hover_text(format!(
                            "标识 {} · {}",
                            &a.key[..8.min(a.key.len())],
                            if a.verified {
                                "官方账号"
                            } else {
                                "未提供邮箱，按设备区分"
                            }
                        ));
                    if !a.plan.is_empty() {
                        ui.label(RichText::new(&a.plan).size(12.).color(t.secondary));
                    }
                    ui.label(
                        RichText::new(format!("{} · {}", a.device.name, at(s.packet.t)))
                            .size(12.)
                            .color(t.secondary),
                    );
                });
                ui.columns(3, |cols| {
                    for (c, (label, value)) in cols.iter_mut().zip([
                        ("5 小时", quota(&s.packet, 0)),
                        ("周额度", quota(&s.packet, 1)),
                        (
                            "今日 Token",
                            model::compact(
                                s.packet
                                    .d
                                    .filter(|d| d[0] == model::day() && d[1] >= 0)
                                    .map(|d| d[1] as u64),
                            ),
                        ),
                    ]) {
                        c.label(RichText::new(label).size(12.).color(t.secondary));
                        c.label(RichText::new(value).font(bold(20.)).color(t.ink));
                    }
                });
            });
            ui.add_space(8.);
        }
        ui.add_space(10.);
        ui.label(RichText::new("Mac 与历史").font(bold(17.)).color(t.ink));
        let mut sources = std::collections::BTreeMap::new();
        for s in &state.cloud_data.snapshots {
            sources.insert(s.origin.id.clone(), s.origin.name.clone());
        }
        if sources.is_empty() {
            ui.label(RichText::new("暂无记录").color(t.secondary));
        } else {
            if self
                .cloud_source
                .as_ref()
                .is_none_or(|id| !sources.contains_key(id))
            {
                self.cloud_source = sources.keys().next().cloned();
            }
            ui.horizontal_wrapped(|ui| {
                egui::ComboBox::from_id_salt("cloud-source")
                    .selected_text(
                        self.cloud_source
                            .as_ref()
                            .and_then(|id| sources.get(id))
                            .cloned()
                            .unwrap_or_default(),
                    )
                    .show_ui(ui, |ui| {
                        for (id, name) in &sources {
                            if ui
                                .selectable_value(&mut self.cloud_source, Some(id.clone()), name)
                                .clicked()
                            {
                                self.cloud_day = None;
                                self.cloud_selection = chart::Selection::default();
                            }
                        }
                    });
                let mut days = state
                    .cloud_data
                    .snapshots
                    .iter()
                    .filter(|s| Some(&s.origin.id) == self.cloud_source.as_ref())
                    .map(|s| s.day)
                    .collect::<Vec<_>>();
                days.sort_unstable_by(|a, b| b.cmp(a));
                days.dedup();
                if self.cloud_day.is_none_or(|d| !days.contains(&d)) {
                    self.cloud_day = days.first().copied();
                }
                egui::ComboBox::from_id_salt("cloud-day")
                    .selected_text(date(self.cloud_day.unwrap_or(0)))
                    .show_ui(ui, |ui| {
                        for day in days {
                            ui.selectable_value(&mut self.cloud_day, Some(day), date(day));
                        }
                    });
            });
            if let Some(s) = state.cloud_data.snapshots.iter().find(|s| {
                Some(&s.origin.id) == self.cloud_source.as_ref() && Some(s.day) == self.cloud_day
            }) {
                for usage in s.usages.iter().filter(|u| u.packet.p != 0) {
                    let name = usage.packet.n.as_deref().unwrap_or(match usage.packet.p {
                        1 => "Claude Code",
                        2 => "DeepSeek",
                        _ => "服务",
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(name).font(bold(14.)).color(t.ink));
                        if let Some([day, n]) = usage.packet.d.filter(|d| d[1] >= 0) {
                            ui.label(format!(
                                "{} · {} Token",
                                date(day),
                                model::compact(Some(n as u64))
                            ));
                        } else if let Some(b) = usage.packet.b {
                            ui.label(format!(
                                "{b:.2} {}",
                                usage.packet.c.as_deref().unwrap_or("")
                            ));
                        } else {
                            ui.label(RichText::new("未提供日统计").size(12.).color(t.secondary));
                        }
                    });
                }
                for env in s.climate() {
                    ui.add_space(12.);
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new("温湿度").font(bold(16.)).color(t.ink));
                        ui.label(
                            RichText::new(format!("{} · {}", env.device, at(env.sampled_at)))
                                .size(12.)
                                .color(t.secondary),
                        );
                    });
                    ui.label(
                        RichText::new(format!(
                            "{} °C    {} %",
                            env.temperature
                                .map(|n| format!("{n:.1}"))
                                .unwrap_or_else(|| "—".into()),
                            env.humidity
                                .map(|n| format!("{n:.1}"))
                                .unwrap_or_else(|| "—".into())
                        ))
                        .font(bold(24.))
                        .color(t.ink),
                    );
                    if let Some(h) = &env.history {
                        chart::show(ui, h, true, t, &mut self.cloud_selection);
                        ui.add_space(8.);
                        chart::show(ui, h, false, t, &mut self.cloud_selection);
                    }
                }
            }
        }
        ui.add_space(14.);
        egui::CollapsingHeader::new("同步说明").show(ui,|ui| {
            ui.label("两台 Mac 登录同一 Apple 账户，选择同一个 iCloud Drive 文件夹。每 30 秒交换一次，App 中可查看最近 90 天的每日摘要与已接收的温湿度曲线。");
            ui.label("Codex 同账号只计一次；无邮箱时按设备标记，不推测账号关系。Argus 共享文献库取最新有效汇总，只计一次；Harness 按电脑来源合并。登录凭据、蓝牙密钥、API Key 和本机路径留在本机。");
            ui.label("暂停后保留已同步的数据。删除 iCloud 文件不清除其他 Mac 的本机缓存。");
        });
    }
}
fn date(day: i64) -> String {
    chrono::NaiveDate::parse_from_str(&day.to_string(), "%Y%m%d")
        .map(|d| d.to_string())
        .unwrap_or_else(|_| "—".into())
}

fn quota(p: &crate::usage::Packet, index: usize) -> String {
    let n = p.w[index][0];
    if n < 0 {
        "—".into()
    } else {
        format!("{:.0}%", n as f64 / 10.)
    }
}
