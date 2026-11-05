# 早点下班（Get off Work Early）

一款本地优先的 Windows 办公助手桌面应用，帮助记录待办和工作时间，并在满九小时后明确显示加班状态。

当前版本为 `0.1.0-alpha.1` 桌面技术小样，已经验证多窗口、系统托盘、单实例、全局快捷键、划词收集、SQLite 持久化和 Windows 当前用户加密。

![早点下班桌面原型](原型/v0.1/redesign-desktop.png)

## 下载

请从 [GitHub Releases](https://github.com/FArizona14/Getoff-Work-Early/releases) 下载：

- `早点下班-0.1.0-alpha.1-x64-setup.exe`：Windows x64 当前用户安装包。
- `早点下班-0.1.0-alpha.1-x64.exe`：免安装运行版。
- `SHA256SUMS.txt`：文件完整性校验值。

这是未签名的内测版本，Windows 可能显示“未知发布者”或 SmartScreen 提示。当前只建议用于技术验证，不要保存唯一一份正式工作资料。

## 已实现

- 主面板、置顶悬浮窗和快捷输入卡三个独立窗口。
- 关闭主面板后继续驻留托盘并计时；重复启动激活已有实例。
- 悬浮窗拖动、边缘吸附、位置恢复与可见工作区校正。
- 上班、九小时加班、下班锁定、跨午夜归属和重启恢复。
- `Ctrl+Shift+Space` 主动采集选中文字；复杂剪贴板格式安全降级为手动粘贴。
- SQLite 版本迁移、迁移前备份、AES-256-GCM 字段加密和 Windows DPAPI 密钥保护。
- Windows x64 NSIS 当前用户安装包。

Office、WPS、微信等软件的逐版本划词兼容，多显示器缩放，初装/覆盖升级/卸载恢复等仍需专项测试。详见 [阶段 1 验收记录](desktop/docs/阶段1验收记录.md)。

## 开发

桌面应用位于 [`desktop/`](desktop/)，技术栈为 Tauri 2、React、TypeScript、Rust 和 SQLite。

环境要求：Node.js 22.12+、Rust stable MSVC、Visual Studio C++ 构建工具、Windows SDK 和 WebView2。

```powershell
cd desktop
npm ci
./scripts/build.ps1 -Test
./scripts/build.ps1 -Dev
```

发布构建：

```powershell
cd desktop
./scripts/build.ps1
```

完整开发与数据说明见 [桌面工程 README](desktop/README.md)。产品需求、视觉规范、技术方案和浏览器原型也保留在仓库中，方便继续开发。

## 数据与隐私

业务数据默认保存在 `%APPDATA%/com.zaodianxiaban.app/data/`。收集标题使用 AES-256-GCM 加密，数据密钥由当前 Windows 用户的 DPAPI 保护。应用不持续监听剪贴板，也不在用户确认保存前写入采集内容。

阶段 1 尚未实现应用锁、完整备份恢复、云同步、AI 报告和自动更新。

## 许可证

[Apache License 2.0](LICENSE)
