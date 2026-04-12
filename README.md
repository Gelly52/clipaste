# clipaste

Fix screenshot paste in terminal AI tools — locally, over SSH, and in WSL2.

**Problem:** You take a screenshot, switch to Claude Code / Codex / Cursor in your terminal, press **Ctrl+V** — nothing happens. Or you're SSH'd into a remote server and can't paste screenshots at all.

**Why:** macOS screenshots only put raw image data (TIFF/PNG) on the clipboard. Terminals like Ghostty and Alacritty can only Cmd+V paste text or file URLs — they can't paste raw image data. Over SSH, the remote server has no access to your local clipboard whatsoever.

**Solution:** clipaste is a tiny background daemon (9 MB RAM, 0% CPU) that:

1. **Local paste:** Saves screenshots as temp PNG files and registers the file path on the clipboard, so **Cmd+V** works in terminals. Also adds the legacy PNGf type so **Ctrl+V** image paste works too.

2. **SSH remote paste:** Runs an HTTP server on `localhost:18340`. Use `clipaste ssh-setup` to configure a remote server — it installs an xclip shim and SSH tunnel so **Ctrl+V** in remote Claude Code fetches the image from your local machine.

## Install

### macOS (Homebrew)

```bash
brew install hqhq1025/clipaste/clipaste
brew services start clipaste
```

### Windows (PowerShell)

```powershell
irm https://raw.githubusercontent.com/hqhq1025/clipaste/main/install.ps1 | iex
```

### Build from source

```bash
git clone https://github.com/hqhq1025/clipaste.git
cd clipaste
cargo build --release
```

## SSH Remote Paste

clipaste can bridge your local clipboard to remote servers over SSH. One-time setup:

```bash
clipaste ssh-setup user@your-server
```

This automatically:
- Installs an xclip shim on the remote server (`~/.local/bin/xclip`)
- Adds `RemoteForward 18340` to your `~/.ssh/config`
- No extra tools needed on the remote server (just `curl`)

After setup, open a **new** SSH session and use **Ctrl+V** in Claude Code / Codex:

```bash
ssh user@your-server
claude   # Ctrl+V pastes screenshots from your local Mac
```

### How SSH paste works

```
Local Mac                          Remote Server (via SSH)
─────────                          ──────────────────────
Screenshot                         Claude Code runs "xclip"
    │                                      │
    ▼                                      ▼
clipaste saves PNG              xclip shim intercepts call
    │                                      │
    ▼                                      ▼
HTTP server ◄──── SSH RemoteForward ────► curl localhost:18340
(:18340)           (tunnel)                    │
    │                                          ▼
    └──── serves PNG ─────────────────► Image delivered ✅
```

## WSL2 Paste

If you run Claude Code / Codex inside WSL2, clipaste bridges the Windows clipboard to WSL2. Run this **inside WSL2**:

```bash
clipaste wsl-setup
```

This installs the same xclip shim, pointed at clipaste.exe running on your Windows host. No SSH tunnel needed — WSL2 connects directly.

**Prerequisites:** clipaste.exe must be running on the Windows side (installed via the PowerShell one-liner above).

```
Windows Host                       WSL2
────────────                       ────
Win+Shift+S screenshot             Claude Code runs "xclip"
    │                                      │
    ▼                                      ▼
clipaste.exe saves PNG          xclip shim intercepts call
    │                                      │
    ▼                                      ▼
HTTP server ◄──── WSL2 network ────────► curl $WIN_HOST:18340
(:18340)        (direct, no tunnel)        │
    │                                      ▼
    └──── serves PNG ──────────────► Image delivered ✅
```

## Paste shortcuts

| Scenario | Shortcut | How it works |
|----------|----------|-------------|
| **Local terminal (macOS)** | **Cmd+V** | Ghostty/iTerm2 paste file path → tool reads file |
| **Local terminal** | **Ctrl+V** | Claude Code reads clipboard image directly |
| **SSH remote** | **Ctrl+V** | xclip shim → HTTP tunnel → local PNG |
| **WSL2** | **Ctrl+V** | xclip shim → HTTP → Windows host PNG |

**Tip:** Ctrl+V works everywhere (local, SSH, WSL2). Cmd+V is a macOS local-only bonus.

## Compatibility

| Terminal | macOS Cmd+V | macOS Ctrl+V | Windows Ctrl+V | SSH Ctrl+V | WSL2 Ctrl+V |
|----------|:-----------:|:------------:|:--------------:|:----------:|:-----------:|
| Ghostty  | ✅          | ✅           | —              | ✅         | —           |
| Alacritty| ✅          | ✅           | —              | ✅         | —           |
| iTerm2   | ✅          | ✅           | —              | ✅         | —           |
| Terminal.app | ✅       | ✅           | —              | ✅         | —           |
| WezTerm  | ✅          | ✅           | ✅             | ✅         | ✅          |
| Kitty    | ✅          | ✅           | ✅             | ✅         | ✅          |
| Windows Terminal | —   | —            | ✅             | —          | ✅          |

| AI Tool | Local | SSH Remote | WSL2 |
|---------|:-----:|:----------:|:----:|
| Claude Code | ✅ | ✅ | ✅ |
| Codex CLI   | ✅ | ✅ | ✅ |
| Cursor CLI  | ✅ | ✅ | ✅ |

## Managing

### macOS

```bash
brew services info clipaste      # status
brew services restart clipaste   # restart
brew services stop clipaste      # stop
```

### Windows

```powershell
taskkill /IM clipaste.exe /F                      # stop
Remove-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run" -Name "clipaste"  # disable auto-start
```

## How is this different from...

- **[cc-clip](https://github.com/ShunmeiCho/cc-clip)** — SSH clipboard bridge only. clipaste handles both local paste fix AND SSH bridge in one tool, with no dependencies on the remote server (just `curl`).
- **[shotpath](https://hboon.com/shotpath-automatically-copy-macos-screenshot-paths/)** — Monitors screenshot *files* on disk. clipaste works with clipboard screenshots (no file saved to Desktop).
- **[impaste](https://til.simonwillison.net/macos/impaste)** — A pipe-based tool (`impaste | pbcopy`). clipaste is fully automatic, no manual step needed.
- **[pngpaste](https://github.com/jcsalterego/pngpaste)** — Extracts clipboard images to files. clipaste does the reverse: it makes clipboard images available *as* files for terminals.

## Related issues

This fixes a long-standing pain point across multiple projects:

**Local paste (macOS/Windows):**
- [anthropics/claude-code#2102](https://github.com/anthropics/claude-code/issues/2102) — Clipboard Image Parsing Failure on macOS
- [anthropics/claude-code#17042](https://github.com/anthropics/claude-code/issues/17042) — Ctrl+V clipboard paste fails on macOS
- [anthropics/claude-code#26901](https://github.com/anthropics/claude-code/issues/26901) — Image paste from clipboard no longer works
- [openai/codex#6080](https://github.com/openai/codex/issues/6080) — Image pasting issue
- [ghostty-org/ghostty#10478](https://github.com/ghostty-org/ghostty/discussions/10478) — Support pasting screenshot images

**SSH remote paste:**
- [anthropics/claude-code#5277](https://github.com/anthropics/claude-code/issues/5277) — Image paste in SSH/SFTP
- [anthropics/claude-code#13738](https://github.com/anthropics/claude-code/issues/13738) — Clipboard image paste not working in WSL
- [anthropics/claude-code#8324](https://github.com/anthropics/claude-code/issues/8324) — Can't paste image from clipboard on Linux

## Community

- [LINUX DO](https://linux.do) — Where we first shared this project

## License

MIT
