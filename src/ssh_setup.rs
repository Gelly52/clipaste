use crate::common;
use std::io::Write;
use std::process::Command;

const XCLIP_SHIM: &str = r#"#!/bin/bash
# clipaste xclip shim — intercepts xclip calls and fetches images from local clipaste
# Installed by: clipaste ssh-setup

CLIPASTE_PORT="${CLIPASTE_PORT:-18340}"
CLIPASTE_URL="http://127.0.0.1:${CLIPASTE_PORT}"
REAL_XCLIP="$(command -v -p xclip 2>/dev/null || echo /usr/bin/xclip)"

# Only intercept clipboard read operations
case "$*" in
    *"-selection clipboard"*"-t TARGETS"*"-o"*|*"-sel clip"*"-t TARGETS"*"-o"*)
        # Claude Code asks: what types are available?
        if curl -sf "${CLIPASTE_URL}/clipboard/type" 2>/dev/null | grep -q '"image"'; then
            echo "TARGETS"
            echo "image/png"
            exit 0
        fi
        ;;
    *"-selection clipboard"*"-t image/png"*"-o"*|*"-sel clip"*"-t image/png"*"-o"*)
        # Claude Code asks: give me the image
        tmpfile=$(mktemp /tmp/clipaste-remote-XXXXXX.png)
        if curl -sf -o "$tmpfile" "${CLIPASTE_URL}/clipboard/image" 2>/dev/null; then
            cat "$tmpfile"
            rm -f "$tmpfile"
            exit 0
        fi
        rm -f "$tmpfile"
        ;;
esac

# Fall through to real xclip for everything else
if [ -x "$REAL_XCLIP" ]; then
    exec "$REAL_XCLIP" "$@"
else
    echo "xclip not found" >&2
    exit 1
fi
"#;

const WL_PASTE_SHIM: &str = r#"#!/bin/bash
# clipaste wl-paste shim — for Wayland environments
# Installed by: clipaste ssh-setup

CLIPASTE_PORT="${CLIPASTE_PORT:-18340}"
CLIPASTE_URL="http://127.0.0.1:${CLIPASTE_PORT}"
REAL_WL_PASTE="$(command -v -p wl-paste 2>/dev/null || echo /usr/bin/wl-paste)"

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
            cat "$tmpfile"
            rm -f "$tmpfile"
            exit 0
        fi
        rm -f "$tmpfile"
        ;;
esac

if [ -x "$REAL_WL_PASTE" ]; then
    exec "$REAL_WL_PASTE" "$@"
else
    echo "wl-paste not found" >&2
    exit 1
fi
"#;

pub fn run(host: &str) {
    println!("clipaste ssh-setup for {host}");
    println!();

    // Step 1: Check local HTTP server
    print!("[1/4] Checking local clipaste server... ");
    std::io::stdout().flush().unwrap();
    let health = Command::new("curl")
        .args(["-sf", &format!("http://127.0.0.1:{}/health", common::DEFAULT_PORT)])
        .output();
    match health {
        Ok(o) if o.status.success() => println!("OK"),
        _ => {
            println!("FAILED");
            eprintln!("  clipaste daemon is not running. Start it first:");
            eprintln!("  brew services start clipaste");
            std::process::exit(1);
        }
    }

    // Step 2: Deploy xclip shim to remote
    print!("[2/4] Installing xclip shim on {host}... ");
    std::io::stdout().flush().unwrap();
    let setup_script = format!(
        r#"
mkdir -p ~/.local/bin
cat > ~/.local/bin/xclip << 'SHIMEOF'
{XCLIP_SHIM}
SHIMEOF
chmod +x ~/.local/bin/xclip

cat > ~/.local/bin/wl-paste << 'SHIMEOF'
{WL_PASTE_SHIM}
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
            child
                .stdin
                .take()
                .unwrap()
                .write_all(setup_script.as_bytes())?;
            child.wait_with_output()
        });

    match result {
        Ok(o) if o.status.success() => {
            println!("{}", String::from_utf8_lossy(&o.stdout).trim());
        }
        Ok(o) => {
            println!("FAILED");
            eprintln!("  {}", String::from_utf8_lossy(&o.stderr).trim());
            std::process::exit(1);
        }
        Err(e) => {
            println!("FAILED");
            eprintln!("  SSH error: {e}");
            std::process::exit(1);
        }
    }

    // Step 3: Configure SSH RemoteForward
    print!("[3/4] Configuring SSH RemoteForward... ");
    std::io::stdout().flush().unwrap();
    let ssh_config_path = dirs_next().join(".ssh/config");
    let port = common::DEFAULT_PORT;

    let config_content = std::fs::read_to_string(&ssh_config_path).unwrap_or_default();

    // Check if RemoteForward already configured for this host
    let forward_line = format!("RemoteForward {port} 127.0.0.1:{port}");
    if config_content.contains(&forward_line) {
        println!("already configured");
    } else {
        // Find the Host block and add RemoteForward
        let host_pattern = extract_hostname(host);
        let mut found = false;
        let mut new_config = String::new();
        for line in config_content.lines() {
            new_config.push_str(line);
            new_config.push('\n');
            if !found && line.trim().starts_with("HostName") && line.contains(&host_pattern) {
                new_config.push_str(&format!("    {forward_line}\n"));
                found = true;
            }
        }
        if !found {
            // Append a new Host block
            new_config.push_str(&format!(
                "\n# clipaste remote paste\nHost clipaste-{host_pattern}\n    HostName {host_pattern}\n    {forward_line}\n"
            ));
        }
        if let Err(e) = std::fs::write(&ssh_config_path, &new_config) {
            println!("FAILED");
            eprintln!("  Cannot write ~/.ssh/config: {e}");
            std::process::exit(1);
        }
        println!("OK (added RemoteForward)");
    }

    // Step 4: Verify tunnel
    print!("[4/4] Verifying (requires new SSH session)... ");
    std::io::stdout().flush().unwrap();
    println!("SKIP (connect with new SSH session to activate tunnel)");

    println!();
    println!("Setup complete! Next steps:");
    println!("  1. Open a NEW SSH session: ssh {host}");
    println!("  2. Take a screenshot on your Mac");
    println!("  3. In remote Claude Code, press Ctrl+V");
    println!();
    println!("The RemoteForward tunnels port {port} to your local clipaste.");
    println!("The xclip shim at ~/.local/bin/xclip intercepts Claude Code's clipboard reads.");
}

fn dirs_next() -> std::path::PathBuf {
    std::env::var("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
}

fn extract_hostname(host: &str) -> String {
    // "user@1.2.3.4" → "1.2.3.4"
    if let Some(at) = host.rfind('@') {
        host[at + 1..].to_string()
    } else {
        host.to_string()
    }
}
