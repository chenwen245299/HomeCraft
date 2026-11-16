# ESP32-S3-ePaper-3.97 墨水屏使用说明

显示流程是：初始化总线和电源 → 分配图像缓冲 → 用 `GUI_Paint` 绘图 → 调用 `EPD_Display_*()` 刷屏 → 空闲时让屏幕休眠。绘图函数只修改内存，不会自动更新屏幕。

本说明对应 [硬件记录](hardware.md) 中的官方仓库提交，主要以完整 IDF 应用的 `components/epaper_port` 和 `components/epaper_lib` 为依据。文中的 `GUI_Paint` 代码片段用于上游移植说明，并非当前仪表盘的调用代码；本项目实现见下节。

## 当前仪表盘实现

0.2.35起，USB插拔单独快速检测，显示空闲时约1秒内差分更新顶部电源标志。插电显示插头+USB，拔电显示真实电量；只重绘电源区域，不提前更新正文或已显示温湿度。正在全刷时等当前操作完成。实现及实体验证见[电源快速局刷](power-events.md)。本文下方早期“电量每5秒/随普通刷新”的记录已由此更新；当前省电和非周期全刷策略以[省电策略](energy-policy.md)为准。

本项目已移植基础 `01` 示例的屏幕命令序列到 `3Inch7EPaper/components/epaper_vendor/`，在 `3Inch7EPaper/main/board.c` 单独管理 PMU 电源。与下文完整 `08` 示例不同，本地驱动的 `EPD_Init()`、`EPD_Sleep()` 不直接操作 PMU；需要断电时由调用方执行 `board_display_power(false)`。本地 BUSY 等待设有 25 秒超时，SPI 传输错误可通过 `epaper_last_operation_ok()` 查询。

仪表盘使用自有 `3Inch7EPaper/main/canvas.c`，不是 `GUI_Paint`。底层缓冲固定原生 800 × 480，黑白 1 bit，最高位在左，`0` 为黑、`1` 为白。两个缓冲各 48,000 字节，优先分配到 PSRAM：当前画面和已成功发送的画面。`canvas_t.rotation` 支持 0/90/180/270，`canvas_width/height()` 返回对应逻辑尺寸；横屏为 800 × 480、竖屏为 480 × 800。只在写像素时映射到原生缓冲，SPI 全刷大小和顺序不变。

`3Inch7EPaper/main/dashboard.c` 为横竖屏分别排版总览、服务详情、温湿度趋势、网络设置和按键说明。板端有8个服务槽位，未启用的服务页自动跳过；仅Codex有数据时共5页。竖屏采用独立排版。IMU稳定方向变化、换页和手动旋转触发全刷，最小交互间隔约2秒；显示任务阻塞期间，独立输入任务继续处理BOOT、PMU按键、IMU和SHTC3，使用队列交付事件及最新采样。

2026-10-21新差分路径已通过用户实屏确认并启用，重启后保留。全刷使用`EPD_Init()`+`EPD_Display_Base()`；同页温湿度、曲线、额度、时钟变化使用`EPD_Display_Differential(previous,current)`，不复位控制器，按全帧地址顺序维护旧/新双RAM。20次差分、30分钟、页面/方向/卡片结构变化、手动BOOT双击及错误恢复仍全刷；无画面变化不发送。通常60秒更新，dirty变化可在30秒后合并显示；失败至少等30秒重试。旧打包矩形局刷仍不使用。运行中屏幕保持供电，未做运行功耗优化；PWR正常关机先显示关机页再断屏幕电源，USB仍供CPU时使用无定时唤醒Deep-sleep兜底，见关机说明。测试、SD日志、局刷开关和待测边界见 [硬件升级记录](hardware-upgrade.md)，使用与编译见 [README](../../README.md)。

当前总览按活跃度与额度紧迫程度选最多3个服务，有一家时扩展卡片；“5小时”优先96px，周额度较小，按卡片宽度回退以容纳100%。Token汇总卡片采用“今日 / Token”两行标签和右侧大数字，只加同日已取得的有效统计；Argus卡片以今日/周Token两行大数字展示，字号层级与Codex5小时/周额度相同。温湿度标签28px并带图标，页脚主体20px。右上角用蓝牙符号/斜杠表示连接/断开，时间和电量随正常刷新更新；电量每5秒采样，屏幕不是秒级刷新。无电池时显示USB供电，未知不伪造百分比。当前排版与各版本视觉确认见 [显示记录](display-redesign.md)。

Argus详情页从0.2.15起使用独立信息布局：今日/本周大数、四项调用指标和模型/供应商排行；分类、缓存与时钟同页变化仍走原差分通道，不恢复旧矩形局刷。详见display-redesign.md与argus-detail-0215验证记录。

时间与日期分开放置：日期已按用户最新要求移回顶部。横屏放在标题和蓝牙符号之间的独立位置（20px），竖屏放在左侧标题下方（16px）；时间仍在右侧，页脚不再显示日期。

## 分辨率与内存

| 模式 | 像素编码 | 整屏缓冲 |
| --- | --- | --- |
| 黑白 | 1 bit，每字节 8 像素 | `800 / 8 * 480 = 48000` 字节 |
| 四级灰度 | 2 bit，每字节 4 像素 | `800 / 4 * 480 = 96000` 字节 |

对应宏为 `EPD_SIZE_MONO`、`EPD_SIZE_4GRAY`。尺寸使用 `size_t` 或至少 32 位整数保存；96,000 超过 `uint16_t` 的范围。可以像完整应用一样用 `heap_caps_malloc(..., MALLOC_CAP_SPIRAM)` 分配，分配失败时停止绘制。

`Paint_NewImage()` 默认重设为黑白模式，使用灰度缓冲时，应在它之后调用 `Paint_SetScale(4)`，再清空、绘制。

## 横竖屏坐标

原生坐标为 800 × 480。创建绘图上下文时选择旋转方向：

```c
// 横屏逻辑坐标 800 × 480
Paint_NewImage(frame, EPD_WIDTH, EPD_HEIGHT, ROTATE_0, WHITE);

// 竖屏逻辑坐标 480 × 800，官方完整菜单采用这个方向
Paint_NewImage(frame, EPD_WIDTH, EPD_HEIGHT, ROTATE_270, WHITE);
```

有效像素坐标范围是 `0 <= x < width`、`0 <= y < height`。`Paint_SetRotate()` 在本版实现中只改旋转值，不同步交换逻辑宽高；切换方向时重新调用 `Paint_NewImage()`。

`EPD_Display_Partial()` 的坐标仍是原生屏幕区域，不能直接拿竖屏 UI 坐标调用。初次实现局刷优先使用 `ROTATE_0`，确认区域和方向后再增加旋转转换。

## 初始化与首屏

以下片段放在已经导入 `08` 示例组件、启用 PSRAM 的 IDF 工程中，相关头文件包括 `stdbool.h`、`esp_err.h`、`esp_heap_caps.h`、`i2c_bsp.h`、`axp_prot.h` 和 `epaper_port.h`。代码应在一个返回类型为 `void` 的初始化函数内执行。

```c
// 应用中只初始化一次；已有这些初始化时不要重复创建总线。
i2c_master_init();
ESP_ERROR_CHECK(i2c_devices_init());
axp_init();
epaper_port_init();

UBYTE *frame = heap_caps_malloc(EPD_SIZE_MONO, MALLOC_CAP_SPIRAM);
if (frame == NULL) {
    return;
}

EPD_Init();
Paint_NewImage(frame, EPD_WIDTH, EPD_HEIGHT, ROTATE_0, WHITE);
Paint_SelectImage(frame);
Paint_SetScale(2);
Paint_Clear(WHITE);
Paint_DrawRectangle(16, 16, 783, 463,
                    BLACK, DOT_PIXEL_1X1, DRAW_FILL_EMPTY);
Paint_DrawString_EN(32, 32, "Hello ePaper", &Font16, BLACK, WHITE);
EPD_Display_Base(frame);
```

`EPD_Init()` 会通过 PMU 开启 `ALDO3`，因此完整应用驱动需要先初始化 I²C 和 PMU。基础 `01` 示例与 `08` 中的同名文件并不完全相同，迁移时整套采用同一版组件。

`EPD_Display_Base()` 同时写入控制器的两组图像 RAM，适合作为后续局刷的底图。`EPD_Clear()` 会写白屏并触发全刷，可用于需要清屏的场景；无需每次更新界面都先清屏。

## 刷新接口

| 用途 | 初始化与显示 | 说明 |
| --- | --- | --- |
| 普通全刷 | `EPD_Init()` + `EPD_Display()` | 整屏显示黑白图像 |
| 全刷并建立底图 | `EPD_Init()` + `EPD_Display_Base()` | 后续局刷前优先建立底图 |
| 快刷 | `EPD_Init_Fast()` + `EPD_Display_Fast()` | 使用驱动中的快刷设置 |
| 快刷并建立底图 | `EPD_Init_Fast()` + `EPD_Display_Fast_Base()` | 同时更新两组 RAM |
| 区域局刷 | `EPD_Display_Partial(buffer, x0, y0, x1, y1)` | 屏幕应已供电并建立底图 |
| 四级灰度 | `EPD_Init_4GRAY()` + `EPD_Display_4Gray()` | 使用 96,000 字节缓冲 |
| 休眠 | `EPD_Sleep()` | 此版实现还会关闭屏幕供电通道 |

`EPD_Init_Partial()` 在本版完整应用的头文件中有声明，但对应 `.c` 未找到实现；不要仅根据头文件直接调用。原例程是在建立底图后直接调用 `EPD_Display_Partial()`。

驱动内部等待 BUSY，实际刷新耗时、残影与刷新策略需上板测量。本记录没有实测刷新速度。对于菜单选中框或时间数字可以优先局刷，页面切换或残影明显时使用全刷；全刷间隔根据实屏效果确定。

## 局刷缓冲必须对应目标区域

此版 `EPD_Display_Partial()` 从传入指针的第 0 字节起连续读取区域数据，**不会从整屏缓冲自动裁剪目标矩形**。若直接传整屏缓冲且目标区域不是整个屏幕，就会取错图像内容。

使用独立的、逐行紧凑排列的局部缓冲。`x0` 和 `x1` 都取 8 的倍数，保证横向按字节对齐；`x1`、`y1` 表示区域结束边界，不包含该边界。

下面在首屏 `EPD_Display_Base(frame)` 之后、屏幕保持唤醒时执行，将 160 × 32 区域放到原生坐标 `(152, 300)`：

```c
enum { PATCH_WIDTH = 160, PATCH_HEIGHT = 32 };
UBYTE patch[PATCH_WIDTH / 8 * PATCH_HEIGHT]; // 640 字节

Paint_NewImage(patch, PATCH_WIDTH, PATCH_HEIGHT, ROTATE_0, WHITE);
Paint_SelectImage(patch);
Paint_SetScale(2);
Paint_Clear(WHITE);
Paint_DrawString_EN(0, 0, "12:34:56", &Font16, BLACK, WHITE);

EPD_Display_Partial(patch, 152, 300,
                    152 + PATCH_WIDTH, 300 + PATCH_HEIGHT);
```

局部缓冲中的绘图坐标从 `(0, 0)` 开始。后续还要全刷时，应用保存的整屏图像也需要更新对应区域，否则下次全刷会恢复旧内容。

`GUI_Paint` 使用全局 `Paint` 状态，切换图像后会改变宽高、旋转和步长。恢复整屏绘图时重新建立整屏上下文；多个任务不能无锁地同时切换缓冲、绘图和刷屏，优先交给一个显示任务统一处理。

## 四级灰度

完整灰度图必须分配 `EPD_SIZE_4GRAY`，再执行：

```c
EPD_Init_4GRAY();
Paint_NewImage(gray_frame, EPD_WIDTH, EPD_HEIGHT, ROTATE_0, WHITE);
Paint_SelectImage(gray_frame);
Paint_SetScale(4);
// 在此绘制 2 bit 灰度图像，或载入官方四级灰度 BMP。
EPD_Display_4Gray(gray_frame);
```

`GRAY1`、`GRAY2`、`GRAY3`、`GRAY4` 的宏值依次为 `3`、`2`、`1`、`0`。头文件的深浅注释与驱动中的部分转换注释不一致，先用官方灰度图片或四块灰阶色表确认实际映射，再确定图像转换规则。

基础 IDF `01_E-Paper_Example/main/main.c` 只分配了 `EPD_SIZE_MONO`，随后却启用整屏四级灰度，按绘图实现会访问超过该分配范围。复用该流程时必须扩大到 96,000 字节或重新分配。Arduino 的 `05_SD_Test` 展示了释放黑白缓冲后重新分配灰度缓冲的流程。

从灰度切回黑白时重新执行黑白初始化，设置 `Paint_SetScale(2)` 并重建底图；不要沿用错误的步长或假定仍有可用的黑白局刷底图。

## 字体与图片

- 英文、数字使用 `Paint_DrawString_EN()`、`Paint_DrawNum()`，中文使用 `Paint_DrawString_CN()` 和匹配编码的字库，如 `Font16_UTF8`。
- 字体函数的最后两个颜色参数分别是前景与背景。黑白模式下 `BLACK` 为 `0x00`，`WHITE` 为 `0xFF`；位图按每字节最高位对应靠左像素。
- `Paint_DrawBitMap()` 复制整幅原始位图；`Paint_ReadBmp()` 绘制的是原始点阵数组，名字中的 Bmp 不代表传入带文件头的 BMP 文件。
- SD 卡上的文件用 `GUI_ReadBmp()`、`GUI_ReadBmp_4Gray()` 读取，例如 `/sdcard/bmp/3in97.bmp`。输入格式应符合对应解析器的位深和编码；任意 PNG、JPEG 或 BMP 不会自动兼容。
- 完整应用在 `menuconfig` 中提供字库和图片来源选项，支持固件内嵌与 SD 卡。完整中文字库较大，选择字号时同步检查 Flash 分区和固件体积；具体构建行为还要核对 `epaper_lib/CMakeLists.txt`。

## 休眠与恢复

显示结束且不再立即更新时，可以执行：

```c
EPD_Sleep();
heap_caps_free(frame); // 仅在应用后续不再使用该缓冲时释放
```

墨水屏休眠不等于 ESP32 进入休眠。完整应用的 `EPD_Sleep()` 会发送屏幕休眠命令、拉低部分控制脚并关闭 `ALDO3`；下一次显示前执行 `EPD_Init()`，重新绘制并用 `EPD_Display_Base()` 建立底图。跨屏幕断电后不依赖控制器仍保存旧局刷状态。

完整应用另有 RTC 闹钟配合 PMU 关机的低功耗流程，见 [硬件记录](hardware.md)。屏幕保持画面和处理器、Wi-Fi、音频、传感器的功耗应分别管理。

## 排查入口

| 现象 | 优先检查 |
| --- | --- |
| BUSY 一直等待 | GPIO3、电源初始化、RST、板卡版本；本版等待循环没有超时 |
| 局刷内容位置错误 | 是否传入独立区域缓冲，是否混用横竖屏坐标，X 边界是否按 8 对齐 |
| 灰度显示异常或内存损坏 | 96,000 字节容量、32 位尺寸变量、`Paint_SetScale(4)` 调用顺序 |
| 旋转后边界异常 | 使用 `Paint_NewImage()` 重建宽高，避免仅调用 `Paint_SetRotate()` |
| 休眠后刷新异常 | 重新上电初始化并建立全刷底图 |
| 中文缺字或字形异常 | 字体编码、字库是否内嵌、SD 挂载、字库文件与字号 |

源码入口：[屏幕驱动][driver]、[GUI_Paint 实现][paint]、[GUI_Paint 声明][paint-h]、[基础 IDF 示例][basic]。

[driver]: https://github.com/waveshareteam/ESP32-S3-ePaper-3.97/blob/9b12d40731a80213b927ee8a421cae4082952819/ESP-IDF/08_ESP32-S3_e-Paper-3.97/components/epaper_port/epaper_port.c
[paint]: https://github.com/waveshareteam/ESP32-S3-ePaper-3.97/blob/9b12d40731a80213b927ee8a421cae4082952819/ESP-IDF/08_ESP32-S3_e-Paper-3.97/components/epaper_lib/GUI_Paint.c
[paint-h]: https://github.com/waveshareteam/ESP32-S3-ePaper-3.97/blob/9b12d40731a80213b927ee8a421cae4082952819/ESP-IDF/08_ESP32-S3_e-Paper-3.97/components/epaper_lib/GUI_Paint.h
[basic]: https://github.com/waveshareteam/ESP32-S3-ePaper-3.97/blob/9b12d40731a80213b927ee8a421cae4082952819/ESP-IDF/01_E-Paper_Example/main/main.c


0.2.17手动采样：App“采样并更新”会让板载SHTC3新测一次，并立即评估当前页差分（仍保留2秒交互节流及周期清残影）。像素未变化则跳过，回传“画面无变化”；成功、无变化、传感器失败和显示失败均有事务回执。仅status读取不会自动触发刷屏。新路径已在4次真实采样中验证更新，串口捕获667/685ms差分；用户实屏操作确认另见最新记录。


2026-10-21用户已亲手验证HomeCraft“采样并更新”，反馈“局刷和完成提示正常”；此次手动采样、实屏局刷及完成反馈确认通过。

0.2.18新增 DeepSeek Harness profile：主页大字今日/周 Token 为各 Mac 最近已同步快照之和，详情页逐台列出今日/本周。全称和鲸鱼图标，名称保持粗体；设备行数变化计入布局键，数值变化继续走已验证双 RAM 差分。HomeCraft 承担记录读取、本机缓存、交互图表及配置；显示端不持有 Harness 账户凭据。验证状态见 3Inch7EPaper/artifacts/homecraft-0218-validation.json，不能把测试双机样例当作两台实体 Mac 实测。

0.2.18最终排版偏好：标题栏所有服务名维持同一字号（横屏32px、竖屏24px），DeepSeek Harness 在标题栏完整同号，**不缩小 Harness**。内容卡片才采用大字 DeepSeek、小字 Harness；图标按位图实际墨迹与字形实际墨迹居中，不按带留白的整张画布估算。顶部完整日期 YYYY-MM-DD 为24px，农历20px；窄屏或长标题占用日期区域时隐藏农历，保持标题字号，时间与日期各自独立位置。农历表由 Apple Foundation 中国历生成，支持2024春节–2100年，1049个香港天文台2025/2026/2033日期（含闰月）交叉验证通过，超界显示未知，不请求网络。DeepSeek Harness下方设备行按数量使用更大粗体名称、数字与行距，1–8台四方向渲染通过。


## 0.2.19 持续差分与低功耗

按用户新要求取消20次/30分钟定期全刷，连接状态页也不再每次强制全刷。只有开机、换页/旋转、手动清残影及失去控制器基准或I/O故障恢复使用全刷。主页卡片重排、来源数量变化也使用双RAM差分。温湿度30秒采样、IMU1秒，按钮映射不变；策略覆盖前文周期清残影要求。


## 0.2.23 Codex重置时间

总览5小时额度下方和详情5小时卡片显示官方重置倒计时，保留大数字及额度分段条。分段条仍表示剩余额度；倒计时只用已接收的reset_at和设备时钟，缺失/未校时/已到期分别显示明确状态，不自行补满额度。随现有30–60秒差分评估变化，不增加秒级刷屏或恢复周期全刷。布局、离线检查及实板状态见[显示改版记录](display-redesign.md)和3Inch7EPaper/artifacts/codex-reset-0223-validation.json。


## 0.2.24 详情排版与倒计时偏好

Argus详情改为按实际字形墨迹分行，排行榜为长名称及数值预留各自列，进度条在文字下方，横竖屏均检查无重叠。Codex重置只显示小时钟+HH:MM，删除“小时/分/后重置”等说明；未知为--:--，已到期为00:00，不代表收到新的满额数据。继续双RAM差分，无秒级动画。QMI内部低功耗和重复计算优化见[省电策略](energy-policy.md)，当前部署以3Inch7EPaper/artifacts/argus-energy-0224-validation.json为准。


## 0.2.26 总览底部

用户同时要求Mac和墨水屏：底部左侧今日／本周Token已知合计，右侧一张温湿度卡。横屏Token并列、竖屏上下排列，日数大于周数，小单位基线对齐；同周有效wt才纳入周合计，缺失不补0。原服务卡、日期顶部和数字倒计时保留，普通日周／温湿度变化继续差分。0.2.26已app-only烧录并哈希验证，原绑定连接、SD续写与671ms差分通过；用户新布局视觉确认待补。详见display-redesign.md与3Inch7EPaper/artifacts/overview-0226-validation.json，前文仅今日汇总／拆分温湿度为历史布局。


0.2.27温湿度字重：按用户澄清，数字和°C／%单位采用真实粗体，中文标签保持原样。总览合卡与温湿度页统一，全部宽度回退保持粗体，°字形已补齐；普通读数变化仍双RAM差分。已app-only烧录、哈希验证，BLE原连接、局刷与SD续写正常；用户实屏观感待确认，见environment-bold-0227-validation.json。


0.2.28对齐规则：温度／湿度数字共用同一粗体字号，空间不足一起回退；单位°C／%独立固定列，数字右缘及小数位对齐，单位与数字共基线。禁止将整串读数各自缩小造成温度比湿度小。总览、温湿度页与兼容显示统一。已app-only部署，原连接、SD续写及671ms差分通过；详见environment-alignment-0228-validation.json。


## 0.2.29 三项Token摘要

总览底部改为今日／本周／本月，去掉三项标签中的Token字样。横屏三列、竖屏三行，今日优先较大，周／月共同选择字号；Argus与Harness详情和设备表同步加入本月。只显示本周期有效日／周／月数，未知为--、真实0保留。温湿度相同粗体字号和对齐保持。普通日周月数值变化继续走差分，不恢复周期全刷。当前部署、字段和验证见display-redesign.md的0.2.29节。


## 0.2.32 今日／本周／本月同字号（2026-10-25，当前固件）

用户明确选择“三项统一字号”，覆盖旧的今日大、周／月小偏好。总览下方、Argus／Harness卡片及详情、Harness设备表三项数字均共用一个真实Arial Bold字号；按卡片空间和固定最长紧凑格式选择一次，0／--／数值长短／KMBTP变化不逐项缩放。单位同号且与数字共基线，数字右对齐。小B值仍可保留两位小数，100B以上用一位，避免100.00B多一位迫使字体变小，舍入仍自动进位单位。

Argus详情三张卡／竖屏三行等宽等高；总览服务名称使用共同粗体字号，Harness后缀与主名同字号，窄卡换行不缩成小字。温湿度同号粗体与原Codex5小时／周额度层级保持。普通数字与时钟继续差分，不恢复周期全刷。

4608组三项稳定字号、3456组旧双项工具、140组摘要实际墨迹边界、220组Argus、64组差分和32组Harness四向／设备数量检查通过ASan/UBSan；22张四向预览是合成值，未发实板。IDF6.1应用1,915,584字节，SHA256 d9f1ba822ecde32e22ce216003e60e958bce756eb53dc6419628e88a1dcb5cf9，3Inch7EPaper/dist/firmware/HomeCraft-0.2.32-uniform-type.bin。已app-only烧录并写入哈希验证，原绑定自动认证、真实Codex／Argus／Harness月数已独立回读，SD985/write_ok=1、partial=1/count=6/ready=1，30秒采样／1秒IMU／40–80MHz保留。当前App与唯一DMG仍0.2.30/build30，已恢复菜单栏真实同步。本次新字体实屏观感尚待用户确认。见3Inch7EPaper/artifacts/typography-0232-validation.json。


0.2.34仅微调总览Codex的5小时：标签与粗体HH:MM成组，右侧数字按几何固定字号并与原周额度数字／%列对齐；普通卡上下排、三卡竖屏同行。周额度／昨日／进度条及其他布局保持，普通更新继续差分。已app-only写入并校验，原绑定同步、SD续写与673ms差分通过；详见display-redesign.md的0.2.34节与codex-primary-0234-validation.json，实屏观感待用户确认。


## 0.2.36 音乐页

温湿度后新增音乐页（协议索引12），横屏列表+状态两栏、竖屏上下排列。文件名采用常用汉字20px字形并按像素省略；状态和时间仍使用原差分通道，页面/方向切换全刷。音乐页BOOT双击改为播放/暂停，手动全刷可用HomeCraft。112组状态/目录数量/四方向实际绘图器边界回归通过，预览为合成数据；实屏确认与支持范围见[SD音乐](music.md)。
