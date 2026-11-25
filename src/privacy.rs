use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Cross-platform monitor that detects when the user has focused a password field
/// (in browsers, native apps, auth dialogs, or terminal sudo/ssh prompts).
pub struct PrivacyMonitor {
    is_password: Arc<AtomicBool>,
    pub backend: String,
    #[cfg(target_os = "linux")]
    restore_screen_reader: bool,
}

impl PrivacyMonitor {
    pub fn start(debug: bool) -> Self {
        let is_password = Arc::new(AtomicBool::new(false));

        #[cfg(target_os = "linux")]
        let (backend, restore_screen_reader) = linux::start(Arc::clone(&is_password), debug);

        #[cfg(target_os = "windows")]
        let backend = windows::start(Arc::clone(&is_password), debug);

        #[cfg(target_os = "macos")]
        let backend = macos::start(Arc::clone(&is_password), debug);

        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        let backend = {
            let _ = debug;
            "Manual only".to_string()
        };

        Self {
            is_password,
            backend,
            #[cfg(target_os = "linux")]
            restore_screen_reader,
        }
    }

    pub fn is_password_focused(&self) -> bool {
        self.is_password.load(Ordering::Relaxed)
    }
}

#[cfg(target_os = "linux")]
impl Drop for PrivacyMonitor {
    fn drop(&mut self) {
        if self.restore_screen_reader {
            linux::set_a11y_property("ScreenReaderEnabled", false);
        }
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use std::ffi::{CStr, c_char, c_int, c_void};
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    use std::process::Command;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    // From /usr/include/at-spi-2.0/atspi/atspi-constants.h
    const ATSPI_ROLE_PASSWORD_TEXT: c_int = 40;

    #[repr(C)]
    struct AtspiEvent {
        event_type: *const c_char,
        source: *mut c_void,
        detail1: c_int,
        detail2: c_int,
    }

    type AtspiInitFn = unsafe extern "C" fn() -> c_int;
    type AtspiEventMainFn = unsafe extern "C" fn();
    type AtspiGetDesktopFn = unsafe extern "C" fn(c_int) -> *mut c_void;
    type AtspiGetRoleFn = unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> c_int;
    type AtspiGetRoleNameFn = unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> *mut c_char;
    type AtspiGetNameFn = unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> *mut c_char;
    type AtspiGetParentFn = unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> *mut c_void;
    type AtspiGetChildCountFn = unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> c_int;
    type AtspiGetChildAtIndexFn =
        unsafe extern "C" fn(*mut c_void, c_int, *mut *mut c_void) -> *mut c_void;
    type GObjectUnrefFn = unsafe extern "C" fn(*mut c_void);
    type GFreeFn = unsafe extern "C" fn(*mut c_void);
    type GErrorFreeFn = unsafe extern "C" fn(*mut c_void);
    type AtspiEventListenerCb = unsafe extern "C" fn(*mut AtspiEvent, *mut c_void);
    type AtspiEventListenerNewFn =
        unsafe extern "C" fn(AtspiEventListenerCb, *mut c_void, *mut c_void) -> *mut c_void;
    type AtspiEventListenerRegisterFn =
        unsafe extern "C" fn(*mut c_void, *const c_char, *mut *mut c_void) -> c_int;

    struct AtspiCtx {
        atspi_pw: Arc<AtomicBool>,
        get_role: AtspiGetRoleFn,
        get_role_name: Option<AtspiGetRoleNameFn>,
        get_name: Option<AtspiGetNameFn>,
        get_parent: Option<AtspiGetParentFn>,
        get_child_count: Option<AtspiGetChildCountFn>,
        get_child_at_index: Option<AtspiGetChildAtIndexFn>,
        g_object_unref: Option<GObjectUnrefFn>,
        g_free: Option<GFreeFn>,
        g_error_free: Option<GErrorFreeFn>,
        debug: bool,
    }

    impl AtspiCtx {
        unsafe fn clear_err(&self, err: &mut *mut c_void) {
            if !err.is_null() {
                if let Some(free_err) = self.g_error_free {
                    unsafe { free_err(*err) };
                }
                *err = std::ptr::null_mut();
            }
        }

        unsafe fn is_password_node(&self, obj: *mut c_void) -> (bool, c_int) {
            if obj.is_null() {
                return (false, 0);
            }
            let mut err: *mut c_void = std::ptr::null_mut();
            let role = unsafe { (self.get_role)(obj, &mut err) };
            unsafe { self.clear_err(&mut err) };
            if role == ATSPI_ROLE_PASSWORD_TEXT {
                return (true, role);
            }

            // Fallback: check role name string in case a toolkit reports a variant role id
            if let Some(get_name) = self.get_role_name {
                let name_ptr = unsafe { get_name(obj, &mut err) };
                unsafe { self.clear_err(&mut err) };
                if !name_ptr.is_null() {
                    let is_pw_name = unsafe { CStr::from_ptr(name_ptr) }
                        .to_str()
                        .is_ok_and(|s| s.to_ascii_lowercase().contains("password"));
                    if let Some(g_free) = self.g_free {
                        unsafe { g_free(name_ptr as *mut c_void) };
                    }
                    if is_pw_name {
                        return (true, role);
                    }
                }
            }

            // Also check immediate parent (Chromium/Firefox sometimes focus an inline text child inside <input type="password">)
            if let Some(get_parent) = self.get_parent {
                let parent = unsafe { get_parent(obj, &mut err) };
                unsafe { self.clear_err(&mut err) };
                if !parent.is_null() {
                    let p_role = unsafe { (self.get_role)(parent, &mut err) };
                    unsafe { self.clear_err(&mut err) };
                    if let Some(unref) = self.g_object_unref {
                        unsafe { unref(parent) };
                    }
                    if p_role == ATSPI_ROLE_PASSWORD_TEXT {
                        return (true, p_role);
                    }
                }
            }

            (false, role)
        }

        /// Queries top-level children of a window/application to trigger Chromium's lazy
        /// accessibility activation (`NotifyAccessibilityApiUsage`) so web DOM `<input type="password">`
        /// focus events start flowing immediately!
        unsafe fn poke_accessible(&self, obj: *mut c_void, depth: usize) {
            if obj.is_null() || depth == 0 {
                return;
            }
            let Some(get_count) = self.get_child_count else {
                return;
            };
            let mut err: *mut c_void = std::ptr::null_mut();
            let count = unsafe { get_count(obj, &mut err) };
            unsafe { self.clear_err(&mut err) };
            if count <= 0 || depth <= 1 {
                return;
            }
            let Some(get_child) = self.get_child_at_index else {
                return;
            };
            let limit = count.min(4);
            for i in 0..limit {
                let child = unsafe { get_child(obj, i, &mut err) };
                unsafe { self.clear_err(&mut err) };
                if !child.is_null() {
                    unsafe { self.poke_accessible(child, depth - 1) };
                    if let Some(unref) = self.g_object_unref {
                        unsafe { unref(child) };
                    }
                }
            }
        }

        unsafe fn get_accessible_name(&self, obj: *mut c_void) -> String {
            if obj.is_null() {
                return String::new();
            }
            let Some(get_name) = self.get_name else {
                return String::new();
            };
            let mut err: *mut c_void = std::ptr::null_mut();
            let ptr = unsafe { get_name(obj, &mut err) };
            unsafe { self.clear_err(&mut err) };
            if ptr.is_null() {
                return String::new();
            }
            let s = unsafe { CStr::from_ptr(ptr) }
                .to_string_lossy()
                .into_owned();
            if let Some(g_free) = self.g_free {
                unsafe { g_free(ptr as *mut c_void) };
            }
            s
        }
    }

    unsafe extern "C" fn on_atspi_event(event: *mut AtspiEvent, user_data: *mut c_void) {
        if event.is_null() || user_data.is_null() {
            return;
        }
        let ctx = unsafe { &*(user_data as *const AtspiCtx) };
        let ev = unsafe { &*event };

        let ev_name = if ev.event_type.is_null() {
            ""
        } else {
            unsafe { CStr::from_ptr(ev.event_type) }
                .to_str()
                .unwrap_or("")
        };

        // Window lost focus -> clear password focus until the newly focused element reports in
        if ev_name.starts_with("window:deactivate") {
            if ctx.debug {
                eprintln!("[gatin-privacy] event={ev_name} -> password=false");
            }
            ctx.atspi_pw.store(false, Ordering::Relaxed);
            return;
        }

        // Window activated -> poke its accessible children to wake up Chromium's web DOM a11y tree!
        if ev_name.starts_with("window:activate") {
            if ctx.debug {
                let name = unsafe { ctx.get_accessible_name(ev.source) };
                eprintln!("[gatin-privacy] event={ev_name} win=\"{name}\" (poking a11y tree)");
            }
            unsafe { ctx.poke_accessible(ev.source, 3) };
            return;
        }

        if ev.source.is_null() {
            return;
        }

        let (is_pw, role) = unsafe { ctx.is_password_node(ev.source) };
        if ctx.debug {
            eprintln!(
                "[gatin-privacy] event={ev_name} detail1={} role={role} is_pw={is_pw}",
                ev.detail1
            );
        }

        if ev_name.contains("state-changed:focused") {
            if ev.detail1 == 1 {
                ctx.atspi_pw.store(is_pw, Ordering::Relaxed);
            } else if ev.detail1 == 0 && is_pw {
                ctx.atspi_pw.store(false, Ordering::Relaxed);
            }
        } else if ev_name.starts_with("focus:") || ev_name.contains("text-caret-moved") {
            ctx.atspi_pw.store(is_pw, Ordering::Relaxed);
        }
    }

    pub fn set_a11y_property(prop: &str, value: bool) {
        let val = if value { "<true>" } else { "<false>" };
        let _ = Command::new("gdbus")
            .args([
                "call",
                "--session",
                "--dest",
                "org.a11y.Bus",
                "--object-path",
                "/org/a11y/bus",
                "--method",
                "org.freedesktop.DBus.Properties.Set",
                "org.a11y.Status",
                prop,
                val,
            ])
            .output();
        if prop == "ScreenReaderEnabled" {
            let _ = Command::new("gsettings")
                .args([
                    "set",
                    "org.gnome.desktop.a11y.applications",
                    "screen-reader-enabled",
                    if value { "true" } else { "false" },
                ])
                .output();
        }
    }

    fn get_a11y_property(prop: &str) -> Option<bool> {
        let out = Command::new("gdbus")
            .args([
                "call",
                "--session",
                "--dest",
                "org.a11y.Bus",
                "--object-path",
                "/org/a11y/bus",
                "--method",
                "org.freedesktop.DBus.Properties.Get",
                "org.a11y.Status",
                prop,
            ])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&out.stdout);
        if text.contains("true") {
            Some(true)
        } else if text.contains("false") {
            Some(false)
        } else {
            None
        }
    }

    /// Synchronously activates `org.a11y.Bus` and sets BOTH `IsEnabled` and `ScreenReaderEnabled`
    /// to true. Chromium/Chrome/Brave/Electron REQUIRE `ScreenReaderEnabled = true` on
    /// `org.a11y.Status` to publish web-page DOM accessibility (including `<input type="password">`)!
    /// Returns true if we flipped `ScreenReaderEnabled` from false -> true (so we can restore it on exit).
    fn enable_atspi_bus_sync(debug: bool) -> bool {
        // 1. D-Bus-activate org.a11y.Bus (starts at-spi-bus-launcher if not running yet)
        let addr_out = Command::new("gdbus")
            .args([
                "call",
                "--session",
                "--dest",
                "org.a11y.Bus",
                "--object-path",
                "/org/a11y/bus",
                "--method",
                "org.a11y.Bus.GetAddress",
            ])
            .output();
        if debug {
            match &addr_out {
                Ok(o) => eprintln!(
                    "[gatin-privacy] org.a11y.Bus.GetAddress: {} {}",
                    String::from_utf8_lossy(&o.stdout).trim(),
                    String::from_utf8_lossy(&o.stderr).trim()
                ),
                Err(e) => eprintln!("[gatin-privacy] gdbus error: {e}"),
            }
        }

        let prev_sr = get_a11y_property("ScreenReaderEnabled").unwrap_or(false);

        // 2. Enable BOTH IsEnabled AND ScreenReaderEnabled (needed for Chromium/Brave web content!)
        set_a11y_property("IsEnabled", true);
        set_a11y_property("ScreenReaderEnabled", true);

        // 3. Also enable GTK/GNOME toolkit-accessibility setting (best-effort)
        let _ = Command::new("gsettings")
            .args([
                "set",
                "org.gnome.desktop.interface",
                "toolkit-accessibility",
                "true",
            ])
            .output();

        if debug {
            let is_en = get_a11y_property("IsEnabled");
            let sr_en = get_a11y_property("ScreenReaderEnabled");
            eprintln!("[gatin-privacy] org.a11y.Status: IsEnabled={is_en:?} ScreenReaderEnabled={sr_en:?}");
        }

        !prev_sr
    }

    pub fn start(combined_pw: Arc<AtomicBool>, debug: bool) -> (String, bool) {
        let atspi_pw = Arc::new(AtomicBool::new(false));
        let heur_pw = Arc::new(AtomicBool::new(false));

        // Activate AT-SPI bus & ScreenReaderEnabled BEFORE atspi_init() so Chromium/Firefox emit events live!
        let restore_sr = enable_atspi_bus_sync(debug);
        let atspi_ok = try_spawn_atspi_listener(Arc::clone(&atspi_pw), debug);
        spawn_system_heuristic_poller(
            Arc::clone(&heur_pw),
            Arc::clone(&atspi_pw),
            combined_pw,
            debug,
        );

        let backend = if atspi_ok {
            "Linux AT-SPI2 + Auth/TTY detector".to_string()
        } else {
            "Linux Auth/TTY detector (AT-SPI2 indisponível)".to_string()
        };
        (backend, restore_sr)
    }

    fn try_spawn_atspi_listener(atspi_pw: Arc<AtomicBool>, debug: bool) -> bool {
        let handle = unsafe { libc::dlopen(c"libatspi.so.0".as_ptr(), libc::RTLD_LAZY) };
        if handle.is_null() {
            if debug {
                eprintln!("[gatin-privacy] dlopen(libatspi.so.0) failed");
            }
            return false;
        }

        macro_rules! load_sym {
            ($name:literal, $ty:ty) => {{
                let ptr = unsafe { libc::dlsym(handle, $name.as_ptr()) };
                if ptr.is_null() {
                    unsafe { libc::dlclose(handle) };
                    return false;
                }
                unsafe { std::mem::transmute::<*mut c_void, $ty>(ptr) }
            }};
        }

        macro_rules! load_opt_sym {
            ($name:literal, $ty:ty) => {{
                let ptr = unsafe { libc::dlsym(handle, $name.as_ptr()) };
                if ptr.is_null() {
                    None
                } else {
                    Some(unsafe { std::mem::transmute::<*mut c_void, $ty>(ptr) })
                }
            }};
        }

        let atspi_init = load_sym!(c"atspi_init", AtspiInitFn);
        let atspi_event_main = load_sym!(c"atspi_event_main", AtspiEventMainFn);
        let get_role = load_sym!(c"atspi_accessible_get_role", AtspiGetRoleFn);
        let listener_new = load_sym!(c"atspi_event_listener_new", AtspiEventListenerNewFn);
        let listener_reg =
            load_sym!(c"atspi_event_listener_register", AtspiEventListenerRegisterFn);

        let get_desktop = load_opt_sym!(c"atspi_get_desktop", AtspiGetDesktopFn);
        let get_role_name = load_opt_sym!(c"atspi_accessible_get_role_name", AtspiGetRoleNameFn);
        let get_name = load_opt_sym!(c"atspi_accessible_get_name", AtspiGetNameFn);
        let get_parent = load_opt_sym!(c"atspi_accessible_get_parent", AtspiGetParentFn);
        let get_child_count =
            load_opt_sym!(c"atspi_accessible_get_child_count", AtspiGetChildCountFn);
        let get_child_at_index = load_opt_sym!(
            c"atspi_accessible_get_child_at_index",
            AtspiGetChildAtIndexFn
        );
        let g_object_unref = load_opt_sym!(c"g_object_unref", GObjectUnrefFn);
        let g_free = load_opt_sym!(c"g_free", GFreeFn);
        let g_error_free = load_opt_sym!(c"g_error_free", GErrorFreeFn);

        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("gatin-atspi".into())
            .spawn(move || {
                // Brief settle time if at-spi-bus-launcher was just D-Bus-activated
                std::thread::sleep(Duration::from_millis(80));

                // atspi_init() returns 0 on success or 1 if already initialized, 2 on error
                let init_res = unsafe { atspi_init() };
                if debug {
                    eprintln!("[gatin-privacy] atspi_init() = {init_res}");
                }
                if init_res > 1 {
                    let _ = ready_tx.send(false);
                    return;
                }

                let ctx = Box::into_raw(Box::new(AtspiCtx {
                    atspi_pw,
                    get_role,
                    get_role_name,
                    get_name,
                    get_parent,
                    get_child_count,
                    get_child_at_index,
                    g_object_unref,
                    g_free,
                    g_error_free,
                    debug,
                }));

                // Poke existing desktop apps on startup to wake Chromium/Electron lazy a11y trees!
                if let Some(get_desk) = get_desktop {
                    let desk = unsafe { get_desk(0) };
                    if !desk.is_null() {
                        let ctx_ref = unsafe { &*ctx };
                        if let (Some(gc), Some(gci)) = (get_child_count, get_child_at_index) {
                            let mut err: *mut c_void = std::ptr::null_mut();
                            let n = unsafe { gc(desk, &mut err) };
                            unsafe { ctx_ref.clear_err(&mut err) };
                            if debug {
                                eprintln!("[gatin-privacy] AT-SPI desktop apps: {n}");
                            }
                            for i in 0..n.min(24) {
                                let app = unsafe { gci(desk, i, &mut err) };
                                unsafe { ctx_ref.clear_err(&mut err) };
                                if !app.is_null() {
                                    if debug {
                                        let aname = unsafe { ctx_ref.get_accessible_name(app) };
                                        eprintln!("[gatin-privacy]   app[{i}] = \"{aname}\"");
                                    }
                                    unsafe { ctx_ref.poke_accessible(app, 3) };
                                    if let Some(unref) = g_object_unref {
                                        unsafe { unref(app) };
                                    }
                                }
                            }
                        }
                        if let Some(unref) = g_object_unref {
                            unsafe { unref(desk) };
                        }
                    }
                }

                let listener = unsafe {
                    listener_new(on_atspi_event, ctx as *mut c_void, std::ptr::null_mut())
                };
                if listener.is_null() {
                    unsafe { drop(Box::from_raw(ctx)) };
                    let _ = ready_tx.send(false);
                    return;
                }

                let mut err: *mut c_void = std::ptr::null_mut();
                let r1 = unsafe {
                    listener_reg(
                        listener,
                        c"object:state-changed:focused".as_ptr(),
                        &mut err,
                    )
                };
                let r2 = unsafe { listener_reg(listener, c"focus:".as_ptr(), &mut err) };
                let r3 = unsafe {
                    listener_reg(listener, c"object:text-caret-moved".as_ptr(), &mut err)
                };
                let r4 = unsafe { listener_reg(listener, c"window:activate".as_ptr(), &mut err) };
                let r5 =
                    unsafe { listener_reg(listener, c"window:deactivate".as_ptr(), &mut err) };

                if debug {
                    eprintln!(
                        "[gatin-privacy] registered listeners: focused={r1} focus={r2} caret={r3} activate={r4} deactivate={r5}"
                    );
                }

                if r1 == 0 {
                    let _ = ready_tx.send(false);
                    return;
                }
                let _ = ready_tx.send(true);
                unsafe { atspi_event_main() };
            })
            .ok();

        ready_rx
            .recv_timeout(Duration::from_millis(1200))
            .unwrap_or(false)
    }

    /// Polls Hyprland active window for polkit/pinentry dialogs & terminal no-echo password prompts
    /// (e.g., `sudo`, `ssh`, `passwd`) and merges with AT-SPI state.
    fn spawn_system_heuristic_poller(
        heur_pw: Arc<AtomicBool>,
        atspi_pw: Arc<AtomicBool>,
        combined_pw: Arc<AtomicBool>,
        debug: bool,
    ) {
        let hypr_sock = std::env::var("HYPRLAND_INSTANCE_SIGNATURE")
            .ok()
            .zip(std::env::var("XDG_RUNTIME_DIR").ok())
            .map(|(sig, rt)| format!("{rt}/hypr/{sig}/.socket.sock"));

        std::thread::Builder::new()
            .name("gatin-privacy-poll".into())
            .spawn(move || {
                let mut tick = 0u32;
                let mut prev_combined = false;
                loop {
                    if tick.is_multiple_of(3) {
                        let mut detected = false;
                        if let Some(ref sock) = hypr_sock
                            && let Some(info) = query_hypr_active_window(sock)
                        {
                            if is_auth_dialog(&info.class, &info.title) {
                                detected = true;
                            } else if let Some(pid) = info.pid
                                && is_terminal_class(&info.class)
                                && process_tree_has_hidden_tty_prompt(pid)
                            {
                                detected = true;
                            }
                        }
                        heur_pw.store(detected, Ordering::Relaxed);
                    }
                    tick = tick.wrapping_add(1);

                    let active =
                        atspi_pw.load(Ordering::Relaxed) || heur_pw.load(Ordering::Relaxed);
                    if debug && active != prev_combined {
                        eprintln!("[gatin-privacy] password_mode -> {active}");
                        prev_combined = active;
                    }
                    combined_pw.store(active, Ordering::Relaxed);
                    std::thread::sleep(Duration::from_millis(60));
                }
            })
            .ok();
    }

    struct ActiveWinInfo {
        class: String,
        title: String,
        pid: Option<i32>,
    }

    fn query_hypr_active_window(sock_path: &str) -> Option<ActiveWinInfo> {
        let mut stream = UnixStream::connect(sock_path).ok()?;
        stream.set_read_timeout(Some(Duration::from_millis(50))).ok();
        stream.set_write_timeout(Some(Duration::from_millis(50))).ok();
        stream.write_all(b"activewindow").ok()?;
        let mut out = String::new();
        stream.read_to_string(&mut out).ok()?;

        let mut class = String::new();
        let mut title = String::new();
        let mut pid = None;
        for line in out.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("class:") {
                class = rest.trim().to_ascii_lowercase();
            } else if let Some(rest) = trimmed.strip_prefix("title:") {
                title = rest.trim().to_ascii_lowercase();
            } else if let Some(rest) = trimmed.strip_prefix("pid:") {
                pid = rest.trim().parse::<i32>().ok();
            }
        }
        Some(ActiveWinInfo { class, title, pid })
    }

    fn is_auth_dialog(class: &str, title: &str) -> bool {
        const AUTH_CLASSES: &[&str] = &[
            "hyprpolkitagent",
            "polkit-gnome-authentication-agent",
            "polkit-kde-authentication-agent",
            "xfce-polkit",
            "lxqt-policykit",
            "gcr-prompter",
            "pinentry",
            "ssh-askpass",
            "ksshaskpass",
            "x11-ssh-askpass",
        ];
        if AUTH_CLASSES.iter().any(|p| class.contains(p)) {
            return true;
        }
        title.contains("authentication required")
            || title.contains("autenticação necessária")
            || title.contains("unlock keyring")
            || title.contains("desbloquear chaveiro")
    }

    fn is_terminal_class(class: &str) -> bool {
        const TERMS: &[&str] = &[
            "ghostty",
            "alacritty",
            "kitty",
            "foot",
            "wezterm",
            "gnome-terminal",
            "konsole",
            "xterm",
            "urxvt",
            "tilix",
            "st-256color",
            "rio",
            "warp",
        ];
        TERMS.iter().any(|t| class.contains(t))
    }

    fn process_tree_has_hidden_tty_prompt(root_pid: i32) -> bool {
        let mut stack = vec![root_pid];
        let mut visited = 0usize;

        while let Some(pid) = stack.pop() {
            visited += 1;
            if visited > 40 {
                break;
            }

            if pid != root_pid
                && let Ok(link) = std::fs::read_link(format!("/proc/{pid}/fd/0"))
                && let Some(path_str) = link.to_str()
                && path_str.starts_with("/dev/pts/")
                && tty_is_password_mode(path_str)
            {
                return true;
            }

            if let Ok(tasks) = std::fs::read_dir(format!("/proc/{pid}/task")) {
                for task in tasks.flatten() {
                    let children_path = task.path().join("children");
                    if let Ok(content) = std::fs::read_to_string(children_path) {
                        for tok in content.split_whitespace() {
                            if let Ok(child_pid) = tok.parse::<i32>() {
                                stack.push(child_pid);
                            }
                        }
                    }
                }
            }
        }
        false
    }

    fn tty_is_password_mode(pts_path: &str) -> bool {
        let Ok(cpath) = std::ffi::CString::new(pts_path) else {
            return false;
        };
        let fd = unsafe {
            libc::open(
                cpath.as_ptr(),
                libc::O_RDONLY | libc::O_NOCTTY | libc::O_NONBLOCK,
            )
        };
        if fd < 0 {
            return false;
        }
        let mut tio = std::mem::MaybeUninit::<libc::termios>::zeroed();
        let ok = unsafe { libc::tcgetattr(fd, tio.as_mut_ptr()) } == 0;
        unsafe { libc::close(fd) };
        if !ok {
            return false;
        }
        let tio = unsafe { tio.assume_init() };
        let canonical = (tio.c_lflag & libc::ICANON) != 0;
        let echo_off = (tio.c_lflag & libc::ECHO) == 0;
        canonical && echo_off
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use std::ffi::c_void;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    #[repr(C)]
    struct Guid {
        data1: u32,
        data2: u16,
        data3: u16,
        data4: [u8; 8],
    }

    const CLSID_CUI_AUTOMATION: Guid = Guid {
        data1: 0xFF48_DBA4,
        data2: 0x60EF,
        data3: 0x4201,
        data4: [0xAA, 0x87, 0x54, 0x10, 0x3E, 0xEF, 0x59, 0x4E],
    };

    const IID_IUI_AUTOMATION: Guid = Guid {
        data1: 0x30CB_E57D,
        data2: 0xD9D0,
        data3: 0x452A,
        data4: [0xAB, 0x13, 0x7A, 0xC5, 0xAC, 0x48, 0x25, 0xEE],
    };

    const COINIT_MULTITHREADED: u32 = 0x0;
    const CLSCTX_INPROC_SERVER: u32 = 0x1;

    #[link(name = "ole32")]
    unsafe extern "system" {
        fn CoInitializeEx(reserved: *mut c_void, coinit: u32) -> i32;
        fn CoUninitialize();
        fn CoCreateInstance(
            rclsid: *const Guid,
            outer: *mut c_void,
            cls_context: u32,
            riid: *const Guid,
            ppv: *mut *mut c_void,
        ) -> i32;
    }

    unsafe fn com_release(obj: *mut c_void) {
        if obj.is_null() {
            return;
        }
        let vtable = unsafe { *(obj as *const *const *const c_void) };
        let release: unsafe extern "system" fn(*mut c_void) -> u32 =
            unsafe { std::mem::transmute(*vtable.add(2)) };
        unsafe { release(obj) };
    }

    unsafe fn query_focused_is_password(uia: *mut c_void) -> bool {
        let vtable = unsafe { *(uia as *const *const *const c_void) };
        let get_focused: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32 =
            unsafe { std::mem::transmute(*vtable.add(8)) };

        let mut elem: *mut c_void = std::ptr::null_mut();
        if unsafe { get_focused(uia, &mut elem) } < 0 || elem.is_null() {
            return false;
        }

        let elem_vt = unsafe { *(elem as *const *const *const c_void) };
        let get_is_pw: unsafe extern "system" fn(*mut c_void, *mut i32) -> i32 =
            unsafe { std::mem::transmute(*elem_vt.add(38)) };

        let mut is_pw: i32 = 0;
        let hr = unsafe { get_is_pw(elem, &mut is_pw) };
        unsafe { com_release(elem) };
        hr >= 0 && is_pw != 0
    }

    pub fn start(is_password: Arc<AtomicBool>, debug: bool) -> String {
        std::thread::Builder::new()
            .name("gatin-win-privacy".into())
            .spawn(move || unsafe {
                let _ = CoInitializeEx(std::ptr::null_mut(), COINIT_MULTITHREADED);
                let mut uia: *mut c_void = std::ptr::null_mut();
                let hr = CoCreateInstance(
                    &CLSID_CUI_AUTOMATION,
                    std::ptr::null_mut(),
                    CLSCTX_INPROC_SERVER,
                    &IID_IUI_AUTOMATION,
                    &mut uia,
                );
                if hr < 0 || uia.is_null() {
                    CoUninitialize();
                    return;
                }

                let mut prev = false;
                loop {
                    let pw = query_focused_is_password(uia);
                    if debug && pw != prev {
                        eprintln!("[gatin-privacy] CurrentIsPassword -> {pw}");
                        prev = pw;
                    }
                    is_password.store(pw, Ordering::Relaxed);
                    std::thread::sleep(Duration::from_millis(120));
                }
            })
            .ok();

        "Windows UI Automation".to_string()
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    #[link(name = "Carbon", kind = "framework")]
    unsafe extern "C" {
        fn IsSecureEventInputEnabled() -> bool;
    }

    pub fn start(is_password: Arc<AtomicBool>, debug: bool) -> String {
        std::thread::Builder::new()
            .name("gatin-macos-privacy".into())
            .spawn(move || {
                let mut prev = false;
                loop {
                    let secure = unsafe { IsSecureEventInputEnabled() };
                    if debug && secure != prev {
                        eprintln!("[gatin-privacy] IsSecureEventInputEnabled -> {secure}");
                        prev = secure;
                    }
                    is_password.store(secure, Ordering::Relaxed);
                    std::thread::sleep(Duration::from_millis(100));
                }
            })
            .ok();

        "macOS SecureEventInput".to_string()
    }
}
