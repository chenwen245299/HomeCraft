# 3Inch7EPaper 第三方来源

本文件说明当前板端固件使用的驱动、组件、字体与素材。软件部分见 [HomeCraft 第三方来源](../software/THIRD_PARTY.md)。

`components/epaper_vendor/epaper_port.c` 的屏幕命令序列来自微雪官方
[ESP32-S3-ePaper-3.97 仓库](https://github.com/waveshareteam/ESP32-S3-ePaper-3.97/tree/9b12d40731a80213b927ee8a421cae4082952819)，
以 `ESP-IDF/01_E-Paper_Example/components/epaper_port/epaper_port.c` 为基础，
对应的 Arduino `EPD_3in97.cpp` 包含的许可声明保留在移植文件顶部。

本项目移植增加 BUSY 超时、显式片选和传输错误记录；电源初始化、温湿度读取、USB 协议、额度同步和界面代码位于项目自己的模块。

`main/fonts.c` 是固定界面所需字形的栅格子集，由 `tools/generate_fonts.py` 渲染生成，不包含源字体文件。正常编译不需要安装 Pillow。

Wi-Fi 版本通过 ESP-IDF 组件管理器引用 [espressif/cjson 1.7.19~2](https://components.espressif.com/components/espressif/cjson/versions/1.7.19~2)，使用其原有 MIT 许可。解析器源码位于构建时下载的 `managed_components/espressif__cjson/`，版本固定在 `dependencies.lock`，未修改上游解析器。

官方蓝牙配网使用 [espressif/network_provisioning 1.3.1](https://components.espressif.com/components/espressif/network_provisioning/versions/1.3.1)，上游提交 `225a1c974215bdf6eeed990c5f9ffe45f883b171`，Apache-2.0 许可保留在 `components/network_provisioning/LICENSE`。本地仅对 `src/manager.c` 的 Wi-Fi RAM 存储、已有连接保留、主动断开事件和日志策略做补丁，详见 `docs/ble-provisioning.md`；未修改加密协议。该组件的 `json_generator` 依赖由锁文件固定。

二维码使用 [espressif/qrcode 0.2.0](https://components.espressif.com/components/espressif/qrcode/versions/0.2.0)，组件包装层为 Apache-2.0，其中 Nayuki QR-Code-generator 编码器遵循其源文件内 MIT 许可。本项目只调用纯编码器，保留其原有源文件声明，不记录配对二维码载荷。

墨水屏OpenAI、Claude、DeepSeek标识来自各自官方组织/站点，Argus标识取自用户本机安装的Argus.app图标；原始来源及文件见`../software/assets/brands/SOURCES.md`。仅用于识别对应服务，商标归各自所有者，不代表合作背书。`main/brand_assets.c`是黑白屏所需的缩放点阵。新增大字号数字同样是系统字体的有限字形栅格子集，不包含字体文件。

SD音乐使用官方[esp_codec_dev1.6.2](https://components.espressif.com/components/espressif/esp_codec_dev/versions/1.6.2)（Apache-2.0）和[esp_audio_codec2.6.2](https://components.espressif.com/components/espressif/esp_audio_codec/versions/2.6.2)（Espressif Modified MIT，仅供Espressif产品使用）。版本固定在dependencies.lock，上游组件与许可证随组件管理器获取；发布固件的对应许可证副本位于dist/firmware/licenses。main/music_fonts.c为系统Arial Unicode有限字形栅格，未分发源字体。七种格式测试片段取自本项目原创器乐，不含中文歌曲录音。
