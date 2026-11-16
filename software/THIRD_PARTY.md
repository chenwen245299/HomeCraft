# HomeCraft 第三方来源

本文件说明Mac软件使用的依赖、协议客户端与素材。板端部分见 [3Inch7EPaper 第三方来源](../3Inch7EPaper/THIRD_PARTY.md)。

Mac 加密客户端的 `tools/vendor/esp_security2/` 来自官方 network_provisioning 1.3.1 工具及 ESP-IDF v6.1 的 Protocomm Python protobuf 文件，保留 Apache-2.0 声明与 LICENSE。`security/srp6a.py` 有最小兼容补丁：保留固定宽度 SHA-512 内层摘要和 salt 的原始字节，避免前导零在整数转换时丢失。协议未改变，回归测试见 `tests/test_ble_packets.py`。

Rust App 依赖版本固定在 `mac-app/Cargo.lock`，主要使用 egui/eframe、tray-icon、btleplug、RustCrypto aes-gcm/sha2、prost 和 num-bigint，遵循各 crate 自带许可。`mac-app/src/security.rs` 的协议字段与 SRP 参数依据上述 Apache-2.0 的 ESP-IDF `session.proto`、`sec2.proto` 及 Security 2 实现；合成互通样例由官方 Python 客户端生成，不含实际设备秘密。macOS 中文字体在本机运行时加载系统字体，没有复制字体进安装包。

App 更新使用 `semver` 比较版本、`zip` 只读检查压缩包目录，版本与依赖许可由 Cargo.lock 固定；HTTPS 下载和解压由 macOS 自带 curl/ditto 完成。发布清单、后台下载、退出后安装的流程参考用户本机 TexCraft；HomeCraft 的来源校验、白名单打包与安装实现独立维护。

开发与 CI 的敏感信息检查使用 [Gitleaks](https://github.com/gitleaks/gitleaks)，遵循其 MIT 许可；不打包进 App 或硬件。CI 固定官方 CLI 8.30.1 及 Linux x64 发行包 SHA-256，本地要求 8.30.1 或更新版本。仓库自有配置继承官方默认规则，只为一个单元测试初始化行设置精确例外；扫描和合成拦截测试不验证真实凭据或调用账号服务。

HomeCraft使用的OpenAI、Claude、DeepSeek标识来自各自官方组织/站点，Argus标识取自用户本机安装的Argus.app图标；原始来源见 [素材来源](assets/brands/SOURCES.md)。标识仅用于识别服务，商标归各自所有者，不代表合作背书。

EPaper Home 0.2.0的海岸背景、家庭/设备线条图标和App图标为本项目原创绘制，未复制Apple家庭的图像或图标资源。开发方式参考用户本机TexCraft教程；AppKit原生窗口模块隔离系统调用。运行时读取本机SF及中文字体，不随包分发系统字体。

HomeCraft 0.2.1的原创图标源文件为`assets/app-icon/homecraft.svg`；仅参考用户本机TexCraft的平涂、粗描边与错位阴影视觉语言，房屋和墨水屏主体及墨绿配色为新绘制，未复制其图标。PNG与ICNS由sharp栅格化SVG生成，透明边角保留。
