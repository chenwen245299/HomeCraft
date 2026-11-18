# HomeCraft 软件与协议中心

本目录维护跨硬件的HomeCraft Mac App、公共协议、数据来源与设备适配。每款硬件的管脚、SDK、分区、驱动和资料位于它自己的同级目录。

2026-11-18 源码另有 Harness 同步对话去重修复：按调用身份合并，各机数字只作观测；候选包与部署边界见 [软件待办](TASKS.md#harness-同步对话去重2026-11-18候选源码)。下方 build 48 的按机器汇总属于历史，日常安装未被本轮替换。

## 当前版本（2026-11-15）

当前源码和项目 App 为 **0.3.1 build 49**，新增标签触发的 Release 工作流，以及设置/菜单栏中的软件更新、自动检查与下载、退出后替换和失败回退。发布来源为公开的 [chenwen245299/HomeCraft](https://github.com/chenwen245299/HomeCraft)；操作与线上待测边界见 [发布指南](docs/releases.md)。

本机 `/Applications/HomeCraft.app` 保持 **0.3.0 build 48**。该版修复跨 Mac AI 用量汇总，并在“家庭设置 → 启动”加入可选的登录自启动（macOS 13+，默认关闭）。Codex 按账号去重，Argus 按已确认的共享文献库取最新有效统计、只计一次，Harness 按电脑来源覆盖合并；首页和支持 `totals:1` 的显示端使用同一口径。各 Mac 需换装包含这些修复的版本并选择同一个 iCloud 文件夹，“iCloud 与来源”显示各来源最后更新时间。

原安装版和历史项目 App、DMG 均保留。build 48 已修正本机失效的数据入口，原绑定未改。小屏配套固件为 0.2.38，部署详情归该板维护；本轮发布工作不连接设备或换装本机 App。验证与真实跨机待测边界见 [软件待办](TASKS.md)。下文 build 47 及更早事项为历史记录。

本机验证入口：[build 49 发布与更新](artifacts/homecraft-031-release-validation.json)、[build 48 汇总修复](artifacts/homecraft-030-build48-validation.json)。验证文件保留在本机，不随源码入库。

## 入口与结构

- [Git / GitHub 上传与依赖恢复](docs/github.md)：只提交源码、构建输入和必要素材；`artifacts/`、`dist/` 中的本机记录和交付物不随仓库分发。
- [Release 工作流与 App 内更新](docs/releases.md)：推送版本标签后自动构建并发布，当前准备 0.3.1 / build 49。
- [软件规则](AGENTS.md) / [软件待办](TASKS.md)
- [协议中心](protocol/README.md) / [官方数据入口与统计口径](docs/usage-providers.md)
- [App使用与历史说明](docs/mac-app.md) / [第三方来源](THIRD_PARTY.md)
- `mac-app/`：Rust/AppKit源码；`assets/`：图标与服务素材。
- `tools/`、`tests/`：构建、打包、兼容工具与检查；`artifacts/`：软件验证；`dist/`：App和安装包。

## 开发与构建

先阅读软件规则。以下从本目录执行，不访问硬件或读取真实账号：

```bash
python3 tools/generate_device_protocol.py --check
cargo test --locked --offline --manifest-path mac-app/Cargo.toml
cargo clippy --locked --offline --all-targets --manifest-path mac-app/Cargo.toml -- -D warnings
zsh tools/build_mac_app.sh
zsh tools/package_mac_dmg.sh
```

本机可选Python兼容检查：

```bash
.ble-venv/bin/python -m unittest discover -s tests -v
```

Python发送器只保留兼容用途，不与Rust App同时运行。普通用户安装HomeCraft不需要Rust/Python开发环境。

## 选择硬件与保存资料

`project.json` 选择本地开发硬件，目前为 `3Inch7EPaper`。Rust和Python核对所选目录含board.json，并使用该板的 `.local` 保存配对、设置、缓存和锁；不在工作区根或software目录保存该板资料。新板需要自己的配置和认证，不复制已有绑定。

当前共享定义为 `protocol/device-v1.json`，由工具生成Mac和已登记板端常量；不能手改一端。HomeCraft是协议中心，不将某板GPIO、电源或全部能力作为公共默认。已安装的0.2.36为单设备连接；项目0.3.0 build 45为每台已绑定设备保持独立会话（最多8台），已观察两屏同时认证连接，3Inch7EPaper 0.2.37的统一合计成功帧与首页精确值一致。逐台/全部暂停等操作仍待实机验证。见 [build 45记录](artifacts/homecraft-030-build45-validation.json)；build 43安装包保留，尚未制作build 45 DMG。

没有工程目录的独立安装继续使用App私有资料入口。本机已安装入口为 `/Applications/HomeCraft.app`；版本、项目包是否匹配及签名情况查 [发布记录](artifacts/homecraft-0236-release-validation.json)。连接和同步状态另行现场确认。

旧Conda环境归档 `.cache/pre-layout-conda`，不直接激活；需用时从 `environment.yml` 在 `.conda` 重建。ESP-IDF的构建与工具环境由各板说明管理。

当前环境页UI为项目App **0.3.0 build 47**，位置卡片与交互曲线的用法见 [环境页说明](docs/mac-app.md#030-build-47-环境页2026-10-28)。当前运行及蓝牙授权后两屏重连结果见 [环境页验证](artifacts/homecraft-030-build47-ui-validation.json)；上文build 45的连接记录是上一轮结果。

2026-10-29已生成 [build 47 安装包](dist/releases/HomeCraft-0.3.0-build47-macos-arm64.dmg)（Apple芯片、macOS 12+），包含统一合计修复及环境页改版。DMG校验、只读挂载后的App签名与逐文件哈希核对通过，原安装包保留；本次没有替换 `/Applications` 内的旧版本。见 [打包记录](artifacts/homecraft-030-build47-dmg.json)。
