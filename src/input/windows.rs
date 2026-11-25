use super::{InputEvent, InputStatus};
use crate::keyboard::{KeyId, MouseButton};
use std::ffi::c_void;
use std::sync::mpsc::Sender;
use std::sync::OnceLock;

type Hhook = *mut c_void;
type Hinstance = *mut c_void;
type Hwnd = *mut c_void;
type Wparam = usize;
type Lparam = isize;
type Lresult = isize;
type HookProc = unsafe extern "system" fn(i32, Wparam, Lparam) -> Lresult;

const WH_KEYBOARD_LL: i32 = 13;
const WH_MOUSE_LL: i32 = 14;
const SM_CXSCREEN: i32 = 0;
const SM_CYSCREEN: i32 = 1;

const WM_KEYDOWN: usize = 0x0100;
const WM_KEYUP: usize = 0x0101;
const WM_SYSKEYDOWN: usize = 0x0104;
const WM_SYSKEYUP: usize = 0x0105;

const WM_MOUSEMOVE: usize = 0x0200;
const WM_LBUTTONDOWN: usize = 0x0201;
const WM_LBUTTONUP: usize = 0x0202;
const WM_RBUTTONDOWN: usize = 0x0204;
const WM_RBUTTONUP: usize = 0x0205;
const WM_MBUTTONDOWN: usize = 0x0207;
const WM_MBUTTONUP: usize = 0x0208;

#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}

#[repr(C)]
struct Msg {
    hwnd: Hwnd,
    message: u32,
    wparam: Wparam,
    lparam: Lparam,
    time: u32,
    pt: Point,
}

#[repr(C)]
struct KbdLlHookStruct {
    vk_code: u32,
    scan_code: u32,
    flags: u32,
    time: u32,
    extra_info: usize,
}

#[repr(C)]
struct MsLlHookStruct {
    pt: Point,
    mouse_data: u32,
    flags: u32,
    time: u32,
    extra_info: usize,
}

#[link(name = "user32")]
unsafe extern "system" {
    fn SetWindowsHookExW(id_hook: i32, lpfn: HookProc, hmod: Hinstance, thread_id: u32) -> Hhook;
    fn CallNextHookEx(hhk: Hhook, ncode: i32, wparam: Wparam, lparam: Lparam) -> Lresult;
    fn UnhookWindowsHookEx(hhk: Hhook) -> i32;
    fn GetMessageW(lpmsg: *mut Msg, hwnd: Hwnd, min: u32, max: u32) -> i32;
    fn GetSystemMetrics(index: i32) -> i32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(module_name: *const u16) -> Hinstance;
}

static GLOBAL_TX: OnceLock<Sender<InputEvent>> = OnceLock::new();

unsafe extern "system" fn keyboard_proc(code: i32, wparam: Wparam, lparam: Lparam) -> Lresult {
    if code >= 0 && lparam != 0 {
        let info = unsafe { &*(lparam as *const KbdLlHookStruct) };
        if let Some(key) = vk_to_key_id(info.vk_code) {
            if let Some(tx) = GLOBAL_TX.get() {
                match wparam {
                    WM_KEYDOWN | WM_SYSKEYDOWN => {
                        let _ = tx.send(InputEvent::KeyDown(key));
                    }
                    WM_KEYUP | WM_SYSKEYUP => {
                        let _ = tx.send(InputEvent::KeyUp(key));
                    }
                    _ => {}
                }
            }
        }
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: Wparam, lparam: Lparam) -> Lresult {
    if code >= 0 && lparam != 0 {
        let info = unsafe { &*(lparam as *const MsLlHookStruct) };
        if let Some(tx) = GLOBAL_TX.get() {
            match wparam {
                WM_MOUSEMOVE => {
                    let sw = unsafe { GetSystemMetrics(SM_CXSCREEN) }.max(1) as f32;
                    let sh = unsafe { GetSystemMetrics(SM_CYSCREEN) }.max(1) as f32;
                    let _ = tx.send(InputEvent::MouseMove {
                        norm_x: (info.pt.x as f32 / sw).clamp(0.0, 1.0),
                        norm_y: (info.pt.y as f32 / sh).clamp(0.0, 1.0),
                        screen_px: Some([info.pt.x as f32, info.pt.y as f32]),
                        look_dir: None,
                    });
                }
                WM_LBUTTONDOWN => {
                    let _ = tx.send(InputEvent::MouseDown(MouseButton::Left));
                }
                WM_LBUTTONUP => {
                    let _ = tx.send(InputEvent::MouseUp(MouseButton::Left));
                }
                WM_RBUTTONDOWN => {
                    let _ = tx.send(InputEvent::MouseDown(MouseButton::Right));
                }
                WM_RBUTTONUP => {
                    let _ = tx.send(InputEvent::MouseUp(MouseButton::Right));
                }
                WM_MBUTTONDOWN => {
                    let _ = tx.send(InputEvent::MouseDown(MouseButton::Middle));
                }
                WM_MBUTTONUP => {
                    let _ = tx.send(InputEvent::MouseUp(MouseButton::Middle));
                }
                _ => {}
            }
        }
    }
    unsafe { CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam) }
}

pub fn start(tx: Sender<InputEvent>) -> InputStatus {
    let _ = GLOBAL_TX.set(tx);
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();

    std::thread::Builder::new()
        .name("gatin-win32-hooks".into())
        .spawn(move || {
            let hmod = unsafe { GetModuleHandleW(std::ptr::null()) };
            let kbd_hook = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, keyboard_proc, hmod, 0) };
            let mouse_hook = unsafe { SetWindowsHookExW(WH_MOUSE_LL, mouse_proc, hmod, 0) };
            let ok = !kbd_hook.is_null() && !mouse_hook.is_null();
            let _ = ready_tx.send(ok);
            if !ok {
                if !kbd_hook.is_null() {
                    unsafe { UnhookWindowsHookEx(kbd_hook) };
                }
                if !mouse_hook.is_null() {
                    unsafe { UnhookWindowsHookEx(mouse_hook) };
                }
                return;
            }

            let mut msg = std::mem::MaybeUninit::<Msg>::zeroed();
            while unsafe { GetMessageW(msg.as_mut_ptr(), std::ptr::null_mut(), 0, 0) } > 0 {}

            unsafe {
                UnhookWindowsHookEx(kbd_hook);
                UnhookWindowsHookEx(mouse_hook);
            }
        })
        .ok();

    let ok = ready_rx.recv().unwrap_or(false);
    if ok {
        InputStatus {
            backend: "Windows Win32 Hooks".to_string(),
            global_active: true,
            hint: None,
        }
    } else {
        InputStatus {
            backend: "Local window only".to_string(),
            global_active: false,
            hint: Some("Falha ao registrar hooks Win32 globais.".to_string()),
        }
    }
}

fn vk_to_key_id(vk: u32) -> Option<KeyId> {
    Some(match vk {
        0x08 => KeyId::Backspace,
        0x09 => KeyId::Tab,
        0x0D => KeyId::Enter,
        0x14 => KeyId::CapsLock,
        0x1B => KeyId::Escape,
        0x20 => KeyId::Space,
        0x25 => KeyId::ArrowLeft,
        0x26 => KeyId::ArrowUp,
        0x27 => KeyId::ArrowRight,
        0x28 => KeyId::ArrowDown,
        0x2E => KeyId::Delete,
        0x30 => KeyId::Digit0,
        0x31 => KeyId::Digit1,
        0x32 => KeyId::Digit2,
        0x33 => KeyId::Digit3,
        0x34 => KeyId::Digit4,
        0x35 => KeyId::Digit5,
        0x36 => KeyId::Digit6,
        0x37 => KeyId::Digit7,
        0x38 => KeyId::Digit8,
        0x39 => KeyId::Digit9,
        0x41 => KeyId::A,
        0x42 => KeyId::B,
        0x43 => KeyId::C,
        0x44 => KeyId::D,
        0x45 => KeyId::E,
        0x46 => KeyId::F,
        0x47 => KeyId::G,
        0x48 => KeyId::H,
        0x49 => KeyId::I,
        0x4A => KeyId::J,
        0x4B => KeyId::K,
        0x4C => KeyId::L,
        0x4D => KeyId::M,
        0x4E => KeyId::N,
        0x4F => KeyId::O,
        0x50 => KeyId::P,
        0x51 => KeyId::Q,
        0x52 => KeyId::R,
        0x53 => KeyId::S,
        0x54 => KeyId::T,
        0x55 => KeyId::U,
        0x56 => KeyId::V,
        0x57 => KeyId::W,
        0x58 => KeyId::X,
        0x59 => KeyId::Y,
        0x5A => KeyId::Z,
        0x5B => KeyId::LeftMeta,
        0x5C => KeyId::RightMeta,
        0x5D => KeyId::Menu,
        0x70 => KeyId::F1,
        0x71 => KeyId::F2,
        0x72 => KeyId::F3,
        0x73 => KeyId::F4,
        0x74 => KeyId::F5,
        0x75 => KeyId::F6,
        0x76 => KeyId::F7,
        0x77 => KeyId::F8,
        0x78 => KeyId::F9,
        0x79 => KeyId::F10,
        0x7A => KeyId::F11,
        0x7B => KeyId::F12,
        0xA0 | 0x10 => KeyId::LeftShift,
        0xA1 => KeyId::RightShift,
        0xA2 | 0x11 => KeyId::LeftCtrl,
        0xA3 => KeyId::RightCtrl,
        0xA4 | 0x12 => KeyId::LeftAlt,
        0xA5 => KeyId::RightAlt,
        0xBA => KeyId::Semicolon,
        0xBB => KeyId::Equal,
        0xBC => KeyId::Comma,
        0xBD => KeyId::Minus,
        0xBE => KeyId::Period,
        0xBF => KeyId::Slash,
        0xC0 => KeyId::Grave,
        0xDB => KeyId::LeftBracket,
        0xDC => KeyId::Backslash,
        0xDD => KeyId::RightBracket,
        0xDE => KeyId::Quote,
        _ => return None,
    })
}
