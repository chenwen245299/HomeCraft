# Release 发布与 App 内更新

源码与公开安装包均使用 [chenwen245299/HomeCraft](https://github.com/chenwen245299/HomeCraft)。当前仓库已经公开，默认 `GITHUB_TOKEN` 即可发布到本仓库，无需额外个人令牌。App 匿名读取公开更新文件，不保存 GitHub 登录凭据。

## 当前发布准备：0.3.6 / build 58（2026-11-26）

本地已有 `v0.3.4` 标签，保留不改。本次 **0.3.6 / build 58** 修正 Harness 实时同步重写外层时间导致的日期归属，保留原始 usage 时间和跨周期纠正证据，识别保留修改时间的日志替换；包含 0.3.5 的大缓存、按周期统计与背景绘制修复。说明见 [v0.3.6](../release-notes/v0.3.6.md)，本机安装与验证边界见 [软件待办](../TASKS.md)。本次不改固件。

版本、commit 说明和 release note 已准备；需要发布时由用户执行下面的命令。最后推送 `v0.3.6` 才会触发 Release 工作流，构建并发布 Mac App、ZIP、DMG及更新清单，不编译或刷写屏幕固件。

```sh
python3 software/tools/release.py check
git add -A
git commit -m "fix: 按原始调用时间统计 Harness 用量并稳定窗口绘制"
git tag -a v0.3.6 --cleanup=verbatim -F software/release-notes/v0.3.6.md
git push origin main
git push origin v0.3.6
```

当前仅准备说明与命令，未执行提交、打标签或推送。

## 0.3.2 发布准备（2026-11-18）

Harness 同步对话去重修复的发布元数据已更新为 **0.3.2 / build 50**，说明见 [v0.3.2](../release-notes/v0.3.2.md)。已有本地 `v0.3.1` 标签，不能用它发布本次改动；先提交，再为该提交创建并推送 `v0.3.2`。此操作会触发下述工作流编译并发布 Mac App，不编译或刷写硬件。尚未执行提交、推送或发布。

## 一次发布

下方 0.3.1 命令和文件名为原发布示例；当前发布使用上方 0.3.6 元数据及同名标签。

当前准备版本为 **0.3.1 / build 49**。发布前先提交源码与工作流，再创建并推送版本标签：

```sh
git tag -a v0.3.1 --cleanup=verbatim -F software/release-notes/v0.3.1.md
git push origin v0.3.1
```

[Release workflow](../../.github/workflows/release.yml) 会自动：

1. 核对标签、仓库、版本和上传边界，再对该发布标签的源码及完整历史执行 Gitleaks 检查；发现问题即停止，不进入构建或发布。
2. 在 `macos-26` 的 Apple 芯片环境用 Rust 1.95.0 获取锁定依赖，运行离线测试、Clippy、格式和协议检查。
3. 从白名单生成干净 App、ad-hoc 签名、ZIP 和 DMG；稳定版还由刚构建的 App 实际读取清单、验证 ZIP 并解压校验身份、签名和架构，全程不安装。
4. 优先采用 annotated tag 的说明；轻量标签使用已提交的 `software/release-notes/v<版本>.md`。
5. 生成 SHA-256 和更新清单，先向草稿 Release 上传所有附件，再公开发布。

工作流也支持在 Actions 页面手动输入已有标签重跑。已经公开的 Release 不覆盖；应使用新版本。预发布标签如 `v0.3.2-beta.1` 发布为 prerelease，不进入稳定版自动更新；重跑旧版本不会把它设为 latest。

## 发布文件

| 文件 | 用途 |
| --- | --- |
| `HomeCraft-0.3.1-build49-macos-arm64.dmg` | 用户手动安装 |
| `HomeCraft-0.3.1-build49-macos-arm64.zip` | App 更新程序下载，内含 `HomeCraft.app` |
| `latest.json` | 版本、构建号、系统要求、更新说明、下载地址、大小和 SHA-256 |
| `SHA256SUMS.txt` | 核对 ZIP / DMG |

当前只提供 Apple 芯片 Mac，最低 macOS 12；登录自启动需 macOS 13+。当前 ad-hoc 签名未经过 Apple 公证，不能称为 Developer ID 签名或 Apple 已认证。

## 下一版本修改入口

- `software/release.json`：产品版本、构建号、系统要求与发布仓库；`source_repository` 是源码仓库，`repository` 是更新附件所在仓库，当前两者相同。
- `software/mac-app/Cargo.toml`、`Cargo.lock`：App 版本必须与 `release.json` 一致；不在 CI 临时篡改包版本。
- `software/release-notes/v<版本>.md`：准备本版本说明，可直接用于 annotated tag。

先运行 `python3 software/tools/release.py check`。本机离线构建与打包：

```sh
zsh software/tools/build_mac_app.sh
zsh software/tools/package_mac_release.sh v0.3.1
software/dist/HomeCraft.app/Contents/MacOS/HomeCraft --verify-update-files \
  software/dist/releases/latest.json \
  software/dist/releases/HomeCraft-0.3.1-build49-macos-arm64.zip
```

构建脚本保留上次本机 App 输出，安装包不覆盖已有同名版本；需要重新制作时使用新版本/构建号。`dist/` 继续被 Git 忽略，只将白名单附件交给 Release。

## App 内更新行为

设置底部的“软件更新”和菜单栏的“检查软件更新…”可打开更新页。默认在启动约 3 秒后及每 6 小时自动检查；后台发现新版后自动下载，检查和自动下载均可在更新设置关闭。手动检查时先展示说明，由用户点击下载。

App 从 `https://github.com/chenwen245299/HomeCraft/releases/latest/download/latest.json` 获取稳定版清单。更新前检查来源仓库、平台、系统版本、版本/构建号和包大小；下载后核对 SHA-256，拒绝越界路径、符号链接、特殊权限及过大解压内容，再核对 App 身份、版本、架构和签名。

下载就绪后可“安装并重启”，或等退出 HomeCraft 时安装；关闭窗口仍是隐藏到菜单栏，不会触发安装。“跳过本次”会取消当前更新，至少等到下一次自动检查或手动检查再尝试。

安装助手等待本进程退出，在应用所在目录完成复制、校验和替换；失败时恢复原 App。只支持系统或当前用户的“应用程序”目录中的 `HomeCraft.app`；开发目录或无写入权限时显示说明，使用 DMG 手动安装。不会申请管理员权限、移除系统安全标记、修改用户设置、复制设备绑定，或自动烧录任何硬件。

0.3.0 build 48 及更早版本尚无更新模块，需要先手动安装 0.3.1 一次。源码、离线安装测试和本机打包通过，不等于 GitHub Actions 或真实线上跨版本更新已经跑过；首次线上验证需在提交并推送标签后单独记录。

## 离线验证

```sh
cargo test --locked --offline --manifest-path software/mac-app/Cargo.toml
python3 -m unittest discover -s software/tests -p 'test_release*.py' -v
```

单元测试使用合成清单、临时 Git 仓库和临时签名 App，验证清单、版本、压缩包边界、签名拒绝、退出后的替换及失败回退。`--verify-update-files` 使用与下载更新共用的校验和解压流程核对实际发行文件，临时内容自动清理；两者均不连接硬件、查询真实用量、替换本机正在使用的 App 或创建线上 Release。
