# PhotoCraft AI 开发与验证

## 当前整合状态（2026-11-02）

AI 打包分支 `codex/package-ai-beta` 已整合 Fork main `fab871dad` 的 0.19.0、一键更新，以及 PR #10593 的 `17e5ee10f` 修复。本次打包版本为 `0.19.0-ai-beta.2`，MSI 版本为 `0.19.2`。下文 v0.17/v0.18 和 PR 分支记录是历史验证，不能代替本次整合验收。实际公开资产与安装包验收结果以 [AI Releases](https://github.com/BLEZZ0102/photocraft-ai/releases) 的对应版本说明为准。

### 源码整合阶段验证（安装包验收另见发布说明）

- AI/Codex/API/更新窗口专项 29 项、语言检查 62 项、桌面单元测试 220 项及集成测试 14 项通过；画布渲染回归 146 项通过。菜单 151 项、缩放输入 5 项通过。未执行真实 AI 生成或账户登录。
- 严格 all-target Clippy、32 个 crate 的层级检查、25 个 crate 的 wasm 检查及 HEIF wasm 检查、命令覆盖、评分文档一致性、文档示例和异常参数 `panic_hunt` 均通过。评分文档由工具重新生成，新增 AI crate 计入现有防崩溃检查。
- 首轮工作区测试为 13,296 项通过、6 项失败、100 项忽略。TCP 正常关闭、SVG 正例深度、更新文案缺失和评分文档过期均已修复并定向复查通过；保留 SVG 的 Windows 防栈溢出限制，未降低测试阈值。两处菜单布尔表达式按 Rust 1.95 Clippy 要求等价简化，并复查既有菜单与缩放测试。
- 已查看 900×760 的深浅主题更新窗口、中文 API 窗口和中文 Codex 已连接窗口；显示和按钮完整。截图的更新版本 `99.0.0` 与账户状态均为离线测试数据。两种参数顺序下，截图工具都在打开文件前拒绝将账户模拟与后台任务组合使用。
- MSI 元数据测试仅使用假载荷核对 `0.19.2` 的独立身份、每用户范围、安装目录、快捷方式和升级 GUID；没有制作或安装真实新版 MSI。实际新包的安装及从 beta.1 升级验收属于后续发布步骤。

包含真实文件样本的优化测试已通过：8 个目标共 5,399 项通过、0 项失败、36 项按原有条件忽略。固定样本与清单校验一致，所有 `corpus` 测试均位于 `psd/codecs/io/engine` 四个 crate 的库测试或 `tests/it` 中；最终按 `--release --lib --test it`、原有 LTO 和 `corpus,heif` 功能完整执行这些目标，限制为两个并行编译任务、四个测试线程。此前的默认全目标构建因内存压力中止，随后聚焦包含样本的全部目标；没有删改样本、断言或优化配置。独立的普通集成测试已由首轮工作区测试覆盖。

GPU 复查保留以下限制，不能表述为全量测试全部通过：

- Intel UHD 的 Windows DX12/FXC 绘图精度组 77 项全部通过；首轮 Vulkan `blend_if` 的 1.06/255 偏差超过原有 1/255 阈值，原阈值保留。
- NVIDIA RTX 5060 Laptop 的 DX12 计时组 5 项通过、1 项失败。多提交用例测得 GPU 约 1.3 ms、提交 38.6 ms、等待总时长 44.1 ms，未满足与等待时长的比例断言。另一个 GPU/CPU 比例比较因显卡过快，按测试原有条件跳过比较。
- 同一进程批量运行完整 DX12 组时，在 `device_loss::render_recovers_effect_maps_after_invalid_encoder` 故障模拟用例处异常退出，退出码 `0xC0000374`。该用例单独运行通过；批量进程异常仍属于发布前待处理项。

以上为源码整合阶段的结果。随后真实发行包验收发现优化 FXC 会拒绝 `blend_channel` 的 switch 返回路径：将默认返回移到函数末尾，保持混合公式和精度阈值不变。新增回归测试关闭调试实例的着色器调试标志、遍历硬件 DX12 显卡，在 RGBA16F/RGBA32F 目标上核对 8/16/32 位的 CPU 参考结果；修复前复现 X3507，修复后在 Intel UHD 与 NVIDIA RTX 5060 Laptop 上通过。实际安装包的验收与发布状态以对应版本的发布说明为准。

更新发布约定见 [AI 更新发布](ai-updates.md)。旧 `0.18.0-ai-beta.1` 用户须手动安装一次包含更新器的新 AI 包，之后才具备一键更新能力。发布入口保持原项目的 Releases 页面；只有新 AI 包正式公开且校验信息完整时才提示更新。

早期开发分支：`feat/ai-provider-codex`，起点为上游 main `5f9b439`。
该阶段合入官方 `v0.18.0`（`8a87a457abb43ac344deeb3fdb67541b91a72ff1`），继续保留 AI 功能。
此前的 v0.17 原型已在本机完成真实 Codex 编辑、后台新图层回填及撤销/重做验证；v0.18.0 的验证范围见下文。已发布版本见 [AI Beta](ai-beta.md)。用户自配 Gemini/Seedream/FLUX API 的使用方式见 [自定义图片 API](ai-api.md)，服务始终由用户明确选择。

## 实现

- `photocraft-gen`（L4）的 `ImageProvider` 接口隔离服务、认证与传输；首个实现是 `CodexProvider`。
- `edit.ai.codex {"prompt":"编辑指令"}` 由 UI、CLI、控制通道和 MCP 共用。
- 编辑菜单的“AI 编辑…”或左侧工具栏顶部的星光快捷按钮打开同一个指令表单；确认后使用现有后台任务。快捷按钮固定在滚动工具区上方，无文档时置灰，仅打开表单不会触发生成。
- 工作线程将可见画布合成为独立文档，通过 ICC 转为 sRGB、8 位 PNG。不会传送隐藏图层、原文件名、EXIF 或 XMP。
- 临时目录只保存本次输入；其生命周期覆盖导出、请求及结果解码，随后自动清理。Codex 自己保存的结果由 Codex 管理。
- 从**原安装位置**启动完整 Codex 运行时，复用其现有 ChatGPT 登录；不复制 codex.exe，不读取或另存登录令牌。
- AI 编辑窗口显示账户连接状态及订阅类型；检查、浏览器登录和取消通过 `ai.codex.account` 引擎命令运行，工作线程负责 I/O，不锁文档。只有明确点击“使用 ChatGPT 登录”才打开官方浏览器页面；由 Codex App Server 管理本地回调与凭据，收到匹配本次登录的完成事件后再次检查账户。取消、失败、超时或关闭窗口均不会触发图片生成；不提供会影响共享 Codex 会话的退出账户按钮。
- `initialize → initialized → account/read → thread/start → turn/start`，等待匹配线程的 `item/completed`，其中 `imageGeneration.status=completed` 且有 `savedPath`；整个 turn 成功后才接收图片。
- Codex 模式的 API-key 登录会失败；强制 `forced_login_method=chatgpt`、OpenAI provider，并从子进程环境移除 API key/base URL。另有明确选择的 API 模式，但 Codex 绝不会自动回退到它。
- 禁用 Codex shell 和网页搜索工具；交互工具请求会返回不支持。图片能力缺失时明确失败。
- 结果经 PNG 大小与宽高比检查、解码后，同宽高比但像素尺寸不同的图片通过现有 Bicubic 重采样适配原画布。随后转换为目标文档色彩/位深，新增一个顶层图层，并产生一个“AI Edit”撤销步骤。命令结果包含 `resizedToCanvas` 和原始输出尺寸。现有图层不变。
- 后台任务使用引擎的文档锁、快照校验及取消机制，取消/失败不会回填。

Install and open the complete Codex desktop runtime. In desktop PhotoCraft, open a document
and choose **Edit > AI Edit...**, or the sparkle button at the top of the left toolbar.
The form shows the Codex account state. Existing ChatGPT sign-in can be reused; otherwise
**Sign in with ChatGPT** opens the browser through PhotoCraft's native URL service.
Checking the connection or opening the form does not generate an image. Enter an instruction
and confirm to submit the visible canvas. Success adds an **AI Edit (Codex)** top-level layer;
Undo removes it and Redo restores it. Existing layers are preserved.
Enter in the focused instruction field inserts a newline; use **OK** to submit.
Reopening the form while a cancelled sign-in is stopping automatically rechecks the
connection once that worker finishes. Confirmation through the UI or control channel
requires a connected account, no pending account operation and a nonempty instruction.

## 在 Windows 本机测试

### AI 版的更新方式

2026-11-02：更新检查改为只读取 `BLEZZ0102/photocraft-ai` 已公开的 GitHub Releases。只有版本更新、适合当前安装方式的 Windows x64 AI 安装包已经上传完成、大小有效且 GitHub 提供 SHA-256 时才提示；上游更新、草稿、仅打标签、缺少安装包或校验值都不提示。AI Beta 接收后续 `ai-beta.N` 和正式 AI 包；正式版不接收 Beta。

已安装的独立 AI MSI 和便携版提供“更新并重启”：先保存文档并等待后台任务结束，后台重新核对发布信息、下载并校验，然后由独立辅助进程等待当前程序退出，安装到原位置并重启。下载期间新增的未保存修改会暂停重启，保存后才继续。便携更新保留 `PhotoCraftData` 和包外文件；MSI 使用独立 AI 版安装身份。失败显示错误，便携文件替换失败会尝试恢复旧文件，MSI 使用 Windows Installer 的回滚机制。开发目录中的程序不自我替换，其他平台目前不提供此更新通道。

发布操作及固定包名见 [AI 版更新发布约定](ai-updates.md)。已经分发的 `0.18.0-ai-beta.1` 没有这个更新器，用户需要手动安装一次包含本改动的新包；本次代码修改不会改变已经发布的旧安装包。

### 启动与验收

先安装 Visual Studio C++ Build Tools（Desktop development with C++）和 Windows SDK（如果尚无）。在普通 PowerShell 中进入项目目录：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\ai-local.ps1 Setup
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\ai-local.ps1 Format
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\ai-local.ps1 Check
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\ai-local.ps1 Run
```

Rust 工具链安装在项目 `.tools`，脚本只为当前进程配置 PATH。微软 C++ Build Tools 和 Windows SDK 按安装器的标准位置安装。上述执行策略设置仅影响该次 PowerShell 进程。
启动脚本保留显式 `-CodexPath` 或现有 `PHOTOCRAFT_CODEX_PATH`，没有指定时由程序自行查找完整安装，避免将会随 Codex 更新变化的路径固定到启动环境。找不到时仍可打开 PhotoCraft，AI 编辑中会显示安装指引，也可明确选择其他图片服务。无需修改全局 PATH。
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

打开或新建 1024×1024 RGB 图像，点击左侧工具栏顶部的星光按钮（悬停显示“AI 编辑…”），或选择编辑 → AI 编辑…，输入“把天空改成日落，保留其余构图”，确认。
应出现 `AI Edit (Codex)` 新图层；检查隐藏它后原画布不变，Ctrl+Z 移除新图层，Ctrl+Shift+Z 恢复。
图片编辑中可用状态栏取消操作，不添加图层。账户流程使用下面的隔离目录验收，无需退出当前 Codex 账户。

## 首次登录的独立桌面验收

默认启动会复用现有 Codex 登录，已有登录时不会再次要求授权。首次登录验收应使用一个新的目录（若示例目录已登录，请改用另一个尚未使用的目录）：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\ai-local.ps1 Launch -CodexHome .\.tools\codex-login-test
```

`Launch` 使用已构建的桌面程序，不等待正在运行的 Cargo 检查；尚未构建时先用 `Run`。`-CodexHome` 只给本次 PhotoCraft 启动的 Codex 子进程设置独立配置目录和文件凭据存储；同一目录同时用于账户检查、登录和图片编辑，不更改主 Codex 的登录。凭据由 Codex 保存在该目录，整个 `.tools` 不提交。

### Review follow-up (Windows, 2026-11-02)

The six findings from the first automated review are addressed. Offline regression tests
cover Enter as a newline, confirmation during sign-in and with a stale connection display,
the allowed confirmation path, reopening during cancellation, closing a deferred refresh,
a missing native browser service, and creating a missing evidence directory. The UI
harness forbids image commands even if confirmation regresses.

The full UI run passed 4611 library tests and 146 graphical integration tests (21 and one
ignored respectively), plus the then-current 13 AI form tests. After adding two more
confirmation checks and the offline command gate, all 15 AI form tests passed separately.
The offline evidence-file test passed; the opt-in subscription test remained ignored.
Both conflicting snapshot-flag orders exited with code 2 before opening a missing input
file. Fresh English/Chinese synthetic multiline-form captures were inspected. Formatting,
32-crate layering and wasm's 25 crates plus HEIF passed. Strict Clippy remains blocked at
the same unchanged upstream IO and UI sites described above; no allowances were added.
The full engine run passed 2896 tests with 23 standard-run ignores; the ignored full-registry
`panic_hunt` passed separately (19.46 s). Both UI and engine documentation compiled.

### UI captures

1. 打开测试图片并点击左侧星光按钮。应显示“需要登录 ChatGPT”，生成确认按钮置灰。
2. 点击“使用 ChatGPT 登录”，应通过 PhotoCraft 的系统浏览器服务打开 Codex 返回的官方登录页面；完成后回到 PhotoCraft，应显示“已连接 ChatGPT”及可用的订阅类型。浏览器已有会话时可能省略部分交互，不要求每次输入密码。系统浏览器打开失败会明确提示，可点击“打开登录页面”重试，或“复制登录链接”后自行粘贴到浏览器。
3. 输入编辑指令后确认按钮才可用；仅验证登录时可直接关闭表单，不生成图片。
4. 用相同命令和目录重启，应复用该次登录，不再次自动打开浏览器。
5. 另选一个尚未使用的新目录验证取消：打开登录页面后点击“取消登录”或关闭 AI 表单，应结束等待、不回填图层，也不退出主 Codex。

登录失败/超时可重试，“检查连接”会要求 Codex 刷新并验证登录。网络错误或登录失效时显示无法验证账户并提供登录入口；不把“连接成功”等同于图片能力或额度保证。自动化测试覆盖成功、取消、失败/超时、其他账户类型和失效后的状态。2026-10-30 用户已按上述隔离目录流程确认浏览器打开、登录成功及重启复用全部通过。

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

## v0.18.0 合并验证（2026-10-31）

- 桌面程序已构建，`--version` 返回 `photocraft 0.18.0 (dev build)`，`--help` 正常；旧版程序备份保留在本机 `.tools/`，不提交二进制。
- 原生回归通过：桌面单元测试 195 项及集成测试 14 项、引擎单元测试 2476 项及其集成测试、图片服务 27 项、界面单元测试 4465 项、API 表单 7 项、账户表单 8 项、图形集成 146 项。新增更新窗口回归用例验证未保存文档保留、链接打开及无官方安装入口；该用例最终单独重跑通过。
- 格式检查、相关模块全部 target 的严格 Clippy、32 crates 分层、wasm 25 crates/HEIF、631/631 菜单覆盖及异常参数 `panic_hunt` 均通过；RAW 单元/集成测试与 Rust 文档构建也通过。
- 合并兼容修复：通用 XML 解析使用独立的 512 层预算，SVG 渲染继续使用原有的 128 层安全预算。新增真实 Krita 导入用例覆盖合法 99 层分组及超限拒绝，并在 1 MB 线程栈检查 XML 的合法上限；文件读写单元测试 440 项通过。
- 标准真实文件语料验证完成：PSD、编解码和文件读写模块在 `corpus`/HEIF 发布模式下共 2410 项通过、15 项按项目标记忽略、0 项失败；修复后的引擎语料集成测试另有 289 项通过、5 项忽略。引擎的注释导入语料分支也单独通过。完整语料按格式模块与引擎目标分别执行；未启用可选 PIXLS RAW 全量语料。XML/Krita 两项新增回归在原生单元测试和发布模式中都通过，原有畸形文件断言未放宽。
- 已目视检查 Codex、Gemini、Seedream、FLUX 在 800×600 窗口的中文表单，以及深浅主题的更新窗口。Codex 连接状态和更新通知使用合成数据，API 表单不含密钥；本轮没有真实登录或图片生成请求，不能替代新基础版本的手动出图验收。

## 当前边界与验证状态

- **账户验收范围：**已实现状态显示、官方浏览器登录入口、完成回调检查和取消。当前 Codex 0.166.0-alpha.5.1 的真实隔离目录检查为未登录；真实登录开始可取得官方链接并取消，取消后仍未登录。额外使用隔离的合成 API-key 凭据验证检查返回不支持、原文件不变，没有推理请求。2026-10-30 用户确认真实浏览器打开、登录成功和重启复用通过；这是用户手动验收反馈。失效/超时等异常有自动化覆盖，不能当作真实账户撤销/过期的验收结果。已有登录态时仍直接复用，不自动要求重新授权。
- 桌面授权链接通过既有系统浏览器服务打开，不能依赖本构建未启用的 eframe `links` 输出。打开失败提供提示、重试和显式复制；PhotoCraft 不把授权链接写入状态历史、文档或 UI 配置。7 项账户/表单测试包含系统浏览器服务调用及打开失败后的复制/重试。
- 账户功能最终回归通过：provider 6 项、引擎单元测试 2355 项（15 项忽略）及其集成测试、界面单元测试 4204 项（18 项忽略）、账户/表单 7 项、图形集成 134 项（1 项忽略）；完整日志 `.tools/account-regression-final.log`。测试使用项目临时目录和 4 个测试线程；现有硬链接测试需要放宽 Windows 执行限制后运行，未跳过或修改断言。新增命令的 `panic_hunt`、三个模块全部 target 的严格 Clippy、32 crates 分层、wasm 25 crates/HEIF、格式检查均通过；菜单覆盖仍为 631/631。14 个主题在两种窗口尺寸的截图及真实未登录/等待界面已检查。真实账户过期/撤销仍不包含在本次手动验收范围中。
- 仅桌面启用。输入与输出最大 16 MP；生成 PNG 最大 64 MiB。同宽高比输出会适配原画布尺寸，允许不超过 1% 的像素舍入误差；明显改变宽高比的输出会明确拒绝，避免拉伸或裁切。
- PNG 交换为 8 位 sRGB，回填转换回目标模式/位深；这不会恢复模型输出中已丢失的高动态范围信息。
- 未实现选择区/遮罩、变体和 ComfyUI。Gemini/Seedream/FLUX API 模式支持用户配置模型和兼容地址；Codex 账户入口仍使用 Codex 的登录存储。
- 程序会自动查找 Codex，无需先运行开发脚本或修改 PATH。Windows 优先查找本机 `OpenAI/Codex/bin` 下最新的运行时（也检查用户目录，兼容启动环境重定向）；macOS 检查系统与用户 Applications 中的 Codex 应用，随后使用 PATH 中的本机可执行文件。macOS 路径仅有合成测试覆盖，真实桌面验收目前限 Windows。发现可执行文件不代表图片能力可用，仍需完整安装与实际编辑验收。
- 每次连接都重新定位运行时，因此 Codex 更新后可以在 AI 编辑中点击“检查连接”。未找到运行时时显示安装指引，不再误报为账户登录故障。安装后可在同一窗口重查。
- 可用 `PHOTOCRAFT_CODEX_PATH` 指定完整运行时内的可执行文件；显式指定路径无效会直接报错，不会悄悄换用另一安装。查找仅访问已知目录与 PATH，限制扫描数量，不递归搜索磁盘，不下载或复制 Codex。
- 2026-10-30 本机补充验收：移除 `PHOTOCRAFT_CODEX_PATH`，PATH 仅保留 Windows 系统目录，使用新的独立 Codex 目录检查账户，成功自动发现完整桌面运行时并返回未登录状态；未打开浏览器或请求图片。随后指定不存在的路径，验证即使本机已安装 Codex 也会明确报错。此测试不替代真实图片编辑或真实 macOS/Linux 验收。
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
- Codex App 更新后 `bin` 子目录可能变化，本地启动脚本可自动查找，也可显式传入当前完整安装位置。2026-10-30 当前 `0.166.0-alpha.5.1` 的初始化、ChatGPT 登录类型预检通过，本地 Schema 仍包含 `localImage`、`imageGeneration` 和 `savedPath`。前述两次自动化真实图片编辑在 `0.165.0-alpha.1.2` 上完成。
- 2026-10-30：用户确认 `0.166.0-alpha.5.1` 的真实桌面验收全部通过：从 PhotoCraft 的 AI 编辑表单提交测试画布与指令、生成结果回填新图层、隐藏新图层查看原图、保持 1024×1024 画布、撤销/重做、窗口可响应，以及保存 `.pcraft` 后关闭重开并保留原图层与 AI 图层。此条为用户手动验收反馈；不代表取消、超时或其他异常场景也在该次手动验收中覆盖。
- 左侧工具栏新增固定的 AI 编辑快捷按钮，复用菜单命令与禁用规则，不切换当前绘图工具。新增两项测试覆盖全部 14 个主题的单列/双列布局、仅打开表单及无文档置灰；原有变换切换测试补充先滚动到抓手工具并点击其可见区域。界面单元测试最终为 4204 项通过、18 项忽略，集成测试 134 项通过、1 项忽略，AI 表单测试 2 项通过；使用项目内临时目录、4 个测试线程，日志 `.tools/toolbar-tests-final.log`。严格 Clippy、32 crates 分层及 wasm 25 crates/HEIF 检查通过。已目视检查 14 个主题在 1280×900 单列和 800×600 双列下的截图，以及实际点击快捷按钮打开中文表单的截图（`.tools/toolbar-qa/`）。桌面程序已重新构建；本次未额外调用图片生成服务。
- 用户已创建 [BLEZZ0102/photocraft-ai](https://github.com/BLEZZ0102/photocraft-ai) Fork，本地 `origin` 已指向该仓库，`upstream` 保留为 `storytold/photocraft`；`main` 继续跟踪 `upstream/main`。功能分支为 `feat/ai-provider-codex`，用户已授权将已验收原型整理提交并推送到 Fork 的同名分支。尚未发布 Alpha 或创建上游 PR。

## 上游研究

- [Issue #41](https://github.com/storytold/photocraft/issues/41)：要求可替换 provider、通过引擎命令调用、生成结果成为新图层，离线/wasm 不受影响。
- [PR #730](https://github.com/storytold/photocraft/pull/730)：仍在审查，以用户自备 API key 实现多类生成命令；可借鉴其分层、后台任务和单步撤销设计。本分支未合并该 PR，也没有复制它的付费 API 实现。
- [已关闭 PR #1449](https://github.com/storytold/photocraft/pull/1449)：将任务栏与账号/生成能力拆开；账号/provider 留待 #41。本原型不添加账号存储或“即将推出”菜单。
- [OpenAI App Server 文档](https://learn.chatgpt.com/docs/app-server)及本机 `0.165.0-alpha.1.2` 生成的 JSON Schema：传输、初始化、账户类型、线程和生成完成事件字段。
- [Codex 配置参考](https://learn.chatgpt.com/docs/config-file/config-reference)：ChatGPT 登录限制、shell 工具开关。实际图片能力仍依赖完整运行时和账号，本原型不会假装所有 codex.exe 都支持它。

Codex 图片编辑闭环、首次浏览器登录及重启复用的手动验收记录对应此前的 v0.17 原型。2026-10-31 的 v0.18.0 升级继续保留这些功能及 Gemini、Seedream、FLUX 配置，本次只保存本地提交，不推送、发布或创建 PR。新基础版本的自动回归与离屏界面检查不替代真实账户登录、图片编辑及付费 API 出图的重新验收；后续测试包和上游 PR 另行安排。
