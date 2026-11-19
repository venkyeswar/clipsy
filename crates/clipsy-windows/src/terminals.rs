//! Terminal detection for paste, by executable name.

/// Stems that must match exactly (compared lowercase, without `.exe`).
const TERMINAL_EXACT: &[&str] = &[
    "windowsterminal",
    "cmd",
    "powershell",
    "pwsh",
    "conhost",
    "wezterm-gui",
    "alacritty",
    "mintty",
    "putty",
    "kitty",
    "warp",
    "tabby",
    "hyper",
];

/// Substrings matched anywhere in the lowercase stem.
const TERMINAL_SUBSTRINGS: &[&str] = &["terminal"];

/// True for a known terminal's executable, or one the user listed in `extra`.
///
/// `exe` may be a bare file name (`WindowsTerminal.exe`) or a full path
/// (`C:\Program Files\...\wt.exe`). `extra` entries match the stem exactly,
/// ignoring surrounding whitespace, ASCII case and an optional `.exe` suffix.
pub fn is_terminal(exe: &str, extra: &[String]) -> bool {
    let stem = exe_stem(exe);
    if stem.is_empty() {
        return false;
    }
    TERMINAL_EXACT.contains(&stem.as_str())
        || TERMINAL_SUBSTRINGS.iter().any(|t| stem.contains(t))
        || extra.iter().any(|e| {
            let lower = e.trim().to_ascii_lowercase();
            lower.strip_suffix(".exe").unwrap_or(lower.as_str()) == stem
        })
}

/// Lowercase file stem of an exe name or path: the last path component
/// without its extension. Accepts both `\` and `/` as separators.
fn exe_stem(exe: &str) -> String {
    let name = exe.rsplit(['\\', '/']).next().unwrap_or(exe);
    let stem = match name.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem,
        _ => name,
    };
    stem.trim().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none() -> Vec<String> {
        Vec::new()
    }

    fn user(entries: &[&str]) -> Vec<String> {
        entries.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn matches_windows_terminal() {
        assert!(is_terminal("WindowsTerminal.exe", &none()));
        assert!(is_terminal("cmd.exe", &none()));
        assert!(is_terminal("conhost.exe", &none()));
        assert!(is_terminal("wezterm-gui.exe", &none()));
        assert!(is_terminal("alacritty.exe", &none()));
        assert!(is_terminal("kitty.exe", &none()));
        assert!(is_terminal("mintty.exe", &none()));
        assert!(is_terminal("putty.exe", &none()));
        assert!(is_terminal("warp.exe", &none()));
        assert!(is_terminal("tabby.exe", &none()));
        assert!(is_terminal("hyper.exe", &none()));
        // Substring rule: any stem containing "terminal".
        assert!(is_terminal("SomeTerminal.exe", &none()));
    }

    #[test]
    fn matches_full_path() {
        assert!(is_terminal(
            r"C:\Program Files\WindowsApps\Microsoft.WindowsTerminal\WindowsTerminal.exe",
            &none()
        ));
        assert!(is_terminal(r"C:\Windows\System32\cmd.exe", &none()));
        assert!(is_terminal("/usr/bin/kitty", &none()));
        assert!(is_terminal("/opt/app/PowerShell.exe", &none()));
    }

    #[test]
    fn matches_powershell_variants() {
        assert!(is_terminal("powershell.exe", &none()));
        assert!(is_terminal("pwsh.exe", &none()));
        assert!(is_terminal("PWSH.EXE", &none()));
        assert!(is_terminal(r"C:\Program Files\PowerShell\7\pwsh.exe", &none()));
    }

    #[test]
    fn rejects_normal_apps() {
        assert!(!is_terminal("notepad.exe", &none()));
        assert!(!is_terminal("chrome.exe", &none()));
        assert!(!is_terminal("code.exe", &none()));
        assert!(!is_terminal(r"C:\Windows\explorer.exe", &none()));
        assert!(!is_terminal("", &none()));
        assert!(!is_terminal(".exe", &none()));
    }

    #[test]
    fn user_list_matches_exactly() {
        let extra = user(&["Code"]);
        assert!(is_terminal("code.exe", &extra));
        assert!(!is_terminal("code-insiders.exe", &extra));
        assert!(!is_terminal("codex.exe", &extra));
    }

    #[test]
    fn user_list_ignores_exe_suffix_and_case() {
        assert!(is_terminal("Foo.exe", &user(&["foo"])));
        assert!(is_terminal("foo.exe", &user(&["FOO.EXE"])));
        assert!(is_terminal("foo.exe", &user(&["  Foo.exe  "])));
        assert!(is_terminal("foo", &user(&["foo.exe"])));
        assert!(!is_terminal("foo.exe", &user(&["bar"])));
        assert!(!is_terminal("foo.exe", &user(&["   "])));
    }
}
