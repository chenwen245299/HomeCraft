# 官方开发教程索引

核对日期：2026-10-26。只研究与编译，不自动执行教程中的上传、Full Flash、配网、云账号绑定或录音步骤。

| 官方入口 | 本次用途 |
| --- | --- |
| [E1002中文入门](https://wiki.seeedstudio.com/cn/getting_started_with_reterminal_e1002/) | 规格、原理图下载、J2、串口和工厂固件操作说明 |
| [Arduino通用准备](https://wiki.seeedstudio.com/epaper_work_with_arduino/) | 板包、OPI PSRAM及图形驱动工作流 |
| [E系列显示](https://wiki.seeedstudio.com/reterminal_e10xx_with_arduino/) | E1002显示示例、六色与SPI配置 |
| [板载外设](https://wiki.seeedstudio.com/reterminal_e10xx_with_arduino_peripherals/) | 按钮、LED、SHT4x、SD、电池与蜂鸣器 |
| [RTC/低功耗/PDM](https://wiki.seeedstudio.com/reterminal_e10xx_with_arduino_peripherals_2/) | PCF8563、PDM与掉电时序的开发入口 |
| [官方示例集合](https://github.com/Seeed-Projects/OSHW-reTerminal-Series-E-D) | 固定提交，参考CI的Arduino core3.3.7 |
| [Seeed_GxEPD2](https://github.com/Seeed-Projects/Seeed_GxEPD2) | 本环境采用的具体E1002驱动和例程 |

官方Seeed_GFX也是可用路线，教程使用BOARD_SCREEN_COMBO521；本次选择官方维护的Seeed_GxEPD2路线完成屏幕编译，避免同时装TFT_eSPI/Seeed_GFX引起冲突。两者的开发入口已记录，后续按功能决定框架，不把目前的编译示例当作应用方案。

官方“GUI开发/ESPHome/Home Assistant/Seeedash”等路线属于后续功能选择，目前不安装服务、不绑定账号、不向本板复制旧设备凭据。原生ESP-IDF骨架用于验证32MB/OPI/UART工具链，不包含现成显示驱动移植或HomeCraft连接。

完整源码提交、PDF哈希与资料冲突见 [来源锁](../reference/sources.lock.json) 和 [硬件核对](hardware.md)。
