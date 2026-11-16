# E1002 参考资料

Git 只保存本说明和 [sources.lock.json](sources.lock.json)。上游仓库副本、PDF、提取文本、工具发行索引与压缩包留在本机，不重复上传。

恢复时按来源锁中的 `repository` 克隆到对应 `path`，再检出准确的 `commit`；例如从本板目录执行：

```sh
git clone https://github.com/Seeed-Projects/Seeed_GxEPD2 reference/Seeed_GxEPD2
git -C reference/Seeed_GxEPD2 checkout --detach 1100ea37c16b910fd79152f4250c13d802b9c20b
git clone https://github.com/Seeed-Projects/OSHW-reTerminal-Series-E-D reference/OSHW-reTerminal-Series-E-D
git -C reference/OSHW-reTerminal-Series-E-D checkout --detach 8d97d8912987c07529287629f7e73039bb0e6384
```

原理图和芯片资料按来源锁中 `schematic`、`chip_datasheet` 的官方地址下载到 `file` 指定的位置，并核对 `sha256`。Arduino CLI、板包与库版本见 [环境锁](../config/environment.lock.json) 和 [开发说明](../docs/development-environment.md)。需要 Apple 芯片 ctags 兼容补丁时，先获取来源锁中的 ctags 固定提交，再使用已入库的 `config/patches/arduino-ctags-macos.patch` 和 `tools/repair_ctags.py`。

当前原生 IDF 固件所需的本地显示移植与 cJSON 源码在 `../components/`，不依赖将这些参考仓库副本提交到 Git。原始许可与来源说明见 [第三方声明](../THIRD_PARTY.md)。
