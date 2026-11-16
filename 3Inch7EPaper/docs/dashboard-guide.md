# HomeCraft · EPaper 智能家居终端

Waveshare ESP32-S3-ePaper-3.97 仪表盘：显示 Codex、Claude Code 的额度、DeepSeek 官方 API 余额和 Argus、DeepSeek Harness 的今日/本周/本月 Token，以及板载 SHTC3 温湿度。**当前使用 Mac → 加密 BLE → 墨水屏**，板子不依赖校园网，不需要配置账户密钥。支持自动横竖屏、BOOT/PWR两键和差分刷新，拨轮已禁用。插入SD卡可保存每分钟温湿度，并播放 `music` 文件夹里的音乐；不插卡仍可显示实时数据。

## 开发目录

```text
EPaper/
├── software/          HomeCraft、Mac 工具、图标、安装包
│   └── project.json   选择当前开发的硬件目录
├── 3Inch7EPaper/      这块 ESP32-S3 墨水屏的独立工程
│   ├── board.json     本板描述
│   ├── sdkconfig*     本板编译配置
│   ├── partitions.csv 本板分区表
│   ├── .local/        本机配对、设置和数值缓存（私密）
│   └── backups/       本板及其配套 App 的私有恢复备份
├── docs/              共享说明与既有设计记录
└── 3Inch7EPaper/artifacts/         验证记录与预览
```

以后每款硬件直接增加一个同级目录，独立放置 `board.json`、SDK 配置、驱动、构建、备份与 `.local`；不要把一款板子的引脚、分区或绑定复制成全局默认。`software/project.json` 只选择硬件目录，不存引脚或凭据。目录结构与迁移边界见 [项目结构](../../software/docs/project-layout.md)。

- [Mac 软件开发入口](../../software/README.md)
- [3Inch7EPaper 开发入口](../README.md)

## Mac 蓝牙同步（当前模式）

设备开机广播 `EPaper_B96AD4`，发送程序自动连接，无需在 macOS 系统蓝牙面板手动配对。0.2.19已部署Security 2加密双向连接，原有绑定直接重连，支持真实用量同步、环境遥测和屏幕控制。首次Mac添加改用实体窗口8位屏幕码，无需USB；用户已确认该首次认证成功。API Key 和账户登录令牌只保留在 Mac，蓝牙包只含经过字段白名单检查的数值、状态和时间。温湿度始终由板载传感器读取。

启动：双击 **[HomeCraft.app](../../software/dist/HomeCraft.app)**，扫描并选择墨水屏连接；Apple芯片安装包为 **[HomeCraft 0.2.36 DMG](../../software/dist/releases/HomeCraft-0.2.36-macos-arm64.dmg)**。**主窗口打开时显示程序坞图标；关闭窗口后继续在菜单栏运行**；点击菜单栏图标可显示窗口、立即同步、暂停连接或退出程序。当前 Rust App 已运行，旧 Python 发送器已停止；不需要终端窗口，没有添加开机自启。

日常页面只显示读数、状态和操作；说明集中在“家庭设置 → 帮助”，详细统计和诊断按需展开。默认每30秒读取并发送官方用量，可在App“刷新设置”开关或调整为30–7200秒；手动请求立即排队，保留各官方入口缓存/限流。蓝牙空闲30秒保活；温湿度每30秒采样，温湿度/电源/SD状态和历史每5分钟回传，手动采样立即执行并回传。Mac睡眠时暂停、唤醒后重连；板子重启后从SD恢复环境历史并等待新的用量快照。

新Mac只需安装HomeCraft并允许蓝牙，在App扫描并输入一次屏幕码；无需蓝牙插件、Python、Rust、Homebrew或手机配网App。账户数据由这台Mac自己的官方客户端/登录提供，与设备连接独立。

“温控”显示真实温湿度、电量/USB/充电状态、SD条数和UTC+8当天曲线（15分钟均值，缺测断线）；家庭EPaper卡片同时显示温湿度。“显示”及设备概览可切换可用页面、锁定四个方向/恢复IMU自动方向、整屏清除残影。“采样并更新”会触发一次新测量并立即评估当前画面的差分更新，收到对应完成回执后才提示成功；像素未变化时明确提示，清除残影仍为独立全刷。App只发送控制和白名单数字，不读取设备配置或任意SD文件。换页/旋转全刷，普通数据继续走原有差分策略。

**仅使用官方公开的数据入口**：Codex 由本机官方 `app-server` 的 `account/rateLimits/read` 和 `account/usage/read` 提供额度、账户累计 Token、单日峰值和可选按日数据；Claude 只接收官方 Claude Code `statusLine` 的额度与当前上下文 Token，不读账户令牌、不访问未公开 HTTP 路径。当前 Codex 已真实验证；Claude 状态栏接收已配置，但未检测到本机 Claude Code 命令，等待官方客户端真实数据。DeepSeek 仍未配置 Key。

累计 Token 与单日峰值在 Mac App 中显示；墨水屏底部采用 **今日 / 本周 / 本月** 三项同字号粗体，合计各自周期内已取得的有效统计，缺失显示 `--`。未提供数据不计为 0，也不混入上下文或 lifetime 数值；它不代表所有服务的完整消费账单。总览以“5小时”大数字为主，“周额度”缩小，数字靠右；右上角显示蓝牙图标（斜杠表示断开）、时间及电池/USB状态。详见 [大字仪表盘与合计范围](display-redesign.md)。旧 Python 命令仅保留作兼容工具，与 Rust 共用发送锁，不能同时运行。

Argus通过官方本机STDIO MCP `get_token_usage`读取当前文献库汇总。卡片以同字号粗体显示今日、本周、本月Token（UTC+8、周一及每月1日零点起）；周统计与额度独立，缓存不重复计数。Codex的官方日统计可能滞后；未返回今天时只在App展示最近统计日，不把昨天填进今日合计。 Argus详情页增加输入/输出/缓存命中率与调用网格，以及当天模型、供应商Token排行和占比；分类来自同一次官方MCP汇总，不重复计入今日合计。官方只返回供应商ID时显示短ID。

新 Mac 的绑定和详细安全边界见 [蓝牙数据传输说明](../../software/protocol/ble-data.md)。`3Inch7EPaper/.local/.ble-pairing.json`、`3Inch7EPaper/.local/.ble-settings.json` 都属于本机私密文件，不提交、不分享。

## 按键与方向

| 手势 | BOOT 键 | PWR 键 |
| --- | --- | --- |
| 单击 | 下一页 | 上一页 |
| 双击 | 请求同步额度并全刷 | 手动旋转 90°，锁定方向 |
| 三击 | 自动旋转开/关；网络页开关配网 | 返回总览，恢复自动旋转和刷新 |
| 长按 | **3 秒：蓝牙配网开/关，进入网络页** | **约3秒后松手：正常关机；1.2至不足3秒松手：暂停/恢复** |

页面依次为总览、已启用的服务详情、温湿度趋势、音乐、网络设置、按键说明，未配置服务自动跳过。仅Codex启用时共6页，BOOT单击两次可从总览到温湿度趋势。音乐页有独立按键映射，见下方。网络设置页的BOOT三击用于开关配网热点，音乐页用于离开；其他页面三击用于自动旋转开关。连击每次间隔不超过400ms；单击/双击需等待连击窗口结束后执行。全刷需要数秒，但独立输入任务会继续收集按键。**PWR持续按住8秒执行硬件关机**，与1.2秒的界面长按功能不同。BOOT是启动配置引脚，重新上电时不要一直按住。

蓝牙数据模式下，网络页的 **BOOT 双击**用于打开 Mac 配对窗口；音乐页双击播放/暂停，其他页面双击请求更新额度。运行时 **BOOT 长按 3 秒**切换到手机 Wi-Fi 配网，完成后恢复 Mac 蓝牙接收。

QMI8658 的加速度计检测重力方向，支持 0°/90°/180°/270°。方向经低通滤波、阈值和约 1 秒稳定等待后切换；平放、斜放在方向边界或快速移动时保持原方向。横屏使用 800 × 480 布局，竖屏使用 480 × 800 布局；旋转始终执行全刷。音乐页的按键以独立映射为准；其余页面PWR 双击锁定后，BOOT 三击可恢复自动方向。IMU 不可用时可继续用 PWR 双击手动旋转。

暂停冻结正文的自动画面更新，采样和蓝牙/USB 数据接收继续，按键仍可操作，不代表深度休眠。电源标志仍会在USB插拔时快速更新。方向锁定、暂停和页面选择目前不跨重启保存。

**正常关机已上板验证：** PWR按约3秒后松手，等待记录关闭、网络停止及关机画面全刷后自动关机；PWR按1.2至不足3秒后松手仍是暂停/恢复。关机页显示睡着的小屏幕与大字“已关机”，断电后保持。USB与纯电池供电下的关机、保持画面及再次开机均已由用户确认；8秒硬件强制关机仍保留，突然断电不能保证画新页面。详见 [关机设计与验证边界](shutdown.md)。

## SD 音乐

0.2.36 增加“音乐”页，自动读取 SD 卡根目录 `/music`，支持 MP3、WAV、FLAC、AAC、M4A、Ogg Vorbis / Opus。放好歌曲后启动设备即可读取；不会上电自动播放。M4A 需文件信息在音频数据前，受保护的平台缓存不支持。

音乐页：**BOOT 单击下一首，双击播放/暂停，三击离开**；PWR 单击上一首，双击音量加，长按 1.2 至不足 3 秒松手音量减，三击停止回主页。PWR 约 3 秒关机保留。完整格式、限制和验证见 [SD 音乐说明](music.md)。

## 备用 Wi-Fi 配网

这是保留的备用实现。用户因校园网访问限制改选 Mac 蓝牙传输，**当前数据模式不会通过板子的 Wi-Fi 查询账户**，也不自动尝试校园网认证；以下说明用于手动启用的 Wi-Fi 配网和网页高级设置。

在任意页面**长按 BOOT 3 秒**开启手机配网。安装乐鑫 **ESP BLE Provisioning**（[iPhone](https://apps.apple.com/app/id1473590141)、[Android](https://play.google.com/store/apps/details?id=com.espressif.provble)），在应用内扫码，选择自己的 **2.4 GHz Wi-Fi** 并输入密码。手机无需切换 Wi-Fi。该应用只负责配置网络，不负责转发额度。

二维码内含本次随机配对码、设备名、用户名 `wifiprov` 和 Security 2 设置；传输使用官方 SRP6a + AES-GCM。每次开启更换配对码，窗口 10 分钟后关闭，成功后约 30 秒自动结束。新 Wi-Fi 获得 IP 后才保存；失败保留旧网络，可在应用重试。不要分享实屏配对二维码。

网页作为账户高级设置入口保留：在设备“网络设置”页**三击 BOOT**，关闭蓝牙配网并打开 `EPaper-…` 加密热点；连接后打开 `http://192.168.4.1`，输入屏幕配网码。网页仅从设备热点接口访问，每次打开更换热点密码和网页码，10 分钟后关闭。日常给 Wi-Fi 配网优先使用官方应用。具体流程和核实状态见 [蓝牙配网记录](ble-provisioning.md)。

独立 HTTPS 查询和设备授权工具保留在源码中，属于此前的实现。**当前固件配网结束后恢复 Mac 蓝牙数据模式，不执行板上账户查询；日常无需给板子授权或导入 API Key。** 手机应用配网及校时已通过，但校园网访问三家服务、板上账户查询和令牌续期没有完成验证。

## 数据与刷新

| 服务 | 显示内容 | 当前限制 |
| --- | --- | --- |
| Codex | 墨水屏显示剩余额度，标签为“5小时 / 周额度”；App 另显示账户 Token 统计 | 墨水屏不显示月额度；Token 不当作额度百分比 |
| Claude Code | 官方状态栏返回的短期/每周额度及当前上下文 Token | 需官方 Claude Code 产生状态栏数据；缺少的月额度不推算 |
| DeepSeek 官方 API | CNY 优先、否则 USD 的可用余额 | 官方接口没有周/月套餐比例，不用余额变化推算月花费 |

Claude 无需向本程序提供账户凭据；DeepSeek API Key 只在 Mac 配置，方法见 [蓝牙数据传输说明](../../software/protocol/ble-data.md)。字段语义见 [用量接口记录](../../software/docs/usage-providers.md)。设备 Flash 未加密，里面的蓝牙配对秘密同样需要保护，不要分享完整 Flash 备份。

SHTC3每30秒采样，界面按30–60秒合并更新。新双RAM差分路径经用户确认后已启用，重启保留；温湿度、曲线、额度与时钟变化共用该路径，实测约0.66–0.69秒。切页、旋转、开机、显示失败恢复时全刷；普通卡片变化持续差分，不再按20次/30分钟自动全刷；音乐页外BOOT双击也可强制全刷。旧打包矩形局刷不再使用。温度未做板体发热补偿，长期供电时可能高于室温。

USB插拔时单独快速局刷顶部：插电显示插头+USB，拔掉显示实际电量，空闲时约1秒可见；全刷忙时等待当前操作。供电标志每100ms只读检测，完整电量仍30秒及插拔后读取一次；新增读取有额外开销，尚未测续航差值。实板插拔已由用户确认正常，详见[电源快速局刷](power-events.md)。

SD卡每分钟保存一条，按UTC+8日期写入`/EPaper/env/YYYYMMDD.csv`，包含时间、温湿度、有效状态和电量。正常重启恢复当天记录，午夜自动新建日期文件；挂载失败不格式化。环境页显示当天两条趋势，采用15分钟有效采样均值，原始逐分钟数据保留在CSV。暂停界面不会停止记录。SD缺失时暂存在当天RAM，断电前未写入的数据不能保留。详情与实测边界见 [板子端升级记录](hardware-upgrade.md)。

旧 `software/tools/codex_bridge.py` 作为可选 USB 兼容工具保留。日常 USB 线可只供电，额度查询由 Mac 完成，数据经蓝牙发送。

## 编译与烧录

本机 ESP-IDF v6.1 的环境入口：

```bash
cd 3Inch7EPaper
source /Users/qichengwen/.espressif/tools/activate_idf_v6.1.sh
idf.py build
idf.py -p /dev/cu.usbmodem14201 app-flash
```

烧录前先停止占用串口的桥接程序。工程配置为ESP32-S3、16MB Flash、8MB Octal PSRAM，通过USB Serial/JTAG通信。已有正确分区表的本板升级使用`app-flash`仅更新应用，保留绑定与配置；首次写入启动程序/分区表另按环境记录操作。原始Flash备份存放在本地`3Inch7EPaper/backups/`，不会纳入版本控制。2026-10-21睡前构建已在早晨调试时烧录并验证；当前部署摘要见`3Inch7EPaper/artifacts/firmware-validation.json`，睡前记录保留在`3Inch7EPaper/artifacts/offline-environment-validation.json`。

只检查额度接口，不打开开发板串口：

```bash
/opt/homebrew/bin/python3 software/tools/codex_bridge.py --once --stdout
```

## 项目资料

- [硬件引脚、总线与核实状态](hardware.md)
- [墨水屏缓冲、刷新与上游接口](epaper-usage.md)
- [本机开发环境](../../software/docs/development-environment.md)
- [三家用量接口及独立联网记录](../../software/docs/usage-providers.md)
- [官方应用蓝牙配网、安全边界与核实状态](ble-provisioning.md)
- [Mac 蓝牙数据传输、运行和安全边界](../../software/protocol/ble-data.md)
- [Rust 菜单栏 App、设备选择与官方 Token 数据](../../software/docs/mac-app.md)
- [墨水屏大字布局、品牌标识与今日 Token 合计](display-redesign.md)
- [差分刷新、SD分钟记录与当天趋势](hardware-upgrade.md)
- [逐项开发清单与待测边界](../../TASKS.md)
- [关机画面、正常关机与强制断电边界](shutdown.md)
- [网页便捷配网备选设计（部分尚未实现）](wifi-provisioning-design.md)
- [上游驱动与字体来源](../THIRD_PARTY.md)

`3Inch7EPaper/main/board.c` 管理共享 I²C、SHTC3、QMI8658、PMU 按键和屏幕供电，`3Inch7EPaper/main/controls.c` 负责按键手势与方向滤波，`3Inch7EPaper/main/main.c` 管理任务、交互和刷新，`3Inch7EPaper/main/dashboard.c` 绘制横竖屏页面。`3Inch7EPaper/main/network.c` 负责独立联网、配网与凭据，`3Inch7EPaper/main/usage.c` 解析用量，`software/tools/configure_device.py` 用于一次性 USB 配置。GPIO39 用于音乐功放；音频/IMU 定义冲突仍记录，当前固件通过 I²C 轮询 IMU，不启用其 GPIO39 中断。

## 验证与预览

桥接数据转换和协议测试：

```bash
/opt/homebrew/bin/python3 -m unittest discover -s software/tests -v
```

离线温湿度与曲线刷新回归（需要Clang及已经下载的IDF托管组件，不访问硬件）：

```bash
sh 3Inch7EPaper/tools/test_environment_refresh.sh
```

`3Inch7EPaper/tests/test_controls.c`检查手势与方向；`3Inch7EPaper/tests/test_environment_csv.c`检查分钟CSV；`3Inch7EPaper/tests/test_refresh_policy.c`检查差分/全刷恢复策略。`3Inch7EPaper/tests/test_environment_refresh.c`通过真实渲染帧检查温湿度、曲线和SD状态变化的差分策略，四方向共48组。`3Inch7EPaper/tests/render_dashboard.c`最新完整矩阵为14种状态×12页×4方向=672组。最新布局预览见 [单服务总览](../artifacts/hardware-data-partial-0-0.png)、[温湿度趋势](../artifacts/hardware-data-history-0-4.png) 和 [竖屏趋势](../artifacts/hardware-data-history-90-4.png)。预览数据是合成测试样例，实际固件没有预置额度或温湿度。

当前0.2.19省电策略：CPU动态40–80MHz、BLE Modem-sleep及自动Light-sleep，BLE数据模式停止Wi-Fi射频；USB连着电脑时保留调试保护，电池运行时实际睡眠。来源、验证边界与完整节奏见[省电策略](energy-policy.md)。
