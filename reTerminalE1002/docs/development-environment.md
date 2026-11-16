# reTerminal E1002 独立开发环境

准备日期：2026-10-26。配置和离线编译已通过，随后完成首次USB只读识别；尚未上传或验证外设功能。应用功能、最终分区与HomeCraft适配均留到后续确定。

## 已有工具与独立安装

- 本机ESP-IDF v6.1、EIM、Python3.12.15已重新核对；共享工具安装不代表共享板级配置。
- 本板Arduino CLI1.5.1使用官方macOS ARM64发行包，并核对官方SHA256。
- ESP32 Arduino core固定3.3.7，与本次固定的Seeed示例CI一致；其自带SDK与原生IDF6.1分开。
- CLI位于toolchains，板包/缓存/用户库均在本板.arduino。config/arduino-cli.template.json保存相对路径，tools/arduino.py每次生成本板绝对路径配置，未写全局Arduino目录。
- Seeed_GxEPD2与官方示例固定提交，来源见reference/sources.lock.json；GFX与SHT4x等依赖的实际版本见config/environment.lock.json。

## 本板配置

原生IDF：ESP32-S3、32MB Flash、OPI PSRAM80MHz、UART0控制台。SDK的启动Flash参数显示dio而应用QIO选项开启是其实际生成配置，不能仅凭烧录提示认为配置失效。默认single-app分区只是环境检查占位。

Arduino：generic ESP32S3 Dev Module、FlashSize32M、FlashMode qio80、PSRAM opi、USB CDC关闭；控制台走外置CH340C。FQBN在config/arduino-profile.json，采用板包自带32MB分区供编译验证，不是最终产品分区设计。

这两种构建分区与首次读回的原厂布局不同，本阶段不烧录；原厂app0从0x90000开始，不能照搬3Inch7EPaper的0x10000升级范围，也不能直接上传本目录占位构建。后续先完成原厂完整备份并确定保留/迁移方案，再确定应用布局。

## Apple芯片旧工具兼容

Arduino内置ctags5.8-arduino11的macOS包为x86_64，本机不能直接执行。已从Arduino官方同版源码构建arm64，并仅覆盖本板.arduino内该工具；保留原二进制。

其保留名__unused__与新版macOS头文件冲突，采用config/patches/arduino-ctags-macos.patch仅重命名私有宏，不改解析逻辑。来源提交、补丁和二进制哈希记录在环境锁；未安装Rosetta，也未改系统ctags。重装板包覆盖工具后运行 `python3 tools/repair_ctags.py` 恢复。

## 使用（在本目录）

```bash
python3 tools/arduino.py version
python3 tools/arduino.py core list
python3 tools/build_arduino.py environment
python3 tools/build_arduino.py display
python3 tools/build_arduino.py sensor
python3 tools/build_arduino.py battery
python3 tools/build_arduino.py rtc
```

这些build脚本只compile，不upload。display/sensor/battery/rtc编译固定官方示例；environment只打印环境信息，不启用屏幕、SD、麦克风、传感器、充电管理或无线。

原生IDF：

```bash
source /Users/qichengwen/.espressif/tools/activate_idf_v6.1.sh
idf.py -B build/idf build
```

构建分别在build/arduino和build/idf；所有配置在本板目录。当前目录整理时HomeCraft选择仍指向原板，本轮没有修改旧板配置、安装App或绑定。

## 首次USB检查与待测

准备期间没有本板串口；用户随后插入USB，`/dev/cu.usbserial-1420`、VID:PID1a86:7523已通过现有驱动连接。ROM识别ESP32-S3 v0.2、32MB Flash和内置8MB PSRAM标识，未运行PSRAM内存测试。原厂分区表4KiB已私有保存并通过MD5、布局和容量边界校验，未读取NVS内容。见[首连记录](../artifacts/usb-first-contact-20261026.json)与[硬件说明](hardware.md)。

本次使用esptool5.6.0、115200、`--no-stub`只读命令，结束后经RTS复位回原固件；没有写Flash、擦除或上传RAM stub。未安装WCH驱动、未接入HomeCraft。PCB修订、PSRAM运行时和外设功能仍待验证；首次改变固件前还需完整私有备份。Seeedash配网、云绑定、工厂固件升级并非环境连通测试的必要操作。

初次平铺的IDF构建仅作环境调通记录，已移入本板.cache/initial-idf-build；当前以build/idf和build/arduino为准。
