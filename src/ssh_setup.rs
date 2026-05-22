use crate::common;
use std::io::Write;
use std::process::Command;

/// xclip shim template — {CLIPASTE_URL} will be replaced at install time
const XCLIP_SHIM_TEMPLATE: &str = r#"#!/bin/bash
# clipaste xclip shim — intercepts xclip calls and fetches images from clipaste
# Installed by: clipaste wsl-setup

CLIPASTE_URL="__CLIPASTE_URL__"
REAL_XCLIP="$(PATH=$(echo "$PATH" | sed "s|$HOME/.local/bin:||g") command -v xclip 2>/dev/null)"

case "$*" in
    *"-selection clipboard"*"-t TARGETS"*"-o"*|*"-sel clip"*"-t TARGETS"*"-o"*)
        if curl -sf "${CLIPASTE_URL}/clipboard/type" 2>/dev/null | grep -q '"image"'; then
            echo "TARGETS"
            echo "image/png"
            exit 0
        fi
        ;;
    *"-selection clipboard"*"-t image/png"*"-o"*|*"-sel clip"*"-t image/png"*"-o"*)
        tmpfile=$(mktemp /tmp/clipaste-remote-XXXXXX.png)
        if curl -sf -o "$tmpfile" "${CLIPASTE_URL}/clipboard/image" 2>/dev/null; then
            if [ -s "$tmpfile" ]; then
                cat "$tmpfile"
                rm -f "$tmpfile"
                exit 0
            fi
        fi
        rm -f "$tmpfile"
        ;;
esac

if [ -n "$REAL_XCLIP" ] && [ -x "$REAL_XCLIP" ]; then
    exec "$REAL_XCLIP" "$@"
else
    echo "xclip not found" >&2
    exit 1
fi
"#;

const WL_PASTE_SHIM_TEMPLATE: &str = r#"#!/bin/bash
# clipaste wl-paste shim — for Wayland environments
# Installed by: clipaste wsl-setup

CLIPASTE_URL="__CLIPASTE_URL__"
REAL_WL_PASTE="$(PATH=$(echo "$PATH" | sed "s|$HOME/.local/bin:||g") command -v wl-paste 2>/dev/null)"

case "$*" in
    *"--list-types"*)
        if curl -sf "${CLIPASTE_URL}/clipboard/type" 2>/dev/null | grep -q '"image"'; then
            echo "image/png"
            echo "text/plain"
            exit 0
        fi
        ;;
    *"--type image/"*|*"-t image/"*)
        tmpfile=$(mktemp /tmp/clipaste-remote-XXXXXX.png)
        if curl -sf -o "$tmpfile" "${CLIPASTE_URL}/clipboard/image" 2>/dev/null; then
            if [ -s "$tmpfile" ]; then
                cat "$tmpfile"
                rm -f "$tmpfile"
                exit 0
            fi
        fi
        rm -f "$tmpfile"
        ;;
esac

if [ -n "$REAL_WL_PASTE" ] && [ -x "$REAL_WL_PASTE" ]; then
    exec "$REAL_WL_PASTE" "$@"
else
    echo "wl-paste not found" >&2
    exit 1
fi
"#;

fn install_shims_via_ssh(host: &str, clipaste_url: &str) -> Result<(), String> {
    let xclip_shim = XCLIP_SHIM_TEMPLATE.replace("__CLIPASTE_URL__", clipaste_url);
    let wl_paste_shim = WL_PASTE_SHIM_TEMPLATE.replace("__CLIPASTE_URL__", clipaste_url);

    let setup_script = format!(
        r#"
mkdir -p ~/.local/bin
cat > ~/.local/bin/xclip << 'SHIMEOF'
{xclip_shim}
SHIMEOF
chmod +x ~/.local/bin/xclip

cat > ~/.local/bin/wl-paste << 'SHIMEOF'
{wl_paste_shim}
SHIMEOF
chmod +x ~/.local/bin/wl-paste

# Ensure ~/.local/bin is in PATH
if ! echo "$PATH" | grep -q "$HOME/.local/bin"; then
    for rc in ~/.bashrc ~/.zshrc; do
        if [ -f "$rc" ] && ! grep -q 'clipaste PATH' "$rc"; then
            echo '# clipaste PATH' >> "$rc"
            echo 'export PATH="$HOME/.local/bin:$PATH"' >> "$rc"
        fi
    done
fi
echo "OK"
"#
    );

    let result = Command::new("ssh")
        .args([host, "bash -s"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child.stdin.take().unwrap().write_all(setup_script.as_bytes())?;
            child.wait_with_output()
        });

    match result {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => Err(String::from_utf8_lossy(&o.stderr).trim().to_string()),
        Err(e) => Err(format!("SSH error: {e}")),
    }
}

fn install_shims_locally(clipaste_url: &str) -> Result<(), String> {
    let xclip_shim = XCLIP_SHIM_TEMPLATE.replace("__CLIPASTE_URL__", clipaste_url);
    let wl_paste_shim = WL_PASTE_SHIM_TEMPLATE.replace("__CLIPASTE_URL__", clipaste_url);

    let bin_dir = dirs_home().join(".local/bin");
    std::fs::create_dir_all(&bin_dir).map_err(|e| format!("mkdir: {e}"))?;

    let xclip_path = bin_dir.join("xclip");
    std::fs::write(&xclip_path, xclip_shim).map_err(|e| format!("write xclip: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&xclip_path, std::fs::Permissions::from_mode(0o755)).ok();
    }

    let wl_paste_path = bin_dir.join("wl-paste");
    std::fs::write(&wl_paste_path, wl_paste_shim).map_err(|e| format!("write wl-paste: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&wl_paste_path, std::fs::Permissions::from_mode(0o755)).ok();
    }

    // Ensure PATH
    let bashrc = dirs_home().join(".bashrc");
    if bashrc.exists() {
        let content = std::fs::read_to_string(&bashrc).unwrap_or_default();
        if !content.contains("clipaste PATH") {
            let mut f = std::fs::OpenOptions::new().append(true).open(&bashrc)
                .map_err(|e| format!("append bashrc: {e}"))?;
            writeln!(f, "\n# clipaste PATH\nexport PATH=\"$HOME/.local/bin:$PATH\"").ok();
        }
    }

    Ok(())
}

// ─── SSH Setup ───

pub fn run_ssh(host: &str) {
    println!("clipaste ssh-setup for {host}");
    println!();

    // Step 1: Check local HTTP server
    print!("[1/3] Checking local clipaste server... ");
    std::io::stdout().flush().unwrap();
    if !check_health(&format!("http://127.0.0.1:{}", common::DEFAULT_PORT)) {
        println!("FAILED");
        eprintln!("  clipaste daemon is not running. Start it first:");
        eprintln!("  brew services start clipaste");
        std::process::exit(1);
    }
    println!("OK");

    // Step 2: Deploy shims to remote
    let url = format!("http://127.0.0.1:{}", common::DEFAULT_PORT);
    print!("[2/3] Installing shims on {host}... ");
    std::io::stdout().flush().unwrap();
    match install_shims_via_ssh(host, &url) {
        Ok(()) => println!("OK"),
        Err(e) => {
            println!("FAILED");
            eprintln!("  {e}");
            std::process::exit(1);
        }
    }

    // Step 3: Configure SSH RemoteForward
    print!("[3/3] Configuring SSH RemoteForward... ");
    std::io::stdout().flush().unwrap();
    match add_remote_forward(host) {
        Ok(msg) => println!("{msg}"),
        Err(e) => {
            println!("FAILED");
            eprintln!("  {e}");
            std::process::exit(1);
        }
    }

    println!();
    println!("Setup complete! Next steps:");
    println!("  1. Open a NEW SSH session: ssh {host}");
    println!("  2. Take a screenshot on your Mac");
    println!("  3. In remote Claude Code / Codex, press Ctrl+V");
}

// ─── WSL Setup ───

pub fn run_wsl() {
    println!("clipaste wsl-setup");
    println!();

    // Step 1: Detect Windows host IP
    print!("[1/3] Detecting Windows host IP... ");
    std::io::stdout().flush().unwrap();
    let win_ip = detect_wsl_host_ip();
    match &win_ip {
        Some(ip) => println!("{ip}"),
        None => {
            println!("FAILED");
            eprintln!("  Cannot detect Windows host IP from /etc/resolv.conf");
            eprintln!("  Make sure you're running this inside WSL2");
            std::process::exit(1);
        }
    }
    let win_ip = win_ip.unwrap();

    // Step 2: Check clipaste HTTP server on Windows host
    let url = format!("http://{win_ip}:{}", common::DEFAULT_PORT);
    print!("[2/3] Checking clipaste on Windows host ({url})... ");
    std::io::stdout().flush().unwrap();
    if !check_health(&url) {
        println!("FAILED");
        eprintln!("  clipaste.exe is not running on Windows, or port {} is blocked.", common::DEFAULT_PORT);
        eprintln!("  Make sure clipaste.exe is running on the Windows side.");
        std::process::exit(1);
    }
    println!("OK");

    // Step 3: Install shims locally (we're inside WSL2)
    print!("[3/3] Installing xclip/wl-paste shims... ");
    std::io::stdout().flush().unwrap();
    match install_shims_locally(&url) {
        Ok(()) => println!("OK"),
        Err(e) => {
            println!("FAILED");
            eprintln!("  {e}");
            std::process::exit(1);
        }
    }

    println!();
    println!("Setup complete! Now:");
    println!("  1. Open a new terminal (or run: source ~/.bashrc)");
    println!("  2. Take a screenshot on Windows (Win+Shift+S)");
    println!("  3. In Claude Code / Codex, press Ctrl+V");
    println!();
    println!("No SSH tunnel needed — WSL2 connects directly to Windows host.");
}

// ─── Helpers ───

fn check_health(base_url: &str) -> bool {
    Command::new("curl")
        .args(["-sf", &format!("{base_url}/health")])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn detect_wsl_host_ip() -> Option<String> {
    // WSL2: Windows host IP is the nameserver in /etc/resolv.conf
    let content = std::fs::read_to_string("/etc/resolv.conf").ok()?;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("nameserver") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                return Some(parts[1].to_string());
            }
        }
    }
    None
}

fn add_remote_forward(host: &str) -> Result<String, String> {
    let ssh_config_path = dirs_home().join(".ssh/config");
    let ssh_dir = ssh_config_path.parent().unwrap();
    if !ssh_dir.exists() {
        std::fs::create_dir_all(ssh_dir)
            .map_err(|e| format!("Cannot create ~/.ssh: {e}"))?;
    }
    let port = common::DEFAULT_PORT;
    let config_content = std::fs::read_to_string(&ssh_config_path).unwrap_or_default();
    let forward_line = format!("RemoteForward {port} 127.0.0.1:{port}");
    let host_pattern = extract_hostname(host);

    // Two-pass approach:
    // Pass 1: find which line to inject after (matching by Host alias or HostName)
    // Pass 2: build new config with injection

    let lines: Vec<&str> = config_content.lines().collect();
    let mut inject_after: Option<usize> = None;
    let mut in_matching_block = false;
    let mut found_existing_forward = false;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        // New Host block starts — reset per-block matching state.
        // Note: found_existing_forward is latching (never reset), so a
        // forward found in an earlier matching block survives later blocks.
        if trimmed.starts_with("Host ") {
            in_matching_block = false;

            let host_value = trimmed.strip_prefix("Host ").unwrap_or("").trim();
            // Skip wildcard-only blocks (Host *)
            if host_value.split_whitespace().all(|h| h.contains('*') || h.contains('?')) {
                continue;
            }
            if host_value.split_whitespace().any(|h| h == host_pattern) {
                in_matching_block = true;
                // Fallback: inject after Host line if block has no HostName
                if inject_after.is_none() {
                    inject_after = Some(i);
                }
            }
        }

        // HostName in current block — prefer injecting after this line
        if trimmed.starts_with("HostName ") || trimmed.starts_with("HostName\t") {
            let hostname_value = trimmed.strip_prefix("HostName").unwrap_or("").trim();
            if hostname_value == host_pattern {
                in_matching_block = true;
            }
            if in_matching_block {
                // Override: prefer injecting after HostName over Host line
                inject_after = Some(i);
            }
        }

        // Check if any matching block already has the forward (latching)
        if in_matching_block && trimmed.contains(&forward_line) {
            found_existing_forward = true;
        }
    }

    if found_existing_forward {
        return Ok("already configured".to_string());
    }

    // Pass 2: build new config
    let mut new_config = String::new();
    for (i, line) in lines.iter().enumerate() {
        new_config.push_str(line);
        new_config.push('\n');
        if Some(i) == inject_after {
            new_config.push_str(&format!("    {forward_line}\n"));
        }
    }

    if inject_after.is_none() {
        new_config.push_str(&format!(
            "\n# clipaste remote paste\nHost clipaste-{host_pattern}\n    HostName {host_pattern}\n    {forward_line}\n"
        ));
    }

    std::fs::write(&ssh_config_path, &new_config)
        .map_err(|e| format!("Cannot write ~/.ssh/config: {e}"))?;
    Ok("OK (added RemoteForward)".to_string())
}

fn dirs_home() -> std::path::PathBuf {
    // HOME is set on Unix/macOS; USERPROFILE is the Windows equivalent
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
}

fn extract_hostname(host: &str) -> String {
    if let Some(at) = host.rfind('@') {
        host[at + 1..].to_string()
    } else {
        host.to_string()
    }
}
