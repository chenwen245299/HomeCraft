# 3Inch7EPaper

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
idf.py build
```

按变更选择离线检查，例如：

```bash
sh tools/test_music.sh
sh tools/test_power_events.sh
sh tools/test_environment_refresh.sh
```

公共协议常量由HomeCraft生成；从工作区根执行 `python3 software/tools/generate_device_protocol.py --check`。不要用其他板的配置或缓存来修复本板构建。

## 部署与实测状态

2026-11-15已部署 **0.2.38**：接收中枢合计后，跨周期缺数据保持未知，不退回单机局部合计；尚未收到过中枢合计时兼容旧版。用户授权后仅写应用区，写入校验与两处 NVS 不变检查通过，原绑定已认证连接 HomeCraft build 48。见 [本次验证](artifacts/hub-0238-validation.json)。以下为 0.2.37 历史：

2026-10-28已部署 **0.2.37**，总览的今日／本周／本月优先显示HomeCraft统一下发的合计（`totals:1`），包含所有已验证Codex账户；匹配周期的未知值仍显示未知。未收到对应周期合计时兼容旧Mac，继续用原来源汇总。配套项目App为0.3.0 build 45，原双RAM差分、旋转、SD与音乐逻辑保留。见 [本次验证](artifacts/totals-0237-validation.json)。

已部署固件与哈希看 [固件验证](artifacts/firmware-validation.json)。构建输出不作为部署证明；2026-10-26的目录整理没有刷机，2026-10-28的0.2.37升级则已单独记录写入与同步验证。用户实屏、完整歌曲、两台实体Mac及故障/测流边界见本板TASKS，不能用合成预览或旧记录替代。

本板升级按明确操作范围仅app-flash并保留NVS、原绑定和SD。BOOT/PWR及音乐页按键、禁用GPIO4/5/6、双RAM差分、电源和音频约束均由本板AGENTS规定，不属于其他硬件的全局规则。
