# 旧工作区上下文归档（2026-10-26）

以下完整保留拆分前的根 AGENTS.md，记录这款墨水屏及配套 HomeCraft 的历次开发现场。它是历史资料，旧版本的“当前”、等待安排和授权不能直接作为本次指令。现行约束见根目录及目标子目录的 AGENTS.md，现行待办见对应 TASKS.md。文内未特别说明的项目路径按工作区根解释。

---

# EPaper 项目上下文

HomeCraft为多硬件的软件和协议中心。当前3Inch7EPaper目录对应微雪 Waveshare ESP32-S3-ePaper-3.97 开发板。与用户沟通及项目说明默认使用中文。

## 多硬件目录规则（2026-10-26，用户最新要求）

- 第三方来源按归属放software/THIRD_PARTY.md和3Inch7EPaper/THIRD_PARTY.md，不能在主目录恢复混合的THIRD_PARTY文件。
- 主目录当前只能有software、3Inch7EPaper两个文件夹。新硬件以后增加独立同级目录；不再创建根docs/artifacts或设备配置。
- HomeCraft是协议中心，公共定义在software/protocol/device-v1.json；通过software/tools/generate_device_protocol.py生成Mac及所选板端常量，修改后必须--check。各板GPIO/供电/SDK/分区只在自己的目录。

- Mac 软件在 `software/`；当前板端独立工程在 `3Inch7EPaper/`，名称按用户指定。以后每种硬件增加自己的同级目录，不合并成全局 hardware 配置。
- 板型描述、引脚、sdkconfig、sdkconfig.defaults、分区表、驱动、managed_components、构建输出、私密配对资料和备份都必须放在对应板子目录；禁止在项目根新建或写入这些运行配置。
- 当前板子私有数据目录为 `3Inch7EPaper/.local`（700），配对/设置文件保持600；原本根目录的绑定/缓存/锁及授权资料已迁移，私有备份在 `3Inch7EPaper/backups`（700/600）。不打印、提交或打包。
- `software/project.json` 仅选择当前开发板目录，Rust 与 Python 共用相同隔离规则；根目录不放该选择文件。每块板子的应用资料也使用其 `.local`。显式 EPAPER_PROJECT 可指定数据目录，若指向工作区则解析选择；不能默默回退到主目录写配置。
- `/Applications/HomeCraft.app` 继续用0.2.36，本机.data-location已更新到该板子的.local并恢复运行。`software/dist/HomeCraft.app` 与DMG仍0.2.36，但已重建以支持新目录选择，不把二者二进制称为相同。
- 本轮没有烧录或连接USB。原部署固件已完整移动到 `3Inch7EPaper/dist/firmware` 且哈希未变；新 `3Inch7EPaper/build` 是重新编译的离线产物，不能视为已部署。旧绝对路径构建缓存保留在本板 `.cache/pre-layout-build-20261026`。
- 以下Waveshare/GPIO/显示/电源约束仅适用于3Inch7EPaper，不能自动套到其他新板。先确定目标板并读其 board.json、README、AGENTS，再查引脚来源。
- Python BLE环境已移入software/.ble-venv并修正入口；旧Conda环境归档software/.cache/pre-layout-conda，不直接激活。共享协议与软件文档在software/protocol和software/docs，本板文档/既有验证在3Inch7EPaper/docs和3Inch7EPaper/artifacts，详情见software/docs/project-layout.md。

## 开始工作前

- 涉及板卡、引脚、外设或固件时，先阅读 `3Inch7EPaper/docs/hardware.md`。
- 涉及显示、界面、图片、字体或刷新策略时，同时阅读 `3Inch7EPaper/docs/epaper-usage.md`。
- 涉及本机 ESP-IDF、EIM 或 Python 环境时，阅读 `software/docs/development-environment.md`，并重新检查当前安装状态。
- `3Inch7EPaper/docs/hardware.md` 和 `3Inch7EPaper/docs/epaper-usage.md` 的上游源码路径相对于官方示例仓库。本项目仅移植了 `3Inch7EPaper/components/epaper_vendor` 屏幕驱动；当前仪表盘代码在 `3Inch7EPaper/main/`，使用方法见 `README.md`。
- 用户反馈拨轮可能损坏，明确要求禁用。GPIO4/5/6 设为禁用、无上下拉、无中断；交互由 BOOT(GPIO0) 与 PMU 上的 PWR 键承担，映射见 README。不得恢复拨轮依赖。
- 横竖屏通过 QMI8658 的重力方向自动切换，支持方向锁定与手动旋转；保持双向布局和稳定等待，避免平放或晃动时反复全刷。
- 早期局刷出现文字错乱或残影。新差分驱动已完成两轮实板测试，用户于2026-10-21明确确认“正常，可以启用局部刷新”；已通过EPAPER_PARTIAL_ON启用并验证重启保留。使用双RAM、保留控制器状态，温湿度数值、曲线、额度和时钟变化共用差分通道；现按后续省电要求取消20次/30分钟定期全刷，布局或方向/手动/错误时全刷，BOOT双击可手动全刷。当前验证状态见 `3Inch7EPaper/artifacts/hardware-upgrade-validation.json`，不要恢复旧打包矩形局刷。
- **当前用户已改选 Mac 读取额度并通过 BLE 发给板子**，原因是校园网访问限制；明确授权轻量 Mac 发送进程。修改前阅读 `software/protocol/ble-data.md` 与 `software/docs/usage-providers.md`。`3Inch7EPaper/main/network.c` 管理蓝牙/备用配网/HTTPS，`3Inch7EPaper/main/usage.c` 解析多服务白名单快照；数据模式不在板上查询账户，不向板子复制账户令牌或 API Key。此前“电脑不得常驻桥接”的目标已被用户这个选择替代。
- 官方应用 Wi-Fi 配网已完成连接与校时，但不能据此认定校园网可访问三家服务。Codex 独立设备授权已取消当前优先级，旧设备码超时；不要自动重新发起授权或导入设备账户凭据。长按 BOOT 3 秒进入手机 Wi-Fi 配网，网络页双击 BOOT 打开 Mac 数据配对窗口，三击 BOOT 打开网页高级设置。参考 `3Inch7EPaper/docs/ble-provisioning.md`，不要把“等待校时”用于未联网状态。
- BLE 使用官方 Security 2；手机 Wi-Fi 配网使用随机窗口凭据，Mac 数据模式使用设备专属的长期随机秘密以支持重连，不降级到无认证/无加密。新版0.2.2首次添加通过实体窗口的8位屏幕码和Security 2蓝牙认证领取长期绑定，USB不再是必要步骤；现已部署，用户已确认屏幕码认证成功，见文末。`3Inch7EPaper/components/network_provisioning` 基于官方 1.3.1 做了 RAM 存储、连接保留和日志策略补丁；更新上游时需保留并复核，不得恢复默认系统 NVS 密码副本或打印二维码载荷。
- **用户明确要求只能使用官方公开的数据入口，不得恢复未公开用量 HTTP 查询。** Codex 使用官方 app-server `account/rateLimits/read` 与 `account/usage/read`；Claude 只接收官方 Code `statusLine`，不读取 OAuth 凭据。DeepSeek 使用官方余额 API。旧板上客户端兼容查询是停用的历史代码，不得启用。字段缺失显示“未提供”，不得用 Token、花费或余额冒充周/月额度。
- 不打印、提交或分享凭据、授权文件或配置后的 Flash 备份。`3Inch7EPaper/.local/.device-auth/`、`3Inch7EPaper/.local/.device-credentials.json` 与 `3Inch7EPaper/backups/` 只留在本机。设备凭据存于独立 `dashboard` NVS，未启用 Flash 加密。
- `3Inch7EPaper/.local/.ble-pairing.json` 含蓝牙长期认证秘密，`3Inch7EPaper/.local/.ble-settings.json` 可含 Mac 侧 DeepSeek Key，二者要求权限 600。`EPAPER_PAIR` 只在限时配对窗口通过本地 USB 导出一次；不要把原始回复保存到串口日志。**当前安装入口为 `/Applications/HomeCraft.app` 0.2.36；项目 `software/dist/HomeCraft.app` 同为0.2.36，源码在 `software/mac-app/`。0.2.36沿用原配置、绑定与真实同步，保留同字号、Codex昨日行及5小时／倒计时／百分比排版，并新增SD音乐页；本机私有.data-location.json指向3Inch7EPaper/.local，不复制绑定到安装包**，阅读 `software/docs/mac-app.md`。关闭窗口隐藏到菜单栏，菜单“退出程序”才停止；无开机自启。旧 Python 发送器已停止；两者共用 `3Inch7EPaper/.local/.ble-sender.lock`，不可同时运行。后续停止/启动只操作本项目进程。
- 用户已确认 Rust App 关闭窗口后菜单栏与同步均正常。Codex 额度与账户累计 Token 已实测；累计/上下文 Token 仍在 Mac App 显示，大字版通过可选 d/ld 传输官方日统计，见 `3Inch7EPaper/docs/display-redesign.md`。Claude statusLine 已接入用户设置，等待官方 Claude Code 真实会话数据；本机 PATH 未检测到 Claude Code CLI，不要把测试样例或 Claude 桌面应用当作 Code 实测。Claude 的 Token 是当前上下文含缓存数值，不能称为账户累计消费。

## 维护记录

- 保留引脚定义的来源与核实状态。源码定义、原理图连接和上板实测分别记录，不能互相替代。
- 留意 GPIO39 的功放与 IMU 中断定义冲突，以及 TG28 与 AXP2101 电源芯片命名差异；具体见硬件记录。
- 新的板卡版本、实测结果、驱动修正和已完成的环境配置，及时更新对应文档，方便其他对话复用。
- 在实际完成编译、烧录或上板验证之前，不把示例片段记为已经验证通过的固件。

## 当前大字显示更新

- 用户确认此次优化对象仅为 ESP32 墨水屏。大字版、服务标识、d/ld 官方日 Token 字段已完成编译、测试与烧录，见 `3Inch7EPaper/docs/display-redesign.md`。
- 用户已明确授权本次短暂退出 App、烧录后自动恢复；已完成。应用写入 0x10000 并通过哈希校验，未改 NVS；当前 Rust 菜单栏进程已恢复，真实 Codex 收到，日志确认整屏全刷 page=0。
- 用户已确认早期大字布局“正常，字大了，布局也正常”。此处属于历史基准；当前刷新策略以本文件开头的新差分验证记录为准。
- 2026-10-21最新实查官方日统计为2026-10-20的94,838,484；2026-10-21尚未提供，后续查询可能更新。只合计同日、同范围的有效记录，不混入 lifetime/上下文 Token 或余额，缺失不当作 0。用户最新要求底部显示当天总Token，标签采用“今日 / Token”两行，右侧保留大数字；此前仅“Token”的偏好由此更新。屏上不显示最近历史日期；这仍然是可取得的官方当天已知数值之和，不能称为三家完整消费账单。当天没有任何有效记录时显示大号 `--`。

## 当前墨水屏显示偏好

- **2026-10-21用户已要求开始Mac App改版，替代此前软件延期安排。** 已阅读本机TexCraft开发教程及原生窗口源码，完成家庭风格EPaper Home 0.2.0：半透明侧栏、橙色选中项、原创背景与图标、EPaper卡片/详情、用量及设置，保留菜单栏后台。14项Rust测试、Clippy及实际窗口导航/关闭重开通过。安装包在`software/dist/releases/EPaper-Home-0.2.0-macos-arm64.zip`；用户最新明确要求先交付安装包，EPaper连接留到晚上验证，本次不再连接/烧录。Mac温湿度遥测与历史接收尚未实现，不把板端曲线称为Mac已支持。
- **2026-10-21用户已要求开始实机调试，已重新连接USB。** 昨晚离线固件现已app-only烧录并校验，Mac助手已恢复真实Codex同步；原先睡前“不要连接/烧录”的阶段已结束。新增温湿度/曲线/SD状态48组回归通过，用户现已确认“总览、温湿度与局刷都正常”。用户已确认USB关机画面、PWR重新开机以及纯电池关机/开机均正常；BLE自动重连、SD455条恢复并继续写入已记录。未测关机电流或异常断电故障，记录在`3Inch7EPaper/artifacts/morning-validation.json`。
- 板端新增SDMMC温湿度日志，每分钟一次，目录`/EPaper/env/YYYYMMDD.csv`；已验证真实SD写入、重读及午夜切新文件，失败不格式化。环境页有24小时温湿度曲线（15分钟均值）；逐分钟原始记录以CSV为准。详情与未实测边界见`3Inch7EPaper/docs/hardware-upgrade.md`。
- 删除AI USAGE旁的“用量总览”，页脚主体20 px，温湿度标签28 px并带图标。主页按启用、额度紧迫、当天用量和新鲜度选最多3张卡，隐藏未配置空卡；板端协议预留8个服务槽，新增n/u/dc受白名单和能力协商保护。Argus类型只展示Token/调用/费用，不冒充额度；新版Rust发送端已部署真实日统计。
- 用户要求Codex、Claude Code、Argus、DeepSeek等所有provider名称加粗。总览及详情标题统一使用Arial Bold栅格，12–32px响应回退也保持粗体；已通过60组渲染、编译并烧录，用户确认当前总览正常。不要仅靠大字号当成加粗，也不要改回普通字重；尚未配置的服务不当作真实数据显示验证。
- 用户要求可识别的关机状态。已部署睡屏/月牙+64px“已关机”页；PWR约3秒后松手正常关机，1.2–3秒松手保留暂停，8秒硬件关机兜底。日志/网络先安全暂停，再全刷完成，最后PMU REG10[0]关机；USB仍供电时Deep-sleep，不在关机图后继续采样。失败取消并恢复任务。代码/测试/预览见`3Inch7EPaper/docs/shutdown.md`，用户已确认USB与纯电池正常关机/开机；日志验证任务停止、SD恢复与BLE重连，未测实际关机电流。突然断电不能保证更新关机图。

- 用户最新要求：墨水屏标签采用“5小时 / 周额度”，删去“5小时额度”里的“额度”，左侧大标签、右侧对齐数字；5小时为主项并大于周额度。总览5小时优先96 px，横屏100%按宽度回退64 px，周额度横屏48 px/竖屏40 px；详情5小时96 px、周64 px。Codex 名称32 px，Token 区最新标签为“今日 / Token”两行，数字80 px并按宽度回退。DeepSeek 保持 API 余额，不展示周/月空字段。不恢复月额度界面。
- 右上角用标准蓝牙符号表示数据链路，断开加斜杠；不恢复 BLE LIVE/WAIT 文字徽章。旁边显示 UTC+8 时间和 PMU 电源状态；用户最新要求日期移到顶部且仍与时间分开：横屏为标题与蓝牙之间的独立日期，竖屏为左侧标题下方；页脚不再放日期。USB供电时均显示插头+USB，拔掉后显示电池实际电量；只有电池存在、计量启用、读数0–100时显示百分比，未知显示 `--`。供电标志每100ms只读检测、稳定100ms后快速差分；百分比平时30秒及插拔后读取，不修改充电/计量参数。
- 原有协议仍保留窗口字段以兼容发送器；保持BLE认证和官方额度入口。Mac新版状态见`3Inch7EPaper/artifacts/home-app-release-validation.json`；本次仅改App，未改固件。当前版本部署与实测以 `3Inch7EPaper/artifacts/firmware-validation.json` 和 `3Inch7EPaper/docs/display-redesign.md` 为准，不沿用早先版本的实屏视觉确认。

## Argus Token统计与Codex日期边界

- Argus现使用官方本机STDIO MCP `get_token_usage`，不读取数据库/逐条记录/服务凭据，默认当前文献库，可继承`ARGUS_LIBRARY`固定库。默认5分钟同步查询日/周汇总，`include_records:false`，输入+输出=总数，缓存不重复相加；费用缺失保持未知。MCP开关必须尊重。
- 用户先要求周Token小字，实屏后改为与Codex的5小时/周额度相同层级。按UTC+8周一00:00起查询；Argus今日主数字优先96px，本周优先64px（窄卡48/40px），日/周标签28/24px，数字右对齐，按可用宽度回退。总览不放调用/费用小字，详情保留。可选`wt:[Monday_YYYYMMDD,total]`仅对`weekly_tokens:true`设备发送，旧周不能带入新周；周查询失败保留今日成功数据。
- Codex能通过官方`account/usage/read`读累计、峰值及可选日桶。2026-10-21实查最近为10/20的94,838,484 Token、累计1,536,260,991，今天尚无桶；缺今天不能用昨天/累计冒充0或今日消费。现有程序会在当天桶出现后自动纳入当天合计。历史10/19记录已经过时，不要当作最新值。

- Argus大字日/周布局已获用户确认“合适，显示正常”。底部为同日已知Token合计：今日/Token标签两行，大数字80px；Codex没有当天日桶时不混入昨日或累计，只计已取得的同日项，缺失不表示0。

## HomeCraft 改名与换机发布（2026-10-21）

- 用户指定名称HomeCraft，只需要Apple芯片版。图标仅参考TexCraft的粗黑描边、奶油色、平涂及错位阴影语言；原创墨绿房屋+金色墨水屏，源文件`software/assets/app-icon/homecraft.svg`，可通过`software/tools/render_homecraft_icon.cjs`（Node + sharp）再生成透明PNG和ICNS。
- 发布包`software/dist/releases/HomeCraft-0.2.1-macos-arm64.zip`，含App与README.txt，不含凭据、配对资料或个人缓存；本机ad-hoc签名并验证解压签名，未Apple公证。15项测试、Clippy、项目目录外空配置预览通过，只链接系统动态库。记录`3Inch7EPaper/artifacts/homecraft-release-validation.json`。
- 全新Mac使用私有`~/Library/Application Support/HomeCraft`；兼容已有EPaper Home私有目录，项目内运行仍用项目配置。移除编译机项目路径回退；Finder启动官方Codex时补充常见Homebrew子进程PATH。新Mac需要本机官方客户端登录及首次USB绑定，不迁移账户凭据。此次未测试BLE或烧录，仍等用户晚上实测；隔离预览已退出。

## 0.2.2 蓝牙首次认证候选（当前离线阶段）

- 用户明确拒绝把USB作为首次认证必需步骤，要求蓝牙连接时安全认证。已实现实体BOOT双击开启的8位随机屏幕码、10分钟窗口、官方Security 2加密领取长期绑定；Mac正常入口不再要求插USB。各Mac只需认证一次，新增Mac不轮换设备密钥，不取消其他Mac绑定；日常原Mac暂停、下一台连接即可，不重复配对。不要再告诉用户每次切换需USB或重新认证。
- `HomeCraft-0.2.2-macos-arm64.zip`已打包，17项Rust测试、Clippy、C窗口过期/单次领取/回复宽限测试、48组环境刷新回归通过；配对屏四方向及App配对表单已检查。记录`3Inch7EPaper/artifacts/ble-enrollment-validation.json`。0.2.1的USB说明已被新流程取代。
- 候选固件`3Inch7EPaper/dist/firmware/HomeCraft-0.2.2-ble-pairing-candidate.bin`（1,855,488字节，SHA256 `2f328d444c15319e05c214b1f98a3a2fc9e1cd6ed9c865b2e6fe25eecc96f5ea`）已在IDF v6.1编译。**未连接USB/BLE、未烧录，板子仍是之前实测版本。** 按用户要求晚上更新后验证PIN握手、错误码拒绝、GATT配对/数据模式切换、5秒回复宽限、两台Mac交替和既有绑定保留。不能把离线通过写成实机完成。
- 配对广播`EPair_`、UUID末尾`ed08`，常规数据仍`EPaper_`、末尾`ed07`；配对端点`pair-device`由Protocomm先解密认证再调用，只能领取一次，长期随机密钥通过加密响应返回。PIN不存盘/不记录/不自动重试，Mac原子保存600文件，普通快照仍不带凭据。旧USB仅折叠维护入口；不改已有账户入口、拨轮、电源、SD或局刷策略。
- 候选构建现在覆盖了`3Inch7EPaper/build/epaper_codex_dashboard.bin`，不要把其哈希当成已部署。后续如获用户开始调试授权，app-only更新并保留NVS与SD，不要擦除绑定或重新生成旧的`ble_key`。

## 0.2.3 菜单栏图标

- 用户要求菜单栏图标同步HomeCraft品牌，但要方正、不倾斜。已替换旧像素文档图标为原创房屋+墨水屏单色模板，源文件`software/assets/app-icon/homecraft-tray.svg`，运行时内嵌44px透明PNG，macOS以18pt模板自动适配深浅色。不得直接缩小带倾斜/底色/阴影的启动图标用于菜单栏。
- Apple芯片包`software/dist/releases/HomeCraft-0.2.3-macos-arm64.zip`，Cargo check、Clippy及隔离App/菜单栏初始化通过，解压签名与ZIP检查通过；预览已退出。本次未触碰原有运行进程、蓝牙或固件，0.2.2候选固件仍待晚上部署。记录`3Inch7EPaper/artifacts/homecraft-tray-validation.json`。替换安装后需退出旧运行实例再打开新版，菜单栏才能显示新图标。

## 0.2.4 Codex CLI跨Mac定位

- 用户另一台Mac已安装CLI，但旧App提示找不到。已修复旧版仅优先两个Homebrew目录及GUI PATH的问题：`software/mac-app/src/codex_cli.rs`统一处理进程PATH、用户独立安装、npm-global、nvm/fnm、pnpm、Volta/asdf/mise及标准目录，必要时有超时地探测zsh/bash登录环境，缓存10分钟；没有把开发机CLI绝对路径编译进包。npm脚本匹配node并补齐子进程PATH，手动路径不拼入shell。
- 家庭设置与AI用量详情加入可执行文件选择、Node可选覆盖、检测当前设置、真实路径/版本及具体错误。`codex_cli`/`codex_node`存在600的`3Inch7EPaper/.local/.home-ui.json`；`EPAPER_CODEX`/`EPAPER_NODE`保留临时覆盖。配置变化会使旧Codex查询缓存失效，避免修正路径后仍显示旧错误。不要要求用户改成Homebrew安装或重新登录来绕过查找bug。
- 25项测试、Clippy通过；正式打包App在PATH只有/usr/bin:/bin时成功检测并启动本机codex-cli 0.141.0；测试只执行--version，没有账户查询或模型调用。另一台Mac的真实安装仍待用户跑新版确认，自动查找之外可用原生文件选择或粘贴command -v codex的绝对路径。
- 最新包`software/dist/releases/HomeCraft-0.2.4-macos-arm64.zip`，记录`3Inch7EPaper/artifacts/codex-cli-discovery-validation.json`。本轮只修Mac程序；0.2.2候选固件、绑定、SD与晚上的实板验证安排不变。诊断期间误启动的旧debug二进制已核对PID并停止，隔离UI预览也已退出，未替换原后台程序。


## 0.2.5 Codex卡片操作与DMG

- 用户要求直接手动检查/刷新Codex以及DMG安装包。AI用量主页与设备用量页的Codex卡片提供检查CLI、刷新Codex用量、CLI设置；未连接/空数据时也可操作，显示进度、CLI诊断、读取时间与失败原因。刷新单独读取官方Codex、跳过旧成功/错误缓存，其他provider读数不覆盖；自动查询仍有60秒缓存。不调用模型或私有接口，不把缺失Token/额度冒充0。
- 27项Rust测试、Clippy与模拟STDIO链路通过；正式App已通过精简PATH下的本机官方额度/累计Token读取，未调用模型。发布脚本`software/tools/package_mac_dmg.sh`以白名单生成`software/dist/releases/HomeCraft-0.2.5-macos-arm64.dmg`，仅含App、安装说明和Applications链接；仍为ad-hoc签名，未Apple公证。验证见`3Inch7EPaper/artifacts/homecraft-025-*-validation.json`。
- 本轮仅改Mac App，未连接墨水屏、未烧录固件，0.2.2蓝牙首次认证候选仍待晚上实板验证。升级安装需退出旧菜单栏实例再替换App，保留用户私密配置与绑定。


## 0.2.6 自动刷新配置

- AI用量页的“刷新设置”及家庭设置顶部可开关自动读取，配置1–120分钟，默认5分钟；保存至600的.home-ui.json.refresh并热应用，保留旧配置兼容。主页显示上次/下次读取和读取状态，时间按Mac本地时区；Token日统计仍UTC+8。
- 轮询现独立于BLE，未连接设备/关闭窗口仍会读取；关自动刷新后保留手动按钮。全量手动请求最短60秒，Codex单独手动刷新仍强制绕过缓存；DeepSeek余额至少5分钟、429后15分钟缓存，Claude只读官方statusLine最新记录，均在App设置中说明。蓝牙10秒保活和屏幕局刷不随查询间隔改变。
- 31项Rust测试、Clippy通过，覆盖持久化、运行中改间隔、关闭/手动、超期只读一次。当前Apple芯片包为HomeCraft-0.2.6-macos-arm64.dmg；未连接/验证EPaper，原配对候选固件仍待晚上测试。后续以3Inch7EPaper/artifacts/homecraft-026-*-validation.json为本轮验证记录。


## 0.2.7 App视觉一致性

- 用户明确要求全面统一设计语言，不混用毛玻璃、白底、直角。全App采用深色磨砂玻璃、白/灰文字与橙色重点；7个页面及所有设备/添加/设置弹窗共用home/theme.rs，普通/暮色背景不改变控件主题。卡片20pt、弹窗24pt、控件10pt，卡片统一细边线与18pt内距；后续新增UI必须走共享surface/modal/按钮，不引入默认浅色组件。
- 默认TextEdit/Checkbox/DragValue/折叠区/进度/滚动条/菜单已统一，App自身窗口和文件选择采用Dark Aqua。服务名称加粗，前三家标识用浅色模板，Argus圆角图像；OpenAI不再带方形白底。弹窗有模糊壁纸衬底，避免透出背景文字。自动化文案读取实际刷新配置，不再写死5分钟。
- --render-ui-preview输出实际组件的GPU离屏样例，明确标记非真实用量，不访问账户/BLE/日常配置，不包含原生窗口边框。图像和清单见3Inch7EPaper/artifacts/ui-glass-027-release；本轮不连接墨水屏或烧录，原候选固件仍留到晚上实测。


## 0.2.8 晚间固件与双向设备数据（当前部署）

- 用户已回来并授权开始固件，后续要求HomeCraft连接后可切换墨水屏页面，并在Mac试读温湿度。0.2.8已app-only烧录、哈希通过，原有绑定、局刷设置和SD保留；此前0.2.2“候选未烧录”的状态由本条更新，但首次屏幕码实测仍待用户操作。
- 固件1,858,496字节，SHA256 `c79f52b8369d01cc9b19287a3110fdfeb54ab05a16dd593fd45e7051ccc0a1e3`，归档`3Inch7EPaper/dist/firmware/HomeCraft-0.2.8-device-control.bin`。私有回退备份`3Inch7EPaper/backups/pre-028-app-and-dashboard-20261021.bin`包含绑定/配置，目录700文件600，不分享或输出内容。
- 新`3Inch7EPaper/main/device_protocol.c`在原Security 2端点提供status/history/page/rotate/refresh，能力`device_control:1`，回复≤480字节，4项队列主循环执行。不给账户/配置/任意文件读取、远程关机或远程首次配对。永久蓝牙秘密不再复制进显示快照；初始化失败不自动擦除dashboard NVS。
- HomeCraft 0.2.8新增`device.rs`，真实温湿度/电源/SD状态每5秒读取，历史每60秒检查修订；12包×8个15分钟均值，同day/revision连续完整才显示，缺测断线。“温控”显示曲线，“显示”/设备概览切页和四向/自动方向；关闭或延迟明确标注上次读数。账户轮询和板端差分节流不变。
- 已实测本机原绑定认证重连、真实Codex/Argus用量及遥测/历史，SD1068→1074增长、partial=1。实际App UI按钮→加密BLE→板子确认环境/总览、竖屏/横屏/自动方向；已恢复总览和自动方向。34项Rust、Clippy、C协议/配对/环境48组回归通过。用户视觉确认、屏幕码首次认证、实际两台Mac切换尚待单独结果，不能把当前原绑定重连当作完成。
- 当前软件发布以本轮`3Inch7EPaper/artifacts/homecraft-028-*-validation.json`为准；上方0.2.7为视觉主题基准，新增UI继续走共享磨砂玻璃组件。


## 0.2.9 按钮反馈审计与重连修复（当前）

- 用户指出App刷新/多数按钮没反馈，且温湿度差0.x。已修复设备刷新：立即busy/禁用重复提交，直接解析这次加密回复并读历史，完成toast及持久“手动读取完成”时间，失败明确提示。新增共享feedback.rs，扫描、连接、暂停、用量/CLI检测刷新、设置保存、文件选择/取消等都有统一磨砂提示；自动后台轮询不持续弹操作提示。刷新草稿标明未保存，恢复自动检测会保存并检查；不同导航/设备页签独立滚动位置。
- 固件0.2.9已app-only烧录，1859056字节，SHA256 `d8b5dae96260db65ca533a2ba2f7c712fcebf6e5048be6401bbb2b17dabb01a8`，归档`3Inch7EPaper/dist/firmware/HomeCraft-0.2.9-display-feedback.bin`；保留原绑定/SD/局刷。status可选shown数组为上次**成功刷屏**的温湿度、时间、页、方向、自动方向、序号。Mac同时显示实时采样及屏幕画面读数，5秒采样与30–60秒屏幕节流差异可核对。控制完成以实际frame回传确认，不用排队ACK冒充已刷新。
- 审计发现CoreBluetooth断开后旧Peripheral失效，已用公开retrieve_peripherals按已知ID恢复原生句柄再连接；未更改长期绑定/安全协议。暂停后手动扫描保持暂停，只有启动首扫按设置自动连接。
- 36项Rust测试、Clippy和C协议/已显示帧边界通过；真实App逐项按钮点测及反馈通过：7项导航、添加/关闭、设置保存、Codex检查/官方刷新、切环境页、整屏全刷、竖屏锁/自动、扫描、暂停、暂停后扫描、重连、设备刷新。记录`3Inch7EPaper/artifacts/homecraft-029-button-audit.json`。当前dist/HomeCraft.app及HomeCraft-0.2.9-macos-arm64.dmg；本机最终App保持菜单栏与真实同步。
- 屏幕码首次领取、错误码和两台实体Mac交替仍待用户一起做，不能把本机暂停重连称为两台Mac实测。用户已亲手确认“现在有进度和完成反馈了”。已切到设备连接页并暂停App，请用户开启BOOT屏幕码流程；当前正等待首次蓝牙认证结果，不要自动恢复旧连接打断窗口。


## 0.2.10 首次PIN发现服务阶段修复（App当前）

- 用户首次输入PIN失败。只读公开GATT诊断证实广告为配对模式，但Mac服务UUID缓存仍是ed07，实际端点已是pair-device；0.2.9按ed08过滤导致尚未发送PIN证明即失败。不能把此错误说成用户输错码。
- App 0.2.10仅从两个已知UUID的单一完整服务组，按预期端点+proto-ver+prov-session识别模式，正确UUID优先，错误模式/双端点/未知或歧义拒绝；Security 2/patch1、服务端证明与AES-GCM及回复身份校验不变。Pair也retrieve原生设备，失败提示分阶段且不输出PIN。38项Rust与Clippy通过。
- 本机dist/HomeCraft.app已替换0.2.10并打开连接页，数据连接保持暂停，用户正按新BOOT窗口/新PIN重试；不要抢回普通连接。固件仍0.2.9，未再次烧录。诊断见3Inch7EPaper/artifacts/homecraft-0210-enrollment-diagnosis.json；首次PIN和两台实体Mac最终验证状态以实测结果更新。


## 0.2.11 连接表单与小提示修复

- 当前Mac交付为`software/dist/HomeCraft.app` 0.2.11、Apple芯片DMG同版本；固件仍为已部署的0.2.9，未再次烧录。
- 用户反馈输码被自动重连打断、usage-data模式错误、上下两个连接按钮混淆、错误浮层过大无法关闭。现采用单个“安全连接”按钮紧随码框；输入不会自动提交，已绑定普通数据设备可留空，配对态须当前8位码。表单编辑暂停重试，实际广播名优先于缓存名；普通连接可取消，后台错误不反复弹提示。
- 反馈统一12pt单行可关闭状态条，主页面预留位置、弹窗内正常布局，不再覆盖按钮；详情保留连接页。43项Rust测试、Clippy和真实暂停/扫描/关闭提示检查通过。
- 既有绑定已在0.2.11重连；当前停在连接表单并暂停，等用户新窗口码的首次认证实测，不能声称PIN领取或双实体Mac已通过。


2026-10-21补充：用户已确认HomeCraft 0.2.11屏幕码“认证成功，已连接”。随后安全状态确认普通数据链路已认证，长期绑定与首次配对前摘要比较一致；原绑定未轮换。两台实体Mac交替仍未单独实测。


## 0.2.12 墨水屏今日/周Token字号层级

- 用户照片确认Argus今日14.2M缩成比周54.6M更小的字。已修复：数字和K/M/B/T/P小单位分开排版、基线对齐，日/周一起选字，周数字严格小于今日。照片双卡横屏现在80px/64px，较长数字及三卡、横竖屏/详情共用联动规则；接近1000的舍入自动进位单位，避免1000.0K额外挤占。
- 固件0.2.12已app-only烧录并通过写入哈希验证；Mac仍为HomeCraft0.2.11，已自动恢复。3,456组字号组合、192组四向总览/详情渲染、48组环境差分回归和IDF6.1编译通过；预览为合成数据，用户实屏视觉确认待补。记录`3Inch7EPaper/artifacts/token-hierarchy-0212-validation.json`与`3Inch7EPaper/docs/display-redesign.md`。


## 0.2.13 日期顶部偏好

- 用户最新要求把日期放到上面，覆盖旧的页脚日期要求。横屏顶部标题与蓝牙间独立20px日期；竖屏左上标题下16px日期，和右侧时间分开，服务页避开标识。页脚不再显示日期，保留页码及操作/状态。
- 源码已更新为固件0.2.13，32组页面/四方向ASan/UBSan渲染及横竖屏预览通过；最终部署状态见`3Inch7EPaper/artifacts/date-header-0213-validation.json`。Mac保持HomeCraft0.2.11。

- 0.2.13最终已app-only烧录并通过写入哈希验证，原绑定重连、真实Codex/Argus同步及总览成功帧通过，SD和partial保留；用户实屏日期位置尚未确认。当前板上版本以0.2.13为准，先前“日期在页脚”为历史偏好。


## 0.2.14 App图标雾蓝底色

- 用户明确只换App图标底色，非窗口背景，原因是绿色与Nomi相似。`software/assets/app-icon/homecraft.svg`圆角底板已改#586EAC雾蓝，原图形保留；各尺寸PNG/ICNS已重新生成。窗口主题及单色直立菜单栏标识继续沿用原设计。
- 当前Mac App0.2.14/build15，安装包`software/dist/releases/HomeCraft-0.2.14-macos-arm64.dmg`；ICNS7档尺寸与透明边角、Cargo check/release构建及现有连接恢复通过。验证见`3Inch7EPaper/artifacts/homecraft-0214-icon-validation.json`。本轮未改固件，板子仍0.2.13。


## 0.2.15 Argus详情信息布局与分类（当前）

- 用户照片指出详情上方留白过大、底部文字拥挤，要求按模型和供应商增加信息。Argus详情现独立argus_page：顶部今日/本周KPI（日数字更大、单位基线对齐）、中部输入/输出/缓存命中率与命中量/调用次数、底部模型及供应商排行/占比；横屏两栏、竖屏堆叠，页脚只保留一行同步/范围/费用。总览未改，日期在顶部，服务名称仍粗体。
- 官方get_token_usage同一日汇总提供by_model/by_provider；不增加逐条records、数据库、配置或密钥读取。当前官方仅给供应商UUID，无显示名称工具；用短ID标明，不从模型名猜厂商。每组最多3行，超过3类前2+Other；类别总数、行数、非负整数、合计与日期校验。缓存已含输入，分类也不重复相加。
- 新能力argus_breakdown:1通过原Security 2传输独立≤480字节白名单包，必须与原Argus成功日统计的day/total/fetched_at完全一致；不匹配清除或拒绝，旧固件仍兼容。只保留一份分类RAM，不在8个服务槽重复。安全USB诊断EPAPER_ARGUS_STATS仅输出接收/匹配/日期/类别数量，不含标签、凭据或PIN。
- 固件0.2.15（1,871,104字节，SHA256 4d30770a2f9a2e37bf430b0d1cf51d5fc3eedaf738f7c955cdb4b111e29d7519）已app-only烧录并校验，NVS/长期绑定/SD/局刷保留。HomeCraft0.2.15/build16已恢复真实Codex及Argus同步，设备确认分类received=1 matched=1，3种模型、2个供应商，验证时日Token19,307,194。SD1196条且write_ok=1、partial=1；新详情实屏观感仍等待用户确认，预览数据是合成值，未发送实板。
- 46项Rust测试、Clippy、C严格解析ASan/UBSan、20组四向渲染、16组Argus与48组环境差分回归通过。当前Apple芯片DMG为software/dist/releases/HomeCraft-0.2.15-macos-arm64.dmg，已核对签名/版本/内容白名单与App/图标/说明一致，未Apple公证；雾蓝图标和窗口风格保留。以3Inch7EPaper/artifacts/argus-detail-0215-validation.json及homecraft-0215-release-validation.json为准。


## 0.2.16 HomeCraft精简日常文案（当前App）

- 用户明确反对App各处堆积教程/实现说明。日常页只保留核心读数、简短状态、必要操作；不要把后端实现、安全声明、缓存策略反复放在卡片底部。帮助集中在家庭设置→帮助，按主题折叠；必要的首次配对操作指引只在需要输码时出现。
- 保留可关闭单行操作反馈、手动读取完成时间、错误入口，不能以“精简”删掉进度或把未知改为0。Codex“检查CLI / 刷新 / 设置”继续存在；路径/版本进Codex设置→诊断详情。账户累计/输入输出/最近统计日/说明进卡片详细用量，错误原因进错误详情；温控中的屏幕读数可展开核对。今日合计仍标明已知范围，详细口径放帮助和悬停。
- App0.2.16/build17；本轮不改固件（仍0.2.15），保留雾蓝图标、共享玻璃主题、BLE/账户入口/刷新配置。46项现有Rust测试及Clippy通过；视觉检查和DMG交付状态见3Inch7EPaper/artifacts/homecraft-0216-release-validation.json。

- 0.2.16已替换本机项目App并恢复真实Codex/Argus加密同步，帮助/Codex设置/家庭设置展示路由正常。96张预览均是隔离样例，不声称用户逐页目视确认；Apple芯片DMG为software/dist/releases/HomeCraft-0.2.16-macos-arm64.dmg。板上仍0.2.15，本轮未烧录；此前Argus新详情实屏确认仍待用户结果。


## 0.2.17 主动采样与不依赖插件的连接（当前）

- 用户要求新Mac只装HomeCraft即可连接和配置硬件，不依赖蓝牙/MCP插件、Python、Rust、Homebrew或手机配网App。设备侧继续内置Security 2与实体8位码首次认证，账号数据源的官方客户端/登录属于Mac侧独立依赖，不能把它们说成硬件驱动。已核对仅链接系统动态库、项目外App+空配置+精简系统路径能初始化；此为隔离启动验证，不是第二台实体Mac测试。
- 用户指出App“刷新数据”后墨水屏未更新：旧Read只是status缓存。新Read映射sample，内部自动遥测改用独立Status；不得让状态轮询意外触发采样。能力sample_refresh:1，队列ACK分配非零编号；通知输入任务做新测量，必须编号一致且测量完成时间晚于请求，再urgent评估原双RAM差分。像素不变跳过；20次/30分钟/布局改变等全刷门限保留，“清除残影”仍单独全刷。
- device状态新增可选sample:[id,result]：0等待、1已更新、2画面无变化、3传感器错误/超时、4显示失败。App只接受当前请求完成，不拿队列ACK、旧sample或status缓存冒充完成。手动用量发送后也请求更新当前画面。官方账户/CLI查询改成单个独立后台串行任务，避免外部网络阻塞BLE控制和保活；同一蓝牙认证会话仍只由设备worker串行读写。
- 固件0.2.17已app-only烧录，写入哈希通过；App0.2.17/build18已部署并恢复原绑定及真实Codex/Argus同步。4次真实采样成功，设备帧递增；串口捕获667ms/685ms差分，SD1236条/write_ok=1/partial=1保留。48项Rust、C采样状态机/协议ASan/UBSan及48组环境差分通过。2026-10-21用户已亲手点击“采样并更新”，确认“局刷和完成提示正常”；本项实屏验证完成。
- 当前DMG为software/dist/releases/HomeCraft-0.2.17-macos-arm64.dmg，验证见3Inch7EPaper/artifacts/manual-sample-0217-validation.json、homecraft-0217-portability-validation.json及homecraft-0217-release-validation.json。新增本地device.action诊断入口只在显式--ui-control私有socket启用时可用，映射原Intent，白名单仅采样/换页/旋转/清残影，不暴露PIN/凭据。
- 用户已对0.2.15 Argus新详情回复“清楚，布局正常”，该项视觉确认完成，替代上方待确认文字；记录已更新。

## 0.2.18 DeepSeek Harness 与交互图表（当前）

- 用户强调 HomeCraft 是中枢，墨水屏负责摘要显示；Harness 无自身跨设备同步，用户选择主页合并、详情按 Mac 分开。UI统一使用全称 **DeepSeek Harness**，不要写DPSK简称。
- 新适配器只读官方公开的本地 JSONL/Zstd 文件规范与实际 provider usage，不读取账户凭据、不安装插件；App 内置解码，v2–v4只取最高代文件。缓存不重复加，attempt/final替换、重试另计、seed继承前缀排除，UTC+8日/周。说明见 software/docs/usage-providers.md。
- HomeCraft 600权限原子数字缓存、同一物理 Mac/用户稳定身份摘要；同ID更新覆盖，绝不将多机合计再次当成单机发送。最多8台，通过已认证显示端的数字缓存交换最后已知快照，尚未连接同步的另一台Mac不能声称实时最新。新协议能力 harness_devices:1；旧固件跳过新数据，详情见 software/protocol/ble-data.md。
- 温湿度图表已实现即时悬停数字/时间段/十字线、双图联动、点击固定、方向键和Esc；缺测不补线，跨日重置。图表为SD 15分钟均值，主数字仍为实时采样。
- 61项Rust测试、Clippy、32组四方向/1–8设备受保护帧渲染、来源覆盖与日周边界C测试已通过。实际构建/部署/用户视觉确认以 3Inch7EPaper/artifacts/homecraft-0218-validation.json 为准；不要以合成两Mac测试代替实体两Mac交替。

### 0.2.18 最终标题与日期要求（覆盖同轮较早预览）

用户再次明确：**标题栏不要把 Harness 缩小，所有服务标题统一字号；空间不足宁可隐藏农历。** 当前横屏服务标题统一32px、竖屏24px，全文DeepSeek Harness；仅内容卡片允许DeepSeek大字/Harness小字。Codex/Argus/DeepSeek图标与文字按真实墨迹中心对齐。完整日期24px，与时钟分开；农历20px只在空间足够时显示。下方设备明细根据数量放大，不固定为20px。农历代码/生成依据/1049个HKO事实及像素对齐测试在 3Inch7EPaper/main/lunar_calendar.c、software/tools/generate_lunar_table.swift、3Inch7EPaper/tests/test_lunar_alignment.c，缺失日期不能编造。

- 0.2.18最终已app-only烧录并通过写入哈希；原Security 2绑定自动重连，真实Codex/Argus/DeepSeek Harness同步及详情成功帧通过，SD1331条/write_ok=1/partial=1保留。HomeCraft0.2.18/build20菜单栏运行，Apple芯片DMG已重新只读核对签名、内容白名单和说明。仅本机真实来源已验证，第二台实体Mac和本轮实屏视觉确认仍待补；完整记录在3Inch7EPaper/artifacts/homecraft-0218-validation.json与homecraft-0218-release-validation.json。


## 0.2.19 用户确认的省电策略（覆盖旧刷新/采样周期）

- 用户确认：Mac每30秒读取/下发用量；板端SHTC3及PMU每30秒、IMU读取每1秒；按键原10ms与映射保留。SD仍每分钟一行。设备遥测/历史和多Mac缓存仅每5分钟读回，手动操作立即按事务完成回传，不拿ACK当作显示完成。
- 取消旧20次/30分钟定期全刷及连接页每次全刷。普通数据/时钟/卡片重排/来源数变化均双RAM差分；开机、换页、旋转、手动清残影及基准丢失/IO恢复保留全刷。用户明确新要求优先于本文件前面的历史策略；不能自行恢复定期闪屏。
- 官方esp_pm配置CPU40–80MHz，Tickless Idle、BLE Modem-sleep、自动Light-sleep；主晶振保留，未假设32k晶振。USB连接电脑时官方USJ保护阻止自动轻睡，以免调试消失；纯电池验证已读取5101次真实睡眠，BLE仍连接。BLE数据模式esp_wifi_stop，配网按用户实体操作开启，不改充电。
- App设置从minutes迁移为seconds（读旧配置兼容），默认30秒，本机已应用30秒；DeepSeek余额保留5分钟/429退避，不能承诺官方日桶30秒生成。Harness缓存NVS写入合并5分钟，正常关机强制提交。
- 固件/App0.2.19/build21已部署，app-only哈希验证、原绑定重连、30.009秒温湿度及约0.67–0.69秒局刷通过，实际回传周期完整结果见3Inch7EPaper/artifacts/homecraft-0219-energy-validation.json。62项Rust/Clippy、按键/1秒IMU稳定/差分策略ASan回归通过。未测电流与长时间取消定期全刷后的残影，不把计数当续航结论。


## 0.2.20 软件阶段与温控精简（最新）

- 用户明确“接下来都是软件的问题，硬件不链接”；后续工作先离线进行，不主动重新连接BLE/USB、不烧录。固件仍为0.2.19，已实测的省电策略保留。App可用隔离离屏渲染验证，不把样例称为真实硬件数据。
- 温控主页面仅一组温湿度大数字、一个采样时间，刷新/更多是图标按钮；不恢复重复设备标题、手动更新时间、屏幕重复读数、电量/SD一堆状态。电量/SD等收在“更多→设备状态”，屏幕显示折叠。数据不称为实时；采样/回传口径维持30秒/5分钟。
- 既有主动采样、进度/结果反馈、交互图表保留；详情入口只显示缓存、不发硬件命令。使用共享玻璃主题，设备断开/加载/空数据和窄窗有离线检查。App0.2.20/build22，63项Rust测试与Clippy通过，交付状态以3Inch7EPaper/artifacts/homecraft-0220-ui-validation.json为准，不能把离屏图当作实屏确认。

- 0.2.20为本轮离线软件发布，App在dist/HomeCraft-0.2.20-candidate.app、安装包在software/dist/releases/HomeCraft-0.2.20-macos-arm64.dmg；未替换/重启原运行实例，避免触发自动蓝牙连接。dist/HomeCraft.app仍是0.2.19，后续不要误以为运行中已应用0.2.20。用户安装新版后须退出旧菜单栏实例再重开。最终离线视图在3Inch7EPaper/artifacts/homecraft-0220-delivery-ui，均为样例数据。


## 0.2.21 iCloud 与 Codex 账号（2026-10-22，最新软件阶段）

- 用户最新要求不同Mac的数据用iCloud同步，并按准确Codex账号区分；继续保持纯软件阶段，不扫描BLE、连接USB或烧录。本轮不替换/重启旧菜单栏实例，`software/dist/HomeCraft.app`仍为0.2.19；新App在`dist/HomeCraft-0.2.21-candidate.app`，DMG在`software/dist/releases/HomeCraft-0.2.21-macos-arm64.dmg`。
- 采用用户主动选择的iCloud Drive文件夹，家庭设置→“iCloud 与来源”，两台Mac登录同一Apple账户并选择同一目录。当前ad-hoc包没有CloudKit容器/团队签名，不能称为原生CloudKit。默认关闭，不自动寻找或访问iCloud容器；真实双Mac经Apple上传下载尚待用户选择目录验证。
- `software/mac-app/src/cloud.rs`保存白名单数值摘要与已接收的温湿度曲线，每30秒交换、每Mac/日独立分片、原子写入和NSFileCoordinator协调，App查看最近90天，不自动删除更早Drive文件。不是SD逐分钟CSV备份。账户令牌、API Key、蓝牙绑定、本机CLI路径和自由文本不进入同步。`.cloud-settings.json`与`3Inch7EPaper/.local/.cloud-cache.json`只本机600权限，已忽略；安装包继续白名单。
- Codex使用官方`account/read`、`refreshToken:false`，在额度/用量查询前后检查账号一致。只从返回的ChatGPT邮箱生成脱敏标签（前1–3字符+***）和域分隔SHA256键；完整邮箱不保存、不打印，不读auth.json或解析令牌。无邮箱/非ChatGPT/不支持时按设备名，不冒充账号。套餐变化不重复创建同一邮箱的统计来源。
- 同Codex账号跨Mac取最新，不相加；不同邮箱保留独立记录，可在账号列表区分。未知账号不和已知账号盲目相加。DeepSeek Harness仍按设备合并；Argus没有可靠跨Mac文献库身份时独立保留，不猜测可相加。云端普通用量/温控回退仅作用于UI副本，标明iCloud与来源Mac，不能回灌为本机原始采集。
- 本机真实官方查询已确认能得到准确邮箱和累计Token，检查输出只保留布尔结果；测试包括75项Rust回归、Clippy、模拟换账号/无邮箱、两来源文件交换及144张离线界面样例。系统文件协调测试需要沙箱外执行，仅使用临时目录；未接触真实iCloud或硬件。以`3Inch7EPaper/artifacts/homecraft-0221-cloud-validation.json`为交付验证记录，不能把模拟文件交换说成已完成两台实体Mac的iCloud同步。


## 0.2.22 全App操作与布局优化（2026-10-22，最新软件交付）

- 用户要求详细优化整个HomeCraft，例子是把Codex更新时间并入按钮行。本轮继续纯软件，不扫描BLE、连接USB或烧录，不替换/重启旧菜单栏实例。`software/dist/HomeCraft.app`仍为0.2.19；交付候选`dist/HomeCraft-0.2.22-candidate.app`，DMG `software/dist/releases/HomeCraft-0.2.22-macos-arm64.dmg`，build24。
- Codex头部合并脱敏账号和入口，数字行等高、进度条对齐，操作栏右侧显示更新时间；失败用“尝试于”，完整时间悬停显示。按钮加载时固定宽度。今后新增时间/状态先考虑与对应操作同排，避免无必要的下一行。
- 家庭设置紧凑分组，刷新设置使用独立`Sheet::Refresh`；子设置有返回栈，关闭/Esc退出整层。短弹窗auto_shrink，滚动条按需；明确重置Area的最大布局高度，避免短子页返回父页后沿用旧尺寸使控件出界。保存只在有修改且有效时可用，成功后更新saved_prefs；不擅自自动保存。
- 显示卡片铺满列、标题旁有清残影图标；无连接提供连接入口但设备操作禁用。首页整块连接提示可点，房间名与箭头按文字宽度排布；家庭/房间/设备长名有截断保护。自动化/探索所有展示入口均转为实际页面或设置导航。
- 增加⌘,设置、⌘S保存、用量/温控页⌘R刷新；配对依旧明确点击，输入PIN不自动发起连接。共享玻璃主题与加粗provider名称保持。详细完成清单在`software/docs/ui-review-0222.md`。
- 验证：82项Rust回归、Clippy所有目标零警告；270张正式组件离屏样例（850×620、1100×780、1440×900与两种背景），包含错误/读取/空数据。原生窗口仅用隔离--ui-preview检查，不启动任何后台。以`3Inch7EPaper/artifacts/homecraft-0222-ui-validation.json`为最终记录，不能把软件预览说成硬件或真实iCloud链路已验证。


## 0.2.23 ESP32 Codex重置倒计时（2026-10-23，当前固件）

- 用户本轮明确指定ESP32墨水屏，随后明确授权“现在连接并烧录”；此前0.2.20之后的软件离线安排在本轮按此授权执行硬件更新。
- Codex总览5小时下加入小时钟和“2小时18分后重置”，横竖屏单/双/三卡及Codex详情均支持；大数字、服务名粗体、原剩余额度分段条保留。条仍代表额度，不是时间进度。使用现有官方resetsAt/BLE reset_at，无新增账户入口、凭据或协议字段；不足1分钟显示“即将重置”，过期“待更新重置时间”，缺失不补0或自动满额。现有30–60秒评估与双RAM差分保持，不恢复20次/30分钟周期全刷。
- 固件0.2.23已app-only烧录0x10000并校验，1,910,608字节，SHA256 `38d62b1479cee22e40cd6bd8df7d94602ff0f34d89908d676eb69af90330970c`，归档3Inch7EPaper/dist/firmware/HomeCraft-0.2.23-codex-reset.bin。原绑定认证重连、partial=1/ready=1，成功差分计数3→4，SD1430→1432/write_ok=1；NVS/SD、30秒采样/1秒IMU及原省电配置保留。不能再把板上当前版本记为0.2.19。
- 本轮实查当前运行入口为`/Applications/HomeCraft.app`0.2.22，短暂退出后已恢复原已安装App；未改Mac源码/软件包，不把项目dist旧版本当成当前安装版本。16组时间边界、144组四向渲染、56组差分及3,456组字号回归、IDF6.1编译通过。预览均为合成数值，新倒计时实屏观感待用户确认。记录3Inch7EPaper/artifacts/codex-reset-0223-validation.json及3Inch7EPaper/docs/display-redesign.md。


## 0.2.24 Argus详情防重叠、数字倒计时与进一步省电（2026-10-24，当前固件）

- 用户依次要求优化ESP32 Argus详情重叠、继续省电、重置处只显示数字。Argus现在按实际字形墨迹分行，标签/数值/缓存量各有空间；排行名称、Token、占比动态分列，长名缩略，进度条放字下，页脚和页码基线对齐。保留今日96/本周64优先层级、粗体服务名、官方分类及范围，不把0或缺失混为一谈。
- **重置最新偏好为小时钟+HH:MM（04:42），不要“小时/分/后重置”等文本。** 未提供/未校时为--:--，不足一分钟00:01，到期00:00，不自动补满额度。覆盖0.2.23的文字偏好。
- QMI内部CTRL2由0x06正常125Hz改0x0E低功耗11Hz，±2g/仅加速度/CTRL7=0x01，启动和只读USB读回验证；宿主仍每秒读取并保留稳定等待。BOOT/PWR仍10ms，30秒温湿度/PMU、SD分钟写入/fsync、差分和蓝牙安全均保留，不用冲突GPIO39/40。遥测仅在新采样/方向/有效性变化时发布；曲线仅在分钟/日期/SD等变化时重建。70.184秒实测仅3次遥测发布、2次曲线重建，静置IMU稳定、2次671ms局刷、SD6→9/write_ok=1；未测整板电流/续航增幅，新模式实体四向体验仍待确认。
- 0.2.24已app-only烧录0x10000并通过哈希，1,910,048字节，SHA256 `b371a24fdc2b0d6b92087c370332de73125f63b42515cb22c0ebc7d8b92e742e`，3Inch7EPaper/dist/firmware/HomeCraft-0.2.24-argus-energy.bin。原绑定已重连、partial=1，已安装/Applications/HomeCraft.app 0.2.22原样恢复，Mac源码未改。220组实际文字不交叠、16组Argus差分、56组环境/倒计时、3456组字号、16组时间及手势/方向回归通过，IDF6.1编译通过；当前板上不是0.2.23。预览均合成，当前真实Argus午夜后分类为0种，不把样例大数当实测。见3Inch7EPaper/artifacts/argus-energy-0224-validation.json、docs/display-redesign.md及3Inch7EPaper/docs/energy-policy.md。


## 0.2.25 主窗口与程序坞（2026-10-24，当前Mac App）

- 用户要求主窗口打开后在程序坞显示HomeCraft。现在启动/菜单栏显示主窗口时切换Regular并激活，红色关闭键后隐藏窗口、切回Accessory，菜单栏与后台继续；最小化保持Dock图标。LSUIElement仍为true以避免关闭后残留运行图标，不能简单删除该项使后台永久占Dock。没有新增设置页或改动用量/BLE策略。
- software/mac-app/native只在AppKit主线程调用公开setActivationPolicy，实际系统策略可在已启用的本地诊断中查看；未使用私有API或自动化系统程序坞。独立预览验证启动、3次关闭重开、最小化/恢复、7页导航、设置返回及大小窗口通过，预览不启动后台或访问配置。82项Rust测试、Clippy全目标零警告、Cargo check/release通过。
- 已更新本机/Applications/HomeCraft.app和项目dist/HomeCraft.app到0.2.25/build25，保留原配置/绑定，恢复原正常后台实例；系统读回Dock=true、完成启动。Apple芯片DMG software/dist/releases/HomeCraft-0.2.25-macos-arm64.dmg，签名/只读挂载/版本/白名单通过，仍ad-hoc未Apple公证。固件继续0.2.24，本轮未烧录。记录3Inch7EPaper/artifacts/homecraft-0225-dock-validation.json。


## 0.2.26 家庭主页摘要（2026-10-24，当前Mac App）

- 用户要求主页下方并列今日／本周Token，右侧温湿度放在一起。保留现有深色磨砂、橙色重点和20pt圆角；左侧AI用量卡显示日／周已知合计，右侧室内环境卡上下显示温度／湿度及一个采样时间，EPaper入口变为下面整行。摘要可进入用量／温控，设备行仍打开设备详情。
- Codex本周来自同一次官方account/usage/read的本周已返回日桶：按UTC+8周一至当天、同日去重、缺失不补0，不使用累计差值／上下文／额度推算。使用现有wt白名单字段与原账号去重；Argus／DeepSeek Harness沿用既有周统计。没有增加本机会话采集或官方请求。官方今日Token延迟依然存在，不把已知周合计称为完整实时账单。
- 温湿度读取现有遥测／iCloud展示副本，缺失为—，断开后保留灰色旧读数并显示状态与采样时间；点击摘要不触发硬件采样。30秒账户读取、5分钟自动遥测、安全绑定和固件均未改。
- 87项Rust回归、Clippy全目标零警告、官方方法模拟STDIO链路、原生隔离窗口导航／关闭重开通过。288张正式组件预览均为样例；36张首页在三种窗口宽度、两种背景及缺失／极大值／断开状态完成边界检查，人工查看主要大／小／暮色／空数据／旧数据视图。未做真实账户／BLE／双实体Mac的新一轮验证。
- 本机/Applications/HomeCraft.app与项目dist/HomeCraft.app已更新0.2.26/build26；更新前未检测到日常HomeCraft实例，备份旧App后替换，未启动日常后台以免自动扫描／连接硬件。用户打开App后使用新版；个人配置／绑定未改。安装包software/dist/releases/HomeCraft-0.2.26-macos-arm64.dmg签名、只读挂载、白名单和说明通过，仍ad-hoc未Apple公证。固件继续0.2.24，本轮未连接或烧录。记录3Inch7EPaper/artifacts/homecraft-0226-dashboard-validation.json与homecraft-0226-package-validation.json。


## 0.2.26 墨水屏主页同步改版与旧包清理（2026-10-24，当前固件）

用户补充“墨水屏也要改”，本轮将ESP32总览下方改为今日／本周Token汇总，右侧温湿度合成一张卡。横屏左侧双列、右侧上下温湿度；竖屏左侧日／周上下两行、右侧温湿度上下两行。仍用“今日／Token”和“本周／Token”两行标签、数字右对齐、小单位基线对齐，日80／周64优先并按宽度联动回退，日数字始终大于周。上方服务卡、大字额度、粗体名称、顶部日期、蓝牙与HH:MM倒计时保持。

新增usage_weekly_total只合计同一UTC+8周一起点的有效wt；未知、上周值不混入，真实0保留。已有HomeCraft0.2.26会从官方已返回的Codex本周日桶生成已知周合计，Argus／Harness使用原周汇总；不从账户累计／上下文／额度推算。没有改认证或协议白名单，也没有增加官方请求。旧Mac发送器尚未发送某家的wt时，该家不会被当作0。

140组实际文字边界、60组环境／日周差分、220组Argus边界回归及24张四方向样例渲染通过ASan/UBSan，ESP-IDF6.1构建通过。极大余额样例发现的旧上方溢出也改用有界K/M/B/T单位，正常余额格式保留。样例未发送到设备，实体新布局观感仍待用户确认。

固件0.2.26已app-only写入0x10000并通过哈希，1,910,992字节，SHA256 1a6d7a740fe9693d252c6d1036b52a3e64b66008a2a5411d8cb8357219b968d7，归档3Inch7EPaper/dist/firmware/HomeCraft-0.2.26-home-summary.bin。NVS／长期绑定／SD保留，私有回退备份位于backups且700/600，不分享。设备回报0.2.26，原认证连接恢复、Argus同日匹配、partial=1/ready=1，page=0/rotation=0/auto=1的671ms差分成功，计数3→4；SD1370→1376/write_ok=1。30秒采样、1秒IMU、40–80MHz和Wi-Fi停止保留。未操作Mac App进程，当前安装软件仍0.2.26；未新增功耗或四向实体测试。证据3Inch7EPaper/artifacts/overview-0226-validation.json。

按用户“只留最新版”删除23个旧DMG/ZIP及dist内10份旧测试App，共约313MiB。现在dist下仅HomeCraft.app，dist/releases仅HomeCraft-0.2.26-macos-arm64.dmg；固件归档与私有备份保留。software/tools/package_mac_dmg.sh在新DMG校验成功后调用prune_mac_installers.py清旧包，发现更高版本则拒绝删除。旧文档中的旧安装包／候选App路径是历史记录，已清理，不要自动恢复。清单3Inch7EPaper/artifacts/installer-cleanup-20261024.json。


## 0.2.27 温湿度数字粗体（2026-10-24，当前固件）

用户进一步澄清“数字要粗体，文字倒是问题不大”：本轮只加强数字与单位，中文温度／湿度标签保持原字重。总览右侧合卡、温湿度页读数和兼容旧总览读数均改用真实Arial Bold；20／24／28／32px所有宽度回退仍为粗体，°C与%一同加粗。粗体字库补入°，避免度数符号落入缺字回退。曲线数值刻度亦用粗体；布局、字号上限、Token和provider字重保持，不以放大字号代替加粗。

140组摘要文字边界、60组环境／日周差分和220组Argus回归通过ASan/UBSan，32张四方向总览／温湿度页预览（合成数据）及各回退尺寸的°字形检查通过。ESP-IDF6.1编译1,911,328字节，固件3Inch7EPaper/dist/firmware/HomeCraft-0.2.27-environment-bold.bin，SHA256 a6fd03a282bacb078aecd4bd631d6435b06b33aae6790737d255a0b51312326b。

已app-only烧录0x10000并通过写入哈希，原NVS／绑定／SD保留；设备回报0.2.27、原认证BLE连接恢复、partial=1/count=3/ready=1，SD1383→1387/write_ok=1。私有回退备份仍只留本机700／600。没有重启或替换Mac App，本机软件／唯一DMG仍0.2.26；没有新建旧安装包或恢复候选App。实屏粗体观感待用户确认，不能以预览代替。记录3Inch7EPaper/artifacts/environment-bold-0227-validation.json。


## 0.2.28 温湿度数字同号与对齐（2026-10-24，当前固件）

用户指出湿度数字比温度大，要求对齐。根因是0.2.27把数字和°C／%作为整串分别按宽度回退；°C更宽导致只有温度缩小。现在两项数字共同选择同一Arial Bold字号，°C／%单独放在固定宽度单位列，两项数字右边缘／小数位对齐，单位和数字共基线，并使用统一数字墨迹高度定位。不按单个读数或单位长度单独缩放；空间不足时两项一起回退。总览合卡、温湿度详情及旧显示读数共用此规则，中文标签保持原样，数字／单位继续粗体。

140组实际渲染检查覆盖相同字号、数字右缘、行距／单位基线、常见25.8／61.2、负温度、130／0、100%、缺失及四方向；60组差分回归与32张四方向样例通过ASan/UBSan。预览是合成数据，未发送实板；物理显示观感仍待用户确认。

ESP-IDF6.1编译并app-only烧录0.2.28至0x10000，写入哈希通过。应用1,911,408字节，SHA256 546d34397a7c3ad6ac3488ed7e58b7789c790f23c7e35d4069a1cc13c143427e，归档3Inch7EPaper/dist/firmware/HomeCraft-0.2.28-environment-aligned.bin。NVS／绑定／SD保留，设备回报0.2.28，原BLE连接恢复、partial=1/count=3/ready=1，page0/rotation0/auto1的671ms差分成功；SD1399→1401/write_ok=1。Mac App／唯一DMG仍0.2.26，本轮未重启或修改App。记录3Inch7EPaper/artifacts/environment-alignment-0228-validation.json。


## 0.2.29 今日／本周／本月（2026-10-24起，当前）

用户要求下方去掉Token字样、增加本月，同时压缩Argus与DeepSeek Harness。Mac首页与墨水屏总览现在显示“今日／本周／本月”，Argus和Harness详情也显示三项；今日保持主层级，周／月同级。墨水屏横屏汇总三列、竖屏三行，Argus横屏三卡、竖屏三行；Harness设备表也有三列。温湿度沿用0.2.28的相同Arial Bold字号、数字右缘与单位基线。这里是月Token，Codex的5小时／周额度界面继续保留，不恢复月额度。

日期按UTC+8：当天、周一、每月1日起算。Argus通过原官方get_token_usage新增一次月范围汇总，include_records:false，确认返回范围和计数；周／月失败互不抹掉当天成功值。Harness复用已解析的官方逐日记录合计当月，结算替换、缓存不重复相加与多Mac按稳定ID覆盖不变。Codex复用同一次account/usage/read已返回的本月有效日桶，同日取最大去重、缺失日不补0，不冒充完整实时账单。缺月为--，真实0保留，旧月不能带入新月。

新增可选mt:[YYYYMM01,total]和monthly_tokens:true能力协商；旧固件不收到mt，原Security2与480字节上限不变。板端Harness数字缓存v1迁移为v2，保留旧日／周／设备记录，月缺失为未知，不动dashboard绑定。iCloud同时输出无mt的HomeCraft-Data-v1和完整HomeCraft-Data-v2；本机旧缓存也继续输出兼容副本，同时间优先保留带月统计的记录。未改变30秒用量读取、5分钟遥测、SD分钟日志、局刷／省电／按键策略。

93项Rust测试、Clippy全目标零警告，140组摘要实际字形边界（含温湿同号／对齐）、220组Argus布局、64组环境／日周月差分、3456组双项与576组三项字号、32组Harness四向1–8设备缓冲检查通过。正式组件288张Mac预览、32张墨水屏样例与隔离原生窗口操作通过，均为合成数据。真实官方读取已得到Argus／Harness月统计及Codex已知月合计；实体新排版观感仍待用户确认。

Mac本机/Applications/HomeCraft.app与项目dist/HomeCraft.app为0.2.29/build29，唯一安装包software/dist/releases/HomeCraft-0.2.29-macos-arm64.dmg；旧包按用户要求清理，不恢复旧候选App。固件0.2.29已app-only写入0x10000且哈希通过，原NVS／绑定／SD保留；两台Mac的Harness缓存迁移后仍devices=2/valid=1。应用1,913,312字节，SHA256 4e29769909bea5baeb8a99d99b457d9517a40516d8c1395bd1f66fabc14c74b0，归档3Inch7EPaper/dist/firmware/HomeCraft-0.2.29-monthly-tokens.bin。验证记录3Inch7EPaper/artifacts/monthly-0229-validation.json；真实同步与最终硬件状态以该记录为准，不以样例代替实体结果。


2026-10-25收尾：Mac最终安装/项目包0.2.29/build29、唯一DMG校验通过，93项Rust与Clippy、288张最终组件预览和最终原生隔离检查通过。本机已启动新版并显示主页，官方Codex／Argus／Harness本月读取成功。设备0.2.29保留原连接，SD1433→1437/write_ok=1、partial=1/count=4/ready=1和671ms差分正常，2台Harness旧缓存迁移后valid=1。本机扫描未发现正在广播的设备，尚未同步到屏幕；用户明确“先不管另外一台”，本轮不再要求暂停另一台或尝试切换，月字段完整上屏验证留待后续。不能把这次本机月读取说成新月字段已通过实板传输。


## 0.2.30 扫描与普通启动配置修复（2026-10-25，当前Mac App）

用户截图为扫描完成但列表为空。只读USB确认板子仍为0.2.29、data=1/link=1；当前NimBLE最大连接数为1，官方Protocomm在连接断开后恢复广播，因此没有广播不能当成设备不在附近。本轮不操作另一台Mac、不重配、不改NVS或固件。

同时实查Finder启动的/Applications/HomeCraft.app使用未绑定的私有Support目录，而旧绑定/设置还在本机项目目录；此前用EPAPER_PROJECT启动成功不代表Finder入口已读回。新增本机600的.data-location.json显式选择已有配置目录，校验绝对路径、目录所有者、非符号链接和现有有效600绑定。选择只保存路径，不复制/轮换秘密，不编译开发机路径进包；新Mac无此本机文件时仍使用原有700的Support目录并自行首次认证。普通启动、CLI/statusLine和旧发送工具继续共用原配置与单实例锁。

扫描仍仅使用已知服务UUID；额外通过CoreBluetooth公开retrieve_peripherals恢复本机已连接服务或认证后保存的设备UUID。恢复项明确标为已记住/暂未收到广播，缓存不证明在线或通过认证，首次自动扫描不自动连接仅恢复的项。原Security2双向认证/服务与端点校验不变，EPair模式不按旧数据绑定连接。UUID缓存.ble-device.json为600，仅含名称与系统UUID，与当前绑定名称匹配后才使用；认证成功后才保存。不打印或返回配对秘密。

已记住但无广播时显示原设备名、广播状态和切换Mac提示，隐藏无意义的配对码表单；蓝牙关闭/权限不可用归为扫描错误，不显示普通空扫描。手动扫描仍尊重暂停。96项Rust、Clippy全目标零警告、300张正式组件样例、隔离原生窗口导航/连接页/关闭重开检查通过。样例不访问账户或BLE。

本机/Applications/HomeCraft.app、项目dist/HomeCraft.app和唯一安装包HomeCraft-0.2.30-macos-arm64.dmg均build30。退出旧的本项目App后更新；不带EPAPER_PROJECT、cwd=/正常启动，已验证原绑定加载且文件内容未变，实际连接页不再显示配对码。当前仍未发现广播/未连接；不能把绑定恢复和UI修复说成已完成BLE连接或本月上屏。用户先前要求另一台先不处理，本轮没有抢占或中断那条连接。实际状态、签名与包白名单见3Inch7EPaper/artifacts/discovery-0230-validation.json。


## 0.2.31 广播异常恢复与判断纠正（2026-10-25）

用户指出屏幕显示未连接。重新清缓冲只读查询确认link=0；独立服务过滤扫描0台、无过滤扫描13台但无墨水屏。原固件正常复位后立即恢复EPaper_B96AD4/ed07广播，因此先前仅根据旧link=1判断“必然是另一台占用”过强，不能再以此解释当前故障。

已app-only部署0.2.31并校验，保留原NVS／Security2／SD。增加EPAPER_BLE_STATUS读取实际host/synced/advertising/connections，区分传输连接和应用已认证数据标志；EPAPER_USAGE_STATS只输出provider编号、状态、日周月日期与数字，不输出账户／秘密。已绑定正常数据模式下，连续15秒无广播且无真实连接才恢复，60秒重试冷却；配对窗口、认证锁定、手机配网、关机及活跃连接均不恢复。EPAPER_BLE_RECOVER仅为USB闲置维护入口，执行前再查真实连接。

NimBLE连接枚举在本机IDF6.1源码核实后封装于ble_transport_probe.c，持host锁查询。该SDK缺少枚举函数的头文件声明，兼容声明集中隔离，升级SDK需重新核查，不涉及任何账户数据入口。正常状态每秒只读本机状态，不发额外BLE请求或恢复Wi-Fi。

实测原绑定认证、真实三家月字段已到板，active恢复请求accepted=0；App正常退出后advertising=1/connections=0，闲置恢复accepted=1/recoveries=1/error=0后仍广播，后续同秘密再次认证成功。用户已确认“终于正常了”。自动计时分支边界通过宿主ASan/UBSan，未在实板人为停止广播注入该定时故障，长期复发观察仍待。见3Inch7EPaper/artifacts/ble-0231-validation.json，当前0.2.32继承同机制。


## 0.2.32 今日／本周／本月同字号（2026-10-25，当前固件）

用户明确选择“三项统一字号”，覆盖旧的今日大、周／月小偏好。总览下方、Argus／Harness卡片及详情、Harness设备表三项数字均共用一个真实Arial Bold字号；按卡片空间和固定最长紧凑格式选择一次，0／--／数值长短／KMBTP变化不逐项缩放。单位同号且与数字共基线，数字右对齐。小B值仍可保留两位小数，100B以上用一位，避免100.00B多一位迫使字体变小，舍入仍自动进位单位。

Argus详情三张卡／竖屏三行等宽等高；总览服务名称使用共同粗体字号，Harness后缀与主名同字号，窄卡换行不缩成小字。温湿度同号粗体与原Codex5小时／周额度层级保持。普通数字与时钟继续差分，不恢复周期全刷。

4608组三项稳定字号、3456组旧双项工具、140组摘要实际墨迹边界、220组Argus、64组差分和32组Harness四向／设备数量检查通过ASan/UBSan；22张四向预览是合成值，未发实板。IDF6.1应用1,915,584字节，SHA256 d9f1ba822ecde32e22ce216003e60e958bce756eb53dc6419628e88a1dcb5cf9，3Inch7EPaper/dist/firmware/HomeCraft-0.2.32-uniform-type.bin。已app-only烧录并写入哈希验证，原绑定自动认证、真实Codex／Argus／Harness月数已独立回读，SD985/write_ok=1、partial=1/count=6/ready=1，30秒采样／1秒IMU／40–80MHz保留。当前App与唯一DMG仍0.2.30/build30，已恢复菜单栏真实同步。本次新字体实屏观感尚待用户确认。见3Inch7EPaper/artifacts/typography-0232-validation.json。


## 0.2.33 Codex昨日行（2026-10-25，当时App与固件）

用户最终要求保留5小时／周额度，并增加“昨日”一行同时显示昨日Token和昨天周额度百分比，底部进度条必须保留；不是把Codex改成日周月Token卡。横竖屏均保留三行、HH:MM及原当前5小时剩余分段条，三卡竖屏将小倒计时放在5小时行旁边。历史两项采用共同稳定粗体，标签仅“昨日”，用M／%区分；Argus、Harness与底部三项同字号规则保持。

昨日Token取原官方account/usage/read对应的UTC+8昨日桶，同日去重，不用最新但更老的日期。昨日周额度取已有600数值缓存中，同一已验证账号、昨日最后一次有效官方额度快照，显示已用比例100%-剩余，不能称为昨日新增消耗。账号读取前后仍核对一致；账号未知、缺快照、日期不符或字段缺失各自显示--，真实0保留。不新增官方请求，不读取登录凭据、私有HTTP或用Token推算额度。

新增可选yd:[YYYYMMDD,tokens_or_-1,week_used_permille_or_-1]，仅Codex且需yesterday_usage:true能力协商，旧设备剥离，480字节、Security2及绑定保持。云端／兼容本机数值摘要剥离派生yd，继续保留原w快照，因此旧App无需支持新字段；同账号历史匹配在本机完成。

98项Rust、Clippy全目标零警告、官方方法模拟链路、严格C字段、600组实际文字边界／进度条10段检查（四向、1–3卡、旧数据、缺失、0、极大数）、140摘要／220Argus／64差分回归及原生隔离窗口通过。24张预览为样例，未发实板。实际官方读到昨日20261024的100,015,606 Token、同账号昨日最后周额度已用28.0%，板端EPAPER_YESTERDAY独立回读吻合。

本机/Applications/HomeCraft.app与项目dist/HomeCraft.app已更新0.2.33/build33并恢复原绑定认证、真实同步。固件1,918,304字节，SHA256 8f03c6eb215e48d7c04238535a6942e787f80cc5fdd3829b5515a14dcee821c7，3Inch7EPaper/dist/firmware/HomeCraft-0.2.33-codex-yesterday.bin，已app-only写入0x10000并哈希验证，NVS／SD／省电／广播恢复与局刷保留。唯一安装包software/dist/releases/HomeCraft-0.2.33-macos-arm64.dmg，签名／只读挂载／版本／内容白名单一致，旧0.2.30包已按要求清理。新行实屏观感尚待用户确认，见3Inch7EPaper/artifacts/codex-yesterday-0233-validation.json。


## 0.2.34 Codex 5小时排版微调（2026-10-25，当前固件）

用户仅要求优化总览Codex的5小时标签、重置倒计时与右侧数字。普通卡片将“5小时”和粗体16px HH:MM／12px时钟组成左侧上下组，以实际墨迹高度与右侧百分比居中；三卡竖屏空间较紧时倒计时与标签同排。标签普通卡24px、紧凑卡20px；右侧数字按卡片几何固定选择80／56／32px上限，预留100%的宽度，百分比变化不逐值缩放；数字和%列与原周额度对齐。

周额度、昨日Token／昨天周额度用量、底部当前5小时进度条及其他卡片均保留；Mac App与唯一DMG仍0.2.33/build33，未改查询／协议／绑定／日周月同字号／温湿度／差分及省电策略。2400组实际墨迹边界检查覆盖四方向、1–3卡、未知／0／100%、四种倒计时和昨日边界，逐组核对字号／相对右缘固定且10段进度条存在；140组摘要与64组差分回归通过ASan/UBSan。21张预览是合成数据，未传实板。

EIM实查选中IDF6.1／IDF Python3.12.15；固件1,919,008字节，SHA256 8246e6de50f4accd49a1892a5c3dcef0507fc157b2a5535c329258398e782c96，归档3Inch7EPaper/dist/firmware/HomeCraft-0.2.34-codex-primary.bin。已app-only写入0x10000并通过哈希，NVS／SD保留；原绑定未变化且App自动认证、真实同步恢复，板上version=0.2.34、connections=1/link=1、昨日20261024/100015606/280、SD1052/write_ok=1、partial=1/ready=1及673ms总览差分已独立回读。新排版实屏观感待用户确认；见3Inch7EPaper/artifacts/codex-primary-0234-validation.json。

## 2026-10-25 喇叭与中文人声试听（已恢复原仪表盘）

用户确认已装外接喇叭并接好USB，授权试听。独立临时程序使用官方ES8311驱动、44.1kHz/16位，GPIO39功放；不启用麦克风、拨轮、IMU中断，不访问NVS／SD／BLE。原创28秒多声部器乐初始电平偏低，最终PCM峰值-1dBFS、驱动85→95→100，用户确认“音量够了，声音正常”。随后播放邓丽君《月亮代表我的心》Apple官方约30秒试听（非整首），用户确认“人声清楚，音量合适”。不能再把音频写成完全未实测；录音仍未测。

播完静音关闭PA并读回确认原音频供电寄存器恢复。原0.2.34 factory应用区已完整写回并通过哈希；dashboard NVS327680字节与试听前逐字节一致，未重写配置或SD。HomeCraft0.2.33原绑定认证及真实同步已恢复，SD1349/write_ok=1、partial=1/count=2/ready=1。日常固件仍无音乐播放入口，本轮仅临时试听，不是正式播放器交付。含配置回退备份继续本机600权限；官方歌曲音频只在临时目录、不进入发行包。见docs/audio-audition.md及3Inch7EPaper/artifacts/audio-audition-20261025-validation.json。

## 0.2.35 USB插拔快速电源局刷（已随0.2.36保留）

用户要求插拔充电立刻更新顶部电源标志。USB有效时均显示插头+USB（含有电池及已充满），拔掉后显示PMU实际电量；不根据“通常已经充满”假定100%。既有按键任务每100ms只读0x00/0x01，稳定100ms后额外读取一次实际SOC；常规电量仍30秒，不修改充电、计量或IRQ配置。局刷底图有效、同页同方向时只重绘顶部104×32电源区域，保持正文/已显示温湿度及手动采样确认正确，不推迟普通内容的30–60秒合并。暂停也可更新电源；全刷忙时等待当前操作，保留原错误恢复和无周期全刷。

768组四方向/12页/暂停/电源边界渲染、64组环境差分、140组主页及手动采样/协议检查通过。0.2.35已app-only烧录并哈希通过，原HomeCraft0.2.33认证与同步恢复、SD1366→1368/write_ok1、partial1/ready1保留。用户实际拔插确认“两次都很快，显示正常，功能没问题”；RAM事件的稳定source→显示为708ms和760ms，另有检测/稳定等待，不能说零延迟。

用户追问耗电：新增约20次I²C寄存器读取/秒；不是每100ms刷屏或读完整电量。存在额外开销，未测整板电流/续航差值，不宣称零增加。维护命令EPAPER_POWER_EVENTS只读最近8项RAM电源/显示事件，无凭据、不写NVS/SD。当前记录docs/power-events.md及3Inch7EPaper/artifacts/power-events-0235-validation.json。


## 0.2.36 SD 音乐页（2026-10-26，当前）

用户要求新增板端音乐页，自动读取SD根目录/music并支持常见音频。固件0.2.36新增PAGE_MUSIC=12，原0–11索引不变，显示顺序在温湿度后。上电自动扫描、不自动播放；停止时每60秒检查目录，最多128首/每名240 UTF-8字节，第一层、忽略隐藏/空文件/子目录。支持MP3、PCM WAV、FLAC、ADTS AAC、M4A以及Ogg Vorbis/Opus；M4A官方解析器要求MOOV在MDAT前，无DRM、无快进/歌词/平台缓存支持，详细范围见3Inch7EPaper/docs/music.md。

音乐页BOOT单击下一首/双击播放暂停/三击离开（后台继续）；PWR单击上一首/双击音量+5/1.2至不足3秒松手音量-5/三击停止回总览。约3秒正常关机和BOOT3秒配网保留，拨轮禁用。不要把音乐页双击恢复成全刷；HomeCraft仍可清残影。音量默认90，双声道平均后单喇叭输出。中文文件名常用字形20px，横竖屏差分和页/方向全刷保持。

官方esp_codec_dev1.6.2与esp_audio_codec2.6.2固定；network_provisioning仍本地1.3.1补丁。ES8311仅DAC，共用I²C，I²S13/14/47/48和PA39，不启用麦克风或IMU GPIO39中断。播放160MHz并锁定所需电源，暂停/停止关闭PA、恢复原ALDO1位及40–80MHz/自动轻睡。解码栈/大缓冲/歌表放PSRAM，SD输入内部DMA缓冲，internal阈值2048/保留64KiB。修复分配失败后I²S句柄泄漏及整4096字节文件末尾EOS，选曲清除旧采样参数。未测电流/续航。

SD继续由原环境日志挂载，音乐持读取租约，日志错误时等文件关闭再卸载，不格式化；每分钟CSV和午夜文件保留。正常关机先停音乐再停日志/网络。七个原创3秒音频实板解码及完整输出通过：MP3/16bit WAV/24bit FLAC/AAC/AAC M4A/Ogg Vorbis/48k Opus；暂停/继续、顺序切歌、曲尾和停止PA0/电源恢复/CPU回退通过。测试结束已完整内容校验清理七文件，卡上1个原有文件保留；未把用户尚未放入的歌曲称为已测。未测物理拔卡/异常断电/完整长曲、全部采样率及M4A ALAC实板组合；用户实屏新音乐页及听感待确认。

音乐目录/PCM边界与112组绘图器四向状态、832组电源头、64差分/140总览/220Argus及严格设备/采样协议回归通过。HomeCraft0.2.36/build36识别13位页掩码和“音乐”选项，99项Rust与Clippy通过；/Applications与dist App一致，唯一DMG为software/dist/releases/HomeCraft-0.2.36-macos-arm64.dmg，本机签名未Apple公证。固件已app-only烧录、哈希通过，原绑定/SD/局刷保留；Mac原绑定认证及切音乐页成功帧已实查，后续最终摘要见3Inch7EPaper/artifacts/music-0236-validation.json与homecraft-0236-release-validation.json。诊断烧录前先退出本项目App，烧录后再恢复，避免旧CoreBluetooth待连接句柄。无新配对/无密钥轮换，私有回退备份不分享。
