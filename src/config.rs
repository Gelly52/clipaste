pub fn load_paste_hotkey() -> Option<(u64, u16)> {
    let c = std::fs::read_to_string(
        format!("{}/.config/clipaste/config.toml", std::env::var("HOME").ok()?)
    ).ok()?;
    let k = if cfg!(target_os = "macos") { "paste_hotkey_macos" } else { "paste_hotkey_windows" };
    let val = find(&c, k).or_else(|| find(&c, "paste_hotkey"))?;
    let (mut m, mut key) = (0u64, None);
    for p in val.split('+').map(|s| s.trim().to_lowercase()) {
        match p.as_str() {
            "ctrl"|"control" => m |= 0x40000, "cmd"|"command" => m |= 0x100000,
            "shift" => m |= 0x20000, "alt"|"option" => m |= 0x80000,
            k => key = keycode(k),
        }
    }
    Some((m, key?))
}

fn find<'a>(c: &'a str, key: &str) -> Option<&'a str> {
    c.lines().find_map(|l| Some(l.trim().strip_prefix(key)?.trim_start().strip_prefix('=')?.trim().trim_matches('"')))
}

fn keycode(k: &str) -> Option<u16> {
    let c = *k.as_bytes().first()?;
    match c {
        b'a'..=b'z' => [0x00,0x0B,0x08,0x02,0x0E,0x03,0x05,0x04,0x22,0x26,
            0x28,0x25,0x2E,0x2D,0x1F,0x23,0x0C,0x0F,0x01,0x11,
            0x20,0x09,0x0D,0x07,0x10,0x06].get((c - b'a') as usize).copied(),
        b'0'..=b'9' => [0x1D,0x12,0x13,0x14,0x15,0x17,0x16,0x1A,0x1C,0x19]
            .get((c - b'0') as usize).copied(),
        _ => None,
    }
}
