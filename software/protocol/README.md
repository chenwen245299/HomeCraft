# HomeCraft 协议中心

HomeCraft 负责用量来源、设备认证、能力协商、状态聚合与控制路由；硬件只实现自己的驱动和协议适配。管脚、电源、分区与板级SDK配置不进入软件的共享协议。

- `device-v1.json`：现有设备控制协议的唯一页面编号定义、操作白名单与回复边界。
- `ble-data.md`：现有BLE认证、数据统计、设备控制与兼容约定。
- `../tools/generate_device_protocol.py`：生成Mac的`device_pages.rs`和所选硬件的`homecraft_wire.h`；`--check`核对两端一致。
- `../project.json`：当前开发目标；具体板型描述和生成文件位置由目标目录的`board.json`提供。

在工作区根更新并核对：

```bash
python3 software/tools/generate_device_protocol.py
python3 software/tools/generate_device_protocol.py --check
```

已有0–12页面编号、v1消息和480字节边界保持。当前BLE适配使用Security2/patch1，认证与账户凭据分离；只传白名单统计和设备状态。未知能力/数值维持未知，不能把排队ACK当成已显示。

新增硬件时，在同级独立目录创建board.json和自己的驱动/配置，并在HomeCraft增加相应传输与能力适配。尽量复用公共操作，不把旧板GPIO复制为默认；新增协议字段须兼容和协商。0.3.0新增：天气扩展`weather:1`（含清除请求，6小时过期）、页面13 `usage`，以及`optional_controls`（设备可以声明不提供旋转或历史，旧固件没有这些字段时按支持处理），全部经能力协商；多设备同时连接由HomeCraft为每台设备开独立会话，协议本身不变。以上均未经实机验证。2026-10-27新增 `totals:1`：HomeCraft下发首页今日/本周/本月合计（包含所有已验证Codex账户，未知为-1），屏幕不再自己相加只含一个Codex账户的来源包。同日新增 `places:1`：HomeCraft按温控页“各位置”的名称下发最多2个位置及各自最新温湿度（没有读数时只发名称，屏幕显示未知），接收屏自己的位置标 `here`、用本机传感器。

详细数据来源：[官方入口](../docs/usage-providers.md)。当前板端：[说明](../../3Inch7EPaper/README.md)。
