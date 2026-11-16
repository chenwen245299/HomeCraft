# 工作区与设备隔离

2026-10-26按用户要求，工作区只放software与各板独立目录。目录整理时为software和3Inch7EPaper，随后新加reTerminalE1002；新板不是全局配置的替代。根README/AGENTS/TASKS是项目说明文件，不保存板级配置。第三方来源分别保存在software/THIRD_PARTY.md和3Inch7EPaper/THIRD_PARTY.md。

## 职责

HomeCraft是软件协议中心。software保存Mac源码、公共协议、共享素材、工具、软件文档、测试与安装包；3Inch7EPaper只保存当前Waveshare板子的固件、引脚、SDK/分区、驱动、测试、固件包和资料。其他板子以后各建独立同级目录。

公共页面常量来自software/protocol/device-v1.json，生成Mac代码和板端头文件，已有线协议和编号不变。每款硬件自己的board.json说明适配类型，协议中心不保存GPIO或供电参数。

## 私有资料

当前板子的配对、原App设置、数值缓存、单实例锁和历史授权资料已整体移至3Inch7EPaper/.local，目录700，文件600；私有恢复备份移至本板backups。没有复制绑定到App/DMG，也没有重新配对、轮换密钥或导入账户登录令牌。Mac的既有.data-location入口已指向这个目录。

本地开发由software/project.json选择板子，Rust与Python解析到其.local；选择不得越出工作区，不会默默在根目录写缓存。EPAPER_PROJECT可明确指定隔离测试的数据目录，若指向工作区则遵循项目选择。独立安装的新Mac继续使用自己的App私有资料，不携带开发机路径或凭据。

## 构建与历史

ESP-IDF从3Inch7EPaper目录构建，Cargo/打包从software目录执行。旧build绝对路径缓存归档在本板.cache/pre-layout-build-20261026，新build为离线重编译产物；本次没有烧录，不用其新哈希覆盖已部署固件。归档固件原文件哈希逐一保留。历史验证文件原来是根artifacts，现归入本板artifacts；当前软件安装包验证在software/artifacts。旧日志中的旧路径代表迁移前现场，不重写日志内容。

Python BLE环境移动到software/.ble-venv并修复入口。旧Conda环境保存于software/.cache/pre-layout-conda，它包含历史绝对路径，不直接激活；需使用时按software/environment.yml在software/.conda重建。当前构建使用EIM自身IDF解释器。

## 增加新硬件

1. 在工作区创建独立板型目录，放该板board.json、固件工程、管脚/SDK/分区与驱动。
2. 在HomeCraft增加相应协议/传输适配，能力不足时禁用操作，不假设与当前ESP32板相同。
3. 将software/project.json指向该目录；该板的.local、backups、build和测试完全独立，不复制原板绑定。
4. 在该板目录记录构建、实机测试和部署事实。编译成功、协议登记与真实硬件验证分别记录。


## 文档分层

根AGENTS只放工作区通用规则，根TASKS只作跨项目索引，根README只作总览。software和每款硬件分别维护AGENTS、README、TASKS和THIRD_PARTY。管脚/SDK/显示/刷机约束不在根规则中规定，新硬件不能默认沿用旧板。

原混合根上下文完整归档至 `3Inch7EPaper/docs/workspace-context-before-split-20261026.md`，原任务清单归档至 `3Inch7EPaper/docs/task-history-before-split-20261026.md`。当前待办依据后来明确记录整理，首次屏幕码认证已完成；旧“候选未烧录/今晚待测”文字仅为历史。
