# 3Inch7EPaper

2026-11-21 已部署 **0.2.42**：总览固定按 Codex、Argus、DeepSeek Harness 显示三张卡片，等待同步、读取失败、未知和旧数据均不再改变卡片数量、顺序或尺寸；缺失数值显示 `--`。仅写应用区并通过固件及配置保留校验，原 HomeCraft 安装保留。验证与实屏确认边界见 [本次部署](artifacts/fixed-cards-0242-deployment.json) 和 [本板待办](TASKS.md)。原固定2秒广播及 Mac 最终用量直显保留。以下为历史记录。

2026-11-18 已部署 **0.2.41**：按用户最新要求，已绑定数据模式从首次广播起固定每2秒一次，取消前30秒快速阶段，保持可连接；仅应用区升级，写入和配置保留校验通过。运行与测流边界见 [本次部署](artifacts/ble-fixed-0241-deployment.json)、[省电策略](docs/energy-policy.md) 和 [本板待办](TASKS.md)。0.2.39开始的Mac最终用量直显继续保留。

本目录对应Waveshare ESP32-S3-ePaper-3.97，文件夹名称按用户指定。HomeCraft通过协议适配与它通信；其他硬件应使用自己的同级目录，不能继承本板管脚或SDK默认值。

## 使用与规则

- [本板规则](AGENTS.md) / [当前待办](TASKS.md)
- [设备完整指南](docs/dashboard-guide.md) / [SD音乐](docs/music.md)
- [硬件来源与管脚](docs/hardware.md) / [显示](docs/epaper-usage.md) / [省电](docs/energy-policy.md)
- [HomeCraft协议中心](../software/protocol/README.md) / [第三方来源](THIRD_PARTY.md)

## 本板目录

| 路径 | 内容 |
| --- | --- |
| `board.json` | 本板描述、资料目录和协议适配 |
| `main/` | 应用、管脚、传感器、音乐、网络与显示 |
| `components/` | 屏幕驱动和保留本地补丁的配网组件 |
| `sdkconfig*`、`partitions.csv` | 本板构建配置和分区 |
| `tools/`、`tests/` | 本板资源生成和C回归 |
| `dist/firmware/` | 已归档固件 |
| `.local/`、`backups/` | 本机私有资料与恢复备份，禁止分享 |
| `build/`、`.cache/` | 当前离线构建及迁移前缓存 |
| `docs/`、`artifacts/` | 本板文档、配套集成历史与验证记录 |

## 开发与验证

先读本板规则和环境记录，重新核对工具链。当前验证使用ESP-IDF v6.1；从本目录编译：

```bash
source /Users/qichengwen/.espressif/tools/activate_idf_v6.1.sh
idf.py -B build/fixed-cards-0242 build
```

按变更选择离线检查，例如：

```bash
sh tools/test_music.sh
sh tools/test_power_events.sh
sh tools/test_environment_refresh.sh
```

公共协议常量由HomeCraft生成；从工作区根执行 `python3 software/tools/generate_device_protocol.py --check`。不要用其他板的配置或缓存来修复本板构建。

## 部署与实测状态

2026-11-18 从 **0.2.40 升级至 0.2.41**，按用户要求固定每2秒广播，不再追查客户端来源；只写应用区，写入及引导区/分区表/两处NVS保留校验通过，SD未改。见 [本次部署](artifacts/ble-fixed-0241-deployment.json)。以下为历史记录：

2026-11-18 从 **0.2.39 升级至 0.2.40**，只写应用区；写入校验通过，引导区/分区表及两处 NVS 逐字节相同，SD 未改。新增广播省电策略的检查与实际运行边界分别记录在 [本次部署](artifacts/ble-idle-0240-deployment.json)，没有整板电流或续航结论。以下为历史记录：

2026-11-18 从 **0.2.38 升级至 0.2.39**，仅写 `0x10000` 应用区，写入校验通过；引导区、分区表及系统/dashboard NVS 升级前后逐字节相同，SD 未改。运行版本已回读为 0.2.39；恢复原安装 HomeCraft 0.3.2 build 50 后，原绑定重新认证并收到真实用量帧。App 0.3.3 build 51 本轮未安装，最终实屏视觉待用户确认。见 [部署记录](artifacts/display-only-0239-deployment.json)。以下为历史记录：

2026-11-15已部署 **0.2.38**：接收中枢合计后，跨周期缺数据保持未知，不退回单机局部合计；尚未收到过中枢合计时兼容旧版。用户授权后仅写应用区，写入校验与两处 NVS 不变检查通过，原绑定已认证连接 HomeCraft build 48。见 [本次验证](artifacts/hub-0238-validation.json)。

2026-10-28已部署 **0.2.37**，总览的今日／本周／本月优先显示HomeCraft统一下发的合计（`totals:1`），包含所有已验证Codex账户；匹配周期的未知值仍显示未知。未收到对应周期合计时兼容旧Mac，继续用原来源汇总。配套项目App为0.3.0 build 45，原双RAM差分、旋转、SD与音乐逻辑保留。见 [本次验证](artifacts/totals-0237-validation.json)。

已部署固件与哈希看 [固件验证](artifacts/firmware-validation.json)。构建输出不作为部署证明；2026-10-26的目录整理没有刷机，2026-10-28的0.2.37升级则已单独记录写入与同步验证。用户实屏、完整歌曲、两台实体Mac及故障/测流边界见本板TASKS，不能用合成预览或旧记录替代。

本板升级按明确操作范围仅app-flash并保留NVS、原绑定和SD。BOOT/PWR及音乐页按键、禁用GPIO4/5/6、双RAM差分、电源和音频约束均由本板AGENTS规定，不属于其他硬件的全局规则。
