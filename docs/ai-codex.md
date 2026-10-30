# PhotoCraft AI 本地原型

开发分支：`feat/ai-provider-codex`，起点为上游 main `5f9b439`。
当前是已在本机完成真实 Codex 编辑、后台新图层回填及撤销/重做验证的桌面原型，尚未发布 Alpha。

## 实现

- `photocraft-gen`（L4）的 `ImageProvider` 接口隔离服务、认证与传输；首个实现是 `CodexProvider`。
- `edit.ai.codex {"prompt":"编辑指令"}` 由 UI、CLI、控制通道和 MCP 共用。
- 编辑菜单的“AI 编辑…”打开指令表单；确认后使用现有后台任务。
- 工作线程将可见画布合成为独立文档，通过 ICC 转为 sRGB、8 位 PNG。不会传送隐藏图层、原文件名、EXIF 或 XMP。
- 临时目录只保存本次输入；其生命周期覆盖导出、请求及结果解码，随后自动清理。Codex 自己保存的结果由 Codex 管理。
- 从**原安装位置**启动完整 Codex 运行时，复用其现有 ChatGPT 登录；不复制 codex.exe，不读取或另存登录令牌。
- `initialize → initialized → account/read → thread/start → turn/start`，等待匹配线程的 `item/completed`，其中 `imageGeneration.status=completed` 且有 `savedPath`；整个 turn 成功后才接收图片。
- API-key 登录会失败；强制 `forced_login_method=chatgpt`、OpenAI provider，并从子进程环境移除 API key/base URL。没有 API 适配器或自动回退。
- 禁用 Codex shell 和网页搜索工具；交互工具请求会返回不支持。图片能力缺失时明确失败。
- 结果经 PNG 大小与宽高比检查、解码后，同宽高比但像素尺寸不同的图片通过现有 Bicubic 重采样适配原画布。随后转换为目标文档色彩/位深，新增一个顶层图层，并产生一个“AI Edit”撤销步骤。命令结果包含 `resizedToCanvas` 和原始输出尺寸。现有图层不变。
- 后台任务使用引擎的文档锁、快照校验及取消机制，取消/失败不会回填。

## 在 Windows 本机测试

先安装 Visual Studio C++ Build Tools（Desktop development with C++）和 Windows SDK（如果尚无）。在普通 PowerShell 中进入项目目录：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\ai-local.ps1 Setup
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\ai-local.ps1 Format
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\ai-local.ps1 Check
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\ai-local.ps1 Run -CodexPath '完整 Codex 安装目录中的 codex.exe'
```

Rust 工具链安装在项目 `.tools`，脚本只为当前进程配置 PATH。微软 C++ Build Tools 和 Windows SDK 按安装器的标准位置安装。上述执行策略设置仅影响该次 PowerShell 进程。
脚本默认关闭增量编译，以复用本次验证构建；可在调用前显式设置 `CARGO_INCREMENTAL=1` 覆盖。
脚本会尝试为下载工具复用 Windows 已有的系统代理，仅设置本次进程及子进程环境，不修改全局 Git 或 Windows 网络配置。
`Run` 使用独立实例，默认将原型配置与恢复数据保存在 `.tools/photocraft-profile`；显式设置的 `PHOTOCRAFT_CONFIG_DIR` 会保留。
需要手动运行 Cargo 时：

```powershell
$env:RUSTUP_HOME = "$PWD/.tools/rustup"
$env:CARGO_HOME = "$PWD/.tools/cargo"
$env:PATH = "$env:CARGO_HOME/bin;$env:PATH"
cargo fmt --all
```

打开或新建 1024×1024 RGB 图像，选择编辑 → AI 编辑…，输入“把天空改成日落，保留其余构图”，确认。
应出现 `AI Edit (Codex)` 新图层；检查隐藏它后原画布不变，Ctrl+Z 移除新图层，Ctrl+Shift+Z 恢复。
用状态栏取消操作，或退出 Codex 登录后再运行，均应明确失败而不添加图层。

可单独测试服务链路（会使用现有订阅额度，测试不会自动执行）：

```powershell
$env:PHOTOCRAFT_CODEX_PATH = '完整安装位置中的 codex.exe'
cargo run -p photocraft-gen --example codex_edit -- 'canvas.png' '把背景改成蓝色'
```

也可显式运行真实后台引擎闭环测试（普通测试及检查脚本不会自动触发该请求）：

```powershell
$env:PHOTOCRAFT_RUN_LIVE_CODEX = '1'
cargo test -p photocraft-engine --test ai_codex_live -- --ignored --nocapture
Remove-Item Env:PHOTOCRAFT_RUN_LIVE_CODEX
```

该测试创建合成画布，通过真实 Codex 后台任务新增图层，检查原图不变、尺寸适配、撤销与重做，并将结果输出到 `target/ai-codex-live-result.png`。

UI 视觉检查（生成后必须查看图片）：

```powershell
cargo run -p photocraft-ui-egui --profile test --example snapshot -- --out ai-dialog.png --open canvas.png --script '[["engine.execute",{"command":"prefs.set","params":{"path":"interface.language","value":"zh-hans"}}],["ui.menu.invoke",{"id":"edit.ai.codex"}]]'
```

## 当前边界与验证状态

- 仅桌面启用。输入与输出最大 16 MP；生成 PNG 最大 64 MiB。同宽高比输出会适配原画布尺寸，允许不超过 1% 的像素舍入误差；明显改变宽高比的输出会明确拒绝，避免拉伸或裁切。
- PNG 交换为 8 位 sRGB，回填转换回目标模式/位深；这不会恢复模型输出中已丢失的高动态范围信息。
- 未实现选择区/遮罩、变体、ComfyUI、付费 API、独立登录 UI、可配置模型和运行时自动发现。
- 默认从 PATH 启动 `codex`；可用 `PHOTOCRAFT_CODEX_PATH` 指定完整运行时内的可执行文件。
- 超时为 10 分钟，取消会关闭本次私有 App Server。外部服务已接受的生成任务是否继续由服务决定。
- 完整工作区已通过 provider 两项、引擎 AI 九项、UI 两项及 `panic_hunt` 对抗参数检查；覆盖登录类型、完成事件过滤、多模式/位深、原图保留、新图层与撤销/重做、失败、取消、尺寸适配和菜单表单。菜单放在“内容识别填充”之后，通过现有扩展机制加入，不改变 Photoshop 对照目录。
- 最终分层检查通过（32 crates），WebAssembly 检查通过（25 crates 及 HEIF 可选功能），provider 的 wasm 示例也通过；菜单覆盖保持 631/631。
- 续修已解决初轮的 21 项缺字失败：原有首帧字体 provider 在没有内嵌 craft-fonts 时被跳过，现允许桌面构建直接使用已安装的系统字体；Web 保留原路径。加入不依赖机器字体的严格首帧回归测试，不放宽缺字策略，不添加字体文件。UI 单元测试复跑结果为 4202 项通过、0 项失败、18 项忽略；22 项语言切换测试全部通过。
- 同时修正两项测试假设：字体搜索框的光标持续重绘，改为推进固定帧数后检查样式；首次启动语言测试检查带快捷键后缀的实际菜单标题。两项均单独复测通过，原有行为断言保留。完整图形集成测试的其余 133 项通过、1 项按设计忽略。
- 引擎单元测试 2350 项通过、15 项忽略，集成测试通过；图片编解码单元测试 143 项、其他格式测试 796 项通过；文字模块 256 项通过、5 项忽略。I/O 单元测试 421 项通过、2 项忽略，集成测试 501 项通过、7 项忽略。记录在 `.tools/quality-*.log`。
- 初轮严格检查发现的布尔表达式和生命周期告警已进行等价修正，保留 NaN 检查及原有进度回调顺序，没有添加忽略规则。六个涉及模块的全部 target 严格 Clippy 已通过（含依赖，`-D warnings`，日志 `.tools/quality-strict-all.log`）；`Check` 现覆盖这六个模块的完整测试及严格检查。最新 WebAssembly 25 crates 与 HEIF 检查通过，`panic_hunt` 再次通过。
- 锁定的真实文件测试集已全部下载并校验，未修改版本或校验清单。编解码模块发布模式下的文件回归通过：144 项单元测试、809 项格式集成测试。I/O 完整发布模式文件回归在默认测试并发和默认工作线程池下通过：546 项成功、0 项失败、7 项按设计忽略，日志 `.tools/quality-io-corpus-fixed.log`。
- 文件回归额外定位并修复了原有 SVG 栈溢出：512 层 XML 预算在 Windows 发布构建的 1 MiB 线程上仍不安全。现上限为 128 层，过深输入在递归解析前明确报错，嵌入 SVG 共享这一预算。固定的旧崩溃输入和其余 21 项 SVG 回归在同样的小栈上通过；未增大栈、忽略断言或降低图像比对通过率。兼容性变化：嵌套超过 128 层的 SVG 会被拒绝。PSD 导出仅移除多余的 `drop(tick)`，保留原有色彩上下文生命周期。
- 2026-10-30：项目内 Rust 1.95.0、Cargo、rustfmt、Clippy 和 wasm32 目标已安装并完成版本检查。Visual Studio 2022 C++ Build Tools 及 Windows SDK 10.0.26100.0 已安装，安装返回 0，无需重启。Rust 所需六个组件均核对了 Rust 官方清单的 SHA-256；下载使用官方源，镜像尝试未取得安装包。
- GitHub 下载问题通过本次进程复用 Windows 现有系统代理解决，大型 Git 依赖只缓存原锁定提交；crates.io 保持其默认连接方式。没有改写上游锁定的依赖版本。
- 2026-10-30：先通过真实适配器将测试红圆改为黄色；随后通过完整引擎的 `Session::start` 后台任务，将 1024×1024 测试画布左侧红方块改为黄色，保留右侧蓝方块。真实输出为 1254×1254，经引擎适配至 1024×1024；验证原图像素完全不变、新增图层、撤销和重做全部通过，耗时约 143 秒。结果 `target/ai-codex-live-result.png` 已目视检查。两次均使用 ChatGPT 登录及原安装位置的完整 Codex 运行时，未调用付费 API。
- 桌面程序已成功构建至 `target/debug/photocraft.exe`。续修后的中文对话框截图 `.tools/ai-dialog-strict.png` 已目视检查，从首帧使用严格缺字策略，无需预热。本机未提供可选的 craft-fonts 输入；使用既有 provider 补齐系统字体。
- Codex App 更新后 `bin` 子目录可能变化，应传入当前完整安装位置。2026-10-30 当前 `0.166.0-alpha.5.1` 的初始化、ChatGPT 登录类型预检通过，本地 Schema 仍包含 `localImage`、`imageGeneration` 和 `savedPath`。前述两次真实图片编辑在 `0.165.0-alpha.1.2` 上完成；续修阶段没有再次触发模型或图片生成。
- 用户已创建 [BLEZZ0102/photocraft-ai](https://github.com/BLEZZ0102/photocraft-ai) Fork，本地 `origin` 已指向该仓库，`upstream` 保留为 `storytold/photocraft`；`main` 继续跟踪 `upstream/main`。已只读验证 Fork 的 `main` 可访问。功能分支为 `feat/ai-provider-codex`，当前开发成果仅保存在本地，没有 push、发布 Alpha 或创建 PR。

## 上游研究

- [Issue #41](https://github.com/storytold/photocraft/issues/41)：要求可替换 provider、通过引擎命令调用、生成结果成为新图层，离线/wasm 不受影响。
- [PR #730](https://github.com/storytold/photocraft/pull/730)：仍在审查，以用户自备 API key 实现多类生成命令；可借鉴其分层、后台任务和单步撤销设计。本分支未合并该 PR，也没有复制它的付费 API 实现。
- [已关闭 PR #1449](https://github.com/storytold/photocraft/pull/1449)：将任务栏与账号/生成能力拆开；账号/provider 留待 #41。本原型不添加账号存储或“即将推出”菜单。
- [OpenAI App Server 文档](https://learn.chatgpt.com/docs/app-server)及本机 `0.165.0-alpha.1.2` 生成的 JSON Schema：传输、初始化、账户类型、线程和生成完成事件字段。
- [Codex 配置参考](https://learn.chatgpt.com/docs/config-file/config-reference)：ChatGPT 登录限制、shell 工具开关。实际图片能力仍依赖完整运行时和账号，本原型不会假装所有 codex.exe 都支持它。

下一步完成新版运行时的真实桌面验收，之后再推送功能分支。Alpha 和上游 PR 留待后续授权；本次仅交付本地原型。后续可扩展 provider 设置、选区/遮罩与其他后端。
