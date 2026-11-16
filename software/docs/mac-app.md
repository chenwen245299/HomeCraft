# HomeCraft · Rust 菜单栏终端

2026-10-26当前已安装 `/Applications/HomeCraft.app` 与项目 `software/dist/HomeCraft.app` 均为0.2.36/build36，唯一安装包 `software/dist/releases/HomeCraft-0.2.36-macos-arm64.dmg`，固件0.2.36增加SD音乐；部署与原绑定同步检查见 `3Inch7EPaper/artifacts/music-0236-validation.json`。

新版参考本机TexCraft的 `docs/development-guide.md`、`docs/architecture.md`、`docs/control-protocol.md` 与原生窗口实现，使用egui/eframe + wgpu和隔离的AppKit模块。按用户家庭App截图完成侧栏、半透明窗口、原创海岸背景、EPaper设备卡片及连接/AI用量详情、家庭设置。0.2.8已接入真实温湿度、电源和SD状态回传、当天曲线与远程切页/方向控制；旧版未接入环境数据的说明属于历史记录。

历史0.2.6通过31项Rust测试与Clippy零警告，刷新设置已在隔离窗口检查加载、开关、保存及850×620布局；此前家庭界面也已通过1100×780检查、实际导航和关闭后菜单栏重开。压缩包CRC、解压后本机ad-hoc签名已验证；未经过Apple公证。包内不含配对文件、设置或账户凭据。用户要求先交付安装包，EPaper连接留到晚上验证；本次未改动固件。验证记录见 `3Inch7EPaper/artifacts/homecraft-release-validation.json`。

## 使用

1. 双击 App 并允许蓝牙。新Mac在App扫描，选择板子实体开启的 `EPair_` 设备，输入8位屏幕码并点安全连接；已记住的 `EPaper_` 可直接连接。无需插件、USB绑定或在系统蓝牙面板手动配对。
2. 主窗口打开时显示程序坞图标，最小化保留；关闭窗口后程序坞运行图标收起，菜单栏墨水屏图标继续存在并同步。菜单提供显示窗口、立即同步、暂停连接、退出程序；退出程序才完全停止。
3. 默认每5分钟读取用量，可在“AI用量 → 刷新设置”或家庭设置里调整为1–120分钟，关闭自动读取后仍可手动刷新。10秒加密保活、断线最长每60秒重试与查询间隔分开。Codex 自动读取缓存 60 秒，卡片上的手动刷新跳过缓存，DeepSeek 至少 300 秒，DeepSeek 429 至少 900 秒。Mac 休眠时暂停，恢复后尝试重连。

没有安装登录项或 launchd 服务。再次打开 App 后在列表选择设备连接；本次已通过 `--connect` 自动连接已绑定设备。设备列表只筛选EPaper数据/首次配对服务；认证由App内置的官方Security 2实现，不能仅凭同名广播通过认证。

首次使用改为蓝牙屏幕码认证：设备网络页双击BOOT，HomeCraft扫描选择`EPair_`设备，输入设备上的8位码。认证后原子保存600私密配对文件并恢复普通`EPaper_`连接。每台Mac只需认证一次，第二台不会清除第一台绑定；日常原Mac暂停、另一台连接，无需USB或重新配对。配套固件已部署，用户已确认屏幕码认证成功；USB只保留为折叠维护入口。

源码开发由 `software/project.json` 选择独立硬件目录，配置保存到该板 `.local`；`EPAPER_PROJECT` 可明确覆盖数据目录。无项目时使用700的 `~/Library/Application Support/HomeCraft`，兼容原有EPaper Home私有目录。包内不带配对文件或账户凭据。家庭设置保存到600的`3Inch7EPaper/.local/.home-ui.json`。

## 仅使用官方公开数据入口

用户明确要求采用官方接口。Codex 在 Mac 上短暂启动已有官方 `codex app-server --listen stdio://`，完成 `initialize`/`initialized` 后只调用：

```json
{"id":2,"method":"account/rateLimits/read"}
{"id":3,"method":"account/usage/read"}
```

读取后子进程退出。认证由官方 Codex 已有登录管理，本程序不读取、复制或导出登录令牌，不调用模型、不启动会话推理。`rateLimitsByLimitId.codex` 优先于旧单桶字段；剩余比例为 `100-usedPercent`，窗口和重置时间以返回值为准。`account/usage/read` 返回账户累计 Token、单日峰值和可选日记录；缺少或不支持时显示未提供，不从日志猜测。[OpenAI 官方文档](https://learn.chatgpt.com/docs/app-server)

Claude 只使用官方 Code `statusLine`：`rate_limits.five_hour` / `seven_day` 的 `used_percentage`、`resets_at`，以及 `context_window.total_input_tokens` / `total_output_tokens`。这些 Token 是最近回复的当前上下文数据，输入含缓存，**不是账户累计或每月消费**。额度字段取决于官方客户端、套餐与会话，可能缺失；不自行填入月额度。[Claude 官方文档](https://code.claude.com/docs/en/statusline)

App 可启用接收命令，添加到 `~/.claude/settings.json` 的 `statusLine`，保留其他设置；若已有其他状态栏命令则拒绝覆盖。本机原先无状态栏，现已接入；未检测到 PATH 中的 Claude Code CLI，需之后运行官方 Claude Code 并重启会话才会产生真实数据。Claude 桌面 App 本身不等于 Code CLI。接收程序只保存白名单数值到 `3Inch7EPaper/.local/.claude-statusline.json`（600），不保存提示词、会话 ID、工作路径或原始 JSON，不读取 Claude OAuth/Keychain，不请求未公开的 HTTP 用量接口。

DeepSeek 只请求官方 `GET https://api.deepseek.com/user/balance`；Key 通过 Mac 环境变量 `DEEPSEEK_API_KEY` 或 `3Inch7EPaper/.local/.ble-settings.json` 提供。现在未配置。[DeepSeek 官方文档](https://api-docs.deepseek.com/api/get-user-balance/)

累计 Token 在 Mac App 显示；大字版墨水屏另有今日 Token 区域。快照新增可选 `d` / `ld` 日统计字段，仍不发送账户凭据；只汇总同日官方数值，缺少数据保持未知，详见 `display-redesign.md`。

## 构建与验证

```bash
software/tools/build_mac_app.sh
cargo test --locked --manifest-path software/mac-app/Cargo.toml
```

构建锁定 `software/mac-app/Cargo.lock`；App 本机 ad-hoc 签名，附带蓝牙权限用途说明。主界面用 egui/eframe，系统菜单栏用 tray-icon，CoreBluetooth 用 btleplug；Security 2 采用与官方固件兼容的 SRP-3072/SHA-512、AES-256-GCM，强制 nonce-counter patch 1。密码文件只本机读取，设备证明验证使用常数时间比较，GCM 完整性校验失败会断开重连，不降级。

早期基准验证：Rust 6 项测试通过，包括与官方 Python 客户端的合成样例互通、前导零摘要和盐、错误设备证明拒绝、未知敏感字段排除及快照边界；原 Python 15 项兼容测试通过。Rust 实物扫描、双向认证、真实 Codex 加密发送、设备 ACK 和 App 后台接收均通过。用户明确确认关闭窗口后菜单栏与墨水屏同步正常。Claude 仅完成状态栏接收集成，没有真实会话样本；DeepSeek 未配置。

本机生成的二进制约 9.3 MiB，刚启动并连接后一次空闲采样 CPU 0.0%、RSS 约 130 MiB；不是持续资源上限。App、原 Python 发送器通过同一 `3Inch7EPaper/.local/.ble-sender.lock` 排他，另有 `3Inch7EPaper/.local/.ble-app.lock` 防止重复窗口进程。以上资源采样属于早期基准。当前板端已经使用用户确认的差分刷新，并已部署SD、正常关机与Argus扩展，最新状态见 `3Inch7EPaper/artifacts/morning-validation.json`。

## 2026-10-21 Argus数据适配

已部署官方Argus STDIO MCP适配器和第四张数据卡，保留原有菜单栏/后台和安全认证。`EPAPER_ARGUS`或私密设置`argus_executable`可选择可执行绝对路径，默认 `/Applications/Argus.app/Contents/MacOS/Argus`。默认当前最近打开的文献库，也可通过官方`ARGUS_LIBRARY`固定。每5分钟读取当天和本周汇总，禁止从库数据库绕过MCP开关。调用结果及范围见`usage-providers.md`。

日/周日期均按UTC+8，周一零点起；只读summary和按服务/模型汇总标签，不读取逐条记录、文本或密钥。校验input+output=total及缓存包含关系；未知费用显示未记录。周查询最多额外5秒，失败保留成功的今日统计。可选wt只对支持weekly_tokens能力的固件发送，老固件协议不变。Rust10项测试通过，包含周界/跨年、范围回显、未知/0/去重及兼容性。

用户已确认Argus今日卡片与Token正常。墨水屏日/周数字按用户最新反馈采用与Codex的5小时/周额度相同的层级，调用/输入/输出/缓存/费用移至详情页。Mac新版复用此官方数据适配器；今日合计只计算同日已知Token，不将周累计、历史或上下文值混入。

## HomeCraft 0.2.1换机包

用户只需要Apple芯片版，未构建Intel版。App名称、可执行文件、设置页及菜单栏提示统一为HomeCraft，保留原bundle identifier以兼容系统权限。原创图标参考TexCraft的平涂粗描边语言，采用不同的墨绿配色与房屋/墨水屏主体。包内README.txt记录新Mac安装、首次USB绑定和官方客户端配置；应用本身不需要Rust或Python环境。源码不再使用编译机项目路径作回退，动态依赖均为系统库；Finder启动Codex的子进程PATH包含常见Homebrew路径。安装后新Mac仍须本机登录官方数据源；不把源Mac凭据放进发布包。

## 0.2.2 蓝牙屏幕码更新

用户指出USB初次认证没有必要，现已添加8位临时PIN的官方Security 2蓝牙领取流程；来源、安全边界及未实测项见`ble-data.md`。原有两台Mac保存的长期绑定不会因添加第三台或切换被轮换；绑定只在主动删除本机配置或设备密钥丢失后需要重做。新版本蓝牙模式切换尚未实板验证，不将离线测试称为完成配对。

## 0.2.3 菜单栏图标

菜单栏使用独立的方正房屋+墨水屏轮廓，内嵌44px透明模板PNG并由系统调整单色；没有旋转、彩色底板或阴影。`software/tools/render_homecraft_icon.cjs`同时生成图标资源，源为`software/assets/app-icon/homecraft-tray.svg`。已检查浅色/深色效果及隔离App菜单栏初始化，发布ZIP的解压签名正常。已运行的旧实例不会自动换图标，安装新版后退出旧实例再打开即可。未改固件，蓝牙配对候选仍待晚上验证。

## 0.2.4 Codex CLI跨Mac查找修复

旧版优先两个Homebrew路径后直接使用GUI进程PATH，遗漏Finder启动时的用户CLI目录与Node版本管理环境。现通过独立`codex_cli.rs`统一查找，支持当前PATH、用户独立安装、npm-global、nvm/fnm、pnpm、Volta/asdf/mise和Homebrew等候选；失败时在4秒内从用户zsh/bash登录环境探测，结果缓存10分钟。配置可指定绝对路径或~/路径，临时`EPAPER_CODEX`/`EPAPER_NODE`仍可覆盖。npm脚本使用对应node启动，实际子进程PATH包含所选运行时目录；用户输入路径不拼接到shell中执行。

家庭设置和设备AI用量页新增CLI文件选择、可选Node路径、重新检测、实际路径/版本/具体错误；选择更改保存后旧查询缓存按配置失效，不继续显示先前的找不到错误。检测仅执行`--version`，不调用模型、不读取/复制登录文件，App-server的官方额度与Token调用保持原样。CLI未找到、Node缺失、文件无法启动、接口失败分别提示。

可用`HomeCraft.app/Contents/MacOS/HomeCraft --diagnose-codex`输出不含凭据的路径/版本诊断。已通过25项测试和Clippy；已在PATH仅/usr/bin:/bin时通过正式打包程序检测到本机codex-cli 0.141.0。另一台Mac的实际安装路径仍需用户运行新版确认；自动查找未覆盖时可直接选择终端`command -v codex`返回的可执行文件。无须重新登录或重新绑定设备。

官方安装与环境说明：[Codex CLI](https://learn.chatgpt.com/docs/codex/cli)、[环境变量](https://learn.chatgpt.com/docs/config-file/environment-variables)。官方独立安装默认路径及CODEX_INSTALL_DIR同样纳入候选，不再只依赖Homebrew。


## 0.2.5 Codex手动检查、刷新与DMG

左侧“AI 用量”及设备用量详情的Codex卡片直接提供“检查 CLI”“刷新 Codex 用量”“CLI 设置”。没有读取过数据、未连接墨水屏时也会保留Codex卡片。检查显示实际路径、版本及具体错误，仅执行--version；刷新只重新读取Codex官方app-server额度与可选Token统计，不调用模型，不查询其他provider，也不要求蓝牙连接。

手动刷新会跳过成功或失败的旧60秒缓存，自动读取仍使用缓存。配置变更和缓存过期也会触发新查询。读取时显示进度并禁用重复操作，总超时75秒；完成后更新读取时间与数字，失败明确显示原因并将额度标为未知，不把旧数字冒充最新结果。其他服务的读数保留；若已经连接设备，只将本次Codex快照通过原有加密BLE发送。

使用`HomeCraft.app/Contents/MacOS/HomeCraft --refresh-codex`可单独验证相同查询链路，不启动蓝牙后台。27项Rust测试和Clippy通过；新增回归覆盖失败/成功缓存的手动绕过、配置变化及过期。模拟STDIO测试只使用initialize、initialized、account/rateLimits/read、account/usage/read，验证窗口与同日Token解析及缺CLI错误。正式App已在精简PATH下成功读取本机官方5小时/周额度和累计Token，今天的Token仍未提供。

构建后运行`software/tools/package_mac_dmg.sh`生成Apple芯片DMG。仅按白名单打包App、安装说明和Applications快捷入口；双击DMG后拖拽安装，升级前从菜单栏退出旧实例。磁盘镜像不改变官方客户端依赖，也不替代Apple公证：当前仍为ad-hoc签名，未公证。当前配对候选固件未部署，EPaper与双Mac连接继续留待晚上实测。


## 0.2.6 App内自动刷新配置

“AI用量”页增加刷新设置入口、上次全部读取时间、下次读取时间与读取状态。家庭设置顶部及设备用量页可开关自动读取，输入1–120分钟或选择常用间隔，点击“保存刷新设置”即保存至600权限的.home-ui.json并更新后续计划，无需重启。旧配置没有refresh字段时兼容默认5分钟；非法间隔不能保存。界面上的刷新时间按电脑本地时区显示，Token统计日仍是UTC+8。

账号轮询由独立的refresh.rs管理，不再以BLE已连接作为前提；关闭窗口继续在菜单栏轮询，暂停蓝牙只释放设备，关闭自动读取才停止定时查询。全量读取完成后再等待设置的间隔，耗时不会造成并发堆积，错过周期只读取一次。修改间隔按上次读取完成时间重新计算；关闭自动刷新会取消已排队的全量读取，用户仍可随后手动请求。正在执行的查询会正常结束。

“立即读取全部用量”和屏幕发起的请求保留最短60秒的全量读取间隔；Codex独立手动刷新仍可绕过其缓存。DeepSeek余额继续至少5分钟缓存，限流15分钟；Claude Code在下次读取时载入官方statusLine最新记录，缩短间隔不等于服务端产生新数据。未知或失败仍按provider标注，不把“读取结束”当成全部服务成功。

BLE仍每10秒加密保活；新读取与连接成功时发送现有数值，读取结果先在Mac呈现，不因BLE发送失败而丢失。无读数时不会标记成成功同步。本轮未连接、烧录或验证硬件，既有候选固件仍待晚上测试。31项Rust测试与Clippy通过，包括默认兼容、私密持久化、运行中变更、关闭后手动读取及错过周期不补发。


## 0.2.7 统一玻璃设计语言

本轮检查家庭、自动化、探索、温控、显示、AI用量、房间七个页面，以及设备概览/用量/连接、添加配件、家庭设置。之前混入的白色卡片、浅灰表单及默认控件已统一为深色磨砂玻璃；背景、字体、圆角、边线和交互状态集中在home/theme.rs中。普通与暮色只改变背景，组件样式一致。

- 大卡片圆角20pt、详情弹窗24pt、按钮/输入框/菜单10pt；胶囊与图形插画按其几何形状处理，不把窗口整片背景当成卡片加圆角。
- 卡片共享同一玻璃底色、细边线和18pt内边距；弹窗使用模糊壁纸衬底，避免透明层透出背后文字。共享按钮保证34pt最小高度、悬停/按压边线与禁用状态；橙色主操作配深色文字以改善对比。
- 默认控件在Light/Dark样式中都安装同一套主题，覆盖文本框、复选框、数值编辑、折叠区、进度条、滚动条、菜单与提示。新组件应使用theme::surface、theme::modal与共享按钮，不新增白底卡片或独立圆角数字。
- 服务名称保留粗体；OpenAI/Claude/DeepSeek作为浅色模板字形绘制，去除OpenAI位图自带的方形白画布；Argus图像用圆角遮罩。原始品牌素材未覆盖。App自身窗口和原生文件选择器使用本窗口的Dark Aqua，不改变系统或其他App外观。
- 清理自动化页与隐私文案里写死的“每5分钟”，前者显示实际配置、关闭时显示手动读取。

为全面检查加入--render-ui-preview绝对目录：用正式Home组件、字体、材质与wgpu渲染离屏图片，覆盖两种背景、1100×780/850×620、设置底部与服务卡片、错误/加载/禁用状态。只用明确标记的样例，不启动蓝牙、账户或修改日常配置；这些图片不包含系统窗口红绿灯，不能视作硬件或原生文件对话框实测。


## 0.2.8 实时环境与屏幕遥控

“温控”页显示板上真实温湿度、采样时间、电量/USB/充电状态、SD已保存条数和UTC+8今日两条趋势曲线。HomeCraft家庭设备卡片同时显示最近温湿度；断开会标为上次读数，缺失保持未知。曲线读取板上96个15分钟均值桶，按缺测断开，不从Mac编造传感器数据；每分钟原始CSV仍存SD。

“显示”页和EPaper概览提供当前可用页面按钮、自动/四向锁定及清除残影。未配置服务不添加空切页按钮，页面是否完成以板子回传状态为准。页面/方向建立全刷底图，正常数据保持既有差分节流。App仅接收环境与显示状态，不读取设备配置或账户凭据。

已在本机正式App完成Security 2原绑定重连、真实官方用量同步、温湿度回传、96个历史桶组装和界面按钮切到环境页；后续验证记录见`3Inch7EPaper/artifacts/homecraft-028-*-validation.json`。新Mac首次屏幕码与两台实体Mac交替须另行确认，不能用本机原绑定重连替代。


## 0.2.9 全App操作反馈与暂停重连

刷新设备现在直接消费当前Security 2回复，更新温湿度/电源/SD及当天历史；点击立即进入读取状态，禁止重复，结果提供完成时间与12秒全局提示，失败给出原因。其他耗时操作（扫描/连接/暂停、官方用量/CLI检查刷新、安全认证/维护、设置与文件选择取消）也采用同一反馈组件；未保存刷新草稿有标识。提示使用与弹窗一致的模糊壁纸衬底，不透出图表文字。

固件status新增可选shown数组，内容仅在实际刷屏成功后更新。App明确区分每5秒的实时采样和按30–60秒刷到屏幕的值；控屏等待真实帧确认或明确超时，不以收到队列ACK就报成功。

实际按钮审计发现暂停重连失败：CoreBluetooth断开后旧Peripheral不再可用。新版本通过公开retrieve_peripherals按旧ID取回有效原生对象再认证，不需USB或更换绑定。暂停后重新扫描保持暂停，只有首次启动扫描按自动连接设置执行。不同主页面/设备页签分开保存滚动位置。

最终实际按钮回归、36项Rust测试与Clippy通过；官方Codex手动刷新没有模型调用。记录3Inch7EPaper/artifacts/homecraft-029-button-audit.json与homecraft-029-release-validation.json。原生文件选择的实际用户选取/取消和首次屏幕码、双Mac切换仍需人工确认。


## 0.2.11 连接表单与可关闭的小提示

输入配对码不触发认证；连接页只有一个“安全连接”主按钮，位于码框下方。已记住且处于普通数据模式的设备可留空码；首次添加或当前配对模式需8位码。已填码时必须通过点击进行一次认证，不自动重试PIN。实际广播名 advertisement_name 优先于可能滞后的 CBPeripheral.name，认证仍按单个已知服务内实际端点和 Security 2 校验，拒绝模式混合或不明服务。

打开连接表单会暂停尚未完成的重连；输入码时释放旧连接。扫描期间积压的表单/暂停命令先于自动连接处理，进行中的普通连接可被新命令取消。已有稳定连接仅查看连接页不会断开。已记住设备日常自动重连保留，手动暂停或检测到配对模式后停止重试；后台一般失败仅更新详情，不反复弹出提示。

提示改为12pt单行、可关闭的状态条，长信息截断：主页面预留底部独立位置，弹窗放在标题下方正常布局，均不使用覆盖按钮的 Tooltip 浮层。详细认证/连接原因保留于连接页“查看连接详情”，关闭提示不影响正在执行的操作。大/小窗口生产组件测试覆盖长错误、关闭点击、码框下按钮、输入完整码不提交、点击才提交。

本轮Mac App 43项Rust测试与Clippy通过；已替换本机0.2.10，既有绑定连接、暂停/重新扫描不重连、弹窗与主页面提示关闭及按钮相对位置经实际窗口检查。固件仍0.2.9，未重刷、未改NVS/SD。首次屏幕码与两台实体Mac验证仍待用户操作，不能用已绑定Mac连接代替首次认证实测。


2026-10-21补充：用户已确认HomeCraft 0.2.11屏幕码“认证成功，已连接”。随后安全状态确认普通数据链路已认证，长期绑定与首次配对前摘要比较一致；原绑定未轮换。两台实体Mac交替仍未单独实测。


## 0.2.14 App图标雾蓝底色

用户明确此次“背景颜色”指Dock/启动台的App图标底色。只将SVG圆角底板从墨绿#257C79改为雾蓝#586EAC，房屋/金色墨水屏造型、窗口背景主题与方正单色菜单栏标识保留。已重新生成16/32/64/128/256/512/1024全套ICNS及128/1024PNG，检查各尺寸底板颜色与透明边角。没有修改固件或账户/蓝牙逻辑。

本机App版本0.2.14/build15已替换并启动、原连接恢复；Cargo check及release构建通过。Apple芯片DMG同版本，签名/只读挂载/文件白名单及ICNS一致性验证见`3Inch7EPaper/artifacts/homecraft-0214-icon-validation.json`。仍为本机ad-hoc签名，未Apple公证。


## 0.2.15 Argus分类与新详情页

同一次官方get_token_usage日汇总中的by_model/by_provider现经白名单归一化后发往支持argus_breakdown:1的设备；旧固件继续仅接收原快照。每组保留最多3行，超过3类为前2+其他；分类必须与当天汇总的日期、抓取时间及总数完全匹配。供应商官方仅有UUID，屏幕使用短ID，不读取配置或猜测名称。现有Mac窗口主题、雾蓝App图标、菜单栏行为与自动刷新设置保持。

固件与App均已更新到0.2.15，原长期绑定重连，真实Argus今日19,307,194 Token及3种模型/2个供应商到板；该值为验证时快照，会随新调用更新。46项Rust测试及Clippy通过；Apple芯片DMG只含App、安装说明及Applications链接，签名、版本、二进制/图标一致性和内容白名单通过，仍未Apple公证。新详情实屏视觉等待用户确认，不能将预览或数据ACK当作视觉确认。记录见3Inch7EPaper/artifacts/argus-detail-0215-validation.json及3Inch7EPaper/artifacts/homecraft-0215-release-validation.json。


## 0.2.16 主界面减文案

用户要求正常使用时不要堆叠说明文字。家庭连接卡、自动任务、温控/显示、AI用量及连接表单删除重复教程与实现说明；主界面保留核心数字、短状态、必要操作以及可关闭的进度/结果提示。Token口径仍以“已知合计”标明，缺失/旧数据语义保持；输入输出、账户累计、最近统计日和错误原因进入卡片“详细用量/错误详情”，屏上读数进入温控页折叠区，CLI路径/版本进入专门设置的“诊断详情”。

统计口径、缓存限制、配对/切换、后台运行和安全说明集中在家庭设置的“帮助”入口，分主题按需展开；Claude Code接收与配置目录入口在设置的“其他服务”。Codex卡片保留“检查CLI / 刷新 / 设置”，手动刷新、首次PIN显式点击和短错误关闭交互均保留。仅改Mac App到0.2.16/build17，固件继续0.2.15；不因减少文案改查询方式、加密、刷新频率或设备数据。


0.2.16交付确认：46项现有Rust回归与Clippy零警告通过；正式组件生成96张仅样例的大小窗口/双背景预览，重点核对用量、温控、连接、设置与帮助。已替换本机项目App并恢复原绑定连接，真实Codex/Argus状态正常；帮助、Codex设置与家庭设置展示路由通过。DMG交付记录见3Inch7EPaper/artifacts/homecraft-0216-release-validation.json，未改板端固件。


## 0.2.17 真正的手动采样与安装独立性

旧“刷新数据”只读status缓存，不触发测量或画面更新。新“采样并更新”使用独立sample命令，读取状态轮询仍用status；设备分配请求编号，通过输入任务通知触发新SHTC3测量，匹配编号和新时间后在原2秒交互间隔内评估差分。仅当收到对应已更新/画面未变化结果才完成，传感器/显示失败单独报错；原20次/30分钟/布局变化全刷保留。旧固件缺sample_refresh能力时仍可回读，并明确未触发新采样。

手动Codex或全量用量发送后也请求更新当前画面。账户查询移至单个独立后台任务串行队列，官方接口慢时BLE保活、状态回传和操作继续运行；不并发修改同一个认证会话。48项Rust、C采样状态机/严格协议ASan/UBSan及48组环境差分通过。已app-only部署固件0.2.17和App0.2.17/build18，原绑定重连，4次真实采样完成，日志捕获到667/685ms差分。

Mac包只链接系统动态库；独立App复制出项目后，用空配置和仅系统路径成功初始化并关闭预览，不访问账户或BLE。默认私密数据目录有现有单测；没有把源码目录或开发机CLI路径编进运行时回退，也不需要安装语言环境/蓝牙插件。该检查不是第二台实体Mac测试。新电脑需它自己的官方数据源客户端和登录，设备连接不依赖这些账号。记录见3Inch7EPaper/artifacts/manual-sample-0217-validation.json和homecraft-0217-portability-validation.json。


2026-10-21用户已亲手验证HomeCraft“采样并更新”，反馈“局刷和完成提示正常”；此次手动采样、实屏局刷及完成反馈确认通过。

## 0.2.18 HomeCraft 中枢、DeepSeek Harness 与交互趋势

新增 DeepSeek Harness 官方本机 Token 适配器；AI 用量页提供服务筛选、全部设备/单台 Mac 筛选、大字今日/本周、输入/输出、缓存命中率、调用次数和设备贡献条。设备行保留最近同步时间，按设备 ID 更新，主页与合计不重复计算本机和远端缓存；显示规则与同步边界见 usage-providers.md。

温控页曲线改为即时数字读数与十字线：悬停显示对应 15 分钟时段的均值和单位；温湿度联动选择；点击固定，左右键切换，Esc 取消；缺测点显示无采样且不插值连接，跨日自动清除旧选择。主卡片仍是最新实时采样，图表是 SD 的 15 分钟均值，两者不能混称。使用既有深色磨砂、圆角与橙/青色主题，短标签，操作说明仅悬停显示。

`--read-harness` 是只读数字诊断入口。Zstandard 原生内置，普通用户不用安装插件或运行时。HomeCraft 仅同步已知记录，不提供另一台未联网 Mac 的实时读取能力。


## 0.2.19 秒数与回传

自动用量设置支持30–7200秒，默认30秒。旧minutes字段读取兼容，新保存seconds；本机按用户指令改为30秒。设备后台遥测/历史五分钟，手动采样立即；温控仍提供采样时间和上次读数，不把五分钟缓存当作每秒实时。DeepSeek余额的官方限流缓存继续保留。当前包HomeCraft-0.2.19-macos-arm64.dmg。


## 0.2.20 温控信息精简（仅Mac软件）

用户反馈温控页重复读数、多个时间、设备状态文字堆叠。主页面现为一组温湿度大数字与图标、一处采样时间、小刷新图标及更多入口；删除重复EPaper温湿度标题、手动更新时间行、展开的屏上读数和电量/SD状态行。普通数据不再称为“实时”，避免五分钟回传时误导。

“更多→设备状态”按需展示连接、电源、SD、采样/回传周期，屏幕显示另行折叠。传感器/断开状态保留，SD异常可通过更多图标变色发现。刷新仍调用原来的主动采样事务，有加载与完成反馈；详情只看已有状态，不请求硬件。趋势维持悬停数值、点击固定与键盘导航，大窗可同时看到两条曲线。

用户明确后续先处理软件，硬件不再连接；本轮不扫描/连接/烧录、不变更采样、BLE、账户入口、刷新和省电策略。固件继续0.2.19。软件63项测试/Clippy及最终离屏预览、安装包校验记录在3Inch7EPaper/artifacts/homecraft-0220-ui-validation.json。

## 0.2.21 iCloud Drive 与 Codex 账号（2026-10-22，软件更新）

家庭设置新增“iCloud 与来源”。通过原生目录选择器，用户在 iCloud Drive 选择或新建同步文件夹，两台Mac使用同一目录。当前ad-hoc发行不伪装启用CloudKit；CloudKit要求开发者计划及相应容器签名能力。这里使用用户选择的普通iCloud文档、NSFileCoordinator后台读写和macOS系统上传下载，界面“已写入”不代表云端或另一台Mac已确认。默认关闭，未替用户选择或访问真实iCloud文件。

每30秒交换同步文件夹里按内容分开的三个子文件夹：`AI用量/`、`温湿度/`（均为 `YYYYMMDD-<来源ID>.json`）和 `待办/`（`todos-<来源ID>.json`），只写本机所属分片，格式版本写在文件内（2026-10-27起）。更早版本写的 `HomeCraft-Data-v1/v2`、`HomeCraft-Todos-v1` 过渡期仍会读取，但不再写入或创建；所有Mac更新后可在用户确认下删除。天气城市与每日古诗各Mac自己设置，不同步。冲突副本按来源、日期、provider和账号合并，较晚观测替代较早观测，不相加同一账号或同一Harness来源；保留不同账号。同一邮箱的套餐变化不产生第二份账号，官方缺邮箱时按Mac区分，未知账号不和已知账号盲目相加。Argus等没有稳定库身份的来源独立显示，不按Mac猜测可相加。日统计维持UTC+8；界面与私有缓存保留最近90天，Drive更早文件不主动删除。普通用量/温控页可展示带来源标签的云端回退值；回退只存在于UI副本，不重新冒充本机采集或回灌云端。

白名单文档含数值快照、经过现有分类校验的Argus汇总、官方账号脱敏标识、设备来源及已接收的温湿度96桶曲线；不是SD原始CSV的完整备份。任何登录令牌、配对秘密、API Key、原始回复、自由说明字段和可执行路径均不导出。配置 `.cloud-settings.json` 与缓存 `3Inch7EPaper/.local/.cloud-cache.json` 在本机以600权限原子写入；发布包不带这些文件。读取拒绝符号链接、未知字段和超限数据；单文件256KiB，单轮读取32MiB，最多800个每日来源缓存。资料文件没有额外自定义密码层，其云端保护由用户自己的iCloud设置决定。

Codex新增官方 `account/read`（`refreshToken:false`），在额度/Token查询前后各确认一次，若账号发生变化则拒绝把读数归给另一个账号。只对 `type:chatgpt` 的有效邮箱建立SHA256账号标识；完整邮箱不保存，显示前1–3字符加***。读取失败、无邮箱或非ChatGPT模式用设备名，不解析令牌或读取auth.json。官方说明：<https://learn.chatgpt.com/docs/app-server>。

账号记录、Mac/日期筛选、温湿度交互曲线在同一玻璃弹窗；帮助细节折叠。当前版本0.2.21/build23，验证与包路径见 `3Inch7EPaper/artifacts/homecraft-0221-cloud-validation.json`。本轮仅软件，不连接/扫描BLE、不烧录、不替换运行中的旧实例；实际双Mac经Apple云传输仍待用户在两边选择目录验证。

Apple依据：<https://developer.apple.com/documentation/cloudkit/enabling-cloudkit-in-your-app>、<https://developer.apple.com/documentation/appkit/nsopenpanel>、<https://developer.apple.com/documentation/foundation/nsfilecoordinator>。


## 0.2.22 全App布局与交互整理（2026-10-22）

按用户要求对所有页面与弹窗做离线审查，具体清单见[界面检查](ui-review-0222.md)。Codex的更新时间移到操作栏右侧，检查/刷新按钮加载时不跳动；数值区等高、进度条对齐。家庭设置拆成紧凑分组，刷新设置单独成页，子设置可返回；短弹窗不再保持整页高度。修复小弹窗返回大表单时沿用旧尺寸导致按钮越界的问题。

显示卡片统一铺满内容列，清除残影收在标题旁；首页连接卡扩大点击区域，房间标题可进入房间，长名称省略而不覆盖控件。自动化和探索条目均有实际目的页。连接表单保持显式点击认证，未选设备/已连接等状态的输入与按钮禁用更准确。iCloud控制与交换时间合并显示，官方账号与来源语义保持不变。

快捷键：⌘, 打开家庭设置；⌘S保存修改；AI用量页⌘R读取用量（Codex筛选时只刷新Codex），温控页⌘R主动采样（需已连接）。关闭窗口继续在菜单栏运行。

本轮只软件，未扫描/连接BLE或USB、未烧录、未替换运行实例。当前候选App0.2.22/build24，Apple芯片DMG同版本，交付验证以 `3Inch7EPaper/artifacts/homecraft-0222-ui-validation.json` 为准。


## 0.2.25 主窗口打开时显示程序坞图标

启动或从菜单栏“显示窗口”时，AppKit切到Regular并激活主窗口；红色关闭键取消真正关闭、隐藏窗口并切回Accessory，不停止后台。最小化不切换策略，保留Dock恢复入口。LSUIElement作为默认启动策略保留，运行时只在打开/关闭边界切换；不根据失焦、后台刷新或最小化隐藏Dock。

使用公开NSApplication.setActivationPolicy（[Apple文档](https://developer.apple.com/documentation/appkit/nsapplication/setactivationpolicy(_:))），仅主线程作用于本App。82项测试、Clippy及实际预览启动/三次关闭重开/最小化检查通过；独立预览不访问真实账户或设备。已更新已安装App与项目App至0.2.25/build25，系统实际策略读回Dock可见，保留用户资料和后台行为。DMG签名、版本和白名单见3Inch7EPaper/artifacts/homecraft-0225-dock-validation.json。固件未改。


## 0.2.26 家庭主页摘要

主页左侧并列“今日Token／本周Token”，标明已知合计，右侧将温度、湿度和单一采样时间收进“室内环境”。点击Token卡进入AI用量，点击环境卡进入温控，下面整行EPaper入口打开原设备详情；保留共享磨砂主题与程序坞／菜单栏行为。小窗口、空数据、大数字和断开后的旧读数均有适配，缺失不显示0。

本周按UTC+8周一至当天，Codex复用官方日桶的已知数值去重合计，不增加查询；Argus／DeepSeek Harness沿用原周汇总。Codex同账号跨Mac仍取最新，不重复相加。官方当天统计延迟仍存在，主页摘要不承诺完整实时消费账单；详细口径见usage-providers.md。

App与Apple芯片DMG版本0.2.26/build26，已更新本机/Applications/HomeCraft.app与项目software/dist/HomeCraft.app。更新前未检测到日常运行实例，未启动新版日常后台；打开App后即可使用。87项回归、Clippy、原生隔离窗口和36组首页布局检查通过，预览为样例数据。未连接BLE／USB或烧录，固件继续0.2.24；个人配置与绑定未改。验证见3Inch7EPaper/artifacts/homecraft-0226-dashboard-validation.json及homecraft-0226-package-validation.json。


0.2.26补充：墨水屏主页现也已更新并app-only部署0.2.26，见overview-0226-validation.json。Mac App未再重启或修改。按用户要求删除所有旧安装包及dist内旧测试App，仅保留dist/HomeCraft.app与software/dist/releases/HomeCraft-0.2.26-macos-arm64.dmg；后续新DMG校验成功后自动清理旧包，旧文档交付路径仅作历史记录。


## 0.2.29 今日／本周／本月（2026-10-24起，当前）

用户要求下方去掉Token字样、增加本月，同时压缩Argus与DeepSeek Harness。Mac首页与墨水屏总览现在显示“今日／本周／本月”，Argus和Harness详情也显示三项；今日保持主层级，周／月同级。墨水屏横屏汇总三列、竖屏三行，Argus横屏三卡、竖屏三行；Harness设备表也有三列。温湿度沿用0.2.28的相同Arial Bold字号、数字右缘与单位基线。这里是月Token，Codex的5小时／周额度界面继续保留，不恢复月额度。

日期按UTC+8：当天、周一、每月1日起算。Argus通过原官方get_token_usage新增一次月范围汇总，include_records:false，确认返回范围和计数；周／月失败互不抹掉当天成功值。Harness复用已解析的官方逐日记录合计当月，结算替换、缓存不重复相加与多Mac按稳定ID覆盖不变。Codex复用同一次account/usage/read已返回的本月有效日桶，同日取最大去重、缺失日不补0，不冒充完整实时账单。缺月为--，真实0保留，旧月不能带入新月。

新增可选mt:[YYYYMM01,total]和monthly_tokens:true能力协商；旧固件不收到mt，原Security2与480字节上限不变。板端Harness数字缓存v1迁移为v2，保留旧日／周／设备记录，月缺失为未知，不动dashboard绑定。iCloud同时输出无mt的HomeCraft-Data-v1和完整HomeCraft-Data-v2（2026-10-27已改为只写 `AI用量/`、`温湿度/` 各一份，旧文件夹只读）；本机旧缓存也继续输出兼容副本，同时间优先保留带月统计的记录。未改变30秒用量读取、5分钟遥测、SD分钟日志、局刷／省电／按键策略。

93项Rust测试、Clippy全目标零警告，140组摘要实际字形边界（含温湿同号／对齐）、220组Argus布局、64组环境／日周月差分、3456组双项与576组三项字号、32组Harness四向1–8设备缓冲检查通过。正式组件288张Mac预览、32张墨水屏样例与隔离原生窗口操作通过，均为合成数据。真实官方读取已得到Argus／Harness月统计及Codex已知月合计；实体新排版观感仍待用户确认。

Mac本机/Applications/HomeCraft.app与项目dist/HomeCraft.app为0.2.29/build29，唯一安装包software/dist/releases/HomeCraft-0.2.29-macos-arm64.dmg；旧包按用户要求清理，不恢复旧候选App。固件0.2.29已app-only写入0x10000且哈希通过，原NVS／绑定／SD保留；两台Mac的Harness缓存迁移后仍devices=2/valid=1。应用1,913,312字节，SHA256 4e29769909bea5baeb8a99d99b457d9517a40516d8c1395bd1f66fabc14c74b0，归档3Inch7EPaper/dist/firmware/HomeCraft-0.2.29-monthly-tokens.bin。验证记录3Inch7EPaper/artifacts/monthly-0229-validation.json；真实同步与最终硬件状态以该记录为准，不以样例代替实体结果。


2026-10-25收尾：Mac最终安装/项目包0.2.29/build29、唯一DMG校验通过，93项Rust与Clippy、288张最终组件预览和最终原生隔离检查通过。本机已启动新版并显示主页，官方Codex／Argus／Harness本月读取成功。设备0.2.29保留原连接，SD1433→1437/write_ok=1、partial=1/count=4/ready=1和671ms差分正常，2台Harness旧缓存迁移后valid=1。本机扫描未发现正在广播的设备，尚未同步到屏幕；用户明确“先不管另外一台”，本轮不再要求暂停另一台或尝试切换，月字段完整上屏验证留待后续。不能把这次本机月读取说成新月字段已通过实板传输。


## 0.2.30 扫描与普通启动配置修复（2026-10-25，当前Mac App）

用户截图为扫描完成但列表为空。只读USB确认板子仍为0.2.29、data=1/link=1；当前NimBLE最大连接数为1，官方Protocomm在连接断开后恢复广播，因此没有广播不能当成设备不在附近。本轮不操作另一台Mac、不重配、不改NVS或固件。

同时实查Finder启动的/Applications/HomeCraft.app使用未绑定的私有Support目录，而旧绑定/设置还在本机项目目录；此前用EPAPER_PROJECT启动成功不代表Finder入口已读回。新增本机600的.data-location.json显式选择已有配置目录，校验绝对路径、目录所有者、非符号链接和现有有效600绑定。选择只保存路径，不复制/轮换秘密，不编译开发机路径进包；新Mac无此本机文件时仍使用原有700的Support目录并自行首次认证。普通启动、CLI/statusLine和旧发送工具继续共用原配置与单实例锁。

扫描仍仅使用已知服务UUID；额外通过CoreBluetooth公开retrieve_peripherals恢复本机已连接服务或认证后保存的设备UUID。恢复项明确标为已记住/暂未收到广播，缓存不证明在线或通过认证，首次自动扫描不自动连接仅恢复的项。原Security2双向认证/服务与端点校验不变，EPair模式不按旧数据绑定连接。UUID缓存.ble-device.json为600，仅含名称与系统UUID，与当前绑定名称匹配后才使用；认证成功后才保存。不打印或返回配对秘密。

已记住但无广播时显示原设备名、广播状态和切换Mac提示，隐藏无意义的配对码表单；蓝牙关闭/权限不可用归为扫描错误，不显示普通空扫描。手动扫描仍尊重暂停。96项Rust、Clippy全目标零警告、300张正式组件样例、隔离原生窗口导航/连接页/关闭重开检查通过。样例不访问账户或BLE。

本机/Applications/HomeCraft.app、项目dist/HomeCraft.app和唯一安装包HomeCraft-0.2.30-macos-arm64.dmg均build30。退出旧的本项目App后更新；不带EPAPER_PROJECT、cwd=/正常启动，已验证原绑定加载且文件内容未变，实际连接页不再显示配对码。当前仍未发现广播/未连接；不能把绑定恢复和UI修复说成已完成BLE连接或本月上屏。用户先前要求另一台先不处理，本轮没有抢占或中断那条连接。实际状态、签名与包白名单见3Inch7EPaper/artifacts/discovery-0230-validation.json。


2026-10-25补充：广播问题现已在固件0.2.31恢复并以实际连接／广播诊断区分；当前固件0.2.32同字号版。HomeCraft仍0.2.30，无需重新配对，原配置／绑定及真实月数据同步已验证。此前未连接、月字段留待验证的记录为历史状态，见ble-0231与typography-0232验证。


## 0.2.33 Codex昨日行（2026-10-25，当前App与固件）

用户最终要求保留5小时／周额度，并增加“昨日”一行同时显示昨日Token和昨天周额度百分比，底部进度条必须保留；不是把Codex改成日周月Token卡。横竖屏均保留三行、HH:MM及原当前5小时剩余分段条，三卡竖屏将小倒计时放在5小时行旁边。历史两项采用共同稳定粗体，标签仅“昨日”，用M／%区分；Argus、Harness与底部三项同字号规则保持。

昨日Token取原官方account/usage/read对应的UTC+8昨日桶，同日去重，不用最新但更老的日期。昨日周额度取已有600数值缓存中，同一已验证账号、昨日最后一次有效官方额度快照，显示已用比例100%-剩余，不能称为昨日新增消耗。账号读取前后仍核对一致；账号未知、缺快照、日期不符或字段缺失各自显示--，真实0保留。不新增官方请求，不读取登录凭据、私有HTTP或用Token推算额度。

新增可选yd:[YYYYMMDD,tokens_or_-1,week_used_permille_or_-1]，仅Codex且需yesterday_usage:true能力协商，旧设备剥离，480字节、Security2及绑定保持。云端／兼容本机数值摘要剥离派生yd，继续保留原w快照，因此旧App无需支持新字段；同账号历史匹配在本机完成。

98项Rust、Clippy全目标零警告、官方方法模拟链路、严格C字段、600组实际文字边界／进度条10段检查（四向、1–3卡、旧数据、缺失、0、极大数）、140摘要／220Argus／64差分回归及原生隔离窗口通过。24张预览为样例，未发实板。实际官方读到昨日20261024的100,015,606 Token、同账号昨日最后周额度已用28.0%，板端EPAPER_YESTERDAY独立回读吻合。

本机/Applications/HomeCraft.app与项目dist/HomeCraft.app已更新0.2.33/build33并恢复原绑定认证、真实同步。固件1,918,304字节，SHA256 8f03c6eb215e48d7c04238535a6942e787f80cc5fdd3829b5515a14dcee821c7，3Inch7EPaper/dist/firmware/HomeCraft-0.2.33-codex-yesterday.bin，已app-only写入0x10000并哈希验证，NVS／SD／省电／广播恢复与局刷保留。唯一安装包software/dist/releases/HomeCraft-0.2.33-macos-arm64.dmg，签名／只读挂载／版本／内容白名单一致，旧0.2.30包已按要求清理。新行实屏观感尚待用户确认，见3Inch7EPaper/artifacts/codex-yesterday-0233-validation.json。


## 0.2.36 SD音乐页兼容

设备显示选择增加“音乐”，识别原协议追加的第12页及13位页掩码；旧页面索引不变。99项Rust测试及Clippy通过。Mac只切页，不传输音乐或读取SD文件；播放器在板端读取/music。部署与最新DMG见software/artifacts/homecraft-0236-release-validation.json。


2026-10-26目录整理：HomeCraft作为协议中心，公共定义移至software/protocol；当前板配套资料移至3Inch7EPaper/.local，安装App的本机.data-location已更新。项目App和DMG同0.2.36已按新目录重建，已安装App仍0.2.36未替换；本次不烧录。详见project-layout.md及软件artifacts/repository-layout-20261026-validation.json。

## 0.3.0 build 47 环境页（2026-10-28）

环境页以位置卡片和多位置趋势为主。卡片显示温度、湿度、在线状态及最后记录时间；点击卡片只看该位置，点击“全部位置”恢复对比。用“编辑位置”进入命名，保存后才修改设置，取消会保留原名称。

趋势可切换最近1、6、24或48小时。移动鼠标显示温度/湿度联动游标和就近采样值，点击或拖动固定时间，方向键逐条查看，Esc或“返回实时”解除固定。超过30分钟的缺口不连线，游标附近没有采样则显示未知，不插值补数。图例保留固定宽度，悬停时图表位置保持稳定。

天气移到趋势之后；设备自己的今日记录与其他Mac来源保留在“设备今日记录与其他来源”展开区域。刷新读数和设备详情入口仍在顶部。蓝牙初始化期间先展示已绑定的位置与本机历史，离线记录标出最后时间；这些缓存不代表在线或新的实测读数。

项目App更新为build 47，旧build 45/46归档，安装目录和旧安装包保留。132项Rust检查、Clippy和三种窗口尺寸预览通过；真实窗口的卡片筛选、时间范围、悬停、固定/返回实时、编辑取消已点测。系统TCC日志确认新签名触发蓝牙授权弹窗；用户允许后，当前build 47两屏已重新认证连接，近期同步与实时温湿度核对通过，见软件验证记录。

## 0.3.0 build 48 汇总与启动（2026-11-15）

各台 Mac 安装同一新版，在“家庭设置 → iCloud 与来源”选择同一个 iCloud Drive 中的 HomeCraft 文件夹。首页展示已知来源统一合计；Codex 同账号去重，Argus 按当前已确认的共享文献库取最新有效观测、只计一次，Harness 按来源电脑覆盖后合并。来源页显示各 Mac 最后更新时间，离线旧数据不会变成今天的读数。各机新增消费需要对应 HomeCraft 运行并完成云同步后才会出现在其他 Mac 上。

“家庭设置 → 启动 → 开机自启动（登录 Mac 后）”使用 macOS 主 App 登录项，macOS 13+ 可用、默认关闭、单独立即生效。系统要求允许时显示按钮前往登录项，App 不自行绕过；取消勾选关闭。登录启动后保持菜单栏同步，普通打开 App 则显示窗口。macOS 12 继续运行 App，自启动控件明确显示所需版本。
