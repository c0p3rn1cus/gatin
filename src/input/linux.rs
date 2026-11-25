use super::{InputEvent, InputStatus};
use crate::keyboard::{KeyId, MouseButton};
use std::collections::HashSet;
use std::ffi::{CStr, CString, c_char, c_int, c_uint, c_ulong, c_void};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

// Linux input event constants (linux/input-event-codes.h)
const EV_KEY: u16 = 0x01;
const EV_REL: u16 = 0x02;
const REL_X: u16 = 0x00;
const REL_Y: u16 = 0x01;

const BTN_LEFT: u16 = 0x110;
const BTN_RIGHT: u16 = 0x111;
const BTN_MIDDLE: u16 = 0x112;

#[repr(C)]
#[derive(Clone, Copy)]
struct InputEventRaw {
    time: libc::timeval,
    type_: u16,
    code: u16,
    value: i32,
}

fn eviocgbit(ev: u32, len: u32) -> c_ulong {
    // _IOC(_IOC_READ=2, 'E', 0x20 + ev, len)
    ((2u32 << 30) | (len << 16) | ((b'E' as u32) << 8) | (0x20 + ev)) as c_ulong
}

fn bit_is_set(bytes: &[u8], bit: usize) -> bool {
    let idx = bit / 8;
    let mask = 1u8 << (bit % 8);
    bytes.get(idx).is_some_and(|b| (b & mask) != 0)
}

struct OpenedDevice {
    fd: c_int,
    path: String,
}

impl Drop for OpenedDevice {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}

fn scan_evdev_devices(existing: &mut Vec<OpenedDevice>) {
    let already: HashSet<String> = existing.iter().map(|d| d.path.clone()).collect();
    let Ok(entries) = std::fs::read_dir("/dev/input") else {
        return;
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if !name_str.starts_with("event") {
            continue;
        }
        let path = format!("/dev/input/{name_str}");
        if already.contains(&path) {
            continue;
        }
        let Ok(cpath) = CString::new(path.as_str()) else {
            continue;
        };
        let fd = unsafe { libc::open(cpath.as_ptr(), libc::O_RDONLY | libc::O_NONBLOCK) };
        if fd < 0 {
            continue;
        }

        let mut ev_bits = [0u8; 4];
        let ok = unsafe { libc::ioctl(fd, eviocgbit(0, ev_bits.len() as u32) as _, ev_bits.as_mut_ptr()) };
        if ok < 0 {
            unsafe { libc::close(fd) };
            continue;
        }

        let has_key = bit_is_set(&ev_bits, EV_KEY as usize);
        let has_rel = bit_is_set(&ev_bits, EV_REL as usize);

        let mut useful = false;
        if has_key {
            let mut key_bits = [0u8; 96];
            if unsafe {
                libc::ioctl(
                    fd,
                    eviocgbit(EV_KEY as u32, key_bits.len() as u32) as _,
                    key_bits.as_mut_ptr(),
                )
            } >= 0
            {
                // Keyboard (KEY_A=30, KEY_SPACE=57) or Mouse (BTN_LEFT=0x110)
                if bit_is_set(&key_bits, 30)
                    || bit_is_set(&key_bits, 57)
                    || bit_is_set(&key_bits, BTN_LEFT as usize)
                {
                    useful = true;
                }
            }
        }
        if has_rel {
            let mut rel_bits = [0u8; 4];
            if unsafe {
                libc::ioctl(
                    fd,
                    eviocgbit(EV_REL as u32, rel_bits.len() as u32) as _,
                    rel_bits.as_mut_ptr(),
                )
            } >= 0
                && (bit_is_set(&rel_bits, REL_X as usize) || bit_is_set(&rel_bits, REL_Y as usize))
            {
                useful = true;
            }
        }

        if useful {
            existing.push(OpenedDevice { fd, path });
        } else {
            unsafe { libc::close(fd) };
        }
    }
}

pub fn start(tx: Sender<InputEvent>) -> InputStatus {
    // 1. Try native Linux evdev first (works on BOTH Wayland and X11!)
    let mut devices = Vec::new();
    scan_evdev_devices(&mut devices);

    if !devices.is_empty() {
        let count = devices.len();
        let tx_ev = tx.clone();
        std::thread::Builder::new()
            .name("gatin-evdev".into())
            .spawn(move || run_evdev_loop(devices, tx_ev))
            .ok();

        // Bonus: on Hyprland Wayland, poll global cursor position via Hyprland IPC socket
        // so the cat's eyes track the cursor everywhere on screen!
        let hypr_active = spawn_hyprland_cursor_tracker(tx);
        let backend = if hypr_active {
            format!("Linux evdev ({count} devices) + Hyprland cursor")
        } else {
            format!("Linux evdev ({count} devices)")
        };

        return InputStatus {
            backend,
            global_active: true,
            hint: None,
        };
    }

    // 2. Fallback: X11 global polling via dynamically loaded libX11.so.6 (works on X11 with no permissions)
    if std::env::var_os("DISPLAY").is_some() && try_start_x11_poller(tx.clone()).is_some() {
        return InputStatus {
            backend: "Linux X11 (libX11)".to_string(),
            global_active: true,
            hint: None,
        };
    }

    // Even without evdev permissions, Hyprland cursor tracking still works for eyes/mouse movement!
    let hypr_active = spawn_hyprland_cursor_tracker(tx);

    let is_wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    let hint = if is_wayland {
        Some(
            "Para capturar teclado global no Wayland, adicione seu usuário ao grupo input:\n  sudo usermod -aG input $USER\ne faça logout/login (ou rode com --demo)."
                .to_string(),
        )
    } else {
        Some(
            "Permissão /dev/input indisponível e libX11 não conectou. Adicione seu usuário ao grupo input ou rode com --demo."
                .to_string(),
        )
    };

    InputStatus {
        backend: if hypr_active {
            "Local window + Hyprland cursor (sem permissão evdev)".to_string()
        } else {
            "Local window only".to_string()
        },
        global_active: false,
        hint,
    }
}

fn run_evdev_loop(mut devices: Vec<OpenedDevice>, tx: Sender<InputEvent>) {
    let mut last_rescan = Instant::now();
    let ev_size = std::mem::size_of::<InputEventRaw>();
    let mut buf = [InputEventRaw {
        time: libc::timeval {
            tv_sec: 0,
            tv_usec: 0,
        },
        type_: 0,
        code: 0,
        value: 0,
    }; 64];

    loop {
        if last_rescan.elapsed() >= Duration::from_secs(3) {
            scan_evdev_devices(&mut devices);
            last_rescan = Instant::now();
        }

        if devices.is_empty() {
            std::thread::sleep(Duration::from_millis(250));
            continue;
        }

        let mut pollfds: Vec<libc::pollfd> = devices
            .iter()
            .map(|d| libc::pollfd {
                fd: d.fd,
                events: libc::POLLIN,
                revents: 0,
            })
            .collect();

        let ret = unsafe { libc::poll(pollfds.as_mut_ptr(), pollfds.len() as libc::nfds_t, 100) };
        if ret <= 0 {
            continue;
        }

        let mut dead_indices = Vec::new();
        let mut rel_dx = 0.0_f32;
        let mut rel_dy = 0.0_f32;

        for (i, pfd) in pollfds.iter().enumerate() {
            if (pfd.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL)) != 0 {
                dead_indices.push(i);
                continue;
            }
            if (pfd.revents & libc::POLLIN) == 0 {
                continue;
            }
            loop {
                let bytes_read = unsafe {
                    libc::read(
                        pfd.fd,
                        buf.as_mut_ptr() as *mut c_void,
                        buf.len() * ev_size,
                    )
                };
                if bytes_read <= 0 {
                    break;
                }
                let count = (bytes_read as usize) / ev_size;
                for ev in &buf[..count] {
                    match ev.type_ {
                        EV_KEY => {
                            if let Some(btn) = match ev.code {
                                BTN_LEFT => Some(MouseButton::Left),
                                BTN_RIGHT => Some(MouseButton::Right),
                                BTN_MIDDLE => Some(MouseButton::Middle),
                                _ => None,
                            } {
                                let msg = if ev.value == 0 {
                                    InputEvent::MouseUp(btn)
                                } else {
                                    InputEvent::MouseDown(btn)
                                };
                                if tx.send(msg).is_err() {
                                    return;
                                }
                            } else if let Some(key) = linux_keycode_to_key_id(ev.code) {
                                // value: 0 = release, 1 = press, 2 = repeat
                                if ev.value == 1 {
                                    if tx.send(InputEvent::KeyDown(key)).is_err() {
                                        return;
                                    }
                                } else if ev.value == 0
                                    && tx.send(InputEvent::KeyUp(key)).is_err()
                                {
                                    return;
                                }
                            }
                        }
                        EV_REL => match ev.code {
                            REL_X => rel_dx += ev.value as f32,
                            REL_Y => rel_dy += ev.value as f32,
                            _ => {}
                        },
                        _ => {}
                    }
                }
            }
        }

        if (rel_dx.abs() > 0.0 || rel_dy.abs() > 0.0)
            && tx
                .send(InputEvent::MouseDelta {
                    dx: rel_dx,
                    dy: rel_dy,
                })
                .is_err()
        {
            return;
        }

        for idx in dead_indices.into_iter().rev() {
            devices.remove(idx);
        }
    }
}

/// Optional Hyprland IPC cursor tracker (gives exact screen cursor position on Hyprland Wayland).
fn spawn_hyprland_cursor_tracker(tx: Sender<InputEvent>) -> bool {
    let (Ok(sig), Ok(runtime_dir)) = (
        std::env::var("HYPRLAND_INSTANCE_SIGNATURE"),
        std::env::var("XDG_RUNTIME_DIR"),
    ) else {
        return false;
    };
    let sock_path = format!("{runtime_dir}/hypr/{sig}/.socket.sock");
    if query_hypr_socket(&sock_path, "cursorpos").is_none() {
        return false;
    }

    std::thread::Builder::new()
        .name("gatin-hypr-cursor".into())
        .spawn(move || {
            let (mut max_x, mut max_y) = (1920.0_f32, 1080.0_f32);
            let mut cat_eye_screen: Option<[f32; 2]> = None;
            let mut last_win_check = Instant::now() - Duration::from_secs(5);
            let mut last_pos = (-1.0_f32, -1.0_f32);
            loop {
                // Periodically query the cat window's exact position on Hyprland so eyes aim straight at the cursor
                if last_win_check.elapsed() >= Duration::from_millis(350) {
                    last_win_check = Instant::now();
                    if let Some(clients) = query_hypr_socket(&sock_path, "clients") {
                        cat_eye_screen = parse_hypr_gatin_eye_pos(&clients);
                    }
                    if let Some(mons) = query_hypr_socket(&sock_path, "monitors")
                        && let Some((mw, mh)) = parse_hypr_desktop_bounds(&mons)
                    {
                        max_x = mw;
                        max_y = mh;
                    }
                }

                if let Some(resp) = query_hypr_socket(&sock_path, "cursorpos")
                    && let Some((x_str, y_str)) = resp.trim().split_once(',')
                    && let (Ok(x), Ok(y)) = (
                        x_str.trim().parse::<f32>(),
                        y_str.trim().parse::<f32>(),
                    )
                {
                    if x > max_x {
                        max_x = x;
                    }
                    if y > max_y {
                        max_y = y;
                    }
                    if (x - last_pos.0).abs() > 0.5 || (y - last_pos.1).abs() > 0.5 {
                        last_pos = (x, y);
                        let norm_x = (x / max_x.max(1.0)).clamp(0.0, 1.0);
                        let norm_y = (y / max_y.max(1.0)).clamp(0.0, 1.0);
                        let look_dir = cat_eye_screen.map(|[ex, ey]| {
                            let dx = x - ex;
                            let dy = y - ey;
                            // Smooth nonlinear mapping so nearby AND far cursor motions both track clearly
                            let dist = (dx * dx + dy * dy).sqrt();
                            let mag = (dist / 220.0).min(1.0);
                            if dist > 1.0 {
                                [(dx / dist) * mag, (dy / dist) * mag]
                            } else {
                                [0.0, 0.0]
                            }
                        });
                        if tx
                            .send(InputEvent::MouseMove {
                                norm_x,
                                norm_y,
                                screen_px: Some([x, y]),
                                look_dir,
                            })
                            .is_err()
                        {
                            return;
                        }
                    }
                }
                std::thread::sleep(Duration::from_millis(18));
            }
        })
        .ok();

    true
}

fn parse_hypr_gatin_eye_pos(clients: &str) -> Option<[f32; 2]> {
    let mut cur_at: Option<[f32; 2]> = None;
    let mut cur_size: Option<[f32; 2]> = None;
    let mut is_gatin = false;

    for line in clients.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("Window ") {
            if is_gatin
                && let (Some([ax, ay]), Some([sw, sh])) = (cur_at, cur_size)
            {
                return Some([ax + sw * 0.505, ay + sh * 0.33]);
            }
            cur_at = None;
            cur_size = None;
            is_gatin = false;
        } else if let Some(rest) = trimmed.strip_prefix("at:") {
            if let Some((x, y)) = rest.trim().split_once(',')
                && let (Ok(x), Ok(y)) = (x.trim().parse::<f32>(), y.trim().parse::<f32>())
            {
                cur_at = Some([x, y]);
            }
        } else if let Some(rest) = trimmed.strip_prefix("size:") {
            if let Some((w, h)) = rest.trim().split_once(',')
                && let (Ok(w), Ok(h)) = (w.trim().parse::<f32>(), h.trim().parse::<f32>())
            {
                cur_size = Some([w, h]);
            }
        } else if let Some(rest) = trimmed
            .strip_prefix("class:")
            .or_else(|| trimmed.strip_prefix("initialClass:"))
            .or_else(|| trimmed.strip_prefix("title:"))
            && rest.trim().eq_ignore_ascii_case("gatin")
        {
            is_gatin = true;
        }
    }

    if is_gatin
        && let (Some([ax, ay]), Some([sw, sh])) = (cur_at, cur_size)
    {
        return Some([ax + sw * 0.505, ay + sh * 0.33]);
    }
    None
}

fn parse_hypr_desktop_bounds(monitors: &str) -> Option<(f32, f32)> {
    let mut max_w = 0.0_f32;
    let mut max_h = 0.0_f32;
    for line in monitors.lines() {
        let trimmed = line.trim();
        // Format: "1920x1080@60.00000 at 0x0"
        if let Some((res_part, at_part)) = trimmed.split_once(" at ")
            && let Some(dims) = res_part.split('@').next()
            && let Some((w_s, h_s)) = dims.split_once('x')
            && let Some((ax_s, ay_s)) = at_part.trim().split_once('x')
            && let (Ok(w), Ok(h), Ok(ax), Ok(ay)) = (
                w_s.trim().parse::<f32>(),
                h_s.trim().parse::<f32>(),
                ax_s.trim().parse::<f32>(),
                ay_s.trim().parse::<f32>(),
            )
        {
            max_w = max_w.max(ax + w);
            max_h = max_h.max(ay + h);
        }
    }
    if max_w > 100.0 && max_h > 100.0 {
        Some((max_w, max_h))
    } else {
        None
    }
}

fn query_hypr_socket(sock_path: &str, cmd: &str) -> Option<String> {
    let mut stream = UnixStream::connect(sock_path).ok()?;
    stream.set_read_timeout(Some(Duration::from_millis(50))).ok();
    stream.set_write_timeout(Some(Duration::from_millis(50))).ok();
    stream.write_all(cmd.as_bytes()).ok()?;
    let mut out = String::new();
    stream.read_to_string(&mut out).ok()?;
    Some(out)
}

/// Dynamic libX11 fallback for X11 desktops when /dev/input is not readable.
fn try_start_x11_poller(tx: Sender<InputEvent>) -> Option<()> {
    type DisplayPtr = *mut c_void;
    type Window = c_ulong;
    type XOpenDisplayFn = unsafe extern "C" fn(*const c_char) -> DisplayPtr;
    type XCloseDisplayFn = unsafe extern "C" fn(DisplayPtr) -> c_int;
    type XDefaultRootWindowFn = unsafe extern "C" fn(DisplayPtr) -> Window;
    type XDefaultScreenFn = unsafe extern "C" fn(DisplayPtr) -> c_int;
    type XDisplayWidthFn = unsafe extern "C" fn(DisplayPtr, c_int) -> c_int;
    type XDisplayHeightFn = unsafe extern "C" fn(DisplayPtr, c_int) -> c_int;
    type XQueryKeymapFn = unsafe extern "C" fn(DisplayPtr, *mut [u8; 32]) -> c_int;
    type XQueryPointerFn = unsafe extern "C" fn(
        DisplayPtr,
        Window,
        *mut Window,
        *mut Window,
        *mut c_int,
        *mut c_int,
        *mut c_int,
        *mut c_int,
        *mut c_uint,
    ) -> c_int;

    let lib_name = c"libX11.so.6";
    let handle = unsafe { libc::dlopen(lib_name.as_ptr(), libc::RTLD_LAZY) };
    if handle.is_null() {
        return None;
    }

    macro_rules! load_sym {
        ($name:literal, $ty:ty) => {{
            let sym = CStr::from_bytes_with_nul($name).ok()?;
            let ptr = unsafe { libc::dlsym(handle, sym.as_ptr()) };
            if ptr.is_null() {
                unsafe { libc::dlclose(handle) };
                return None;
            }
            unsafe { std::mem::transmute::<*mut c_void, $ty>(ptr) }
        }};
    }

    let x_open_display = load_sym!(b"XOpenDisplay\0", XOpenDisplayFn);
    let x_close_display = load_sym!(b"XCloseDisplay\0", XCloseDisplayFn);
    let x_default_root_window = load_sym!(b"XDefaultRootWindow\0", XDefaultRootWindowFn);
    let x_default_screen = load_sym!(b"XDefaultScreen\0", XDefaultScreenFn);
    let x_display_width = load_sym!(b"XDisplayWidth\0", XDisplayWidthFn);
    let x_display_height = load_sym!(b"XDisplayHeight\0", XDisplayHeightFn);
    let x_query_keymap = load_sym!(b"XQueryKeymap\0", XQueryKeymapFn);
    let x_query_pointer = load_sym!(b"XQueryPointer\0", XQueryPointerFn);

    let dpy = unsafe { x_open_display(std::ptr::null()) };
    if dpy.is_null() {
        unsafe { libc::dlclose(handle) };
        return None;
    }
    let dpy_addr = dpy as usize;
    let handle_addr = handle as usize;

    std::thread::Builder::new()
        .name("gatin-x11".into())
        .spawn(move || {
            let dpy = dpy_addr as DisplayPtr;
            let handle = handle_addr as *mut c_void;
            let root = unsafe { x_default_root_window(dpy) };
            let screen = unsafe { x_default_screen(dpy) };
            let width = unsafe { x_display_width(dpy, screen) }.max(1) as f32;
            let height = unsafe { x_display_height(dpy, screen) }.max(1) as f32;

            let mut prev_keys = [0u8; 32];
            let mut prev_x = -1;
            let mut prev_y = -1;
            let mut prev_mask: c_uint = 0;

            loop {
                let mut cur_keys = [0u8; 32];
                unsafe { x_query_keymap(dpy, &mut cur_keys) };

                for byte_idx in 0..32 {
                    let diff = cur_keys[byte_idx] ^ prev_keys[byte_idx];
                    if diff == 0 {
                        continue;
                    }
                    for bit in 0..8 {
                        if (diff & (1 << bit)) != 0 {
                            let x11_keycode = (byte_idx * 8 + bit) as u16;
                            // Standard Xorg evdev offset is +8
                            if x11_keycode >= 8
                                && let Some(key) = linux_keycode_to_key_id(x11_keycode - 8)
                            {
                                let pressed = (cur_keys[byte_idx] & (1 << bit)) != 0;
                                let ev = if pressed {
                                    InputEvent::KeyDown(key)
                                } else {
                                    InputEvent::KeyUp(key)
                                };
                                if tx.send(ev).is_err() {
                                    unsafe {
                                        x_close_display(dpy);
                                        libc::dlclose(handle);
                                    }
                                    return;
                                }
                            }
                        }
                    }
                }
                prev_keys = cur_keys;

                let (mut r_ret, mut c_ret) = (0, 0);
                let (mut rx, mut ry, mut wx, mut wy) = (0, 0, 0, 0);
                let mut mask: c_uint = 0;
                unsafe {
                    x_query_pointer(
                        dpy,
                        root,
                        &mut r_ret,
                        &mut c_ret,
                        &mut rx,
                        &mut ry,
                        &mut wx,
                        &mut wy,
                        &mut mask,
                    );
                }

                if rx != prev_x || ry != prev_y {
                    prev_x = rx;
                    prev_y = ry;
                    if tx
                        .send(InputEvent::MouseMove {
                            norm_x: (rx as f32 / width).clamp(0.0, 1.0),
                            norm_y: (ry as f32 / height).clamp(0.0, 1.0),
                            screen_px: Some([rx as f32, ry as f32]),
                            look_dir: None,
                        })
                        .is_err()
                    {
                        break;
                    }
                }

                // X11 Button1Mask = 1<<8, Button2Mask = 1<<9, Button3Mask = 1<<10
                for (bit, btn) in [
                    (1u32 << 8, MouseButton::Left),
                    (1u32 << 9, MouseButton::Middle),
                    (1u32 << 10, MouseButton::Right),
                ] {
                    let was = (prev_mask & bit) != 0;
                    let now = (mask & bit) != 0;
                    if now != was {
                        let ev = if now {
                            InputEvent::MouseDown(btn)
                        } else {
                            InputEvent::MouseUp(btn)
                        };
                        if tx.send(ev).is_err() {
                            break;
                        }
                    }
                }
                prev_mask = mask;

                std::thread::sleep(Duration::from_millis(10));
            }

            unsafe {
                x_close_display(dpy);
                libc::dlclose(handle);
            }
        })
        .ok()?;

    Some(())
}

fn linux_keycode_to_key_id(code: u16) -> Option<KeyId> {
    Some(match code {
        1 => KeyId::Escape,
        2 => KeyId::Digit1,
        3 => KeyId::Digit2,
        4 => KeyId::Digit3,
        5 => KeyId::Digit4,
        6 => KeyId::Digit5,
        7 => KeyId::Digit6,
        8 => KeyId::Digit7,
        9 => KeyId::Digit8,
        10 => KeyId::Digit9,
        11 => KeyId::Digit0,
        12 => KeyId::Minus,
        13 => KeyId::Equal,
        14 => KeyId::Backspace,
        15 => KeyId::Tab,
        16 => KeyId::Q,
        17 => KeyId::W,
        18 => KeyId::E,
        19 => KeyId::R,
        20 => KeyId::T,
        21 => KeyId::Y,
        22 => KeyId::U,
        23 => KeyId::I,
        24 => KeyId::O,
        25 => KeyId::P,
        26 => KeyId::LeftBracket,
        27 => KeyId::RightBracket,
        28 | 96 => KeyId::Enter,
        29 => KeyId::LeftCtrl,
        30 => KeyId::A,
        31 => KeyId::S,
        32 => KeyId::D,
        33 => KeyId::F,
        34 => KeyId::G,
        35 => KeyId::H,
        36 => KeyId::J,
        37 => KeyId::K,
        38 => KeyId::L,
        39 => KeyId::Semicolon,
        40 => KeyId::Quote,
        41 => KeyId::Grave,
        42 => KeyId::LeftShift,
        43 => KeyId::Backslash,
        44 => KeyId::Z,
        45 => KeyId::X,
        46 => KeyId::C,
        47 => KeyId::V,
        48 => KeyId::B,
        49 => KeyId::N,
        50 => KeyId::M,
        51 => KeyId::Comma,
        52 => KeyId::Period,
        53 => KeyId::Slash,
        54 => KeyId::RightShift,
        56 => KeyId::LeftAlt,
        57 => KeyId::Space,
        58 => KeyId::CapsLock,
        59 => KeyId::F1,
        60 => KeyId::F2,
        61 => KeyId::F3,
        62 => KeyId::F4,
        63 => KeyId::F5,
        64 => KeyId::F6,
        65 => KeyId::F7,
        66 => KeyId::F8,
        67 => KeyId::F9,
        68 => KeyId::F10,
        87 => KeyId::F11,
        88 => KeyId::F12,
        97 => KeyId::RightCtrl,
        100 => KeyId::RightAlt,
        103 => KeyId::ArrowUp,
        105 => KeyId::ArrowLeft,
        106 => KeyId::ArrowRight,
        108 => KeyId::ArrowDown,
        111 => KeyId::Delete,
        125 => KeyId::LeftMeta,
        126 => KeyId::RightMeta,
        127 => KeyId::Menu,
        _ => return None,
    })
}
