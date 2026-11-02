# PhotoCraft AI Beta 0.19.0-ai-beta.2

本版本为待验证、未发布的 0.19.0 整合包；公开下载仍是先前的 beta.1。

这是基于 PhotoCraft 的非官方社区测试版，由 BLEZZ0102 分发。它包含 Codex 订阅图片编辑和
Gemini、豆包 / Seedream、FLUX API 接入。编辑结果作为可撤销的新图层加入文档。
This is an unofficial community build of PhotoCraft, distributed by BLEZZ0102.
It is not an official ArtCraft release. PhotoCraft and its original contributors retain their credits.

## 安装与使用 / Installation

- 适用于 Windows 10/11 64 位。下载 `.msi` 并双击安装，无需 Rust、Visual Studio 或 .NET。
- 默认只为当前用户安装到 `%LOCALAPPDATA%\Programs\PhotoCraftAI`，开始菜单名称为
  **PhotoCraft AI Beta**。官方 PhotoCraft 可以同时保留；此安装包不会注册或接管文件默认打开方式。
- 设置位于 `%APPDATA%\PhotoCraftAI`，恢复文件位于 `%LOCALAPPDATA%\PhotoCraftAI\Recovery`。
  不会复制官方版的设置、恢复文件、API 密钥或本项目的开发配置。
- 便携 ZIP 解压后运行 `photocraft.exe`，数据保存在程序旁的 `PhotoCraftData` 中。
- 本测试包尚未进行代码签名，Windows 可能显示未知发布者。请核对发布仓库与 SHA256，
  遵循设备的安全策略；不要关闭系统安全功能来安装。

Windows x64, per-user installation. Use the MSI, or extract the portable ZIP and run
`photocraft.exe`. No development tools are required. The unsigned package is independent
of official PhotoCraft. The uninstaller removes program files and leaves user settings
and documents in place. It does not sign out a separately installed Codex application.

## AI 编辑 / AI editing

1. 打开或新建文档，点击左侧星光按钮或“编辑 → AI 编辑”。
2. **Codex**：先自行安装完整的 Codex 桌面运行时，再在 AI 编辑中检查连接或使用 ChatGPT 登录。
   必须由你的账号支持相应图片能力。安装包不附带 Codex、ChatGPT 账号、订阅或任何密钥。
3. **API**：选择 Gemini、豆包 / Seedream 或 FLUX，填写自己的模型、服务地址与密钥，
   点击保存，再输入指令确认。调用费用由服务商收取；保存配置不会生成图片。
4. 等待结果，成功后可撤销新图层。审核拒绝会显示对应提示；失败不会添加图层。

Install the complete Codex runtime separately for subscription-based editing and sign in
with your own ChatGPT account. Alternatively configure your own supported image API.
Provider usage can incur charges. There is no automatic fallback to a paid API or automatic
generation retry. No models, credentials, personal images or generated results are bundled.

Codex 和豆包已在开发版中由用户实测；Gemini、FLUX 通过协议模拟测试，仍需真实账号验收。
这属于 Beta：请保留文档备份。新版 AI 安装包从此项目下载，不使用官方安装包覆盖。

## 卸载、来源与许可证 / Uninstall and source

通过 Windows“设置 → 应用 → 已安装的应用”卸载 PhotoCraft AI Beta。
用户设置、恢复文件、个人文档和自行安装的 Codex 保留不动。

- AI project: https://github.com/BLEZZ0102/photocraft-ai
- AI downloads: https://github.com/BLEZZ0102/photocraft-ai/releases
- Upstream PhotoCraft: https://github.com/storytold/photocraft
- MIT OR Apache-2.0; see the bundled LICENSE files, NOTICE and ATTRIBUTION.md.
- The app icon is separately MIT OR Apache-2.0 licensed in `LICENSE-app-icon.txt`.
  Restricted ArtCraft brand artwork from `docs/brand/` is not included in the installer.
