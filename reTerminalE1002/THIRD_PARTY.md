# reTerminal E1002 第三方来源

- Seeed官方V1.2原理图标注CC BY-SA4.0；原始PDF未修改，来源和哈希见reference/sources.lock.json，提取文本仅供本地检索。
- Seeed-Projects/OSHW-reTerminal-Series-E-D示例集合为MIT，仓库原许可保留在reference副本。
- Seeed-Projects/Seeed_GxEPD2为基于ZinggJM/GxEPD2的官方维护分支，遵守其GPLv3声明；构建时使用固定提交，未将它的代码当成自有驱动。
- Arduino CLI1.5.1遵守GPLv3，原许可在toolchains；ESP32 Arduino core3.3.7及依赖按其原许可，工具和包保持本板隔离。
- Adafruit GFX/BusIO、Sensirion I2C SHT4x/Core等原许可随安装库保留，实际版本在环境锁。
- Arduino ctags5.8-arduino11为GPLv2；本机arm64构建仅对保留宏名做macOS兼容补丁，源码、补丁和原许可均保留。
- ESP-IDF v6.1由现有EIM安装提供，遵守其组件各自许可；本项目仅新增环境检查骨架，不复制另一块板的代码/配置。

## HomeCraft E1002 固件（0.3.0，未部署）

- `components/e1002_display/`：把固定提交 `1100ea37c16b910fd79152f4250c13d802b9c20b` 的 Seeed_GxEPD2 `GxEPD2_730c_GDEP073E01`（作者 Jean-Marc Zingg）时序移植到 ESP-IDF，按 GPL-3.0-or-later 保留原许可（同目录 LICENSE）。厂商波形与寄存器参数未改，只改传输层和窗口边界校验。由于该组件为 GPLv3，发布含此驱动的固件二进制时须同时提供对应源码。
- `components/cjson/`：cJSON 1.7.19（Dave Gamble 及贡献者，MIT），原许可在同目录 LICENSE；本板固定副本，不依赖另一块板的 managed_components。
- `main/usage.c/h`、`main/protocol.c/h`：本项目自有的用量白名单与协议解析，从 3Inch7EPaper 复制且不含该板的 GPIO、屏幕或电源代码；`main/homecraft_wire.h` 由 HomeCraft 协议中心生成。
- `main/fonts.c`：由 `tools/generate_assets.py` 从 macOS 自带 Arial Unicode / Arial Bold / Hiragino Sans GB W6（粗体中文）栅格化出的字形子集，只含位图，不分发原字体文件。`main/logos.c` 是由已有服务标识量化成六色得到的位图。

- `main/privacy.c`：隐私模式画面，由 `tools/generate_privacy.py` 把梵高《星月夜》（1889，公有领域；Wikimedia Commons `File:Van_Gogh_-_Starry_Night_-_Google_Art_Project.jpg` 的 1280px 缩略图）裁成 800×480 并抖动到六色得到；原图不入库，来源、SHA-256 与处理参数见 `artifacts/privacy-picture.json`。

上述工具仅用于开发和离线编译；固件构建记录见 artifacts，是否部署以本板验证记录为准。
