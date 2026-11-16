# EPaper 开发环境记录

2026-10-25电源标志0.2.35：实查EIM选中IDF6.1／IDF Python3.12.15；768组电源补丁渲染、64组环境差分、140组主页及手动采样/严格协议检查通过。应用区更新通过哈希，原HomeCraft0.2.33恢复认证、SD与局刷保留。用户完成实际USB拔插并确认正常，稳定事件到显示完成708ms/760ms；新增读取的整板功耗尚未测量。见power-events-0235验证记录。

2026-10-25音频试听：重新实查EIM选中IDF6.1、IDF Python3.12.15。独立临时工程使用官方esp_codec_dev1.6.2构建，原项目build与仪表盘源码未改；仅应用区试听后写回原factory区，恢复写入哈希通过，dashboard NVS逐字节未变、原App认证与SD/局刷恢复。详见[喇叭试听](../../3Inch7EPaper/docs/audio-audition.md)与3Inch7EPaper/artifacts/audio-audition-20261025-validation.json。

记录日期：2026-10-20。环境状态会变化，继续安装、编译前重新运行版本与路径检查。

## 当前进度

- 开发机为 macOS，使用 Apple Silicon Homebrew，前缀 `/opt/homebrew`。
- Homebrew 实际读取版本为 `7.0.9`。
- EIM 图形版 `0.20.1` 已安装到 `/Applications/eim.app`，同时提供 `/opt/homebrew/bin/eim` 命令，无需再装一份 CLI。
- 已通过 Homebrew 在本机安装 `python@3.12`（3.12.15），并将 `/opt/homebrew/bin/python3`、`pip3` 链接到该版本。未替换 Apple 自带的 `/usr/bin/python3`。
- EIM CLI 的 Python Sanity Check 已实测全部通过：Python Version、pip、venv、Standard Library、ctypes、SSL/HTTPS；检查后已退出向导，没有开始安装 ESP-IDF 或工具链。
- 2026-10-20 通过 `eim list` 确认 ESP-IDF **v6.1** 已安装且为选中版本，路径为 `/Users/qichengwen/.espressif/v6.1/esp-idf`，EIM 状态为 `[ok]`。本项目仪表盘已在该版本编译通过，并于当日烧录到开发板；Boot 日志确认启动成功。

官方板卡教程当前要求 IDF 5.5.0 以上，示意版本为 5.5.2；上游 `08` 示例的 `3Inch7EPaper/sdkconfig` 和锁文件记录 5.4.1。构建时以实际所选版本进行验证，不以 manifest 的宽泛最低版本认定兼容性。[微雪 IDF 教程](https://docs.waveshare.net/ESP32-S3-ePaper-3.97/ESP-IDF/)

## Python 版本与路径

2026-10-20 本机安装完成后的解释器状态：

| 解释器 | 版本 |
| --- | --- |
| `/opt/homebrew/bin/python3` | 3.12.15，本次命令环境中的默认 `python3` |
| `/usr/bin/python3` | 3.9.6，实际执行路径解析到 Xcode 内的 Python |
| `/Users/qichengwen/miniconda3/bin/python3` | 3.14.7 |
| `/Users/qichengwen/.openscience/conda/envs/python/bin/python3` | 3.11.16，属于其他工具管理的环境 |

安装前，当前命令环境的 `python3` 优先找到系统 3.9.6；安装后已解析到 `/opt/homebrew/bin/python3`。用户终端或 Finder 启动的 GUI 可能有不同 PATH，不能仅凭终端的 `(base)` 判断 EIM 实际用了哪个解释器。

用户这版 EIM 界面明确要求 `3.10 <= Python < 3.14`，并要求 pip、venv、SSL。最新在线文档已说明支持 Python 3.14，与截图存在差异；当前采用已经通过 EIM 检查的本机 Python 3.12。

早期Conda环境已归档到 `software/.cache/pre-layout-conda`，不直接激活，因为其中含旧绝对路径。当前ESP-IDF使用EIM自身解释器，不需要该环境。需要重建通用Python环境时，从工作区根执行（本次未执行安装）：

```bash
conda env create -p software/.conda -f software/environment.yml
conda activate ./software/.conda
```

EIM 现在可直接使用本机 Homebrew Python，不必激活项目 Conda 环境。启动前完全退出原 EIM，再从终端执行 `PATH="/opt/homebrew/bin:$PATH" eim gui`，明确优先使用本机 Python 3.12。

这一查找差异及从终端启动 GUI 的方法见 [EIM 官方 FAQ](https://docs.espressif.com/projects/idf-im-ui/en/latest/faq.html#why-does-eim-find-the-wrong-python-on-macos)。用户已确认安装问题解决，`eim list` 已确认 v6.1 安装成功；实际工程已完成编译与烧录，运行验证见下节。

## Homebrew 安装来源

用户已解决 `espressif/eim` 的未信任来源问题。若其他机器遇到同样报错，可对具体 cask 授权：

```bash
brew tap espressif/eim
brew trust --cask espressif/eim/eim-gui
brew install --cask espressif/eim/eim-gui
```

安装器列出的 macOS 依赖包括 `libgcrypt glib pixman sdl2 libslirp dfu-util`。前五个主要用于 QEMU，`dfu-util` 用于 DFU 烧录；EIM 会检查依赖。用户截图中的 Prerequisites Check 已通过，但没有逐项记录每个包的安装版本。[EIM 依赖说明](https://docs.espressif.com/projects/idf-im-ui/en/latest/prerequisites.html)

## 仪表盘构建与上板记录

2026-10-20 使用 ESP-IDF v6.1 构建 `epaper_codex_dashboard` 成功，初始应用体积 289,776 字节；启动、分区表和应用均通过 esptool 写入后的哈希校验。开发板识别为 ESP32-S3 rev0.2、16 MB Quad Flash、8 MB Octal PSRAM（80 MHz），启动内存测试通过。当前 USB 端口 `/dev/cu.usbmodem14201`，重新插拔后应重新检查。

烧录前已读取完整 16,777,216 字节原始 Flash，存放于 `3Inch7EPaper/backups/original-20261020-44b176b96ad4.bin`，旁边的 JSON 保存长度与校验值。SHA-256 为 `b050ae75b48fa25c894860c75f649d05b4053730087d2316576fbe7c2f23929f`。备份包含原固件与设备本地数据，留在本机，不提交或分享。

如需恢复，先停止桥接程序并确认串口，使用 IDF 环境执行以下命令；这会恢复整个 Flash 的备份内容：

```bash
python -m esptool --chip esp32s3 --port /dev/cu.usbmodem14201 --baud 460800 write-flash 0 3Inch7EPaper/backups/original-20261020-44b176b96ad4.bin
```

日常编译、烧录和运行 Mac 同步程序的命令见 [README](../../README.md)。

本机 `/opt/homebrew/bin/codex` 版本为 `0.141.0`。Mac 桥接已实际读取账户额度，开发板返回 `EPAPER_ACK` 并完成局部刷新；SHTC3 连续采样正常。桥接为当前运行的终端进程，未配置开机自启；停止后按 README 的一行命令重新启动。

初版局刷经用户反馈有文字错乱或残影，随后停用局刷并重新编译、烧录，当时全刷应用大小 289,232 字节。整屏全刷版本的真实额度接收和刷新日志正常，用户明确确认显示恢复正常。当前固件哈希及验证状态保存在 `3Inch7EPaper/artifacts/firmware-validation.json`。

软件验证：ESP-IDF v6.1 编译通过；Python 额度转换与协议 6 项测试通过；C 协议边界、画布边界和四种界面状态在 AddressSanitizer/UndefinedBehaviorSanitizer 下检查通过。首屏日志保存在本地 `3Inch7EPaper/artifacts/boot-20261020.log`，布局预览保存在 `3Inch7EPaper/artifacts/*.png`。

## 双按键和自动旋转增量

2026-10-20 增加 BOOT/PWR 单击、双击、三击、长按，禁用拨轮 GPIO，加入 QMI8658 重力方向切换与手动锁定。最终构建应用 301,376 字节，已烧录并通过 Flash 哈希校验，运行时 IMU 地址 `0x6B`、ID `0x05`，连续加速度数据正常。按键和旋转均保留整屏全刷。Mac 桥接已支持开发板发起 `EPAPER_REFRESH`，按最短 15 秒间隔提前查询额度。

Python 测试增至 8 项，新增 USB 分段消息和过长行丢弃检查；C 手势、方向稳定判定和四向像素映射在 ASan/UBSan 下通过；4 种状态 × 4 页 × 4 个方向共 64 个渲染组合通过边界检查。横竖屏布局已检查预览，最新启动日志见 `3Inch7EPaper/artifacts/boot-controls-20261020.log`，实体连击与旋转朝向仍等待用户操作确认。当前桥接仍为手动启动的终端进程，未配置开机自启。

## Wi-Fi 独立查询增量

2026-10-20 Wi-Fi 版本使用 ESP-IDF v6.1 构建成功，应用体积 1,175,472 字节，已烧录启动；配置增加独立 `dashboard` NVS 分区。cJSON 依赖固定为 `espressif/cjson 1.7.19~2`，版本见 `3Inch7EPaper/dependencies.lock`；从官方组件仓库获取，JSON 最大嵌套深度设置为 32。启用完整证书包、2 个 NTP 服务器，HTTP 请求头上限 2048 字节。

启动日志保存在 `3Inch7EPaper/artifacts/boot-wifi-20261020.log`。确认 STA/SoftAP、DHCP、配网服务与网络页首屏全刷正常，非法 USB 配置被拒绝。网页在本机浏览器做过排版检查，调试用本地 HTTP 服务器已停止。电脑上的 `codex_bridge.py` 也已停止；Wi-Fi 日常运行不需要它。

Python 测试共 11 项，覆盖一次性授权文件字段和“不复制电脑 refresh token”。C 用量结构测试验证三家响应、缺少窗口、月度金额与订阅额度的区分及 UTC 恢复时间。真实家庭 Wi-Fi、设备 HTTPS 查询和自动续期待用户完成配网/授权后继续验证；不要将其记录为已通过。

## 当前 Mac 蓝牙数据版本

2026-10-20 用户改选 Mac 获取额度、BLE 发送，替代此前的 Wi-Fi 独立查询目标。最新应用为 1,512,800 字节，已使用 ESP-IDF v6.1 编译、烧录并通过 Flash 哈希校验；文件摘要及当前验证状态见 `3Inch7EPaper/artifacts/firmware-validation.json`。官方手机配网与 SNTP 已通过，板上账户 HTTPS/续期未验证，当前不启用。

项目 `software/.ble-venv/` 使用 Homebrew Python 3.12.15，BLE/加密依赖见 `software/tools/requirements-ble.txt`，本机完整版本快照见 `software/tools/requirements-ble.lock`。发送程序为 `software/tools/ble_bridge.py`，启动/停止命令见 README；当前持续运行，未安装开机自启。旧 USB 桥接未运行。本机蓝牙授权已通过，实际设备名称为 `EPaper_B96AD4`；USB 仅在首次导出配对秘密时需要。

真实 Codex 读取、Security 2 认证及加密传输、错误配对拒绝、重启自动重连均已通过；用户确认当前实屏显示正常。Python 15 项测试通过，包含快照字段白名单、凭据排除和 SRP 前导零回归；C 用量解析、手势、方向与画布检查通过 ASan/UBSan，界面验证覆盖 8 种状态 × 7 页 × 4 个方向共 224 个组合。Claude/DeepSeek 没有真实凭据，其账户查询不记为已验证。

配对文件 `3Inch7EPaper/.local/.ble-pairing.json` 权限为 600，不记录其内容；USB 的 `EPAPER_PAIR` 原始回复不可保存为普通日志。运行日志用 `software/tools/codex_bridge.py` 的 POSIX `UsbLink` 读取，避免串口工具打开/关闭时复位设备。账户密钥只留在 Mac，设备保存蓝牙认证秘密；未启用 Flash 加密。

## Rust 菜单栏 App（当前 Mac 入口）

同日按用户要求新增 `software/mac-app/`，Rust 1.95.0 / Cargo 1.95.0 构建。`software/tools/build_mac_app.sh` 生成 `dist/EPaper 蓝牙助手.app`，本机签名并写入蓝牙用途说明，二进制约 9.3 MiB。直接使用 CoreBluetooth，无 Python 子进程。关闭窗口仅隐藏，菜单栏继续同步；用户已确认正常。没有添加开机自启，旧 Python 发送器已停止，App 与兼容工具共用单实例发送锁。

仅采用官方公开接口：Codex app-server 的额度与 Token 查询已实测；Claude Code 官方 statusLine 接收已写入本机原先为空的状态栏配置，保留其他设置，目前未检测到 Code CLI，等待真实会话。Claude 不再读取账户凭据或访问未公开 HTTP 路径。Rust 6 项测试与实板 Security 2 / 真实快照回执通过；原 Python 15 项兼容测试通过。本次未修改板上固件。使用、统计含义和限制见 `software/docs/mac-app.md`。

## 墨水屏大字版部署

2026-10-20 用户明确授权短暂退出菜单栏 App、烧录后自动恢复。大字版应用 1,606,528 字节，通过 `idf.py app-flash` 仅写入 0x10000，写入哈希验证通过，NVS 与绑定保留。Rust App 已更新并恢复持续运行，真实 Codex 加密数据与总览全刷日志均通过。新增 d/ld 官方日统计及新旧固件能力协商，8 项 Rust 测试、252 种 UI 状态/方向组合和 C 日合计边界检查通过；用户随后确认实屏字号、布局和文字均正常。详见 `3Inch7EPaper/docs/display-redesign.md`。

2026-10-20 两项额度版：用户要求移除月额度并采用“5小时额度 / 周额度”。应用为 1,640,544 字节，总览64 px、详情80–96 px，已通过252种 UI 组合与 app-only 写入哈希校验；烧录后重启既有菜单栏助手并恢复真实同步，未修改绑定或账户配置。实屏新布局尚未单独确认。

2026-10-20 5小时优先版：标签改成“5小时 / 周额度”，右对齐主数字96 px（100%时按宽度回退）、周数字48/40 px，Token80 px，新增蓝牙图标、时间和只读PMU电量。应用1,681,552字节，336种UI组合通过ASan/UBSan；app-only写入哈希验证通过。菜单栏助手已恢复，设备确认真实Codex正常数据和总览全刷，PMU连续报告USB接入、电池存在、100%、未充电。未改账户配置或充电参数；最新实屏视觉待用户观察，电池电量准确度及充放电未测试。

同日日期位置微调：时间保留右上角，日期移到页脚页码旁。应用1,681,728字节，28个页面/方向组合通过ASan/UBSan与预览检查；app-only烧录哈希校验通过，菜单栏助手已恢复真实Codex同步及总览全刷，实屏视觉待用户观察。

## 2026-10-21 SD与差分刷新硬件增量

ESP-IDF v6.1构建应用1,778,352字节，SHA256 `44f9d48d8ff21da0ec9fd319cbdcc85a15220156d55906f10b591690a65cac48`，已app-only写入0x10000并校验，NVS和绑定保留。稳定菜单栏App恢复真实Codex同步；Rust源码中的Argus初稿未部署。

672种页面/状态/方向渲染和C协议/CSV/刷新恢复测试通过。SDMMC4位20MHz挂载，每分钟写入、重新打开CSV、20261020→20261021切日及正常重启后恢复当天记录均已实测。第二轮局刷由用户观察并明确确认正常，随后启用并写入NVS；重启后仍为partial=1，真实数据更新记录689ms差分。完整结果见`3Inch7EPaper/artifacts/hardware-upgrade-validation.json`及`3Inch7EPaper/docs/hardware-upgrade.md`。

用户随后拔走硬件休息，要求离线继续，明早一起调试。将布局判定提取为共用函数，新增48组真实环境渲染帧回归并通过ASan/UBSan，温湿度、曲线、SD状态和午夜清图仍选差分。新构建编译成功但未烧录，文件摘要见`3Inch7EPaper/artifacts/offline-environment-validation.json`；`3Inch7EPaper/build/epaper_codex_dashboard.bin`现在是这个候选构建，不要用其新哈希替换已烧录版本的验证事实。没有继续连接USB或改动固件上的NVS。明早待验证实际环境变化的局刷画质及横竖屏趋势。

## 2026-10-21早晨实机收尾与Argus

睡前候选已按用户要求app-only烧录，NVS/绑定及SD保留。用户确认总览、温湿度/曲线局刷正常，USB及纯电池均可正常关机、保持关机图并PWR开机，SD455条恢复后继续增加且BLE自动重连。没有测量关机电流或做异常掉电破坏测试。

更新后的Argus已通过官方本机MCP get_token_usage查询当前文献库今日及本周Token，Rust10项测试通过并打包签名部署。Argus卡片日/周字号按最新反馈与Codex额度层级一致，用户确认“合适，显示正常”。底部明确今日Token合计，40种状态/方向/页面渲染通过ASan/UBSan；环境刷新48例保持差分。今日Token只加同日可得数据，不把Codex昨日或累计当成今天。

最新固件、App摘要及真实运行日志统一记于`3Inch7EPaper/artifacts/morning-validation.json`、`3Inch7EPaper/artifacts/firmware-validation.json`与`3Inch7EPaper/artifacts/argus-validation.json`，不再把前文睡前候选当作未烧录。完整Apple Home风格App改版仍是后续任务。


## 2026-10-21晚 0.2.8双向设备固件

重新核对本机IDF v6.1、Python3.12.15与激活脚本，成功编译并app-only部署0.2.8。组件管理器读取进程信息需在允许该能力的环境构建；激活脚本虽然输出成功但可能返回1，构建不使用&&依赖其退出码。新增esp_app_format依赖供安全版本诊断EPAPER_INFO。

新固件1,858,496字节，SHA256 c79f52b8369d01cc9b19287a3110fdfeb54ab05a16dd593fd45e7051ccc0a1e3；0x10000应用写入hash通过，NVS和SD保留，原绑定通过Security 2重连。首次8位码/实际两台Mac交替独立记录，不以原绑定重连代替。当前状态见3Inch7EPaper/artifacts/firmware-validation.json和homecraft-028验证记录，上述早期离线/未部署文字为历史。


2026-10-21 Token层级修复：重新核对Python3.12.15与ESP-IDF v6.1，固件0.2.12编译/应用分区烧录和写入哈希通过。应用1859408字节，SHA256 `4e840bbc88d74bde3aa62dd0bb152e73ac1e46d061f1cb722a6efcb2b74b987b`。HomeCraft仍0.2.11，已恢复真实Codex/Argus加密同步；SD由1153条恢复后增长到1156条，partial=1保留。3,456组字号、192组四向渲染及48组环境差分回归通过。实屏新字号等待用户观察；记录`3Inch7EPaper/artifacts/token-hierarchy-0212-validation.json`。

2026-10-21日期顶部版：Python3.12.15/IDF6.1复核，固件0.2.13应用1859424字节，SHA256 `c6451238d06cabcb366fa401a64c1dedf6f47d95d2be63b5d18864f6e606e89c`，app-only写入哈希通过；真实BLE用量同步恢复。见`3Inch7EPaper/artifacts/date-header-0213-validation.json`。


2026-10-21 Argus详情分类版：复核Python3.12.15与ESP-IDF6.1，0.2.15应用1,871,104字节，SHA256 `4d30770a2f9a2e37bf430b0d1cf51d5fc3eedaf738f7c955cdb4b111e29d7519`，app-only写入0x10000并通过哈希校验；保留NVS/SD。HomeCraft0.2.15真实重连，安全诊断确认当天分类matched=1、SD正常写入及partial=1。已编译和到板验证与用户实屏视觉确认分开记录，见3Inch7EPaper/artifacts/argus-detail-0215-validation.json。


2026-10-21主动采样版：复核Python3.12.15、ESP-IDF6.1，固件0.2.17为1872480字节，SHA256 `c8abe9a06e55b37f1b6a5346790dce24c38c78231353ce706c0559c69c52d52f`；app-only写入哈希通过，NVS/绑定/SD保留。HomeCraft0.2.17已恢复，4次真实采样及667/685ms差分日志通过，当前验证见manual-sample-0217-validation.json。Mac安装包不需要用户安装这些开发工具；项目外空配置/系统PATH启动检查单独记录于homecraft-0217-portability-validation.json。


2026-10-21 0.2.19省电版：本机IDF6.1激活与Python环境复核，开启PM/Tickless/BLE Modem-sleep/主晶振保留及USB主机自动轻睡保护；应用1907184字节，SHA256 8baa7b45cac57b27cd6a8d0585c79eb80d748f64d5af542239c7f0bb924f6ea7，app-only写入哈希验证，NVS/SD/绑定保留。用户纯电池一分钟后读取到真实轻睡累计计数；完整记录见3Inch7EPaper/artifacts/homecraft-0219-energy-validation.json，未测实际电流。


2026-10-23 Codex倒计时固件：重新核对IDF v6.1和IDF Python3.12.15，正常构建0.2.23。组件管理器在沙箱内读取父进程受限，获准后完成离线构建；没有修改开发环境。字体再生使用Codex捆绑Python/Pillow，保留全部数字与粗体字体。用户随后授权连接烧录，app-only写入哈希通过；原应用/绑定/SD/局刷恢复，验证见3Inch7EPaper/artifacts/codex-reset-0223-validation.json。


2026-10-24：再次核对IDF6.1及IDF Python3.12.15，固件0.2.24编译通过；字体使用已加载的捆绑Python/Pillow再生。220组Argus实际字形防重叠、16组分类差分、56组环境/倒计时差分、3456组字号、16组时间边界及手势/方向检查通过。固件包含Argus详情、HH:MM及QMI低功耗/减少重复计算；交付与实板结果见3Inch7EPaper/artifacts/argus-energy-0224-validation.json。


2026-10-24总览0.2.26：重新核对EIM选中IDF6.1、IDF Python3.12.15及/dev/cu.usbmodem14201。140组摘要实际文字边界／60组差分／220组Argus回归通过ASan/UBSan，24张四向合成预览。构建1,910,992字节，app-only写入0x10000并通过哈希；原BLE连接、SD1370→1376及671ms差分成功。未修改开发环境或NVS／SD。记录overview-0226-validation.json。


2026-10-24温湿度数字粗体0.2.27：实查EIM选中IDF6.1、IDF Python3.12.15。捆绑Python/Pillow重生成真实Arial Bold字形并补入°；140／60／220组C回归、32张四向样例及字形检查通过。1,911,328字节app-only写入并哈希验证，原BLE重连、partial=1和SD1383→1387/write_ok=1。Mac仍0.2.26，见environment-bold-0227-validation.json。


2026-10-24温湿度同号0.2.28：EIM选中IDF6.1、Python3.12.15复查通过。140组同号／数字边缘／单位基线、60组差分、32张四方向合成预览通过。1,911,408字节app-only写入0x10000，哈希校验、原BLE重连、SD1399→1401及671ms差分正常。未改开发环境或Mac App，见environment-alignment-0228-validation.json。


2026-10-24月统计0.2.29：重新实查EIM选中IDF6.1/ok、IDF Python3.12.15与/dev/cu.usbmodem14201；固件1,913,312字节app-only烧录并校验。Rust93项及Clippy、C解析／缓存迁移／日周月差分／字号／四向布局回归通过，未修改工具链。见3Inch7EPaper/artifacts/monthly-0229-validation.json。


2026-10-25：实查EIM选中IDF6.1/ok、IDF Python3.12.15。0.2.31广播状态／闲置恢复及0.2.32稳定字号均编译并app-only烧录校验，私有恢复备份保持700/600，原绑定／SD保留。SDK连接枚举兼容声明见ble_transport_probe.c，升级IDF需复核。最新结果3Inch7EPaper/artifacts/typography-0232-validation.json。


2026-10-25 Codex主项排版0.2.34：重新核对EIM选中IDF6.1/[ok]、IDF Python3.12.15与/dev/cu.usbmodem14201。2400组实际Codex字形／固定字号右缘、140组摘要、64组差分通过ASan/UBSan，21张四方向合成预览；固件1,919,008字节app-only写入0x10000并校验，私有回退备份700/600，原绑定／SD保留。App0.2.33已恢复原认证同步，实体0.2.34／673ms差分／SD1052/write_ok1回读通过；未修改工具链。见3Inch7EPaper/artifacts/codex-primary-0234-validation.json。


2026-10-26 SD音乐0.2.36：重新检查EIM选中v6.1/[ok]及实际IDF Python3.12.15；官方esp_codec_dev1.6.2与esp_audio_codec2.6.2锁定，network_provisioning继续使用本地1.3.1补丁。解码栈/大缓冲移至PSRAM，always-internal阈值2048字节、内部保留64KiB，避免音频和显示DMA分配失败。最终构建、app-only写入/哈希、格式实测及清理状态见3Inch7EPaper/artifacts/music-0236-validation.json。未改工具链。
