# Git 与 GitHub 上传说明

仓库保存源码和重建项目所需资料。本机现有的构建、安装包、设备数据和备份继续保留在磁盘，由忽略规则排除，不需要为了上传而删除。

## 哪些内容入库

| 保留 | 原因 |
| --- | --- |
| `software/mac-app/` 的 Rust 源码、`Cargo.toml`、`Cargo.lock` | 固定 App 依赖，构建输出 `target/` 除外 |
| `software/protocol/` 及生成的协议常量 | 保持软件与各板协议一致 |
| 各板的 `main/`、`components/`、工具与测试 | 包含本地修改过的驱动、配网和协议实现，不能当下载缓存忽略 |
| `board.json`、`sdkconfig.defaults`、`sdkconfig.homecraft`、分区表、依赖锁 | 重建各板所需的板型与构建配置 |
| 图标、服务素材、字体/画面点阵的 C 源码 | 编译直接引用；生成工具依赖本机字体，不能仅因它们是生成文件就删除 |
| 文档、第三方许可、来源锁和补丁 | 保留使用说明、固定上游版本和本地差异 |

`.gitattributes` 将大块点阵 C 源码标记为生成文件，避免普通代码审查被像素数据淹没；它们仍会正常入库。最大的文件目前是 E1002 的字体 C 源码，约 12.5 MiB。

## 哪些内容只留本机

- `build/`、`build-*/`、`target/`、`managed_components/`、对象文件、固件镜像和日志。
- `dist/`、App、DMG、旧版 App 归档；安装包或纯固件交付物需要时单独放 GitHub Releases。
- `artifacts/` 内的截图、渲染预览、音频、现场用量和验证记录，只提交各目录的 `README.md`。
- `.local/`、`backups/`、配对/授权/用量缓存、`.env`、`auth.json`、`credentials.json`、SSH/AWS 凭据目录和私钥文件。
- Gitleaks 扫描报告；报告可能包含敏感片段，不提交或作为 CI 附件上传。
- Python/Conda 环境、Arduino 板包和工具链、编辑器本机设置。
- E1002 的参考仓库副本、PDF 和发行索引；保留 [来源锁](../../reTerminalE1002/reference/sources.lock.json) 与 [恢复说明](../../reTerminalE1002/reference/README.md)。

历史文档中的 `artifacts/`、`dist/`、`build/` 链接指向本机资料，克隆源码不会带有这些文件。它们不是已公开的下载或远程验证证明。需要公开验证结果时，先另行整理脱敏摘要，不直接强制添加整目录。

## 首次启用自动拦截

每次克隆到新电脑后，在仓库根目录执行一次：

```sh
brew install gitleaks
python3 software/tools/install_git_hooks.py
```

使用 [Gitleaks 官方 CLI](https://github.com/gitleaks/gitleaks) 8.30.1 或更新版本；本仓库不需要额外安装 pre-commit 框架。安装器仅为当前仓库设置 `core.hooksPath=software/tools/git-hooks`，不改全局 Git 配置。检测到其他自定义 hook 或路径时会停止并保留它们，需先合并现有检查。

- **提交前：** 自动检查暂存内容和文件边界，使用 Gitleaks 扫描暂存差异。工作区已改干净也不能掩盖暂存区里的敏感内容。
- **推送前：** 检查当前源码、暂存内容，以及所有本地引用可达的完整提交历史；历史中已删除的密钥或私有路径仍会阻止推送。纯删除远程引用的操作允许通过，便于清理误传引用。
- **缺工具或检查失败：** 阻止提交/推送，不静默跳过。所有扫描在本机运行，临时文件位于权限 700 的自动清理目录；输出只显示文件、行号、规则和提交标识，不打印原文或匹配值。

检查范围是 Git 跟踪文件和未被忽略的新文件，不遍历本机 `.local/`、备份或下载目录。被强制暂存的私有文件会由边界规则拒绝；外部符号链接也会被拒绝。不要用真实账号或设备凭据验证拦截，测试均使用临时仓库内合成数据。

## 上传前检查

在工作区根目录运行：

```sh
python3 software/tools/check_secrets.py
git status --short
git add .
python3 software/tools/check_secrets.py --staged
git diff --cached --stat
git diff --cached
git commit -m "整理 HomeCraft 源码与多硬件项目"
```

检查工具不修改项目文件或索引。`check_secrets.py` 结合原有的 [上传边界检查](../tools/check_git_contents.py) 和 Gitleaks 默认密钥规则；默认包含完整历史，`--staged` 仅检查本次暂存的新增/修改内容。边界检查还会拒绝被强制加入的忽略文件、本机/构建路径、仓库外符号链接和超过项目 20 MiB 阈值的单文件。

配置位于根目录 [.gitleaks.toml](../../.gitleaks.toml)，保留默认规则；当前只为 `registry.rs` 单元测试的固定示例密码设置“规则 + 文件 + 完整初始化行”同时匹配的精确例外。不按目录或整文件屏蔽，也不使用包含真实泄漏的基线来消除告警。新增误报先确认是合成数据，再添加同样精确且说明原因的例外。

本地 hooks 可被主动绕过，也不会随克隆自动启用；检查工具无法保证识别所有敏感信息，仍需审阅将要提交的差异。若发现真实凭据已进入历史，先撤销或轮换，再清理历史并重新扫描；仅在新提交中删除文件不能移除旧提交里的内容。

## GitHub 自动复查与发布检查

[Secret scan 工作流](../../.github/workflows/secret-scan.yml) 在 push、pull request 和手动触发时扫描，拉取完整历史，使用固定版本和 SHA-256 校验的 Gitleaks CLI，无需 Gitleaks 许可证或额外服务令牌。不上传扫描报告，不向 PR 发表评论；工作流只有源码读取权限。每次还运行隔离的拦截回归检查。

[Release 工作流](../../.github/workflows/release.yml) 复用同一检查，并明确检查要发布的标签；扫描未通过时不会进入 App 构建和发布。

GitHub Actions 在代码到达 GitHub 后执行，上传前的拦截依赖本地 hooks。仓库管理员还可在 GitHub 的安全设置开启 [Secret scanning / Push protection](https://docs.github.com/en/code-security/secret-scanning/introduction/about-push-protection)，并将 `Gitleaks and upload boundaries` 设为分支合并的必需检查。仓库文件不会自动开启这些服务端设置。

已有 GitHub 仓库时，按 GitHub 仓库页面提供的远程地址连接并推送当前分支即可。提交源码不需要停止 HomeCraft、连接设备、查询账号或刷机。

## 换电脑后恢复开发

普通用户使用独立安装包即可。源码开发需要按各项目说明安装开发工具，Git 不分发完整工具链或个人数据。

- **Mac App：** 安装 Rust 和 Xcode Command Line Tools，先运行 `cargo fetch --locked --manifest-path software/mac-app/Cargo.toml` 获取锁定依赖，再按 [软件 README](../README.md) 检查和构建。可选 Python 兼容工具按 `software/tools/requirements-ble.lock` 在本地虚拟环境中安装。
- **小屏：** 使用本板指定的 ESP-IDF，首次构建按 `dependencies.lock` 与 `main/idf_component.yml` 获取托管组件。已修改的 `components/network_provisioning/` 会随源码保留。遵守 [本板规则](../../3Inch7EPaper/AGENTS.md)，不要复用其他板的 SDK 配置或绑定。
- **E1002：** 当前 IDF 固件保留必要组件源码；若需要 Arduino 参考例程，再按 [参考资料说明](../../reTerminalE1002/reference/README.md) 和 [本板开发说明](../../reTerminalE1002/docs/development-environment.md) 恢复锁定依赖。
- **设备与账号：** 新电脑使用自己的官方客户端登录和独立设备绑定；私有目录、登录令牌、整片 Flash 备份不通过 Git 搬运。
