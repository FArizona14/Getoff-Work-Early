# 早点下班 · 本地可用版

对应根目录技术方案「阶段 2：本地可用闭环」。首发 Windows x64，Tauri 2 + React + TypeScript + Rust + SQLite。

## 运行

安装包与便携可执行文件见 `releases/`（首次构建完成后生成）。便携版直接运行，不需要安装；仍将数据保存在当前 Windows 用户的应用数据目录。安装包按当前用户安装，不申请管理员权限；尚未签名，Windows 可能显示未知发布者。

- 主面板关闭后隐藏到托盘，悬浮窗与计时继续运行。
- 悬浮窗顶部可拖动，接近工作区边缘时吸附，停止移动后保存位置；托盘可显示或隐藏悬浮窗。
- 默认 `Ctrl+Shift+Space` 采集选中文字，快捷键冲突时可在主面板修改。
- 收集前只支持原剪贴板为空或纯文本；图片、文件、富文本等复杂格式会跳过复制，提示手动粘贴。
- 快捷卡保存才写库，取消或关闭不提前保存。原剪贴板在采集后立即尝试恢复；恢复时在独占锁内比较序列号，发现新内容就保留新内容。
- 上班按时间点计算，满 9 小时显示加班；下班冻结并锁定开始打卡的工作日，不能同日重开。跨午夜仍归属开始日。
- 点击「退出应用」或托盘菜单「退出应用」真正退出。

当前版本已迁移确认的主视觉，并接入任务、工时、薪资估算、隐私、应用锁和备份恢复。AI 与日报 / 周报生成属于阶段 3。

## 本地开发

需要 Node.js 22.12+、Rust stable MSVC、Visual Studio C++ 构建工具、Windows SDK、WebView2。

```powershell
cd desktop
npm ci
./scripts/build.ps1 -Dev
./scripts/build.ps1 -Test
./scripts/build.ps1
```

脚本自动识别工作区 `.tools` 中的 Rust 工具链，也可使用系统已有 Rust；不修改系统 PATH。前端单独检查：`npm run build`。发布命令：`npm run desktop:build`（内置锁定依赖参数）。

`package-lock.json` 和 `src-tauri/Cargo.lock` 必须一起维护。不要删除 lockfile 后直接发布。

## 数据与权限

正式数据通过 Tauri 路径 API 保存到 `%APPDATA%/com.zaodianxiaban.app/data/`，不跟随安装目录。

- `app.db`：SQLite、WAL、外键、版本化事务迁移；旧小样收集项会自动迁入任务收集箱。
- `user-key.dpapi`：当前 Windows 用户保护的 AES 数据密钥。不要单独删除或分享它；跨账户不能直接解密。
- `before-migration-v1.db` / `before-migration-v2.db`：结构升级前的一致性快照。
- `backups/`：用户创建的本机滚动备份和密码加密便携备份。

收集标题使用 AES-256-GCM，格式为版本号 + 随机 12 字节 nonce + 带认证标签的密文。计时、设置等元数据为明文。前端无任意 SQL、文件系统或 shell 接口；新增、设置和计时命令校验调用窗口。悬浮窗只读取摘要，不能读取收集正文。事件不携带正文。

卸载默认保留用户数据。跨电脑请使用应用内创建的 `.eowbak` 加密备份，不要只复制 `app.db`。应用锁和便携备份密码无开发者后门，请妥善保管。

## 验证记录

见 [阶段1验收记录](docs/阶段1验收记录.md)。已测与待测分开记录；未完成测试的 Office、WPS 等软件不视为兼容。

## 官方参考

- [Tauri 2](https://v2.tauri.app/)
- [全局快捷键](https://v2.tauri.app/plugin/global-shortcut/)
- [Windows 安装包](https://v2.tauri.app/distribute/windows-installer/)
- [Windows CryptProtectData](https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata)

许可证：Apache-2.0。
