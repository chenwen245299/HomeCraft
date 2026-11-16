# 官方应用蓝牙配网

记录日期：2026-10-20。用户选择先完成 Wi-Fi 配置，再处理 Codex 授权；此前设备码已超时，不应继续使用。

后续调整：手机官方配网已成功，设备获得 IP 并完成 SNTP；用户反馈校园网访问受限，现改为 **Mac 蓝牙发送额度**，见 [当前数据模式](../../software/protocol/ble-data.md)。开机默认提供数据接收，手机配网仅通过 BOOT 长按 3 秒开启。下文的 Wi-Fi 操作作为备用，不再自动先配 Wi-Fi。

## 操作

安装 Espressif 的 **ESP BLE Provisioning**：[iPhone](https://apps.apple.com/app/id1473590141)、[Android](https://play.google.com/store/apps/details?id=com.espressif.provble)。不是 ESP RainMaker、BluFi 或 ESP SoftAP Provisioning。

在任意页面长按 BOOT 约 3 秒，临时切换到手机配网。应用内扫描屏幕二维码，设备名为 `PROV_` 加 MAC 后六位，选择 2.4 GHz Wi-Fi 并输入密码。配对用户名为 `wifiprov`，二维码自动提供用户名、随机配对码、`transport=ble`、`security=2`。手机不需要加入设备热点。应用如手工添加，使用 Security 2 和屏幕上的配对信息，不要选择 Security 0。

配网窗口 10 分钟有效，每次开启生成新的 128 位随机配对秘密与 SRP salt/verifier。成功后由官方管理器在约 30 秒内结束；运行时再次长按 BOOT 可以关闭。蓝牙内存不做永久释放，因此本次上电可以再次开启配网。

提交新网络后先试连，得到路由器 IP 才写入项目配置。失败时保留旧 SSID/密码，复位官方管理器的失败状态并恢复旧网络，可再次在应用提交。45 秒仍未完成试连则退出本轮并恢复旧网络，需要长按 BOOT 重新开启。配对认证失败累计五次会关闭窗口，需要实体操作重开。Wi-Fi 连接成功、网络校时成功、账户查询成功是三个不同状态。

网络页三击 BOOT 保留网页高级设置入口，用于后续账户和查询参数；此时关闭 BLE 配网，启用临时 WPA2 热点。网页沿用屏幕配网码，限定从 AP 接口访问；每次窗口轮换热点密码和网页码，始终在 10 分钟后关闭，包括尚未配置 SSID 时。USB 一次性导入账户仍仅在这个网页配网窗口接受。官方应用当前仅配置 Wi-Fi，不提供本项目三家账户的自定义表单。

## 协议与源代码

- [官方 network_provisioning 1.3.1](https://components.espressif.com/components/espressif/network_provisioning/versions/1.3.1)；使用 `network_prov_scheme_ble`、NimBLE 和 `NETWORK_PROV_SECURITY_2`。
- [官方 Protocomm](https://docs.espressif.com/projects/esp-idf/en/latest/esp32s3/api-reference/provisioning/protocomm.html)：Security 2 使用 SRP6a 认证/密钥协商和 AES-GCM；只启用 Security 2，不回退到 Security 0/1。
- [官方 Android 客户端](https://github.com/espressif/esp-idf-provisioning-android) 的二维码解析确认支持 `username`、`pop`、`transport`、`security`；采用其默认 Wi-Fi 用户名 `wifiprov`，兼容手工添加。iOS 商店版本记录包含 Security 2 的 IV 兼容修正，仍需用用户实际安装版本验证。
- `3Inch7EPaper/main/network.c` 生成临时凭据，处理官方事件和应用配置；`3Inch7EPaper/main/pairing_qr.c` 直接使用 Nayuki 编码器，**不调用会记录完整载荷的 `esp_qrcode_generate()` 包装函数**。

官方组件放在 `3Inch7EPaper/components/network_provisioning`，通过 manifest 的 `override_path` 固定本地副本，来自 registry 1.3.1，原仓库提交 `225a1c974215bdf6eeed990c5f9ffe45f883b171`。只修改 `src/manager.c`：Wi-Fi 存储始终使用 RAM；打开配网时不清空或断开现有 STA；忽略提交新配置前主动断开产生的 ASSOC_LEAVE；去除包括部分掩码密码在内的调试凭据日志。应用在收到新配置时才主动断开旧 STA，并只在验证成功后写入专用 `dashboard` NVS。未修改官方 SRP、AES-GCM 或手机协议。

## 安全边界

官方加密保护手机到设备的配置交换，随机屏幕配对秘密用于认证；知道本轮二维码的人仍有本轮配置权限。限时开放和实体按键控制减少暴露时间。已存 Wi-Fi 密码与账户密钥不通过状态接口读回，也不输出到串口日志。

当前 Flash/NVS 仍未加密，BLE Security 2 不提供防拆机或本地 Flash 读取保护。未烧写 eFuse、未锁定调试端口；不把官方应用等同于整个设备已完成所有安全加固。

## 已验证与待验证

- ESP-IDF v6.1 编译通过；应用 1,504,608 字节，已烧录并通过 Flash 哈希校验，保留配置分区。
- 11 项 Python 测试、C 按键/方向测试通过；新增 BOOT 3 秒阈值检查。168 个界面状态/页面/方向组合通过 ASan/UBSan 边界检查；横竖屏二维码用独立 ZXing 解码器解析，全部官方字段一致。
- 上板 USB 诊断显示已存网络，但尚未连接、尚未校时，最近断线原因 `210`，即 `WIFI_REASON_NO_AP_FOUND_W_COMPATIBLE_SECURITY`。这表示没有找到安全设置兼容的 AP，不能单凭该原因认定密码错误或 5 GHz 网络；需通过应用扫描和重新配网确认。原先“等待校时”提示已改为区分未联网与校时中。
- 后续已确认蓝牙广播启动、真实手机配网成功并提交到 NVS，获得 IP `192.168.3.23`，SNTP 完成，蓝牙窗口自动关闭；用户随后反馈校园网访问受限。没有据此验证三家服务的板上 HTTPS。当前固件与核实状态见 `3Inch7EPaper/artifacts/firmware-validation.json`。
