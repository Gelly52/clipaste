# clipaste

> Fork 自 [hqhq1025/clipaste](https://github.com/hqhq1025/clipaste)，新增了自定义粘贴快捷键功能。

修复终端 AI 工具中的截图粘贴问题 -- 支持本地、SSH 远程和 WSL2。

**clipaste** 是一个轻量级的剪贴板守护进程，面向使用 Claude Code、Codex CLI、Cursor 等终端 AI 编程工具的开发者。通过 Homebrew (macOS) 或 PowerShell (Windows) 一行命令安装，截图粘贴即可在 Ghostty、Alacritty、iTerm2、Kitty、WezTerm 等终端中正常工作。同时支持通过 SSH 桥接剪贴板到远程服务器以及 WSL2 环境。使用 Rust 编写，仅占用 9 MB 内存，0% CPU 开销。

**问题：** 你截了一张图，切换到终端中的 Claude Code / Codex / Cursor，按下 **Ctrl+V** -- 什么都没发生。或者你正通过 SSH 连接远程服务器，根本无法粘贴截图。

**原因：** macOS 截图只会将原始图片数据（TIFF/PNG）放入剪贴板。Ghostty、Alacritty 等终端只能通过 Cmd+V 粘贴文本或文件 URL，无法粘贴原始图片数据。通过 SSH 时，远程服务器完全无法访问你的本地剪贴板。

**解决方案：** clipaste 是一个极小的后台守护进程（9 MB 内存，0% CPU），它会：

1. **本地粘贴：** 将截图保存为临时 PNG 文件并将文件路径注册到剪贴板，使 **Cmd+V** 在终端中正常工作。同时添加旧版 PNGf 类型，让 **Ctrl+V** 图片粘贴也能使用。

2. **SSH 远程粘贴：** 在 `localhost:18340` 运行 HTTP 服务器。使用 `clipaste ssh-setup` 配置远程服务器 -- 它会安装 xclip shim 和 SSH 隧道，使远程 Claude Code 中的 **Ctrl+V** 能从本地机器获取图片。

## 安装

### macOS (Homebrew) -- 原版，不含快捷键自定义功能

```bash
brew install hqhq1025/clipaste/clipaste
brew services start clipaste
```

### 本 fork（含自定义粘贴快捷键）

```bash
git clone https://github.com/hqhq1025/clipaste.git
cd clipaste
cargo build --release
# macOS 安装并设置自启动：
cp target/release/clipaste ~/.local/bin/
codesign --force --sign - ~/.local/bin/clipaste
# 创建 LaunchAgent 或直接运行二进制文件
```

### Windows (PowerShell)

```powershell
irm https://raw.githubusercontent.com/hqhq1025/clipaste/main/install.ps1 | iex
```

### 从源码构建

```bash
git clone https://github.com/hqhq1025/clipaste.git
cd clipaste
cargo build --release
```

## SSH 远程粘贴

clipaste 可以通过 SSH 将本地剪贴板桥接到远程服务器。一次性配置：

```bash
clipaste ssh-setup user@your-server
clipaste ssh-setup user@your-server -p 22222   # 自定义 SSH 端口
```

自动完成以下操作：

- 检测远程操作系统（`uname -s`）并安装对应的辅助工具
- Linux 远程：安装 xclip/wl-paste shim（`~/.local/bin/`）
- 在所有远程安装通用的 `clipaste-paste` 命令
- 向 `~/.ssh/config` 添加 `RemoteForward 18340`（如传入 `-p` 则同时添加 `Port`）
- 远程服务器无需额外工具（仅需 `curl`）

配置完成后，打开**新的** SSH 会话：

```bash
ssh user@your-server
claude   # Ctrl+V 可从本地 Mac 粘贴截图（Linux 远程）
codex    # 运行 `clipaste-paste`，然后粘贴打印的路径（见下文）
```

### 在 Codex CLI / macOS 远程中粘贴

Codex CLI 通过**进程内**方式（X11/NSPasteboard）读取剪贴板，会绕过 xclip shim，因此无法通过 SSH 原生粘贴图片。macOS 远程也有同样的问题（工具读取的是远程 Mac 自己的空剪贴板）。两种情况都使用 `ssh-setup` 安装的 `clipaste-paste` 辅助工具：

```bash
clipaste-paste            # → /tmp/clipaste-<ts>.png（远程上的真实文件）
```

在 Mac 上截图（或复制图片文件），在远程运行 `clipaste-paste`，将打印的路径提供给 Codex / Claude Code -- 两者都接受图片文件路径。Linux 和 macOS 远程上的使用方式相同。

### SSH 粘贴工作原理（Claude Code，Linux 远程）

```
本地 Mac                            远程服务器（通过 SSH）
─────────                          ──────────────────────
截图                                Claude Code 运行 "xclip"
    │                                      │
    ▼                                      ▼
clipaste 保存 PNG                  xclip shim 拦截调用
    │                                      │
    ▼                                      ▼
HTTP 服务器 ◄──── SSH RemoteForward ────► curl localhost:18340
(:18340)           (隧道)                    │
    │                                        ▼
    └──── 提供 PNG ─────────────────► 图片送达 ✅
```

## WSL2 粘贴

如果你在 WSL2 中运行 Claude Code / Codex，clipaste 可以将 Windows 剪贴板桥接到 WSL2。在 **WSL2 内部**运行：

```bash
clipaste wsl-setup
```

这会安装相同的 xclip shim，指向 Windows 宿主机上运行的 clipaste.exe。无需 SSH 隧道 -- WSL2 直接连接。

**前提条件：** Windows 端必须运行 clipaste.exe（通过上述 PowerShell 一行命令安装）。

```
Windows 宿主机                      WSL2
────────────                       ────
Win+Shift+S 截图                   Claude Code 运行 "xclip"
    │                                      │
    ▼                                      ▼
clipaste.exe 保存 PNG              xclip shim 拦截调用
    │                                      │
    ▼                                      ▼
HTTP 服务器 ◄──── WSL2 网络 ────────► curl $WIN_HOST:18340
(:18340)        (直连，无隧道)          │
    │                                    ▼
    └──── 提供 PNG ──────────────► 图片送达 ✅
```

## 粘贴快捷键

| 场景                                | 快捷键           | 工作原理                                   |
| ----------------------------------- | ---------------- | ------------------------------------------ |
| **本地终端 (macOS)**                | **Cmd+V**        | Ghostty/iTerm2 粘贴文件路径 → 工具读取文件 |
| **本地终端**                        | **Ctrl+V**       | Claude Code 直接读取剪贴板图片             |
| **SSH 远程 -- Claude Code (Linux)** | **Ctrl+V**       | xclip shim → HTTP 隧道 → 本地 PNG          |
| **SSH 远程 -- Codex / macOS 远程**  | `clipaste-paste` | 辅助工具获取 PNG → 粘贴打印的路径          |
| **WSL2 -- Claude Code**             | **Ctrl+V**       | xclip shim → HTTP → Windows 宿主 PNG       |
| **WSL2 -- Codex**                   | `clipaste-paste` | 辅助工具获取 PNG → 粘贴打印的路径          |

**提示：** 在 Linux 远程上，Claude Code 使用 Ctrl+V 粘贴。Codex CLI 和 macOS 远程使用 `clipaste-paste` 辅助工具（Codex 绕过 xclip shim）。

> **重要：** 在 SSH 会话中使用 Claude Code 时，**请使用 Ctrl+V**，不要用 Cmd+V --
> Cmd+V 会将本地 Mac 路径作为文本粘贴，远程代理无法读取。
> Ctrl+V 触发 xclip shim，通过 SSH 隧道获取图片。
> 对于 **Codex CLI**（不使用 shim）或 **macOS 远程**，运行
> `clipaste-paste` 并将打印的路径提供给代理。

## 兼容性

| 终端             | macOS Cmd+V | macOS Ctrl+V | Windows Ctrl+V | SSH Ctrl+V | WSL2 Ctrl+V |
| ---------------- | :---------: | :----------: | :------------: | :--------: | :---------: |
| Ghostty          |     ✅      |      ✅      |       —        |     ✅     |      —      |
| Alacritty        |     ✅      |      ✅      |       —        |     ✅     |      —      |
| iTerm2           |     ✅      |      ✅      |       —        |     ✅     |      —      |
| Terminal.app     |     ✅      |      ✅      |       —        |     ✅     |      —      |
| WezTerm          |     ✅      |      ✅      |       ✅       |     ✅     |     ✅      |
| Kitty            |     ✅      |      ✅      |       ✅       |     ✅     |     ✅      |
| Windows Terminal |      —      |      —       |       ✅       |     —      |     ✅      |

| AI 工具     | 本地 |         SSH 远程         |           WSL2           |
| ----------- | :--: | :----------------------: | :----------------------: |
| Claude Code |  ✅  |        ✅ Ctrl+V         |        ✅ Ctrl+V         |
| Codex CLI   |  ✅  | ⚠️ 通过 `clipaste-paste` | ⚠️ 通过 `clipaste-paste` |
| Cursor CLI  |  ✅  |        ✅ Ctrl+V         |        ✅ Ctrl+V         |

## 自定义粘贴快捷键

你可以将粘贴快捷键重映射为其他组合键（例如在 macOS 上用 **Ctrl+V** 代替 **Cmd+V**）。创建配置文件：

- **macOS / Linux:** `~/.config/clipaste/config.toml`
- **Windows:** `C:\Users\<用户名>\.config\clipaste\config.toml`

```toml
paste_hotkey_macos = "ctrl+v"
paste_hotkey_windows = "alt+v"
```

也支持通用的 `paste_hotkey` 作为两个平台的后备选项。支持的修饰键：`ctrl`、`cmd`、`shift`、`alt`。支持的按键：`a`-`z`、`0`-`9`。不配置则不注册快捷键，默认粘贴行为不变。

> **注意：** 在 macOS 上，全局快捷键需要**辅助功能权限** -- 首次启动时系统会弹窗提示。在"系统设置 → 隐私与安全性 → 辅助功能"中授权。

## 管理

### macOS

```bash
brew services info clipaste      # 查看状态
brew services restart clipaste   # 重启
brew services stop clipaste      # 停止
```

### Windows

```powershell
taskkill /IM clipaste.exe /F                      # 停止
Remove-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run" -Name "clipaste"  # 禁用自启动
```

## 常见问题

### 如何在 Claude Code 中粘贴截图？

在 macOS 上运行 `brew install hqhq1025/clipaste/clipaste && brew services start clipaste`，或在 Windows 上运行 PowerShell 一行安装命令。运行后截图并在 Claude Code 中按 **Ctrl+V** -- 图片会自动粘贴，无需任何配置。clipaste 作为后台守护进程运行，自动处理剪贴板转换。

### 为什么在 macOS 终端中无法粘贴图片？

macOS 截图将原始 TIFF/PNG 图片数据放入剪贴板，但 Ghostty、Alacritty 等终端只能粘贴文本或文件路径。clipaste 通过拦截剪贴板变化、将图片保存为临时 PNG 文件并将文件路径写回剪贴板来解决此问题，使终端可以正常粘贴。

### 如何通过 SSH 粘贴剪贴板图片？

在本地机器上运行一次 `clipaste ssh-setup user@your-server`（非默认 SSH 端口加 `-p PORT`）。它会检测远程操作系统，安装轻量级 xclip shim（Linux）和通用的 `clipaste-paste` 辅助工具，并配置 SSH 隧道。配置完成后打开新的 SSH 会话：

- **Linux 远程上的 Claude Code：** 按 **Ctrl+V** -- 图片会通过隧道自动获取。
- **Codex CLI 或 macOS 远程上的任何工具：** 运行 `clipaste-paste` 并将打印的路径提供给代理。Codex 通过进程内方式读取剪贴板并绕过 xclip shim，因此无法通过 SSH 原生粘贴；辅助工具是可行的方案。

### clipaste 支持 WSL2 吗？

支持。在 WSL2 环境中运行 `clipaste wsl-setup`。这会安装 xclip shim（Claude Code 使用）和 `clipaste-paste` 辅助工具（Codex 使用），直接连接 Windows 宿主机上的 clipaste.exe，无需 SSH 隧道。配置完成后，Claude Code 中的 **Ctrl+V** 可从 Windows 剪贴板获取截图；Codex 则运行 `clipaste-paste` 并粘贴打印的路径。

### clipaste 使用多少内存和 CPU？

clipaste 大约使用 9 MB 内存，空闲时 0% CPU。使用 Rust 编写，作为极小的后台守护进程运行。在 macOS 上通过 `brew services` 管理；在 Windows 上通过注册表 Run 键自启动。除操作系统剪贴板 API 外没有运行时依赖。

### 支持哪些终端和 AI 工具？

clipaste 支持 Ghostty、Alacritty、iTerm2、Terminal.app、WezTerm、Kitty 和 Windows Terminal。支持 Claude Code、Codex CLI 和 Cursor CLI。**Cmd+V**（macOS 本地）和 **Ctrl+V**（本地，以及 SSH/WSL2 中基于 shim 的工具如 Claude Code）均受支持；Codex CLI 和 macOS 远程使用 `clipaste-paste` 辅助工具。详细信息请参阅上方的兼容性表格。

## 与其他工具的区别

- **[cc-clip](https://github.com/3on/cc-clip)** -- 仅 SSH 剪贴板桥接。clipaste 一个工具同时处理本地粘贴修复和 SSH 桥接，远程服务器无需额外依赖（仅需 `curl`）。
- **[shotpath](https://github.com/thewh1teagle/shotpath)** -- 监控磁盘上的截图*文件*。clipaste 处理剪贴板截图（无需将文件保存到桌面）。
- **[impaste](https://github.com/mattduck/impaste)** -- 基于管道的工具（`impaste | pbcopy`）。clipaste 完全自动化，无需手动步骤。
- **[pngpaste](https://github.com/jcsalterego/pngpaste)** -- 将剪贴板图片提取为文件。clipaste 做相反的事：让剪贴板图片*作为*文件对终端可用。

## 相关 Issue

此工具修复了多个项目中长期存在的痛点：

**本地粘贴（macOS/Windows）：**

- [anthropics/claude-code#2102](https://github.com/anthropics/claude-code/issues/2102) -- macOS 剪贴板图片解析失败
- [anthropics/claude-code#17042](https://github.com/anthropics/claude-code/issues/17042) -- macOS 上 Ctrl+V 剪贴板粘贴失败
- [anthropics/claude-code#26901](https://github.com/anthropics/claude-code/issues/26901) -- 剪贴板图片粘贴不再工作
- [openai/codex#6080](https://github.com/openai/codex/issues/6080) -- 图片粘贴问题
- [ghostty-org/ghostty#10478](https://github.com/ghostty-org/ghostty/issues/10478) -- 支持粘贴截图图片

**SSH 远程粘贴：**

- [anthropics/claude-code#5277](https://github.com/anthropics/claude-code/issues/5277) -- SSH/SFTP 中的图片粘贴
- [anthropics/claude-code#13738](https://github.com/anthropics/claude-code/issues/13738) -- WSL 中剪贴板图片粘贴不工作
- [anthropics/claude-code#8324](https://github.com/anthropics/claude-code/issues/8324) -- Linux 上无法从剪贴板粘贴图片

## 社区

- [LINUX DO](https://linux.do) -- 我们最初分享此项目的地方

## 许可证

MIT
