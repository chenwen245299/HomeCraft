# E1002-AI-MULTI-01 · 暂停检查点

日期：2026-10-26。用户要求“先总结还要干啥，先别干，后面再继续”。当前已暂停开发及设备操作，没有后台备份或编译任务占用串口。恢复时先读本文件，不自动开始烧录。

## 用户已确定的目标

- HomeCraft是唯一数据中枢，统一获取、汇总官方AI用量及天气，两款硬件负责显示与本机操作；不能让每块屏分别查询账户或复制账户令牌。
- HomeCraft保存多个设备的独立绑定，并同时同步，不是只能切换一台连接。
- E1002沿用Security2/patch1、8位屏幕码、实体配对窗口和设备专属长期秘密；不降级安全、不把USB作为日常配对必需步骤。
- 彩色主页必须同时突出Codex、Argus、DeepSeek，三个主要区域；不是Codex大卡、另外两家页脚小卡。DeepSeek Harness用量与DeepSeek API余额不能混淆。
- 剩余额度100%绿色、接近用完红色；服务标识尽量保留原色并映射到实际六种墨色。大屏重新布局，显示更多日/周/月信息，保留缺失与0的区别。
- 天气由HomeCraft自动获取城市，用户已明确同意自动识别；目前设计为公网IP估计、可手动修改/关闭，不将IP或坐标发到屏幕。
- 用户特别要求局部刷新，必须验证面板真正只刷新指定区域，并分别记录耗时、区域外画面保持与残影，不能只根据显存写入窗口认定成功。

## 设备与安装状态（不要误认为已部署）

- USB已确认：`/dev/cu.usbserial-1420`，VID:PID `1a86:7523`，ESP32-S3 v0.2，32MB Flash，芯片识别内置8MB PSRAM。端口名可能变化，操作前重新确认对象。
- **E1002仍是原厂固件，本轮没有烧录、擦除、修改分区或NVS。** 新固件没有运行，PSRAM运行时、屏幕、按键、传感器及新蓝牙服务均未实测。
- **原有3Inch7EPaper固件和已安装HomeCraft均未替换，也未退出原有HomeCraft进程。** 本轮没有实际读取账户用量，没有进行真实天气网络查询或新的蓝牙配对。
- HomeCraft源码与打包脚本版本仍是0.2.36；E1002工程源码拟为0.3.0，不能把它当成已发布或板上版本。
- `software/project.json`仍选择3Inch7EPaper。已有绑定没有迁移或覆盖，新设备的私有资料设计放在对应板目录。

## 已完成到哪里

### E1002固件第一版源码

- `components/e1002_display/`：移植固定提交Seeed_GxEPD2的GDEP073E01时序到ESP-IDF，原波形参数不改；native4bpp六色帧、窗口写入、窗口刷新、BUSY等待和超时。
- `main/color_canvas.*`、`dashboard.*`、`fonts.c`、`logos.c`：实际固件绘图；三栏主页、用量明细、配对画面、原色标识、额度颜色、天气与已知合计。
- `main/ble_link.c`：官方Protocomm Security2、独立NVS随机秘密、左键长按实体窗口、8位码、一次领取与5秒回复宽限。源码已补首次RF启动前的随机熵源处理，尚未重新编译/上板验证。
- `main/main.c`：白名单用量接收、天气摘要、控屏/成功帧状态、SHT4x读取、像素比较与区域刷新调度。最新修改加入初次数据/低额度触发、自动最短3分钟、手动更新、旧数据保留；均仍待最终复核。
- `partitions.csv`保留读回的原厂布局。**该文件是编译描述，不代表已经改写设备分区。**
- 仅复用了旧板不含GPIO的`usage.c/h`、`protocol.c/h`数据解析；复制了固定版本cJSON依赖到本板，未复用旧板屏幕/PMU/管脚。

### HomeCraft第一版源码

- `mac-app/src/registry.rs`：最多8台独立设备，保留旧单设备原路径，不覆盖原绑定；新板秘密按板型/设备分开保存，600权限。
- `mac-app/src/fleet.rs`：独立后台BLE会话、连接/重试/暂停/发送状态；接收中枢同一份用量，不重复发起账户查询。
- `backend.rs`：多设备扫描、配对登记、当前控制设备与其他会话、能力协商、天气下发、彩屏完成等待延长。
- `weather.rs`：IPWho城市估计、Open-Meteo天气、城市/天气缓存、手动城市与开关、边界检查；**只通过离线测试，未验证真实网络可用性。**
- Home界面已加入其他设备状态卡、逐台暂停/全体暂停、在另一台仍连接时输入新设备配对码、天气设置。最新界面与连接逻辑还需复测。

### 公共协议

- `software/protocol/device-v1.json`保留0–12，新增页面13 `usage`及按能力协商的天气扩展。
- 生成器改为核对所有登记的device-v1适配器，并支持`--hardware`；Mac和两板生成常量均已更新。
- **3Inch7EPaper本轮仅生成头文件随公共定义更新，未编译或烧录旧板。** 需要补旧板解析/能力兼容检查。

## 已跑过的检查及有效范围

1. E1002原生工程在`build/homecraft/`曾完整编译通过，日志`artifacts/homecraft-build.log`，当时bin大小`0x1b34c0`。**其后又改了源码，当前bin不能视为最新构建。**
2. 实际C绘图代码的6种合成预览通过ASan/UBSan及六色像素检查，见`artifacts/ui-preview/`。已目视看过三栏主页；其后明细页/部分文字有变化，需重新生成字体和预览。图中数字均为测试值，未发送到设备。
3. Rust完整测试曾通过105项，日志`software/artifacts/multidevice-build.log`。之后又改了多设备配对表单、逐台/全部暂停、其他状态，**需要对最新源码重跑**。
4. 最近Clippy失败项：`fleet.rs`天气发送处嵌套if可合并（collapsible_if）。没有修复，不要把Clippy写成已通过。
5. 没有完成新App实际窗口点测、DMG、安装、首次PIN、两台硬件并行BLE或E1002实屏局刷验证。

## 当前最先要解决：完整原厂备份没有成功

- 只有先前保存的4KiB分区表：`backups/factory-partition-table-20261026.bin`，目录700/文件600，校验记录见`artifacts/usb-first-contact-20261026.json`。
- 32MiB完整读取尝试921600、460800和115200均未完成；最后一次stub/115200报`Corrupt data, expected 0x1000 bytes but received 0xfe6 bytes`。
- **不存在有效`factory-full-20261026.bin`，也不存在有效`artifacts/factory-backup-20261026.json`。不能把分区表当成完整恢复备份。**
- 所有读取进程已结束，esptool已执行RTS复位；串口空闲。没有为修复读取问题擦除或改写设备。
- 恢复后先排查串口可靠性和esptool读取路径，可考虑受校验的分段读取、保守速度或ROM读取；需要核实>16MiB寻址，避免把两半读重。未经验证的片段不能拼成“完整备份”。
- 只有完整私有备份通过大小/分段/哈希核对后，才考虑第一次部署。
- 原厂分区：NVS0x9000/0x7D000，otadata0x86000/0x2000，phy0x88000/0x1000；app0=0x90000、app1=0xC90000，各12MiB；SPIFFS0x1890000/6MiB。
- **烧录前需确认当前启动槽与保留方案。之前implementation-plan写“只更新app0”只是初步计划，不能据此直接写入。** 保留原bootloader、分区、otadata与NVS，不用默认`idf.py flash`或旧板0x10000地址。

## 恢复后的建议顺序

1. 解决可靠完整备份，确认原启动槽及可恢复部署路径。
2. 复核、收尾中枢多设备：逐台绑定/暂停/表单编辑只影响对应设备；当前控制与后台发送不抢同一会话；菜单全停；每台失败独立重试。补“两台会话同时运行，停一台另一台继续”的检查。
3. 明确E1002不支持的能力：目前没有重力旋转/历史曲线实现，Mac不应显示可用旋转控件或“历史正在更新”；补能力协商与相应界面分支。SD/电源未实测保持未提供。
4. 收尾天气：真实验证自动城市和天气；验证手动城市、关闭天气和过期状态。当前固件支持天气清除请求，但主机尚未完整接入清除动作。保留数据来源标注。
5. 复核E1002状态与显示：成功帧必须对应实际完成，单次请求/忙状态/失败重试/采样互斥；设备名称需正确传给配对画面；页面13/8193改用协议生成常量；严查日期、过期额度与缺失值。
6. 重新生成字体/预览，目视检查主页、低额度红色、明细的昨日/余额、空数据、超长数值和配对画面。检查真实六色映射和实际局刷区域；不要修改未知波形电压。
7. 运行最新Rust测试/Clippy、协议生成检查、新板C渲染/协议边界、旧板公共协议兼容检查，再重新编译新固件。
8. 更新README/AGENTS/TASKS/THIRD_PARTY：当前部分文档仍写“仅环境/未实现”，不可当成最终完成记录。记录GPLv3显示驱动、cJSON许可与数据解析来源。
9. 构建版本一致的HomeCraft App/安装包，检查包不含任何绑定/缓存/备份；必要时短暂退出并恢复原App，保留原绑定。当前尚未做这些动作。
10. 新板按确定的安全范围更新后，实测PSRAM、按键、传感器、BLE屏幕码/错误码/重连；让用户在HomeCraft输入真实屏幕码，不记录或打印PIN。验证两台硬件同时收到同一批真实官方数据、控制不串台。
11. 实测区域刷新耗时、区域外画面保持、彩色残影及周期全刷，并请用户目视确认。不能用接口名或日志里的partial替代实屏确认。

## 续做命令与路径

工作区：`/Users/qichengwen/My_App_UI/EPaper`。所有新板配置/构建/私有备份仍放`reTerminalE1002/`，HomeCraft及公共协议放`software/`。

```bash
# 协议与Mac离线检查（根目录）
python3 software/tools/generate_device_protocol.py --check
cargo test --locked --offline --manifest-path software/mac-app/Cargo.toml
cargo clippy --locked --offline --all-targets --manifest-path software/mac-app/Cargo.toml -- -D warnings

# 字体与真实C渲染预览（使用带Pillow的Python）
/Users/qichengwen/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/bin/python3 reTerminalE1002/tools/generate_assets.py
/Users/qichengwen/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/bin/python3 reTerminalE1002/tools/render_preview.py

# 新板构建（在reTerminalE1002内；不是烧录命令）
source /Users/qichengwen/.espressif/tools/activate_idf_v6.1.sh
IDF_COMPONENT_MANAGER=0 idf.py -B build/homecraft -D SDKCONFIG=sdkconfig.homecraft build
```

本机IDF组件均为本地依赖，`IDF_COMPONENT_MANAGER=0`用于避开受限环境中psutil进程查询，不修改全局SDK。部分既有macOS文件协调测试需要系统访问权限；此前105项是在隔离临时目录下取得结果。

官方局刷依据：[Good Display示例入口](https://www.good-display.com/companyfile/2037.html)、[上游作者确认1.6.8加入GDEP073E01区域刷新](https://github.com/ZinggJM/GxEPD2/discussions/161)。当前固定Seeed_GxEPD2为1.6.9，具体实现使用0x83窗口与0x12刷新，仍待实屏确认。

可直接续说：**“从E1002-AI-MULTI-01继续，先解决完整备份，再收尾多设备和天气，验证后上板测局刷。”**
