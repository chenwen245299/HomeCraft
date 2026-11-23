# 用量来源：仅使用官方公开入口

最近核对日期：2026-10-24（Codex 日统计延迟与刷新周期）。用户明确要求只使用官方提供的数据接口；当前默认入口为 Rust 菜单栏 App，见 [使用说明](mac-app.md)。Mac 获取数据并加密 BLE 发送，板子不读取账户，不依赖校园网。

| 服务 | 当前官方入口 | 范围与限制 |
| --- | --- | --- |
| Codex | 官方 app-server `account/rateLimits/read`、`account/usage/read` | 额度窗口、重置时间、账户 Token 汇总及可选按日统计，缺失字段不推算 |
| Claude Code | 官方 `statusLine` JSON | 套餐短期/每周额度、当前上下文输入/输出 Token；随官方客户端会话更新 |
| Argus | 本机STDIO MCP `get_token_usage` | 当前文献库今日及本周实际Token、调用与已记录费用；不是额度 |
| DeepSeek 官方 API | `GET /user/balance` | CNY / USD 可用余额；没有订阅周/月套餐比例 |

## Codex

[OpenAI 官方 App Server 文档](https://learn.chatgpt.com/docs/app-server) 公开了两种查询。Mac 调用已有官方 CLI 的 `codex app-server --listen stdio://`，完成初始化后读取数据并关闭子进程。官方客户端管理已有 ChatGPT 登录；程序不直接读取登录文件、不导出刷新令牌，不发起模型调用。

`account/rateLimits/read`：优先 `rateLimitsByLimitId.codex`，其次 `rateLimits`。`usedPercent` 为已用比例，`windowDurationMins` 为窗口长度，`resetsAt` 为 Unix 秒。显示剩余值 `100 - usedPercent`，未知窗口显示未提供。API 花费、Token 数和订阅剩余比例是不同含义。

`account/usage/read`：读取 `summary.lifetimeTokens`、`peakDailyTokens` 及 `dailyUsageBuckets`。App 当前显示累计 Token、单日峰值和最近统计日。服务端可能延迟更新、返回 null，旧 CLI 也可能不支持；此时只把 Token 标为未提供，额度查询继续工作。它不是尚可使用多少 Token 的额度承诺。没有今天的桶不等于0，现有发送器会在其出现后自动纳入今日合计。

2026-10-24 22:20（UTC+8）直接调用官方接口，返回54个日桶，最新仍为2026-10-23的49,876,983 Token，累计2,015,871,113；没有10月24日日桶。22:21通过已安装HomeCraft 0.2.25的`--refresh-codex`绕过缓存复核，得到相同累计和最近日统计，今日为`[20261024,-1]`，5小时/周额度可正常读取。本机配置与默认轮询均为30秒，缓存也是30秒；未发现等到日末才发起查询的逻辑。此次证据表明当天Token缺失来自官方返回，而非HomeCraft刷新间隔。最初一次查询额度失败，后续直接接口与App均成功，不把瞬时失败当作统计延迟证据。脱敏数值记录见`3Inch7EPaper/artifacts/codex-daily-latency-20261024.json`。

官方文档仅说明账户汇总与可选日桶，未承诺日桶实时/每小时更新，也未说明固定生成时刻；不能据此断言每天某个时间结算或自行改变官方日期标签。`thread/tokenUsage/updated`是活动会话的用量通知，不是全账户实时日汇总。只提高轮询频率无法生成服务端尚未提供的当天数；不得用累计差值、上下文或会话统计冒充官方今日账户Token。

0.2.26首页新增本周Token“已知合计”。Codex复用同一次`account/usage/read`返回的有效日桶，按显示日（UTC+8）所属周一至当天筛选后合计；同日重复记录取最大值只计一次，排除上周、未来、负数、无效日期和超界值。有至少一个有效日桶才生成本周值，缺失日期不补0，整周无数据保持未知，不使用累计或额度百分比推算。该已知周合计使用现有`wt`字段，iCloud同账号仍取最新观测，不按Mac重复相加；Argus和Harness继续使用各自已有的本周统计。没有增加官方查询或更改30秒轮询，不表示Codex当天数据已能实时提供。

## Argus

官方入口 `/Applications/Argus.app/Contents/MacOS/Argus --mcp-stdio`，尊重设置中的MCP开关。完成initialize与tools/list后调用`get_token_usage`，读取`structuredContent.summary`，`include_records:false`、`granularity:none`、`timezone_offset_minutes:480`。今日范围为UTC+8当天00:00至次日00:00；本周范围为周一00:00至今日结束，读取查询时已记录的数据。校验服务返回的起止时间，防止全历史/错误日期混入。

输入+输出必须等于total；缓存命中属于输入，不能再次相加。`reported_cost_usd:null`保持未知，未记录费用的调用数独立显示。当前文献库的汇总只计一次，不把by_provider拆分再加到总数。周统计失败不丢弃成功的今日统计；换周后旧周值不再显示。

可用`EPAPER_ARGUS`或本机私密设置`argus_executable`指定绝对可执行路径；固定文献库可继承官方`ARGUS_LIBRARY`，默认最近打开的文献库。每5分钟同步时查询一次日/周汇总，不读取库数据库、逐条文本或服务密钥。本周字段只向协商了`weekly_tokens:true`的固件发送。

2026-10-21真实MCP核对：当前库今日0、本周40,384,230 Token（起始2026-10-19）。数值随库和新调用变化；今日0已由用户确认正常，不代表整台Mac或全部文献库用量。

## Claude Code

[Claude 官方状态栏文档](https://code.claude.com/docs/en/statusline) 公开了 `rate_limits.five_hour`、`seven_day`，字段为 `used_percentage` 与 `resets_at`；这些对象仅在官方客户端和套餐满足条件时出现，每个窗口都可能独立缺失，过期后丢弃。

`context_window.total_input_tokens`、`total_output_tokens` 是当前上下文/最近响应的 Token，输入包括缓存读写，不是账户累计、周/月消费。当前方案不推算账户历史用量，不把额外花费当作订阅额度。

Rust `--claude-statusline` 从标准输入接收官方 JSON，只保存白名单数字与时间，写入本项目 `3Inch7EPaper/.local/.claude-statusline.json`（600）；不保存提示词、路径、会话 ID 或原始输入。App 的启用按钮在用户已有 `~/.claude/settings.json` 添加状态栏命令，保留其他设置，发现已存在状态栏时拒绝覆盖。

本机原无状态栏，现已接入，但 PATH 尚未检测到 Claude Code 命令；等待官方 Code 会话提供真实数据。没有读取 OAuth、Keychain 或 Cookie，也没有请求未公开的 HTTP 用量接口。原 Python 兼容发送器也已改为只读取这份官方状态栏缓存，不再调用私有端点。未用用户真实 Claude 会话验证，不把合成测试样例当成真实用量。

## DeepSeek

[官方余额接口](https://api-docs.deepseek.com/api/get-user-balance/)：

```text
GET https://api.deepseek.com/user/balance
Authorization: Bearer <本机 DeepSeek API Key>
```

Key 只在 Mac 环境变量 `DEEPSEEK_API_KEY` 或权限 600 的 `3Inch7EPaper/.local/.ble-settings.json` 配置。优先显示 `balance_infos` 中的 CNY，否则 USD 的 `total_balance`。余额含赠金和充值余额，不等于月花费；官方没有周/月套餐比例，当前不伪造。未配置本用户 Key，尚无真实查询验证。

## 频率与安全边界

按0.2.19起用户确认的设置，默认每30秒读取并下发用量，Codex缓存30秒；手动Codex刷新绕过成功/错误缓存。轮询间隔从一次全部读取完成后计算，不保证外部请求在30秒内完成；这也不是官方日桶生成周期。DeepSeek余额最短300秒，HTTP 429后至少900秒。Claude读取本机官方状态栏缓存，不额外联网查询。HTTPS校验证书与主机名，不携带Key跟随重定向。

蓝牙只发送白名单数值字段；新增 d/ld 官方日统计可用于墨水屏同日已知合计，累计与上下文 Token 不混入。失败状态保留已有成功数据并标记旧数据；缺少周/月窗口保持未提供。板子自身重启后等待新快照。Mac 睡眠或退出 App 时不会继续联网查询。

旧板上独立 HTTPS 与设备授权代码是此前方案的历史实现，包含客户端兼容路径，当前数据模式不启用；**按照用户最新要求，不得启用这些未公开用量路径，也不要再向板子导入账户凭据**。若将来需要独立联网，先找到官方支持的数据入口，再另行实现和验证。


## 0.2.15 Argus模型与供应商分类

继续使用同一次官方get_token_usage日汇总中的by_model/by_provider，不另读数据库、配置、逐条records或发起模型查询；include_records:false仍保持，日/周仍按原自动刷新设置读取。每组校验非负整数、类别唯一与合计等于当天总数，异常分类只标为未提供，不丢失有效日汇总。缓存比例=cache_hit_tokens/input_tokens，缓存仍已含输入，不叠加进总Token。

屏幕每组最多3行，超过3类显示前2类+其他合计并保留真实类别数。当前Argus官方MCP只给供应商UUID，没有显示名称工具；使用短ID区分，不按模型名猜供应商，也不绕过MCP读取供应商配置。仅传入有界ASCII标签和Token计数，URL、控制字符和疑似密钥标签不转发。模型与供应商是同一日总量的两个分类视图，不能相加成额外消费。

## 0.2.18 DeepSeek Harness（官方本机记录、多 Mac 汇总）

- HomeCraft 自动只读当前用户的 `~/.dsh/sessions`，只打开官方规范的 `session.vN.jsonl[.zstd]`。支持 v2–v4；同一会话只选择最高版本，不同时加迁移前后的文件。Zstandard 解码静态内置在 App，不依赖 Node、Python、CLI 或额外插件。自定义目录可用 `HOMECRAFT_HARNESS_SESSIONS` 临时覆盖。
- 依据官方公开持久化文件规范及 tokenUsage 投影规则提取供应商报告的数字；不是上下文估算或云端账户账单。输入为未缓存输入 + cacheRead + cacheWrite；输出不重复加 reasoning。最终 settlement 替换同一 attempt 的流式用量，retry-started 新计一次；继承/分叉的 seed 前缀不重复计。按事件时间在 UTC+8 划日、周一划周。只缓存数字，未知格式/损坏记录显示读取失败，不伪装 0。
- 官方依据（核对源码提交 `687f282e8625c6b8d11ff58211a90524be2e4317`）：[JSONL 文件规范](https://github.com/deepseek-ai/deepseek-harness/blob/687f282e8625c6b8d11ff58211a90524be2e4317/packages/session/session-persistence-jsonl/README.md)、[Token 计数语义](https://github.com/deepseek-ai/deepseek-harness/blob/687f282e8625c6b8d11ff58211a90524be2e4317/packages/llm/token-meter/README.md)、[四个互斥计数桶](https://github.com/deepseek-ai/deepseek-harness/blob/687f282e8625c6b8d11ff58211a90524be2e4317/packages/llm/llm/src/types.ts)。本机安装版 0.2.1-rc.2，真实记录为 v4。
- HomeCraft 为数据和配置中枢：600 权限 `3Inch7EPaper/.local/.harness-origin.json` 保存应用域隔离的机器/用户标识摘要与可读名称，`3Inch7EPaper/.local/.harness-devices.json` 原子保存最多 8 台 Mac 的最近已同步数字快照。机器原始 UUID 不保存、不传输；同一 Mac 同一用户换 App 安装路径不重复计。安装包不含这些文件，勿复制另一台 Mac 的统计身份。
- 用户要求**主页合并显示，详情按设备分开**。App 合并 ID 相同的最新快照并覆盖，发往显示端时仍逐台发送，绝不把合计冒充本机数重新上传。另一台 Mac 的用量只有该机 HomeCraft 运行并同步后才会更新。各 App 可通过已认证墨水屏的少量同步缓存交换已知快照，并在本机长期保存；这不是 Harness 云同步。断网/未连接时可查看 HomeCraft 已存明细与最后同步时间，不声称远端实时消费。
- 固定 profile 4，名称统一 **DeepSeek Harness**；保留 profile 2 的 DeepSeek 官方 API 余额，两者不混用。普通日统计只进当天已知 Token 合计，不冒充 5 小时或周额度。默认跟随 App 的 5 分钟刷新配置。
- 从未打开 `.dsh/.credentials.yaml`，不读取账户凭据、浏览器令牌或调用未公开 HTTP 接口，也不安装/修改 Harness 插件。


## 0.2.21 Codex账号来源

除额度与Token外，HomeCraft只新增官方app-server的`account/read`（`refreshToken:false`），读取前后确认当前身份一致。ChatGPT邮箱仅在内存解析，显示前1–3字符加***，持久化仅保存脱敏标签、SHA256账号键和来源设备；不读取登录文件或解析JWT。没有有效邮箱时按Mac标记。iCloud Drive跨Mac记录由相同账号键替换为最新观测，不重复加总账户级用量；不同账号可在“iCloud与来源”查看，未知身份不盲目与其他来源相加。官方返回可缺失邮箱，不能推测补全。完整同步边界见`mac-app.md`的0.2.21节。


## 0.2.29 本月Token

所有日期按UTC+8，月从1日00:00起，不是滚动30天。Codex对原account/usage/read已返回日桶取本月到当天的有效记录，同日取最大，不把缺失补0；与日／周一样只代表官方已知范围，不使用累计差值、上下文、额度或费用。没有新增Codex方法。

Argus在日、周之后用原官方get_token_usage查询本月1日至明日00:00，include_records:false/granularity:none/timezone_offset_minutes:480，严格确认返回范围与input+output=total/cache<=input。月读取5秒独立超时，不影响当天或本周成功值，日／周／月共用现有刷新节奏。MCP开关与当前文献库仍生效，不读取数据库或逐条records。

DeepSeek Harness复用已解析官方日志的逐日合并结果，仅合当月到当天；不增加文件种类或账户查询。多台按稳定ID覆盖后合计，旧设备未提供月数则未知，不当作0。DeepSeek API余额仍是独立provider，不用余额推Token。真实读取数字与时间在3Inch7EPaper/artifacts/monthly-0229-real-usage.json，缺少今日Codex不妨碍已有本月日桶合计。


## 0.2.33 昨日Token与周额度百分比

UTC+8昨日Token使用同次官方usage/read的精确昨日桶，最新桶为今天或更老日期都不能替代。昨日周额度百分比使用已有本机数值缓存中、同一已验证账号、昨日最后有效官方rateLimits快照的已用比例；只接受10080分钟周窗口，过期快照不使用。它描述昨天最后记录的周额度状态，不是昨天新增消耗。缺值为未知，不读OAuth／私有HTTP、不新增查询、不从Token反推额度。源数据在账户读取前后身份一致时才附加，见codex-yesterday-0233-validation.json。

## 0.3.0 build 48 中枢汇总（2026-11-15）

HomeCraft 统一解析本机原始读数、已同步摘要和 Harness 来源缓存；窗口、首页及显示端下发共用这一结果。Codex 同账号按周期选最新有效官方观测，不把缺字段覆盖成未知；当日同账号摘要合并也保留已知同周期值，不从差值推算。Harness 按稳定来源 ID 覆盖，派生合计再次解析不会覆盖单机来源，本机没有 Harness 也能看到已同步来源。

用户确认两台 Mac 的 Argus 共用同一个同步文献库。因此它是同一份库级汇总的多个观测，不具备按 Mac 相加的依据；采用最新有效观测，日、周、月不重复相加，各机来源保留供核对。不同库的自动识别仍未实现，不能把本策略称为任意多个库的总消费。

云同步仍需各机运行新版并选择同一个 iCloud 文件夹。旧快照保留原日期，不作今日数据；来源页显示最后更新时间，文件写入不代表已送达另一台实体 Mac。登录自启动为本机显式选择，不进入云端数值摘要。


## 2026-11-18 Harness 同步对话去重

用户确认 Harness 对话会跨 Mac 同步，先前按机器 ID 分别覆盖后求和的口径不再适用。以下为当前源码实现；上文 0.2.18 / build 48 的多 Mac 求和保留为历史，不能作为新口径。

- 按 `SHA-256(应用域 + session ID + turn + step + retry 序号)` 标记一次实际 attempt；不使用机器名、目录、文件修改时间或 Token 大小识别消费。同步副本保留这些会话与事件身份是去重前提；重建 session ID 的第三方导入尚未验证，不按文字相似度猜测归属。
- 同一日志只读最高支持版本，支持的格式迁移不把物理行号加入调用身份；同 ID 的多路径副本取调用并集。`llm/retry-started` 增加 attempt，最终 message 替换该 attempt 的中间 usage；seed 继承前缀排除。原有四桶和 UTC+8 自然日/周/月口径不变。
- 同一调用、结算状态及逻辑修订的 Token 数值或 UTC+8 日期不同视为冲突，整个 Harness 合计保持未知、提示待核对。2026-11-23 修正：数值与日期相同、仅毫秒时间不同的同步副本只计一次，采用其中最早时间使合并顺序和重复导入不影响结果；不凭后上传时间或较大用量消除真实冲突。真实重试和不同会话即使数值相同，也分别计入。
- 每台只发布自己读取到的调用观测，跨机先取并集再汇总；反复导入、冲突副本、缓存重载和重复展示都不增加已有调用。设备详情中的各机数字含同步副本，只供观测核对，不是可相加的设备消费。
- iCloud 新增 `AI用量/Harness/<来源摘要>.json`，version 4 独立旁路文件，白名单只有来源标签、脱敏调用键、事件毫秒时间、逻辑修订、结算状态及数字。最多 8 个来源、每来源 100,000 次当前月/周所需调用；每文件 24 MiB、总读取预算 192 MiB。普通 AI 用量文件仍使用旧格式，不混入新字段或新 Harness 总数，其他服务继续兼容旧 App。没有会话正文、提示词、模型请求或账号凭据。
- 新的本机 `.harness-devices-v2.json`、`.cloud-cache-v3.json` 原子写入且权限 600；兼容读取旧缓存，但旧版只有总数的 Harness 来源只展示、不求和。旧文件不删除；新本机日志扫描会重建调用明细。旧数据没有当前日期观测时，不能据此把今日写成 0。
- Mac App 完成全部用量去重、汇总、跨机同步和更新；BLE 只发送最终 profile 数值和 `totals:1`，不传机器来源/调用明细、不从屏幕读回来源缓存。`harness_aggregate:1` 只表示能接收 Harness 最终值，不表示屏幕负责聚合。
- 两款接收端均增加 `display_snapshots:1`：屏幕只整体替换显示快照，缺日期/字段保持未知。总览无论开机后是否收到过合计，都只显示 App 提供的匹配周期值，不能把 provider 包自行相加。
- 小屏已删除 `harness_stats` 模块、机器来源表、来源回读、旧 NVS 缓存加载/写入和分机展示；旧 NVS 键保留未动但不再读取。两款屏幕拒绝 `o` 来源扩展；退役的 `op:harness` 在小屏明确返回不支持。输入/输出、调用数等均为 Mac 发送的最终显示字段。
- 未收到最终值显示未知；新快照显式未知/失败时替换旧值，不在屏幕修补或推算。旧固件缺少直显能力时，App 在设备状态和 Harness 页提示未完成同步，不能静默保留旧专页假称成功。
- 本轮是清理旧固件里违反职责的统计实现；配套接收端只负责直显，不迁入任何去重或消费更新规则。当前候选 Mac 0.3.3/build 51、小屏 0.2.39、E1002 0.3.1，尚未安装/部署。

验证范围：临时合成日志/双来源/文件交换与缓存重启、Rust 回归、C ASan/UBSan。真实两台 Mac 使用新版后的同步收敛、当前用户同步方式是否保留会话身份，以及实屏结果仍待验证；候选 App 和离线固件构建不等于已安装/部署。所有参与统计的 Mac 都应更新。
