# reTerminal E1002 硬件与管脚核对

核对日期：2026-10-26。以下针对Seeed reTerminal E1002，不适用于3Inch7EPaper或E1001/E1003/E1004。记录分为官方描述、图纸/源码核对、实机确认三层。环境准备后用户插入USB，已完成ROM和存储识别；没有烧录，实物PCB修订和所有外设均待核实。

## 资料基线

- [官方中文入门](https://wiki.seeedstudio.com/cn/getting_started_with_reterminal_e1002/)。
- [官方V1.2原理图](https://files.seeedstudio.com/wiki/reterminal_e10xx/res/202004321_reTerminal_E1002_V1_2_SCH_251120.pdf)，设计日期2025-11-20，共9页；本地原件在reference，来源/哈希见 [锁定记录](../reference/sources.lock.json)。
- [官方示例集合](https://github.com/Seeed-Projects/OSHW-reTerminal-Series-E-D) 与 [Seeed_GxEPD2](https://github.com/Seeed-Projects/Seeed_GxEPD2) 已固定提交。管脚可对照其E1002显示、Battery_Monitor、MicroSD、SHT4x和RTC示例。
- [ESP32-S3数据手册](https://files.seeedstudio.com/wiki/SeeedStudio-XIAO-ESP32S3/res/esp32-s3_datasheet.pdf)用于核对ADC和启动绑带；不能把PCB网络名称直接当作芯片外设通道。

原理图页码：4电源、5主控/Flash、6串口/按钮/J2、7传感器/RTC/PDM/SD/蜂鸣器、8主屏FPC、9预留FPC。已渲染并目视核对这些页面；第1–3页为封面、版本记录和空白框图。

## 资源与芯片

官方标称：ESP32-S3、8MB PSRAM、32MB Flash、7.3英寸800×480 E Ink Spectra6、2000mAh电池、2.4GHz Wi-Fi/BLE。这些是资料规格，不是本机读回或续航结果。

V1.2图纸标U6 ESP32-S3R8、U5 W25Q256JVEIQ，SPI Flash和芯片内OPI PSRAM分别配置；不能把PSRAM设为QSPI或把Flash设成OPI。图中被划掉的旧Flash选件、J3触摸座、24P FPC等不视为已装部件。

SHT40-AD1B-R2与PCF8563T位于同一I²C总线。PDM数字麦克风与蜂鸣器各自独立；它没有旧板的ES8311/喇叭输出路径，不能直接搬音乐播放器。

## GPIO表

数字均为ESP32 GPIO，非连接器针序。`config/pins.h` 和 `config/pins.json` 保留本板配置，目前均未实机测试。

| 接口 | 信号 | GPIO | 核对依据 |
| --- | --- | --- | --- |
| 屏幕/SD共享SPI | SCK / MOSI / MISO | 7 / 9 / 8 | 图纸5、7、8；官方示例。屏幕不需要MISO |
| 屏幕 | CS / DC / RESET / BUSY | 10 / 11 / 12 / 13 | 图纸8、E1002显示示例 |
| SD | CS / DET / EN | 14 / 15 / 16 | 图纸7、MicroSD示例；DET低表示插卡，EN高使能 |
| 传感器/RTC I²C | SDA / SCL | 19 / 20 | 图纸7，SHT40地址0x44、PCF8563地址0x51 |
| V1.2充电管理I²C | SDA / SCL | 39 / 40 | 图纸4；SY6974B 0x6B（Seeed SenseCraft_HMI）。2026-10-27实机读REG08有应答（充电中）；HomeCraft固件只读REG08，不写任何寄存器 |
| USB转串口 | ESP TX / RX | 43 / 44 | 图纸5、6，CH340C连UART0；115200为示例基准 |
| 用户按钮 | KEY0 / KEY1 / KEY2 | 3 / 4 / 5 | 图纸6，10K上拉、按下接地；物理位置为KEY0右侧绿键、KEY1中键、KEY2左键（Seeed官方示例；2026-10-26实机按左键触发GPIO5）。HomeCraft固件：长按左键配对、中键切页、绿键单击全屏刷新、双击切换隐私画面 |
| 用户绿LED | LED | 6 | 图纸6、官方LED示例，低有效 |
| 蜂鸣器 | BUZZER_EN | 45 | 图纸7，经晶体管驱动 |
| 电池测量 | ADC / EN | 1 / 21 | 图纸4，ADC1_CH0，1:1分压，EN高使能 |
| PDM麦克风 | DATA / CLK / EN | 41 / 42 / 38 | 图纸7、官方PDM示例 |
| 扩展UART | TX / RX | 17 / 18 | J2；与USB控制台43/44分开 |

电池分压的恢复系数为2，但ADC校准、采样建立时间和电量百分比模型尚未实测。官方示例有5ms等待，入门建议10ms；以后实现时留充分建立时间，不用线性电压百分比冒充已校准SOC。

### J2扩展排针（官方图示从上到下）

| 针序 | 信号 |
| --- | --- |
| 1 | HEADER_3V3 |
| 2 | GND |
| 3 | GPIO46 |
| 4 | GPIO2（芯片ADC1_CH1） |
| 5 | GPIO17 / TX |
| 6 | GPIO18 / RX |
| 7 | GPIO20 / I²C SCL |
| 8 | GPIO19 / I²C SDA |

3.3V电源不是5V供电脚。GPIO3/45/46涉及启动采样，外接模块不能随意强拉启动电平。GPIO33–37属于R8的存储接口约束，不能把原理图未引出的脚当作自由扩展资源。

## 显示与总线约束

- 六种目标颜色为黑、白、红、绿、蓝、黄；Seeed示例明确不使用橙色。GxEPD2_7C类名和README中的“7-color”不能当成实际有第七种颜色。
- 官方示例选择GDEP073E01，2MHz、SPI mode0；示例注释称ED2208，但驱动头仍写controller unknown，因此不把控制器型号当成已独立核实结论。
- 示例给出的全刷约25–30秒，驱动头有15秒参考值；它们是软件参考，不是本项目测量。驱动宣告partial支持但无fast partial，不能继承旧屏约0.7秒的双RAM差分策略。2026-10-27实测：局刷与全刷耗时相同（约27 s），且会让窗口外像素偏青、黑字褪淡，本项目已停用局刷。
- 屏与SD共用7/9，分别片选；后续实现必须规划共享总线、事务/频率切换与互斥，不能认为不同SPIClass对象就能安全并发驱动相同引脚。
- 图纸8中GPIO12的复位网也参与50P屏电源开关控制，需按面板时序处理。J3触摸和24P屏连接在V1.2图中标为未装，不把预留GPIO39/40/47/48当作现成触摸功能。

## 电源与版本差异

V1.2将充电芯片ETA6003改为SY6974B，并修改显示升压器件。电源页还包含TPS563100和外设负载开关；不能沿用旧板AXP2101/TG28寄存器或供电逻辑。当前不调整充电参数，不根据参考图纸推断实物已是V1.2。

USB-C经CH340C到UART，GPIO19/20用于传感器总线。因此环境默认UART控制台、关闭USB CDC；不把ESP32原生USB引脚作为本板USB连接方式。是否需要WCH驱动应连接实物后检查，当前未安装系统驱动扩展。

## 已知资料冲突

1. Wiki和图纸把GPIO2网络写作ADC1_CH4；ESP32-S3数据手册及SDK adc_channel.h确认GPIO2为ADC1_CH1。按GPIO与芯片通道处理。
2. 入门J2表给GPIO46标“GPIO/ADC”，但芯片没有该脚ADC通道；本配置只记GPIO和启动绑带属性。
3. 单品入门写SD≤32GB，系列外设教程写≤64GB。准备阶段保守使用FAT32≤32GB；64GB兼容性待测，不格式化用户现有卡。
4. 通用教程推荐XIAO_ESP32S3_PLUS，示例CI使用XIAO_ESP32S3的8MB分区。图纸/单品规格明确32MB，本环境用generic ESP32S3 Dev Module并显式配置32MB、OPI PSRAM及全部GPIO，避免默认XIAO容量/管脚混淆。
5. 官方示例/驱动的颜色与控制器描述并不完全一致，采用固定提交的E1002具体示例及驱动类，实际面板修订后续再核对。

上述冲突不靠实机“猜测验证”，落板前先确认PCB、原固件分区/备份和安全烧录范围。厂商三个月续航属于特定使用条件，不作为本项目承诺。

## 首次USB只读实测（2026-10-26）

- 本机串口为`/dev/cu.usbserial-1420`，USB VID:PID为`1a86:7523`，与图纸的WCH串口桥系列相符；现有驱动可用，未安装额外系统驱动。端口名可能随插口变化，不作为设备唯一身份。
- 使用ESP-IDF环境的esptool5.6.0、115200、`--no-stub`进入ROM并读取，无RAM stub上传、无Flash写入。确认ESP32-S3 QFN56修订v0.2、40MHz晶振；Flash厂商ID ef、设备ID4019、容量32MB、quad3.3V。芯片识别报告内置8MB PSRAM，但尚无运行时初始化和压力测试。
- 读取0x8000的4KiB原厂分区表，MD5、无重叠及32MB边界检查通过。私有原件在backups，目录700、文件600；没有读取NVS内容或导出配对秘密。此文件只是分区表，不是完整恢复备份。

| 原厂分区 | 偏移 | 长度 |
| --- | --- | --- |
| nvs | 0x9000 | 0x7D000 |
| otadata | 0x86000 | 0x2000 |
| phy_init | 0x88000 | 0x1000 |
| app0 / ota_0 | 0x90000 | 0xC00000（12MiB） |
| app1 / ota_1 | 0xC90000 | 0xC00000（12MiB） |
| spiffs | 0x1890000 | 0x600000（6MiB） |

ROM重启引导可见，14秒串口观察未匹配到panic/brownout签名；仅保存白名单过滤后的引导行，没有保存原始串口日志。当前证据不能证明原厂应用、PSRAM运行时、屏幕或传感器功能正常。结构化记录见[首次连接验证](../artifacts/usb-first-contact-20261026.json)。
