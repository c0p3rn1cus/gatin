use std::path::{Path, PathBuf};

/// Snapshot of the current autostart configuration and executable location.
#[derive(Debug, Clone)]
pub struct AutostartInfo {
    pub supported: bool,
    pub enabled: bool,
    pub permanent_path: Option<PathBuf>,
    pub running_from_permanent: bool,
    pub running_from_transient: bool,
}

impl AutostartInfo {
    pub fn can_copy_to_permanent(&self) -> bool {
        self.permanent_path.is_some() && !self.running_from_permanent
    }
}

/// Inspects current autostart state and whether the binary is running from a transient folder.
pub fn inspect() -> AutostartInfo {
    let supported = cfg!(any(
        target_os = "windows",
        target_os = "linux",
        target_os = "macos"
    ));
    let current_exe = std::env::current_exe().ok();
    let permanent_path = permanent_install_path();
    let running_from_permanent = match (&current_exe, &permanent_path) {
        (Some(cur), Some(perm)) => paths_match(cur, perm),
        _ => false,
    };
    let running_from_transient = !running_from_permanent
        && current_exe
            .as_deref()
            .is_some_and(is_transient_path);

    AutostartInfo {
        supported,
        enabled: is_enabled(),
        permanent_path,
        running_from_permanent,
        running_from_transient,
    }
}

/// Short, human-readable label for the permanent install path on the current platform.
pub fn permanent_install_label() -> Option<&'static str> {
    #[cfg(target_os = "windows")]
    {
        Some(r"%LOCALAPPDATA%\Programs\Gatin\gatin.exe")
    }
    #[cfg(target_os = "linux")]
    {
        Some("~/.local/bin/gatin")
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        None
    }
}

/// Returns the standard per-user permanent location for the executable (no admin required).
pub fn permanent_install_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        directories::BaseDirs::new()
            .map(|d| d.data_local_dir().join("Programs").join("Gatin").join("gatin.exe"))
    }
    #[cfg(target_os = "linux")]
    {
        directories::BaseDirs::new().map(|d| {
            d.executable_dir()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| d.home_dir().join(".local").join("bin"))
                .join("gatin")
        })
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        None
    }
}

/// Returns true if autostart is currently registered with the OS.
pub fn is_enabled() -> bool {
    platform::read_registered_path().is_some()
}

/// Enables autostart for the current user. If `copy_to_permanent` is true and the platform
/// supports a permanent per-user path, copies the executable there first and registers that copy.
pub fn enable(copy_to_permanent: bool) -> Result<PathBuf, String> {
    let current_exe = std::env::current_exe()
        .map_err(|e| format!("Não foi possível localizar o executável atual: {e}"))?;

    let target_exe = if copy_to_permanent {
        if let Some(dest) = permanent_install_path() {
            install_binary_copy(&current_exe, &dest)?;
            dest
        } else {
            current_exe
        }
    } else {
        current_exe
    };

    platform::enable(&target_exe)?;
    Ok(target_exe)
}

/// Disables autostart for the current user. Never deletes the executable itself.
pub fn disable() -> Result<(), String> {
    platform::disable()
}

/// If autostart is enabled but points to an executable that no longer exists (e.g. the user moved
/// the binary), updates the registration to point to the currently running executable.
pub fn sync_broken_path() {
    let Some(registered) = platform::read_registered_path() else {
        return;
    };
    if registered.exists() {
        return;
    }
    if let Ok(current_exe) = std::env::current_exe() {
        let _ = platform::enable(&current_exe);
    }
}

fn install_binary_copy(src: &Path, dest: &Path) -> Result<(), String> {
    if paths_match(src, dest) {
        return Ok(());
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Não foi possível criar a pasta {}: {e}", parent.display()))?;
    }
    std::fs::copy(src, dest).map_err(|e| {
        format!(
            "Não foi possível copiar para {} (feche outra instância do Gatin se estiver aberta): {e}",
            dest.display()
        )
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(dest) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = std::fs::set_permissions(dest, perms);
        }
    }

    Ok(())
}

fn paths_match(a: &Path, b: &Path) -> bool {
    let a = std::fs::canonicalize(a).unwrap_or_else(|_| a.to_path_buf());
    let b = std::fs::canonicalize(b).unwrap_or_else(|_| b.to_path_buf());
    #[cfg(target_os = "windows")]
    {
        a.to_string_lossy().eq_ignore_ascii_case(&b.to_string_lossy())
    }
    #[cfg(not(target_os = "windows"))]
    {
        a == b
    }
}

/// Detects whether a path looks like Downloads, Desktop, or a Temp folder.
fn is_transient_path(path: &Path) -> bool {
    let normalized = path
        .to_string_lossy()
        .replace('\\', "/")
        .to_lowercase();

    const MARKERS: &[&str] = &[
        "/downloads/",
        "/download/",
        "/desktop/",
        "/área de trabalho/",
        "/area de trabalho/",
        "/appdata/local/temp/",
        "/temp/",
        "/tmp/",
    ];

    MARKERS.iter().any(|m| normalized.contains(m))
}

#[cfg(any(target_os = "windows", test))]
fn quote_windows_command(exe_path: &Path) -> String {
    format!("\"{}\"", exe_path.display())
}

#[cfg(any(target_os = "windows", test))]
fn unquote_windows_command(raw: &str) -> Option<PathBuf> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let path_str = if let Some(rest) = trimmed.strip_prefix('"') {
        rest.split('"').next().unwrap_or("")
    } else {
        trimmed.split_whitespace().next().unwrap_or("")
    };
    if path_str.is_empty() {
        None
    } else {
        Some(PathBuf::from(path_str))
    }
}

#[cfg(any(target_os = "linux", test))]
fn format_desktop_entry(exe_path: &Path) -> String {
    let raw = exe_path.to_string_lossy();
    let mut escaped = String::with_capacity(raw.len() + 4);
    for ch in raw.chars() {
        if matches!(ch, '"' | '`' | '$' | '\\') {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Version=1.0\n\
         Name=Gatin\n\
         Comment=Desktop pet que digita com você\n\
         Exec=\"{escaped}\"\n\
         Terminal=false\n\
         X-GNOME-Autostart-enabled=true\n"
    )
}

#[cfg(any(target_os = "linux", test))]
fn parse_desktop_entry_exec(contents: &str) -> Option<PathBuf> {
    let mut exec_line: Option<&str> = None;
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case("Hidden=true")
            || trimmed.eq_ignore_ascii_case("X-GNOME-Autostart-enabled=false")
        {
            return None;
        }
        if let Some(rest) = trimmed.strip_prefix("Exec=") {
            exec_line = Some(rest.trim());
        }
    }
    let raw = exec_line?;
    if raw.is_empty() {
        return None;
    }
    if let Some(inner) = raw.strip_prefix('"') {
        let mut out = String::new();
        let mut chars = inner.chars();
        while let Some(ch) = chars.next() {
            if ch == '\\' {
                if let Some(next) = chars.next() {
                    out.push(next);
                }
            } else if ch == '"' {
                break;
            } else {
                out.push(ch);
            }
        }
        if out.is_empty() {
            None
        } else {
            Some(PathBuf::from(out))
        }
    } else {
        Some(PathBuf::from(raw.split_whitespace().next()?))
    }
}

#[cfg(any(target_os = "macos", test))]
fn format_macos_plist(exe_path: &Path) -> String {
    let raw = exe_path.to_string_lossy();
    let mut escaped = String::with_capacity(raw.len());
    for ch in raw.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(ch),
        }
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n\
         <dict>\n\
           <key>Label</key>\n\
           <string>com.c0p3rn1cus.gatin</string>\n\
           <key>ProgramArguments</key>\n\
           <array>\n\
             <string>{escaped}</string>\n\
           </array>\n\
           <key>RunAtLoad</key>\n\
           <true/>\n\
           <key>KeepAlive</key>\n\
           <false/>\n\
         </dict>\n\
         </plist>\n"
    )
}

#[cfg(target_os = "windows")]
mod platform {
    use super::{quote_windows_command, unquote_windows_command};
    use std::ffi::c_void;
    use std::path::{Path, PathBuf};
    use std::ptr;

    type Hkey = *mut c_void;

    // ((HKEY)(ULONG_PTR)((LONG)0x80000001)) from <winreg.h>
    const HKEY_CURRENT_USER: Hkey = (-2147483647_i32 as isize) as Hkey;
    const KEY_QUERY_VALUE: u32 = 0x0001;
    const KEY_SET_VALUE: u32 = 0x0002;
    const REG_SZ: u32 = 1;
    const REG_OPTION_NON_VOLATILE: u32 = 0;
    const ERROR_SUCCESS: i32 = 0;
    const ERROR_FILE_NOT_FOUND: i32 = 2;

    const RUN_SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const VALUE_NAME: &str = "Gatin";

    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegCreateKeyExW(
            h_key: Hkey,
            lp_sub_key: *const u16,
            reserved: u32,
            lp_class: *const u16,
            dw_options: u32,
            sam_desired: u32,
            lp_security_attributes: *const c_void,
            phk_result: *mut Hkey,
            lpdw_disposition: *mut u32,
        ) -> i32;

        fn RegOpenKeyExW(
            h_key: Hkey,
            lp_sub_key: *const u16,
            ul_options: u32,
            sam_desired: u32,
            phk_result: *mut Hkey,
        ) -> i32;

        fn RegSetValueExW(
            h_key: Hkey,
            lp_value_name: *const u16,
            reserved: u32,
            dw_type: u32,
            lp_data: *const u8,
            cb_data: u32,
        ) -> i32;

        fn RegDeleteValueW(h_key: Hkey, lp_value_name: *const u16) -> i32;

        fn RegQueryValueExW(
            h_key: Hkey,
            lp_value_name: *const u16,
            lp_reserved: *mut u32,
            lp_type: *mut u32,
            lp_data: *mut u8,
            lpcb_data: *mut u32,
        ) -> i32;

        fn RegCloseKey(h_key: Hkey) -> i32;
    }

    struct RegKeyGuard(Hkey);

    impl Drop for RegKeyGuard {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe {
                    RegCloseKey(self.0);
                }
            }
        }
    }

    fn to_wide_null(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn enable(exe_path: &Path) -> Result<(), String> {
        let subkey = to_wide_null(RUN_SUBKEY);
        let value_name = to_wide_null(VALUE_NAME);
        let cmd = quote_windows_command(exe_path);
        let wide_cmd = to_wide_null(&cmd);

        let mut hkey: Hkey = ptr::null_mut();
        let status = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                subkey.as_ptr(),
                0,
                ptr::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                ptr::null(),
                &mut hkey,
                ptr::null_mut(),
            )
        };
        if status != ERROR_SUCCESS {
            return Err(format!("Erro ao abrir chave de inicialização (código {status})"));
        }
        let _guard = RegKeyGuard(hkey);

        let byte_len = (wide_cmd.len() * 2) as u32;
        let status = unsafe {
            RegSetValueExW(
                hkey,
                value_name.as_ptr(),
                0,
                REG_SZ,
                wide_cmd.as_ptr().cast::<u8>(),
                byte_len,
            )
        };
        if status != ERROR_SUCCESS {
            return Err(format!("Erro ao salvar no Registro do Windows (código {status})"));
        }
        Ok(())
    }

    pub fn disable() -> Result<(), String> {
        let subkey = to_wide_null(RUN_SUBKEY);
        let value_name = to_wide_null(VALUE_NAME);
        let mut hkey: Hkey = ptr::null_mut();
        let status = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                subkey.as_ptr(),
                0,
                KEY_SET_VALUE,
                &mut hkey,
            )
        };
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(());
        }
        if status != ERROR_SUCCESS {
            return Err(format!("Erro ao abrir chave de inicialização (código {status})"));
        }
        let _guard = RegKeyGuard(hkey);

        let status = unsafe { RegDeleteValueW(hkey, value_name.as_ptr()) };
        if status == ERROR_SUCCESS || status == ERROR_FILE_NOT_FOUND {
            Ok(())
        } else {
            Err(format!("Erro ao remover do Registro do Windows (código {status})"))
        }
    }

    pub fn read_registered_path() -> Option<PathBuf> {
        let subkey = to_wide_null(RUN_SUBKEY);
        let value_name = to_wide_null(VALUE_NAME);
        let mut hkey: Hkey = ptr::null_mut();
        let status = unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                subkey.as_ptr(),
                0,
                KEY_QUERY_VALUE,
                &mut hkey,
            )
        };
        if status != ERROR_SUCCESS {
            return None;
        }
        let _guard = RegKeyGuard(hkey);

        let mut reg_type: u32 = 0;
        let mut cb_data: u32 = 0;
        let status = unsafe {
            RegQueryValueExW(
                hkey,
                value_name.as_ptr(),
                ptr::null_mut(),
                &mut reg_type,
                ptr::null_mut(),
                &mut cb_data,
            )
        };
        if status != ERROR_SUCCESS || reg_type != REG_SZ || cb_data == 0 {
            return None;
        }

        let mut buf = vec![0u16; (cb_data as usize).div_ceil(2)];
        let mut buf_bytes = (buf.len() * 2) as u32;
        let status = unsafe {
            RegQueryValueExW(
                hkey,
                value_name.as_ptr(),
                ptr::null_mut(),
                &mut reg_type,
                buf.as_mut_ptr().cast::<u8>(),
                &mut buf_bytes,
            )
        };
        if status != ERROR_SUCCESS {
            return None;
        }

        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        let raw = String::from_utf16_lossy(&buf[..len]);
        unquote_windows_command(&raw)
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use super::{format_desktop_entry, parse_desktop_entry_exec};
    use directories::BaseDirs;
    use std::path::{Path, PathBuf};

    fn desktop_file_path() -> Option<PathBuf> {
        BaseDirs::new().map(|d| d.config_dir().join("autostart").join("gatin.desktop"))
    }

    pub fn enable(exe_path: &Path) -> Result<(), String> {
        let path = desktop_file_path()
            .ok_or_else(|| "Não foi possível localizar ~/.config/autostart".to_string())?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Não foi possível criar {}: {e}", parent.display()))?;
        }
        let content = format_desktop_entry(exe_path);
        std::fs::write(&path, content)
            .map_err(|e| format!("Não foi possível salvar {}: {e}", path.display()))
    }

    pub fn disable() -> Result<(), String> {
        let Some(path) = desktop_file_path() else {
            return Ok(());
        };
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("Não foi possível remover {}: {e}", path.display())),
        }
    }

    pub fn read_registered_path() -> Option<PathBuf> {
        let path = desktop_file_path()?;
        let contents = std::fs::read_to_string(path).ok()?;
        parse_desktop_entry_exec(&contents)
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::format_macos_plist;
    use directories::BaseDirs;
    use std::path::{Path, PathBuf};

    fn plist_path() -> Option<PathBuf> {
        BaseDirs::new().map(|d| {
            d.home_dir()
                .join("Library")
                .join("LaunchAgents")
                .join("com.c0p3rn1cus.gatin.plist")
        })
    }

    pub fn enable(exe_path: &Path) -> Result<(), String> {
        let path = plist_path()
            .ok_or_else(|| "Não foi possível localizar ~/Library/LaunchAgents".to_string())?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Não foi possível criar {}: {e}", parent.display()))?;
        }
        let content = format_macos_plist(exe_path);
        std::fs::write(&path, content)
            .map_err(|e| format!("Não foi possível salvar {}: {e}", path.display()))
    }

    pub fn disable() -> Result<(), String> {
        let Some(path) = plist_path() else {
            return Ok(());
        };
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("Não foi possível remover {}: {e}", path.display())),
        }
    }

    pub fn read_registered_path() -> Option<PathBuf> {
        let path = plist_path()?;
        let contents = std::fs::read_to_string(path).ok()?;
        // Extract first <string> inside <array>...</array>
        let array_part = contents.split("<array>").nth(1)?;
        let start = array_part.find("<string>")? + "<string>".len();
        let end = array_part[start..].find("</string>")?;
        let raw = &array_part[start..start + end];
        if raw.is_empty() {
            None
        } else {
            let unescaped = raw
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&quot;", "\"")
                .replace("&apos;", "'")
                .replace("&amp;", "&");
            Some(PathBuf::from(unescaped))
        }
    }
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
mod platform {
    use std::path::{Path, PathBuf};

    pub fn enable(_exe_path: &Path) -> Result<(), String> {
        Err("Inicialização automática não suportada neste sistema".to_string())
    }

    pub fn disable() -> Result<(), String> {
        Ok(())
    }

    pub fn read_registered_path() -> Option<PathBuf> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transient_path_detects_downloads_temp_and_desktop() {
        assert!(is_transient_path(Path::new(
            r"C:\Users\Joao\Downloads\gatin-windows-x86_64\gatin.exe"
        )));
        assert!(is_transient_path(Path::new(
            r"C:\Users\Joao\AppData\Local\Temp\Temp1_gatin.zip\gatin.exe"
        )));
        assert!(is_transient_path(Path::new(
            r"C:\Users\Joao\Desktop\gatin.exe"
        )));
        assert!(is_transient_path(Path::new("/home/user/Downloads/gatin")));
        assert!(is_transient_path(Path::new("/tmp/gatin")));

        assert!(!is_transient_path(Path::new(
            r"C:\Users\Joao\AppData\Local\Programs\Gatin\gatin.exe"
        )));
        assert!(!is_transient_path(Path::new("/home/user/.local/bin/gatin")));
    }

    #[test]
    fn windows_command_quoting_roundtrips_with_spaces() {
        let p = Path::new(r"C:\Users\Maria Silva\AppData\Local\Programs\Gatin\gatin.exe");
        let quoted = quote_windows_command(p);
        assert_eq!(
            quoted,
            r#""C:\Users\Maria Silva\AppData\Local\Programs\Gatin\gatin.exe""#
        );
        assert_eq!(unquote_windows_command(&quoted).as_deref(), Some(p));
    }

    #[test]
    fn linux_desktop_entry_roundtrips_with_spaces_and_special_chars() {
        let p = Path::new("/home/my user/.local/bin/gatin");
        let entry = format_desktop_entry(p);
        assert!(entry.contains("X-GNOME-Autostart-enabled=true"));
        assert_eq!(parse_desktop_entry_exec(&entry).as_deref(), Some(p));
    }

    #[test]
    fn macos_plist_escapes_xml_characters() {
        let p = Path::new("/Applications/Gatin & Friends.app/Contents/MacOS/gatin");
        let plist = format_macos_plist(p);
        assert!(plist.contains("<string>/Applications/Gatin &amp; Friends.app/Contents/MacOS/gatin</string>"));
        assert!(plist.contains("<key>RunAtLoad</key>"));
    }
}
