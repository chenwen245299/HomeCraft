# Mac 蓝牙数据传输

2026-10-20 用户在校园网场景下改选：Mac 获取用量，ESP32-S3 只通过 BLE 接收显示，不在板子配置账户密钥。**这是当前默认模式，替代此前 Wi-Fi 独立查询目标。**

## 已完成的链路

Mac上的Rust菜单栏App（`software/mac-app/`）→官方Protocomm Security 2→设备`usage-data`端点→白名单解析→仪表盘。当前差分刷新经用户确认后已启用，切页/旋转/周期清残影全刷。温湿度、SD分钟日志、两键手势和QMI8658自动旋转继续由板子独立处理。

本机已使用真实 Codex 登录完成发送，设备在 `connected=0`（未连接 Wi-Fi）时报告 `data=1` 和有效 `received` 时间。重启板子后，Mac 自动重连并重新获取、发送真实额度。错误配对码不能建立认证会话。Claude、DeepSeek 暂无凭据，发送“未配置”，未使用模拟值。

## 当前默认入口

双击 `software/dist/HomeCraft.app`，在列表扫描并连接设备。关闭窗口后菜单栏常驻，菜单“退出程序”才停止；用户已确认菜单栏与后台同步正常。Rust 直接处理 BLE 与官方额度/Token 查询，不启动 Python。使用与构建见 [Rust App 记录](../docs/mac-app.md)。以下 Python 命令只作为兼容工具保留，当前已停止，不能与 App 同时运行。

## 兼容命令与资源历史记录

项目目录执行：

```bash
software/.ble-venv/bin/python software/tools/ble_bridge.py
```

也可以双击 `software/tools/start_ble_sender.command`。默认 300 秒读取一次，约 10 秒一次加密保活；没有持续扫描循环。断线按 3/6/12 秒递增到最长 60 秒重试。Codex 查询临时启动官方 app-server，取回 `account/rateLimits/read` 后退出该子进程；不运行模型推理。Claude/DeepSeek 最短 300 秒查询，429 后至少等待 900 秒。BOOT 双击会排队提前查询，仍遵守最低间隔。

停止：`software/.ble-venv/bin/python software/tools/ble_bridge.py --stop`，或双击 `software/tools/stop_ble_sender.command`。进程使用本项目单实例锁；未安装 launchd、登录项或定时自动任务。Mac 需要开机并运行发送程序；休眠、关机或断开后板子保留 RAM 内最后数据并显示旧数据，板子自身重启则等待新快照。空闲实测一次为 CPU 0.0%、RSS 22,816 KiB，只代表该次采样，不是持续功耗或资源上限保证。

## 当前蓝牙首次认证与双Mac切换（0.2.8实机阶段）

用户明确要求取消USB首次认证的必需步骤。2026-10-21晚已app-only部署0.2.8，保留原NVS与SD；已验证原有长期绑定直接重连，屏幕码首次领取及实际两台Mac交替仍需单独实测。

- 设备网络页双击BOOT实体开启10分钟配对窗口，随机8位码只显示在设备屏幕；广播`EPair_`和独立服务UUID `1775244d-6b43-439b-877c-060f2d9bed08`。不通过广播或未认证端点读回该码。
- Mac手动输入屏幕码，经原有官方Protocomm Security 2（SRP-3072/SHA512、AES-GCM、patch1）证明双方知道该码；`pair-device`端点只接收加密的`get`。官方Protocomm先解密/认证，再调用处理器，回复也被加密。
- 窗口最多成功领取一次，失败5次关闭；到期或领取后恢复普通数据服务。成功领取后预留5秒读取回复，App等待6秒再扫描，实际时序待实板确认。
- 已认证回复内的设备名须匹配广告名称后缀、用户名及数据UUID必须匹配；长期128位随机密钥仅在此加密会话传递，Mac原子保存600私密文件。配对码不存盘、不打印、不自动重试。
- 新增Mac不会重新生成设备长期密钥，不取消原有绑定；两台各认证一次，日常切换时原Mac暂停连接，另一台连接已保存设备。无需每次BOOT/配对码/USB；同一时刻一台Mac同步。
- 独立配对UUID使旧数据客户端在认证前找不到数据端点，避免拿旧长期密钥反复尝试临时PIN。账户Token/API Key从不参与配对或进入设备。
- 旧USB绑定只作为旧固件维护入口，不是新流程的必需步骤。旧文档下节保留历史实现，不应再用来要求用户每次换机插线。

依据[官方Protocomm文档](https://docs.espressif.com/projects/esp-idf/en/stable/esp32s3/api-reference/provisioning/protocomm.html)及本机IDF v6.1的`protocomm_req_handle`源码核对端点的先解密、后处理与加密回复顺序。没有改动加密算法或降级安全版本。离线测试不能代替实际GATT模式切换、PIN认证和双Mac测试。

## 历史USB配对与凭据

本机已绑定，日常无需 USB 数据链路，USB 可只用于供电。新 Mac：设备网络页双击 BOOT 开启十分钟配对窗口，连接 USB 后运行：

```bash
software/.ble-venv/bin/python software/tools/ble_bridge.py --pair-usb
```

本地 USB 将设备名、用户名、蓝牙认证秘密和服务 UUID 写入 `3Inch7EPaper/.local/.ble-pairing.json`，权限 600；成功导出后关闭导出窗口。这不是账户授权，不包含 Codex/Claude/DeepSeek 密钥。配对文件不能分享，完整 Flash 备份也含蓝牙秘密。Mac 的 CoreBluetooth 通过应用程序连接 BLE，不需要把设备在系统设置中当作耳机或串口手动配对。[Bleak macOS 后端说明](https://bleak.readthedocs.io/en/latest/backends/macos.html)

数据模式使用设备专属的 128 位长期随机秘密，保存在 `dashboard` NVS 的独立键中，用于断电后自动重连；每次连接通过 SRP 产生新的会话密钥。这与手机 Wi-Fi 配网的“每次窗口生成新秘密”是两种模式，不能混淆。数据服务首次未绑定时十分钟后关闭，网络页双击 BOOT 可再次打开；已绑定后持续提供认证后的数据接收。认证失败累计五次暂停服务十分钟，也可实体操作重新开启配对。

账户来源都只在 Mac：

- Codex：本机官方 CLI 已有 ChatGPT 登录，Rust 只调用官方文档公开的 app-server `account/rateLimits/read`、`account/usage/read`；不读取 auth.json 或把令牌放到板子。累计 Token 在 Mac App 显示；日统计通过可选 d/ld 字段发送到大字版墨水屏。
- Claude：只接收官方 Claude Code `statusLine`，读取本机 `3Inch7EPaper/.local/.claude-statusline.json` 的数值缓存；不读取 OAuth/Keychain，不调用未公开的用量 HTTP 端点。官方客户端需要运行并产生状态栏数据。
- Argus：官方本机STDIO MCP get_token_usage，只读当前文献库日/周汇总，不读取数据库或服务凭据。
- DeepSeek：`DEEPSEEK_API_KEY` 或本机 `3Inch7EPaper/.local/.ble-settings.json` 的 `deepseek_api_key`，只查询余额。可参照 `software/tools/ble-settings.example.json` 创建私密文件并 `chmod 600 3Inch7EPaper/.local/.ble-settings.json`。

蓝牙快照字段为 `v,p,t,s,w,b,c,mu,ml,d,ld,n,u,dc,wt,o,mt`；传输层拒绝其他字段，设备也独立拒绝未知键、重复键、非整数窗口及超长包。每包最多 480 字节，AES-GCM 后不超过 GATT 上限；缺少的月/周窗口仍用 `-1` 表示，不填成满额。接收端状态接口不会读回账户密钥。

2026-10-21板端扩展已烧录：最多8个服务槽，原三家ID保持不变，Argus为3、后续适配器4–7。状态回复增加`profiles:true,max_providers:8`，同时保留`daily:true`。新可选字段`n`为1–24字符ASCII名称（内置名称禁止冒用），`u`为`[input,output,cached,calls,missing_cost_calls]`，`dc`为已记录USD费用；仅在能力协商后发送。所有计数为有限整数且在JS安全整数范围内，输入+输出必须等于当天Token，缓存≤输入、缺费用调用≤总调用，不把缓存另加一次。仍限制480字节、拒绝未知/重复字段，不包含凭据。2026-10-21已部署新版Rust发送器并真实读取更新后Argus的get_token_usage，今日卡片及Token合计已获用户确认。

新增`wt:[Monday_YYYYMMDD,total]`表示本周实际Token，不是额度。设备单独协商`weekly_tokens:true`，仅有profiles能力的旧固件不会收到wt；双方校验周一日期、0至JS安全整数范围。新周不显示旧周，周查询失败不丢弃今日数据。

## 协议与兼容性

沿用官方 `network_provisioning 1.3.1` / Protocomm Security 2，关闭自动结束并增加 `usage-data` 端点。服务广播为 `1775244d-6b43-439b-877c-060f2d9bed07`，设备名 `EPaper_` 加 MAC 后六位。Mac 读取 GATT 0x2901 描述符寻找 `proto-ver`、`prov-session`、`usage-data`，不假定特征 UUID 的基底；官方头文件中旧默认 UUID 注释与当前实现不一致，实际值已按源码和广播核实。

Python 兼容工具复用 `software/tools/vendor/esp_security2` 中官方 Security 2 客户端，强制 `sec_ver=2`、`sec_patch_ver=1`，拒绝降级；BLE ATT MTU 设置 517。实测发现官方 Python 工具把内层 SHA-512 摘要先转整数，丢弃开头的零字节，导致部分配对秘密始终认证失败。本项目仅修正固定宽度摘要和 salt 字节的保留，并用公开合成测试值覆盖；未改变认证协议或减弱验证。修正后真实双向 SRP 认证、AES-GCM 数据发送及重连均通过。Rust 客户端按相同官方协议实现，另有跨语言合成样例测试和实物认证/发送验证。

当前 NVS/Flash 未加密，不能防拆机读取；没有烧写 eFuse 或关闭调试端口。BLE 加密保护传输，Mac 文件权限保护本机保存，均不能保护被控制的 Mac 或已泄露的配对文件。

## 备用手机 Wi-Fi 配网

BOOT 长按三秒临时停止数据接收并开启 `PROV_…` 官方手机配网；结束后恢复 Mac 数据服务。网络页三击 BOOT 打开旧网页高级设置，关闭后也恢复数据模式。当前用户不需要在这些入口填入账户令牌，也没有配置校园网网页登录或企业 EAP 登录。Wi-Fi 配网历史和核实状态见 [蓝牙配网记录](../../3Inch7EPaper/docs/ble-provisioning.md)。


## 0.2.8 双向设备数据与控制

HomeCraft除了发送官方用量，还通过同一个已认证的`usage-data`端点读取真实设备数据、切换页面及方向。能力握手增加`device_control:1`；旧固件保持用量同步，App不会向旧版发送新命令。所有新命令仍由Protocomm Security 2先解密和校验，不能通过广播或未认证GATT读取。

- `{"v":1,"op":"status"}`：真实SHTC3温湿度、采样时间/年龄、当前页、可用页掩码、方向/自动方向、暂停/刷新状态、局刷开关、PMU电源与SD记录状态。无效传感器值为null，未知电量为-1，绝不伪装0。
- `{"v":1,"op":"history","offset":0}`：UTC+8当天96个15分钟均值点，每包8点，按0/8/.../88查询。回复带day/revision/offset，缺测点null；App仅拼接同日同版本连续完整分块，避免跨午夜混图。逐分钟原始CSV仍只在SD。
- `{"v":1,"op":"page","page":"environment"}`：只允许实际可用页面；支持overview/codex/claude/deepseek/environment/network/help/argus/extra1–4。排队回复不当作已显示，随后状态确认实际页面；页和方向变化沿用全刷底图。
- `{"v":1,"op":"rotate","rotation":"auto"}` 或rotation 0–3：恢复IMU自动方向，或锁定四个方向。
- `{"v":1,"op":"refresh"}`：手动清除残影、整屏刷新。

命令只允许规定字段、版本和整数边界，拒绝未知/重复字段及尾随内容；不提供读取配置/密钥、任意文件、远程关机或远程开启首次配对的能力。每个明文回复最多480字节，加GCM标签仍低于512字节GATT值上限；保持原480字节用量快照兼容。主循环从4项队列执行控制，BLE回调不直接驱动屏幕。

App连接后立即读取状态，正常每5秒回传；每60秒检查历史修订，变化才分块读取。账户查询间隔与此分开。断开或采样超时显示上次/延迟状态；缺测区不补线。原有SD每分钟落盘、墨水屏温湿度/时钟/用量共用差分通道及周期清残影策略不变。

本次修正配对窗口截止前成功领取时仍保留完整5秒回复时间；永久蓝牙密钥不再进入显示快照。NVS初始化失败停止网络初始化，不自动擦除既有绑定。


## 0.2.9 已显示帧回传

status.device的可选shown为`[temperature,humidity,displayed_at,page_index,rotation,auto_rotate,sequence]`。仅在实际屏幕传输成功后增加sequence并保存该帧状态；失败保留上一帧，不能把队列接受或内存当前页当作实屏完成。传感器无效时两值null。App控制操作等待相应成功帧或明确超时；全刷需sequence增加。配套App同时显示实时采样与已显示帧时间和值，解释SHTC3每5秒采样与屏幕30–60秒节流的0.x差异。回复仍≤480字节。

0.2.9实际按钮审计修复了CoreBluetooth断开后的无效句柄，重连先按已知ID通过公开retrieve_peripherals重新取得对象，再执行原Security 2认证。暂停后扫描不会自动抢回连接，方便两台Mac交替。已有绑定的暂停/扫描/重连已实测，用户已确认手动刷新现在有进度和完成反馈；首次PIN及两台实体Mac仍单独等待实测。


## 0.2.10 macOS服务缓存与模式识别

首次实测发现：广告为EPair/配对UUID，但Mac公开GATT列表仍将服务标为原ed07，实际0x2901端点已经是pair-device。0.2.9按ed08严格过滤，因此在发送任何屏幕码证明前就报“缺少配对入口”，不是据此认定用户输错PIN。

0.2.10仅在ed07/ed08这两个已知服务中，按实际pair-device或usage-data、proto-ver和prov-session同组选择模式；优先正确UUID，否则只接受唯一完整候选。错误模式、两端点并存、未知UUID或歧义均在SRP前拒绝。仍校验Security 2/patch1、服务端证明、加密完整性及领取回复身份，不改验证码/密钥/窗口策略。蓝牙首次连接也先retrieve已知原生设备，避免失效句柄。错误消息补充失败阶段并移除输入码，不打印秘密或认证载荷。

已新增旧UUID/新端点兼容及错误模式拒绝回归，38项Rust测试和Clippy通过；本机App已换0.2.10，固件仍是已部署0.2.9。本次不再烧录，正请用户用新窗口重试，最终实测以3Inch7EPaper/artifacts/homecraft-0210-enrollment-diagnosis.json为准。


2026-10-21 App 0.2.11：修复当前广播名与缓存名混用导致误走usage-data的问题。连接表单打开/输码会挂起重连，仅显式点击码框下的安全连接才认证；模式不符时停止重试并给出打开/关闭配对窗口的具体提示。0.2.10的端点按实际名称解析仍保留，安全条件不降低。0.2.9板端不变；首次码实测待用户完成。


2026-10-21补充：用户已确认HomeCraft 0.2.11屏幕码“认证成功，已连接”。随后安全状态确认普通数据链路已认证，长期绑定与首次配对前摘要比较一致；原绑定未轮换。两台实体Mac交替仍未单独实测。


## 0.2.15 Argus分类快照

原有日/周汇总包不变。设备认证ACK新增argus_breakdown:1，新HomeCraft仅向该能力设备追加单独分类包：`{"v":1,"p":3,"t":读取时间,"d":[日期,当日总量],"g":[模型总类数,供应商总类数],"m":[[标签,Token],...],"r":[[标签,Token],...]}`。每组最多3行，超过3类时第3行必须是Other；有分类时组内合计必须等于d的总量。每包仍≤480字节，走既有Protocomm Security2加密端点，旧设备不会收到扩展。

板端只在日、总数、读取时间与已有Argus成功汇总完全匹配时应用，拒绝混入不同查询结果；新日汇总到达会清掉不匹配的旧分类。重复/额外字段、错误日期、非整数、超限计数、重复/非法标签及组和不一致全部拒绝。仅存RAM。诊断EPAPER_ARGUS_STATS只输出匹配状态/日/类别数，不输出凭据或原始传输。


## 0.2.17 主动采样事务

能力ACK新增sample_refresh:1，认证端点新增严格无参数命令{v:1,op:sample}；status仍只读，旧App轮询不会触发测量。sample的队列ACK返回非零32位sample编号，同一时刻只接受一个采样请求；板端task通知携带编号，新测量必须匹配编号且时间晚于请求。状态可选sample:[编号,结果]，0等待、1已更新、2画面无变化、3传感器失败/超时、4刷屏失败；仅在真实渲染完成或确认像素相同后结束。状态及加密包仍≤480字节，字段、编号、错误均不涉及任何账户或配对凭据。

App以对应事务结果作为完成条件，忽略旧事务与单纯队列ACK。手动采样/用量更新走原差分策略和全刷恢复门限，不改变Security 2、长期绑定、首次屏幕码与账户来源。实际验证见manual-sample-0217-validation.json。

## 0.2.18 按 Mac 来源的 Harness 快照

认证 ACK 增加 `harness_devices:1`。仅该能力设备收到 profile 4 / DeepSeek Harness 的 `o:{id,name}` 来源对象；id 为 16 位小写十六进制应用域摘要，name 为 1–24 位受限 ASCII 名称。它是统计归属，不是新认证凭据；传输仍由现有 Security 2 双向认证及 GCM 保护。旧固件继续同步原 provider，跳过新 Harness 数据，不发送不认识的键。

每台上传自己的 d/wt/u，按 id 和时间覆盖。板端仅保留最多 8 台的数值同步缓存并汇总显示；NVS 独立 harness_stats 键，在数值/日期/名称变化时写入，不因每次保活或相同读数写 Flash。错误记录/旧时间/同时间冲突/第9台会拒绝，日期和周边界独立过滤，无同日记录显示未知。正常关机前尝试保存，不抹除任何既有 NVS 或绑定。

新增只读命令 `{v:1,op:"harness",offset:0..7}`，每包1行，回复有 count/offset/devices，每行包含 id/name/t/d/wt/u；App 校验数量、日期、计数与唯一身份后与本机缓存合并。HomeCraft 可将各台已缓存的原始快照恢复给显示端，绝不回传合计。明文仍不超过480字节，不返回密码、密钥或会话内容。安全串口诊断 EPAPER_HARNESS_STATS 仅打印数量、日/周总量和校验状态。


## 0.2.19 传输节奏

用户确认用量30秒由Mac下发，板端遥测/曲线/多Mac数字缓存5分钟回传，手动采样或控屏立即按完成事务回读。数据ACK仍立即。新App freshness容纳五分钟回传间隔，缺测/断开不冒充实时；旧分钟配置兼容，默认为30秒。BLE模式停止Wi-Fi射频，官方配网仍按实体操作启动。


## 0.2.29 月Token

可选`mt:[YYYYMM01,total]`使用`monthly_tokens:true`独立协商。只接受真实月份首日和0至JS安全整数，缺失不补0；显示仅取当前UTC+8月。Mac对旧固件剥离mt。原Security2、白名单、GATT480字节、长期绑定不变。

Harness每台快照及harness回复可带mt；与日统计同月且月总数不得小于当天，不要求大于周（跨月周会含上月）。稳定ID按时间覆盖，最多8台；板端harness_stats v1缓存迁移至v2，旧月统计为未知，旧日／周缓存保留，不擦dashboard或SD。App旧设备回复缺mt仍可读。真实两台缓存迁移验证见monthly-0229-validation.json。


## 0.2.30 扫描与普通启动配置修复（2026-10-25，当前Mac App）

用户截图为扫描完成但列表为空。只读USB确认板子仍为0.2.29、data=1/link=1；当前NimBLE最大连接数为1，官方Protocomm在连接断开后恢复广播，因此没有广播不能当成设备不在附近。本轮不操作另一台Mac、不重配、不改NVS或固件。

同时实查Finder启动的/Applications/HomeCraft.app使用未绑定的私有Support目录，而旧绑定/设置还在本机项目目录；此前用EPAPER_PROJECT启动成功不代表Finder入口已读回。新增本机600的.data-location.json显式选择已有配置目录，校验绝对路径、目录所有者、非符号链接和现有有效600绑定。选择只保存路径，不复制/轮换秘密，不编译开发机路径进包；新Mac无此本机文件时仍使用原有700的Support目录并自行首次认证。普通启动、CLI/statusLine和旧发送工具继续共用原配置与单实例锁。

扫描仍仅使用已知服务UUID；额外通过CoreBluetooth公开retrieve_peripherals恢复本机已连接服务或认证后保存的设备UUID。恢复项明确标为已记住/暂未收到广播，缓存不证明在线或通过认证，首次自动扫描不自动连接仅恢复的项。原Security2双向认证/服务与端点校验不变，EPair模式不按旧数据绑定连接。UUID缓存.ble-device.json为600，仅含名称与系统UUID，与当前绑定名称匹配后才使用；认证成功后才保存。不打印或返回配对秘密。

已记住但无广播时显示原设备名、广播状态和切换Mac提示，隐藏无意义的配对码表单；蓝牙关闭/权限不可用归为扫描错误，不显示普通空扫描。手动扫描仍尊重暂停。96项Rust、Clippy全目标零警告、300张正式组件样例、隔离原生窗口导航/连接页/关闭重开检查通过。样例不访问账户或BLE。

本机/Applications/HomeCraft.app、项目dist/HomeCraft.app和唯一安装包HomeCraft-0.2.30-macos-arm64.dmg均build30。退出旧的本项目App后更新；不带EPAPER_PROJECT、cwd=/正常启动，已验证原绑定加载且文件内容未变，实际连接页不再显示配对码。当前仍未发现广播/未连接；不能把绑定恢复和UI修复说成已完成BLE连接或本月上屏。用户先前要求另一台先不处理，本轮没有抢占或中断那条连接。实际状态、签名与包白名单见3Inch7EPaper/artifacts/discovery-0230-validation.json。


## 0.2.31 广播异常恢复与判断纠正（2026-10-25）

用户指出屏幕显示未连接。重新清缓冲只读查询确认link=0；独立服务过滤扫描0台、无过滤扫描13台但无墨水屏。原固件正常复位后立即恢复EPaper_B96AD4/ed07广播，因此先前仅根据旧link=1判断“必然是另一台占用”过强，不能再以此解释当前故障。

已app-only部署0.2.31并校验，保留原NVS／Security2／SD。增加EPAPER_BLE_STATUS读取实际host/synced/advertising/connections，区分传输连接和应用已认证数据标志；EPAPER_USAGE_STATS只输出provider编号、状态、日周月日期与数字，不输出账户／秘密。已绑定正常数据模式下，连续15秒无广播且无真实连接才恢复，60秒重试冷却；配对窗口、认证锁定、手机配网、关机及活跃连接均不恢复。EPAPER_BLE_RECOVER仅为USB闲置维护入口，执行前再查真实连接。

NimBLE连接枚举在本机IDF6.1源码核实后封装于ble_transport_probe.c，持host锁查询。该SDK缺少枚举函数的头文件声明，兼容声明集中隔离，升级SDK需重新核查，不涉及任何账户数据入口。正常状态每秒只读本机状态，不发额外BLE请求或恢复Wi-Fi。

实测原绑定认证、真实三家月字段已到板，active恢复请求accepted=0；App正常退出后advertising=1/connections=0，闲置恢复accepted=1/recoveries=1/error=0后仍广播，后续同秘密再次认证成功。用户已确认“终于正常了”。自动计时分支边界通过宿主ASan/UBSan，未在实板人为停止广播注入该定时故障，长期复发观察仍待。见3Inch7EPaper/artifacts/ble-0231-validation.json，当前0.2.32继承同机制。


## 0.2.33 Codex昨日摘要

可选yd:[YYYYMMDD,tokens,week_used_permille]只允许provider0，能力yesterday_usage:true独立协商，缺项可为-1。真实日期、Token JS安全整数、周已用0–1000或未知，重复／未知字段仍拒绝。旧固件不收到yd。派生yd不写入共享v1/v2摘要；原官方w快照保持供本机同账号历史匹配。Security2／480字节／NVS绑定不变。板端新增EPAPER_YESTERDAY仅输出日期与数字，验证见codex-yesterday-0233-validation.json。


## 2026-11-18 最终结果直显

AI 用量采集、去重、汇总和跨机更新仅在 Mac App 完成。屏幕只接收最终 profile 数字和 `totals:1`，整体替换最后的显示快照，不保存各 Mac 来源、不将屏幕作为同步中转或统计来源。profile 不含 `o`；两款接收端均拒绝机器来源扩展。

`display_snapshots:1` 声明严格直显：总览仅复制 App 发来的匹配周期值，即使第一次连接前也不按各 provider 回退求和。未知字段/失败快照替换旧值，日期不匹配显示未知；不把缺值补成 0 或留作旧数。`harness_aggregate:1` 沿用为 profile 4 最终数值接收能力，统计不在板上执行。

小屏移除 `harness_stats` 模块及 NVS 读取/写入，旧键原样保留但不再参与显示。`op:harness` 已退役，返回不支持；Mac 已删除该命令调用和缓存回流。当前 App 对缺少 `display_snapshots:1` 的旧固件明确提示结果直显未支持，不静默将跳过的 Harness 专页算作全部更新完成。页面编号和 480 字节边界不变，绑定、认证与硬件刷新策略不变。
