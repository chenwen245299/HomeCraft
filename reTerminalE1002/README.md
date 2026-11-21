# reTerminalE1002

2026-11-18 当前源码另有 **0.3.1 直显候选**：用量全部由 Mac App 处理，本板只替换显示最终值，取消本地来源缓存/合计回退。尚未部署，见 [本板待办](TASKS.md)；下文为既有实机记录。

**当前检查点：[E1002-AI-MULTI-03](docs/CHECKPOINT.md)。** 0.3.0固件已只写app0烧录并与HomeCraft配对同步，主页按用户反馈重设计；局部刷新会扰动窗口外画面，已停用并改为一律全刷（实屏已确认干净）；合计由HomeCraft下发（`totals:1`）、充电图标读SY6974B，均已实屏确认。以下资料准备说明保留作基线，当前进度与待办以检查点为准。

- [0.3.0固件构建记录](artifacts/firmware-0.3.0-build.json) / [第三方来源](THIRD_PARTY.md)
- 备份与检查：`tools/backup_factory_flash.py`（只读分段校验）、`tools/inspect_factory_backup.py`（离线读取启动槽）

Seeed Studio reTerminal E1002独立硬件目录。开发环境已准备，2026-10-26已完成首次USB只读连通检查，应用功能以后确定。没有烧录、配网、录音、云绑定或接入HomeCraft。

- [硬件/原理图/完整管脚核对](docs/hardware.md)
- [本板环境与编译方法](docs/development-environment.md)
- [官方开发教程](docs/tutorials.md)
- [本板规则](AGENTS.md) / [当前清单](TASKS.md) / [第三方来源](THIRD_PARTY.md)
- [准备与验证记录](artifacts/environment-validation.json)
- [首次USB实机检查](artifacts/usb-first-contact-20261026.json)

## 本板资料

官方标称ESP32-S3、32MB Flash、8MB PSRAM，7.3英寸800×480 Spectra6六色屏。首次USB检查已确认ESP32-S3 v0.2、32MB Flash和芯片内8MB PSRAM标识；PSRAM运行时测试、实物PCB修订和外设功能仍待确认。管脚不能套用3Inch7EPaper，GPIO3/4/5是本板按钮，SD用SPI而非旧板SDMMC。

board.json描述本板，config/pins.h及pins.json保留引脚；reference保存官方原理图、芯片资料、固定提交的示例。下载/缓存/工具均在本目录，HomeCraft当前开发目标与旧板数据不变。

## 环境入口

在本目录：

```bash
python3 tools/build_arduino.py environment
python3 tools/build_arduino.py display
source /Users/qichengwen/.espressif/tools/activate_idf_v6.1.sh
idf.py -B build/idf build
```

两条路线只验证编译。原生最小工程不启用外设或无线；Arduino显示等目标为官方参考例程，不是本项目功能设计。最终分区、通信协议、安全绑定与实际部署需待需求和实机条件确认。

USB桥VID:PID为1a86:7523，现有系统串口驱动可用。原厂分区表已只读保存并通过MD5和布局校验，app0位于0x90000，不能直接套用环境骨架的分区或旧板0x10000升级地址。本次未做完整Flash备份，未改写固件或NVS；启动观察只确认ROM引导，不代表原厂应用和外设已完成测试。
