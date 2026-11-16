# ESP32-S3-ePaper-3.97 硬件连接记录

本项目使用微雪 ESP32-S3-ePaper-3.97。以下引脚用于后续固件与界面开发，数字均为 ESP32-S3 的 GPIO 编号，**不是连接器针脚序号**。

记录更新至2026-10-26。引脚依据官方示例源码核对，尚未逐项对照原理图。实测情况单独记录，未测接口不能由其他外设正常工作推断。实际板卡的PCB版本、预留焊盘、连接器针序和电池极性仍需按实物确认。

## 板卡资源

| 项目 | 配置 |
| --- | --- |
| 主控 | ESP32-S3，双核，最高 240 MHz |
| Flash 与 PSRAM | 16 MB Flash，8 MB PSRAM |
| 屏幕 | 3.97 英寸墨水屏，原生 800 × 480 |
| 无线 | 2.4 GHz Wi-Fi、BLE |
| 传感器 | SHTC3 温湿度、QMI8658 六轴 IMU |
| 时钟 | PCF85063 RTC，预留 RTC 备用电池接口 |
| 音频 | ES8311 编解码器、NS4150B 功放、麦克风、喇叭接口 |
| 存储与交互 | Micro SD 卡槽、三向拨轮、BOOT、PWR |
| 外部连接 | USB Type-C、3.7 V 主锂电池接口、RTC 备用电池接口、喇叭接口 |

硬件资源依据[官方产品文档](https://docs.waveshare.net/ESP32-S3-ePaper-3.97/)。电池与喇叭接口使用 MX1.25 连接器；外接前核对实际针序、极性及规格。

## 墨水屏

| 信号 | GPIO | 源码含义 |
| --- | --- | --- |
| SCLK / SCK | 11 | SPI 时钟 |
| MOSI | 12 | 向屏幕发送数据 |
| CS | 10 | 屏幕片选 |
| DC | 9 | 命令与数据选择 |
| RST | 46 | 屏幕复位 |
| BUSY | 3 | 驱动等待其变为低电平后继续 |

完整 ESP-IDF 应用使用 `SPI3_HOST`、模式 0、20 MHz、DMA，MISO 配置为 `-1`。屏幕电源由 PMU 的 `ALDO3` 通道控制；该映射是本版驱动的实现，需连同电源芯片版本核实。

Arduino 的 `DEV_Config.h` 与此屏幕引脚一致，但其 `DEV_Config.cpp` 使用 GPIO 模拟 SPI。不要将 Arduino 示例中的 `EPD_PWR_PIN = -1` 当作真实电源控制引脚。

来源：[屏幕头文件][epaper-h]、[屏幕实现][epaper-c]。绘图、局刷及休眠方法见 [墨水屏使用说明](epaper-usage.md)。

## 共用 I²C 总线

| 信号或参数 | 配置 |
| --- | --- |
| SDA | GPIO41 |
| SCL | GPIO42 |
| 控制器 | I²C 0 |
| 源码配置频率 | 400 kHz |

以下为 **7 位地址**，使用 ESP-IDF 地址参数时无需左移：

| 设备 | 地址 | 说明 |
| --- | --- | --- |
| PCF85063 | `0x51` | RTC |
| SHTC3 | `0x70` | 温湿度 |
| ES8311 | `0x18` | 编解码器控制，音频流另走 I²S |
| PMU | `0x34` | 源码按 AXP2101 初始化，产品文档称 TG28，型号关系待确认 |
| QMI8658 | `0x6A` 或 `0x6B` | IDF 驱动依次尝试两者；Arduino 示例优先使用高地址 |

初始化顺序为 `i2c_master_init()` → `i2c_devices_init()` → 各器件初始化。共用总线只初始化一次。来源：[I²C 定义][i2c-h]、[I²C 实现][i2c-c]、[IMU 实现][imu-c]。

## 拨轮与按键

用户反馈拨轮可能已损坏，当前仪表盘按无拨轮设计，自动采样、接收额度与刷新；不采集 GPIO4/5/6 输入。以下事件表仅记录上游示例的行为。

当前双按键固件进一步将 GPIO4/5/6 配置为 `GPIO_MODE_DISABLE`，禁用上下拉和中断。BOOT 使用 GPIO0 上拉输入，PWR 经 PMU 的按键 IRQ 状态轮询；具体交互以 [README](../../README.md) 的当前映射为准，下面表格中的事件码只属于上游按键库。

| 输入 | GPIO | 电平与事件 |
| --- | --- | --- |
| 拨轮上 | 4 | 低电平有效；单击事件码 `0` |
| 拨轮中键 | 5 | 低电平有效；单击 `7`、双击 `8`、长按开始 `12` |
| 拨轮下 | 6 | 低电平有效；单击事件码 `14` |
| BOOT | 0 | 低电平有效；单击 `21`、双击 `22`、长按 `23` |
| PWR | 未按普通 GPIO 按键处理 | 源码由 PMU 配置电源键行为，勿与 BOOT 混用 |

调用 `button_Init()` 后，通过 `wait_key_event_and_return_code(timeout)` 读取事件，超时返回 `-1`。驱动每 5 ms 扫描，支持去抖、单击、双击、长按等状态。

完整应用主菜单中，上下键移动、中键单击进入、中键长按全刷、BOOT 双击关机、BOOT 长按进入设置。不同页面可能另有按键含义。来源：[按键实现][button-c]、[应用入口][main-cc]。

## Micro SD 卡

| SDMMC 信号 | GPIO |
| --- | --- |
| CLK | 16 |
| CMD | 17 |
| D0 | 15 |
| D1 | 7 |
| D2 | 8 |
| D3 | 18 |

当前板端日志已使用上述4位总线、20MHz验证SD挂载、按分钟写入和重新打开CSV解析。数据在卡内`/EPaper/env/YYYYMMDD.csv`，挂载失败不格式化；温湿度来自板载SHTC3。已观察到本地午夜从20261020切换20261021的新文件。原始每分钟CSV与屏上15分钟均值曲线分开，详见 [硬件升级记录](hardware-upgrade.md)。重启恢复、异常拔卡等验证以该文档和最新验证JSON为准。

完整 IDF 应用使用 4 位 SDMMC，挂载到 `/sdcard`；Arduino 示例 `SD_MMC.begin("/sdcard", true)` 使用 1 位模式。卡槽按官方文档准备 FAT32 文件系统。

`_sdcard_init()` 挂载失败时只记录错误并返回，调用方仍需确认挂载是否成功。字体、图片、阅读文件和音频功能按实际配置依赖 SD 卡。来源：[SDMMC 实现][sd-c]。

## 音频 I²S

| 信号 | GPIO | 方向以 ESP32-S3 为准 |
| --- | --- | --- |
| MCLK | 13 | 主时钟输出 |
| BCLK / BCK | 14 | 位时钟 |
| WS / LRCK | 47 | 帧时钟 |
| DIN | 21 | 从编解码器接收录音数据 |
| DOUT | 48 | 向编解码器发送播放数据 |
| PA 控制 | 39 | 功放使能，Arduino 示例置高开启；与 IMU 宏定义冲突，见下文 |

编解码器通过 I²C 配置，音频数据走 `I2S_NUM_0`。IDF 示例提供 `i2s_driver_init()`、`es8311_codec_init()` 及播放、录音句柄。来源：[音频定义][audio-h]。

2026-10-25用户确认已安装外接喇叭，已用独立临时固件实测44.1kHz/16位播放。原创多声部器乐最终提高到-1dBFS峰值、驱动音量85→95→100，用户确认“音量够了，声音正常”；邓丽君《月亮代表我的心》官方30秒试听也获确认“人声清楚，音量合适”。播完静音、PA拉低并恢复音频供电寄存器。此为早期试听记录；0.2.36 已集成 SD 音乐页，见[SD音乐](music.md)。试听恢复与边界见[喇叭试听记录](audio-audition.md)和对应验证JSON。此次未测试录音。

## RTC 和电源状态信号

| 信号 | GPIO | 源码用法 |
| --- | --- | --- |
| `RTC_INT` | 45 | PCF85063 中断输入 |
| `PWR_OUT` | 1 | 上拉输入，用于启动模式判断；不是在此处配置的电源输出 GPIO |

RTC 提供读写时间、设置闹钟、清除闹钟标志等接口。完整应用的部分时钟模式先设置 RTC 闹钟，再调用 PMU 关机，启动时读取保存的模式；不要将它等同于 ESP32 的普通定时 deep sleep。

`get_battery_power()` 从 PMU 获取电量，`get_usb_connected()` 读取 VBUS 状态，`axp_pwr_off()` 先清理音频再请求 PMU 关机。来源：[RTC 定义][rtc-h]、[PMU 实现][pmu-c]、[时钟页面][clock-cc]。

## 已知差异和待确认连接

1. **GPIO39 重复定义。** [音频驱动][audio-h]将 GPIO39 定义为功放控制；[IMU 头文件][imu-h]将 `DOF_INT1` 定义为 GPIO39、`DOF_INT2` 定义为 GPIO40。Arduino IMU 示例也定义了 `SENSOR_IRQ = 39`。这是源码中的冲突，不能据此认定实物将两路信号相连。普通 IMU 初始化主要使用 I²C；启用运动唤醒等中断功能前先查原理图，勿直接运行重配 GPIO39 为输入的初始化。
2. **电源芯片名称不同。** 官方产品文档称 TG28，本版源码使用 `XPowersPMU`、AXP2101 名称和地址 `0x34`。目前不把两者记为已经确认完全兼容。
3. **原理图与实物仍需确认。** IMU 中断实际连接、USB/预留焊盘连接、接口极性、PCB 修订版及全部电源轨尚未逐项核实。资料入口见[相关资料和原理图](https://docs.waveshare.net/ESP32-S3-ePaper-3.97/Resources-And-Documents/)。

## 示例结构与引用版本

上游：[waveshareteam/ESP32-S3-ePaper-3.97](https://github.com/waveshareteam/ESP32-S3-ePaper-3.97)。本记录对应提交 `9b12d40731a80213b927ee8a421cae4082952819`，提交日期 2026-03-20。以下链接固定到该提交。

- `Arduino/examples`：音频、屏幕、RTC、温湿度、SD 卡、IMU 六个示例。
- `ESP-IDF/01` 至 `07`：屏幕、麦克风回放、SD 音乐、SD 图片、IMU、RTC、温湿度示例。
- `ESP-IDF/08_ESP32-S3_e-Paper-3.97`：完整菜单应用，包含文件、时钟、日历、闹钟、天气、网络、音频、阅读。
- `Firmware`：现成固件及说明；小智源码由仓库 README 指向独立上游。

本记录中的 `components/...`、`main/...` 均相对于上述 `08` 示例目录。临时下载副本不作为项目永久依赖。

## 本项目实现

仪表盘的 I²C 与传感器驱动位于本项目 `3Inch7EPaper/main/board.c`，屏幕驱动位于 `3Inch7EPaper/components/epaper_vendor/`。SHTC3 流程为唤醒 `0x3517` → 等待 300 μs → 不拉伸时钟测量 `0x7866` → 等待 20 ms → 读取 6 字节并分别校验温湿度的 CRC8 → 休眠 `0xB098`。CRC 多项式 `0x31`，初值 `0xFF`；温度按 `-45 + 175 × raw / 65536`、湿度按 `100 × raw / 65536` 换算，不附加温度偏移。

屏幕供电只设置ALDO3为3.3V并使能，读改写寄存器以保留其他位；未修改充电参数。初始化前读取PMU的`0x03` ID寄存器，检查`(id & 0xCF) == 0x4A`，未识别时停止配置电源。0.2.36起GPIO39与ES8311已用于SD音乐播放；IMU中断、RTC和拨轮仍未启用。SDMMC同时用于分钟环境日志与音乐读取，音频停止恢复原ALDO1供电位。

## 2026-10-20 上板记录

- esptool 识别 ESP32-S3 rev0.2，16 MB Flash、8 MB PSRAM；PSRAM 启动测试通过。
- 通过 GPIO41/42 的 I²C 总线读取 PMU `0x34`，ID 寄存器实测 `0x4A`。按上述 ALDO3 配置后，屏幕驱动完成初始化与首屏全刷，未触发 BUSY 超时。这验证了本板的基本供电与驱动路径，尚不能单凭 ID 断定 TG28 与 AXP2101 所有寄存器完全兼容。
- SHTC3 在 `0x70` 连续读取并通过 CRC，首轮记录为 31.65°C、45.81% RH，随后约 31.7°C、46.8% RH。数值只代表当时传感器读数，没有与外部仪表校准；板体发热会影响温度。
- USB Serial/JTAG 已验证烧录、运行日志以及双向额度协议：Mac 发送后收到 `EPAPER_ACK`。19:50（UTC+8）快照为 5 小时窗口剩余 94.0%、7 天窗口剩余 55.0%；这些是当时真实账户额度，后续会变化，不应写成固定显示数据。
- 早期局刷固件虽记录`EPAPER_DISPLAY partial`，用户仍反馈文字、数字错乱或残影；改为整屏全刷后，当日19:54左右用户确认正常。这是旧路径记录。2026-10-21新双RAM差分路径已通过两轮实板测试及用户视觉确认，已启用并验证重启保留，详见[硬件升级记录](hardware-upgrade.md)。具体旧版问题不声称已确定唯一根因。
- 原始 Flash 已完整备份，路径和 SHA-256 见 [开发环境记录](../../software/docs/development-environment.md)。未使用的 SD、RTC、IMU、音频、外接连接器及按键不记为已上板验证。

### 双按键和自动横竖屏版本

- 2026-10-20 新固件已烧录并通过 Flash 哈希校验。QMI8658 实测地址 `0x6B`、WHO_AM_I `0x05`，连续加速度读取正常；板子原放置方向的采样约 `(-0.04, +0.94, -0.10) g`。
- IMU 配置：先关闭 `CTRL7(0x08)`，`CTRL1(0x02)=0x60`（自动增址、小端），`CTRL2(0x03)=0x06`（±2g、125 Hz），`CTRL5(0x06)=0`，`CTRL7=1`（只开加速度计）。从 `0x35` 连读 6 字节，按有符号小端值除以 16384 得到 g。屏幕方向使用重力向量，不积分陀螺仪角速度，避免静态漂移；不配置 GPIO39/40。
- PWR 按键寄存器依据微雪提供的 [TG28 官方数据手册](https://files.waveshare.com/wiki/common/TG28_Datasheet_V1.0_en.pdf) 核对：`0x41[3:0]` 使能短按、长按、下降沿和上升沿事件；轮询 `0x49[3:0]`，只对已读取的按键位写 1 清除，不清除电池、VBUS 等其他状态。下降沿记为按下，上升沿记为松开。
- 软件手势使用 20 ms 去抖、400 ms 连击间隔和 1.2 秒长按。`0x27[3:2]=10b` 将 PWR 硬件关机时长设为 8 秒，`0x22[1]=1` 保留硬件长按关机，`0x22[0]=0` 选择关机而非重启；读改写保留其他位，未修改充电参数。
- 首屏全刷与真实额度 USB 回执已确认；实体按键手势及 IMU 与屏幕实际方向的对应关系仍需用户操作验证。软件侧已检查单/双/三击互斥、长按无额外点击、四向像素映射、平放保持方向和晃动抑制。

### Wi-Fi 独立查询版本

- 2026-10-20 已编译、烧录并通过 Flash 哈希校验。沿用全部屏幕、I²C 与按键引脚，无需外接无线模块。
- 实板日志确认 STA + SoftAP 启动，DHCP 地址为 `192.168.4.1`，配网 HTTP 服务已启动，网络设置页全刷成功。系统 NVS 初始化成功；新增配置位于 `dashboard` NVS 分区，偏移 `0x310000`、大小 `0x50000`，不擦除原系统 NVS。
- 已验证 USB 一次性配置入口对非法查询间隔、凭据中的 CR/LF 控制字符返回 `ESP_ERR_INVALID_ARG`；没有把测试内容存为真实凭据。
- 本机直接 HTTPS 读取 Codex 已成功；**开发板连接家庭 Wi-Fi、SNTP、TLS 查询与令牌自动续期仍待用户完成配网/授权后验证**。Claude 和 DeepSeek 没有用户凭据，尚未实测其账户接口；不得把结构测试、模拟预览或热点启动当作这些步骤已通过。
- 网络页三击 BOOT 用于开关配网热点，其他页面仍用于开关自动旋转；PWR 行为保持不变。来源与操作见 [用量接口记录](../../software/docs/usage-providers.md)。

### 当前 Mac 蓝牙数据版本

- 用户因校园网访问限制改选Mac读取额度、加密BLE发送；板子无需Wi-Fi或账户密钥。沿用上述全部引脚、板载温湿度、重力方向和双按键实现，继续禁用拨轮。最初使用整屏全刷，当前已升级为用户确认后的差分与周期全刷策略。
- 官方手机应用已完成 Security 2 Wi-Fi 配网，设备取得 IP 并完成 SNTP；此前 Wi-Fi 记录中的“待配网/校时”已解决，但校园网下的板上账户 HTTPS 查询与续期仍未验证。当前固件配网结束后恢复 Mac 数据模式，不自动执行板上账户查询。
- 已实际验证 Mac Security 2 认证、真实 Codex 快照、设备无 Wi-Fi 时接收、错误配对秘密拒绝、设备重启后自动重连。用户确认实屏 BLE LIVE、额度、温湿度与文字数字显示正常。Claude/DeepSeek 尚无 Mac 侧凭据，显示未配置。
- BOOT 长按阈值现为 3 秒，PWR 软件长按仍为 1.2 秒；网络页 BOOT 双击打开 Mac 配对窗口。未完成的实体按键逐项测试、四种实物放置方向对应关系仍不记为全部通过。启动、停止及安全边界见 [蓝牙数据传输说明](../../software/protocol/ble-data.md)。

### 电池与USB电源读取

- 依据微雪提供的 [TG28数据手册](https://files.waveshare.com/wiki/common/TG28_Datasheet_V1.0_en.pdf) 第24–25、27、44页：`0x00[5]` VBUS可用，`0x00[3]` 电池存在，`0x01[6:5]=01b` 充电电流方向，`0x18[3]` 电量计启用，`0xA4` 电量百分比。与官方示例 XPowers 的读取路径一致，不据此宣称所有AXP2101寄存器兼容。
- `board_read_power()` 每5秒只读采样；前后核对电源/电池存在位，热插拔变化时本轮无效。只有电池存在、计量启用且读数0–100时才发布百分比。无电池时不会把`0xA4`默认0误报成0%；禁用计量、读失败、非法值显示未知。没有修改充电电流、电压、检测或电量计配置。
- 百分比是芯片电量计估算，没有针对用户电池校准或做续航验证；未连接电池的USB测试不能代替电池SOC/充电实测。主锂电池与RTC备用电池分开，显示的是主电池状态。
- USB命令`EPAPER_POWER`只输出valid/usb/battery/charging/percent，便于后续诊断，不包含账户或配对秘密。实际本板读数以最新部署记录为准。
- 2026-10-20 本增量烧录后连续5次读取均为 `valid=1 usb=1 battery=1 charging=0 percent=100`，验证本板上述状态/电量读取路径可用。只说明PMU当时报告USB、电池和100%估算值；没有执行拔电池、放电或充电曲线测试。

[epaper-h]: https://github.com/waveshareteam/ESP32-S3-ePaper-3.97/blob/9b12d40731a80213b927ee8a421cae4082952819/ESP-IDF/08_ESP32-S3_e-Paper-3.97/components/epaper_port/epaper_port.h
[epaper-c]: https://github.com/waveshareteam/ESP32-S3-ePaper-3.97/blob/9b12d40731a80213b927ee8a421cae4082952819/ESP-IDF/08_ESP32-S3_e-Paper-3.97/components/epaper_port/epaper_port.c
[i2c-h]: https://github.com/waveshareteam/ESP32-S3-ePaper-3.97/blob/9b12d40731a80213b927ee8a421cae4082952819/ESP-IDF/08_ESP32-S3_e-Paper-3.97/components/i2c_bsp/i2c_bsp.h
[i2c-c]: https://github.com/waveshareteam/ESP32-S3-ePaper-3.97/blob/9b12d40731a80213b927ee8a421cae4082952819/ESP-IDF/08_ESP32-S3_e-Paper-3.97/components/i2c_bsp/i2c_bsp.c
[button-c]: https://github.com/waveshareteam/ESP32-S3-ePaper-3.97/blob/9b12d40731a80213b927ee8a421cae4082952819/ESP-IDF/08_ESP32-S3_e-Paper-3.97/components/button_bsp/button_bsp.c
[sd-c]: https://github.com/waveshareteam/ESP32-S3-ePaper-3.97/blob/9b12d40731a80213b927ee8a421cae4082952819/ESP-IDF/08_ESP32-S3_e-Paper-3.97/components/sdcard_bsp/sdcard_bsp.c
[audio-h]: https://github.com/waveshareteam/ESP32-S3-ePaper-3.97/blob/9b12d40731a80213b927ee8a421cae4082952819/ESP-IDF/08_ESP32-S3_e-Paper-3.97/components/es8311_bsp/es8311_bsp.h
[rtc-h]: https://github.com/waveshareteam/ESP32-S3-ePaper-3.97/blob/9b12d40731a80213b927ee8a421cae4082952819/ESP-IDF/08_ESP32-S3_e-Paper-3.97/components/pcf85063_bsp/pcf85063_bsp.h
[imu-c]: https://github.com/waveshareteam/ESP32-S3-ePaper-3.97/blob/9b12d40731a80213b927ee8a421cae4082952819/ESP-IDF/08_ESP32-S3_e-Paper-3.97/components/qmi8658_bsp/qmi8658_bsp.c
[imu-h]: https://github.com/waveshareteam/ESP32-S3-ePaper-3.97/blob/9b12d40731a80213b927ee8a421cae4082952819/ESP-IDF/08_ESP32-S3_e-Paper-3.97/components/qmi8658_bsp/qmi8658_bsp.h
[pmu-c]: https://github.com/waveshareteam/ESP32-S3-ePaper-3.97/blob/9b12d40731a80213b927ee8a421cae4082952819/ESP-IDF/08_ESP32-S3_e-Paper-3.97/components/axpPower/axp_prot.cpp
[main-cc]: https://github.com/waveshareteam/ESP32-S3-ePaper-3.97/blob/9b12d40731a80213b927ee8a421cae4082952819/ESP-IDF/08_ESP32-S3_e-Paper-3.97/main/main.cc
[clock-cc]: https://github.com/waveshareteam/ESP32-S3-ePaper-3.97/blob/9b12d40731a80213b927ee8a421cae4082952819/ESP-IDF/08_ESP32-S3_e-Paper-3.97/main/page_clock/page_clock.cc


## 0.2.19 省电采样

用户要求SHTC3及PMU每30秒、QMI重力每1秒读取，按键仍10ms及原手势。IMU仅降低读取频率，芯片原125Hz加速度配置未改、不启用冲突中断。CPU配置40–80MHz动态频率，BLE Modem-sleep和自动Light-sleep，数据模式停止Wi-Fi射频。用户纯电池约1分钟后，安全诊断读取5101次睡眠、累计3492300us，BLE link=1、Wi-Fi running=0、SD写入正常；这不是电流或续航测量。完整见docs/energy-policy.md及对应验证JSON。


## 0.2.24 IMU内部低功耗（2026-10-24）

QMI8658初始化由正常125Hz（CTRL2=0x06）改为低功耗11Hz（CTRL2=0x0E）；仍±2g、CTRL7=0x01只开加速度计，GPIO39/40未用。依据微雪提供的QST Rev0.9手册30–31及45页，低功耗模式要求关闭陀螺仪；宿主仍每1秒读取，原量程换算、稳定等待、平放保持不变。已在本板读回0x0E/0x01，静置1秒采样正常；新模式实体四向效果、整板电流及续航仍未完成对比测量。原125Hz实测属于上方历史。见docs/energy-policy.md及3Inch7EPaper/artifacts/argus-energy-0224-validation.json。

## 0.2.35 USB供电变化（2026-10-25）

PMU0x00/0x01供电标志独立每100ms只读检测，稳定100ms后读取一次实际电量、快速差分顶部图标；常规完整电量仍30秒。USB有效始终显示插头+USB，拔掉后显示实际百分比，未知为--。用户实测两次切换均快且正常，稳定source到画面完成分别708ms/760ms；包括前端检测预计空闲时约1秒，正在全刷则等待。新增约20次寄存器读取/秒，不修改充电/计量/中断参数；未测此增量电流或续航，详见[电源快速局刷](power-events.md)。
