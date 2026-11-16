# HomeCraft 软件规则

适用于 `software/` 下的Mac App、协议中心、工具、素材与发布。先遵守根规则，再读本目录 [README](README.md)、[待办](TASKS.md)、[协议说明](protocol/README.md)；当前用量口径见 [官方入口](docs/usage-providers.md)。

## 职责与资料路径

- HomeCraft 是软件中枢：管理官方数据来源、认证、设备能力、状态聚合与控制路由。板端负责自己的驱动和执行；GPIO、供电、分区和SDK配置不得进入通用软件默认值。
- 本地开发由 `project.json` 选择硬件目录。Rust `mac-app/src/config.rs` 与 Python `tools/project_paths.py` 解析到该硬件 `.local/`；跨板路径不得越出工作区或静默回退到根目录。
- 新增适配器时验证资料隔离、身份匹配和能力边界，不复制其他板的绑定。显式 `EPAPER_PROJECT` 可指定隔离测试数据目录，指向工作区时仍按目标选择解析。
- 独立安装使用App自己的私有资料入口；已有本机 `.data-location.json` 可指向有效的私有目录。只保存路径，不把开发机路径或绑定写入安装包。
- `mac-app/` 为源码，`protocol/` 为公共协议，`tools/` 为构建/打包/协议生成工具，`artifacts/` 记录软件验证，`dist/` 为交付物。第三方来源放 [本目录声明](THIRD_PARTY.md)。

## 官方数据入口与口径

- Codex 仅用官方app-server公开方法读取账号、额度和Token；不读auth.json、解析登录令牌、发起模型调用或恢复私有HTTP用量接口。
- Claude 只接官方Claude Code `statusLine`，不读OAuth凭据。其Token是当前上下文含缓存，不能称为账户累计消费；桌面Claude或样例不是Code真实会话验证。
- Argus只用官方本机STDIO MCP `get_token_usage`，尊重MCP开关，只读汇总，不读数据库、逐条records、供应商配置或凭据。缓存已包含输入，不重复相加。
- DeepSeek官方API余额与DeepSeek Harness本机用量是独立来源，UI使用全称DeepSeek Harness。Harness只读公开本机记录、按稳定来源覆盖合并，不能把多机合计再作为本机数发送。
- 同一Codex账号跨Mac不重复相加；账号读取前后核对一致，仅保存允许的脱敏标识。iCloud只同步允许的数值摘要，历史绑定、账户凭据不随包或数据摘要迁移。
- 周/月按UTC+8自然周期，缺日期/字段保持未知，不能补0、用昨天冒充今天或用累计差值推算。Codex昨日行取真实昨日桶和同账号昨日最后有效周额度快照，后者是历史已用比例，不是昨日新增消费。
- 保留默认30秒读取/下发、独立BLE保活与手动刷新。DeepSeek余额至少300秒，429后至少900秒；自动轮询频率不等于官方日桶生成频率。完整规则以实现和来源文档为准。

## 连接与协议

- 现有BLE适配保持官方Security2/patch1、双向证明、AES-GCM、服务/端点及设备身份校验，不降级到无认证或无加密。新传输按明确安全适配处理，不能默认套用旧板。
- 当前支持实体窗口8位屏幕码首次绑定，每台Mac认证一次；已有绑定不因换Mac而轮换，不把USB恢复成首次认证必需步骤。PIN不存盘、不打印、不自动重试。
- 输入配对表单暂停重连，不自动提交；手动扫描尊重暂停。恢复的CoreBluetooth缓存条目不证明在线、广播或认证，不能据此断言另一台Mac占用。
- 同一认证会话由设备worker串行读写，保留单实例锁；Python兼容发送器不与Rust同时运行。只操作本项目进程。
- 控屏结果按匹配的请求/采样编号和成功帧确认，不用排队ACK或旧状态冒充完成。普通Status轮询不触发主动采样；账户查询不能阻塞BLE保活和控制。
- `protocol/device-v1.json` 是当前公共页面编号定义。改协议后从工作区根执行 `python3 software/tools/generate_device_protocol.py --check`，核对受影响板端。

## 产品与发布

- 新Mac正常使用只需HomeCraft和对应官方数据客户端的登录；Python、Rust、Homebrew属于开发环境，不当作硬件连接的必要安装项。不添加未经要求的开机自启。
- 关闭窗口隐藏到菜单栏并移除Dock入口；打开窗口恢复，最小化保留Dock，菜单退出才停止同步。
- 继续使用 `mac-app/src/home/theme.rs` 的深色磨砂玻璃、圆角、橙色重点与共享surface/modal/控件，不能新增浅色默认组件或遮住操作的大错误层。
- 日常页保留读数、短状态、进度、完成时间和必要操作，教程集中帮助。Codex检查CLI/刷新/设置继续可用，错误和详细用量按需展开，未知不改0。
- 保留原创雾蓝App图标和独立方正单色菜单栏模板；不直接缩小带底色/倾斜的启动图标当菜单栏图标。
- 打包只包含白名单App、说明及安装链接，不能含 `.local`、绑定、缓存或恢复备份。核对实际版本、架构、签名与包内容；同版本号不代表已安装App与项目包二进制相同，ad-hoc签名不称为Apple公证。
- 构建、检查与安装按 [README](README.md) 执行；迁移资料时短暂退出并恢复本项目App，核对原绑定不变。当前安装与打包事实见 [发布记录](artifacts/homecraft-0236-release-validation.json)，连接状态需要当次检查。

早期软件与这块屏的集成记录保留在 [旧工作区上下文](../3Inch7EPaper/docs/workspace-context-before-split-20261026.md)；其中旧版本的“当前”和授权不作为本次操作依据。
