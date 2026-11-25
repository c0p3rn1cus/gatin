use super::{InputEvent, InputStatus};
use crate::keyboard::{KeyId, MouseButton};
use std::ffi::c_void;
use std::mem::{MaybeUninit, size_of};
use std::sync::OnceLock;
use std::sync::mpsc::Sender;

type Hwnd = *mut c_void;
type Hinstance = *mut c_void;
type Hrawinput = *mut c_void;
type Wparam = usize;
type Lparam = isize;
type Lresult = isize;
type WndProc = unsafe extern "system" fn(Hwnd, u32, Wparam, Lparam) -> Lresult;

const HWND_MESSAGE: Hwnd = (-3isize) as Hwnd;
const WM_INPUT: u32 = 0x00FF;

const RID_INPUT: u32 = 0x1000_0003;
const RIDEV_INPUTSINK: u32 = 0x0000_0100;

const HID_USAGE_PAGE_GENERIC: u16 = 0x01;
const HID_USAGE_GENERIC_MOUSE: u16 = 0x02;
const HID_USAGE_GENERIC_KEYBOARD: u16 = 0x06;

const RIM_TYPEMOUSE: u32 = 0;
const RIM_TYPEKEYBOARD: u32 = 1;

const RI_KEY_BREAK: u16 = 0x0001;
const RI_KEY_E0: u16 = 0x0002;

const RI_MOUSE_LEFT_BUTTON_DOWN: u16 = 0x0001;
const RI_MOUSE_LEFT_BUTTON_UP: u16 = 0x0002;
const RI_MOUSE_RIGHT_BUTTON_DOWN: u16 = 0x0004;
const RI_MOUSE_RIGHT_BUTTON_UP: u16 = 0x0008;
const RI_MOUSE_MIDDLE_BUTTON_DOWN: u16 = 0x0010;
const RI_MOUSE_MIDDLE_BUTTON_UP: u16 = 0x0020;

const SM_CXSCREEN: i32 = 0;
const SM_CYSCREEN: i32 = 1;
const SM_XVIRTUALSCREEN: i32 = 76;
const SM_YVIRTUALSCREEN: i32 = 77;
const SM_CXVIRTUALSCREEN: i32 = 78;
const SM_CYVIRTUALSCREEN: i32 = 79;

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
struct WndClassW {
    style: u32,
    lpfn_wnd_proc: WndProc,
    cb_cls_extra: i32,
    cb_wnd_extra: i32,
    h_instance: Hinstance,
    h_icon: *mut c_void,
    h_cursor: *mut c_void,
    hbr_background: *mut c_void,
    lpsz_menu_name: *const u16,
    lpsz_class_name: *const u16,
}

#[repr(C)]
struct RawInputDevice {
    usage_page: u16,
    usage: u16,
    flags: u32,
    hwnd_target: Hwnd,
}

#[repr(C)]
struct RawInputHeader {
    dw_type: u32,
    dw_size: u32,
    h_device: *mut c_void,
    w_param: Wparam,
}

#[repr(C)]
struct RawMouse {
    us_flags: u16,
    _pad: u16,
    us_button_flags: u16,
    us_button_data: u16,
    ul_raw_buttons: u32,
    last_x: i32,
    last_y: i32,
    ul_extra_information: u32,
}

#[repr(C)]
struct RawKeyboard {
    make_code: u16,
    flags: u16,
    reserved: u16,
    vkey: u16,
    message: u32,
    extra_information: u32,
}

#[link(name = "user32")]
unsafe extern "system" {
    fn RegisterClassW(wc: *const WndClassW) -> u16;
    fn CreateWindowExW(
        ex_style: u32,
        class_name: *const u16,
        window_name: *const u16,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: Hwnd,
        menu: *mut c_void,
        instance: Hinstance,
        param: *mut c_void,
    ) -> Hwnd;
    fn DestroyWindow(hwnd: Hwnd) -> i32;
    fn DefWindowProcW(hwnd: Hwnd, msg: u32, wparam: Wparam, lparam: Lparam) -> Lresult;
    fn GetMessageW(lpmsg: *mut Msg, hwnd: Hwnd, min: u32, max: u32) -> i32;
    fn TranslateMessage(lpmsg: *const Msg) -> i32;
    fn DispatchMessageW(lpmsg: *const Msg) -> Lresult;
    fn RegisterRawInputDevices(devices: *const RawInputDevice, num_devices: u32, size: u32) -> i32;
    fn GetRawInputData(
        hrawinput: Hrawinput,
        ui_command: u32,
        data: *mut c_void,
        size: *mut u32,
        header_size: u32,
    ) -> u32;
    fn GetCursorPos(pt: *mut Point) -> i32;
    fn GetSystemMetrics(index: i32) -> i32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(module_name: *const u16) -> Hinstance;
}

static GLOBAL_TX: OnceLock<Sender<InputEvent>> = OnceLock::new();

unsafe fn handle_raw_input(lparam: Lparam) {
    let Some(tx) = GLOBAL_TX.get() else {
        return;
    };

    let hraw = lparam as Hrawinput;
    let header_size = size_of::<RawInputHeader>() as u32;
    let mut size: u32 = 0;

    let query = unsafe {
        GetRawInputData(
            hraw,
            RID_INPUT,
            std::ptr::null_mut(),
            &mut size,
            header_size,
        )
    };
    if query == u32::MAX || size < header_size {
        return;
    }

    let mut stack_buf = [0u64; 16];
    let mut heap_buf: Vec<u64>;
    let ptr: *mut c_void = if (size as usize) <= size_of::<[u64; 16]>() {
        stack_buf.as_mut_ptr() as *mut c_void
    } else {
        heap_buf = vec![0u64; (size as usize).div_ceil(size_of::<u64>())];
        heap_buf.as_mut_ptr() as *mut c_void
    };

    let copied = unsafe { GetRawInputData(hraw, RID_INPUT, ptr, &mut size, header_size) };
    if copied == u32::MAX || (copied as usize) < size_of::<RawInputHeader>() {
        return;
    }

    let header = unsafe { &*(ptr as *const RawInputHeader) };
    let data_ptr = unsafe { (ptr as *const u8).add(size_of::<RawInputHeader>()) };

    match header.dw_type {
        RIM_TYPEKEYBOARD => {
            if (copied as usize) < size_of::<RawInputHeader>() + size_of::<RawKeyboard>() {
                return;
            }
            let kbd = unsafe { &*(data_ptr as *const RawKeyboard) };
            // 0xFF is Windows' sentinel for virtual/discarded prefix keys (e.g. PrintScreen)
            if kbd.vkey == 0xFF {
                return;
            }
            if let Some(key) = raw_key_to_key_id(kbd.vkey, kbd.make_code, kbd.flags) {
                let is_up = (kbd.flags & RI_KEY_BREAK) != 0;
                let _ = tx.send(if is_up {
                    InputEvent::KeyUp(key)
                } else {
                    InputEvent::KeyDown(key)
                });
            }
        }
        RIM_TYPEMOUSE => {
            if (copied as usize) < size_of::<RawInputHeader>() + size_of::<RawMouse>() {
                return;
            }
            let mouse = unsafe { &*(data_ptr as *const RawMouse) };

            if mouse.last_x != 0 || mouse.last_y != 0 {
                let mut pt = Point { x: 0, y: 0 };
                if unsafe { GetCursorPos(&mut pt) } != 0 {
                    let vx = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
                    let vy = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
                    let mut vw = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
                    let mut vh = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
                    if vw <= 0 || vh <= 0 {
                        vw = unsafe { GetSystemMetrics(SM_CXSCREEN) }.max(1);
                        vh = unsafe { GetSystemMetrics(SM_CYSCREEN) }.max(1);
                    }
                    let norm_x = ((pt.x - vx) as f32 / vw as f32).clamp(0.0, 1.0);
                    let norm_y = ((pt.y - vy) as f32 / vh as f32).clamp(0.0, 1.0);
                    let _ = tx.send(InputEvent::MouseMove {
                        norm_x,
                        norm_y,
                        screen_px: Some([pt.x as f32, pt.y as f32]),
                        look_dir: None,
                    });
                }
            }

            let bf = mouse.us_button_flags;
            if (bf & RI_MOUSE_LEFT_BUTTON_DOWN) != 0 {
                let _ = tx.send(InputEvent::MouseDown(MouseButton::Left));
            }
            if (bf & RI_MOUSE_LEFT_BUTTON_UP) != 0 {
                let _ = tx.send(InputEvent::MouseUp(MouseButton::Left));
            }
            if (bf & RI_MOUSE_RIGHT_BUTTON_DOWN) != 0 {
                let _ = tx.send(InputEvent::MouseDown(MouseButton::Right));
            }
            if (bf & RI_MOUSE_RIGHT_BUTTON_UP) != 0 {
                let _ = tx.send(InputEvent::MouseUp(MouseButton::Right));
            }
            if (bf & RI_MOUSE_MIDDLE_BUTTON_DOWN) != 0 {
                let _ = tx.send(InputEvent::MouseDown(MouseButton::Middle));
            }
            if (bf & RI_MOUSE_MIDDLE_BUTTON_UP) != 0 {
                let _ = tx.send(InputEvent::MouseUp(MouseButton::Middle));
            }
        }
        _ => {}
    }
}

unsafe extern "system" fn raw_input_wnd_proc(
    hwnd: Hwnd,
    msg: u32,
    wparam: Wparam,
    lparam: Lparam,
) -> Lresult {
    if msg == WM_INPUT {
        unsafe { handle_raw_input(lparam) };
    }
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

pub fn start(tx: Sender<InputEvent>) -> InputStatus {
    let _ = GLOBAL_TX.set(tx);
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();

    std::thread::Builder::new()
        .name("gatin-win32-rawinput".into())
        .spawn(move || {
            let hmod = unsafe { GetModuleHandleW(std::ptr::null()) };
            let class_name: Vec<u16> = "GatinRawInputSink\0".encode_utf16().collect();

            let wc = WndClassW {
                style: 0,
                lpfn_wnd_proc: raw_input_wnd_proc,
                cb_cls_extra: 0,
                cb_wnd_extra: 0,
                h_instance: hmod,
                h_icon: std::ptr::null_mut(),
                h_cursor: std::ptr::null_mut(),
                hbr_background: std::ptr::null_mut(),
                lpsz_menu_name: std::ptr::null(),
                lpsz_class_name: class_name.as_ptr(),
            };

            let _atom = unsafe { RegisterClassW(&wc) };
            let hwnd = unsafe {
                CreateWindowExW(
                    0,
                    class_name.as_ptr(),
                    std::ptr::null(),
                    0,
                    0,
                    0,
                    0,
                    0,
                    HWND_MESSAGE,
                    std::ptr::null_mut(),
                    hmod,
                    std::ptr::null_mut(),
                )
            };

            if hwnd.is_null() {
                let _ = ready_tx.send(false);
                return;
            }

            let devices = [
                RawInputDevice {
                    usage_page: HID_USAGE_PAGE_GENERIC,
                    usage: HID_USAGE_GENERIC_KEYBOARD,
                    flags: RIDEV_INPUTSINK,
                    hwnd_target: hwnd,
                },
                RawInputDevice {
                    usage_page: HID_USAGE_PAGE_GENERIC,
                    usage: HID_USAGE_GENERIC_MOUSE,
                    flags: RIDEV_INPUTSINK,
                    hwnd_target: hwnd,
                },
            ];

            let registered = unsafe {
                RegisterRawInputDevices(
                    devices.as_ptr(),
                    devices.len() as u32,
                    size_of::<RawInputDevice>() as u32,
                )
            } != 0;

            let _ = ready_tx.send(registered);
            if !registered {
                unsafe { DestroyWindow(hwnd) };
                return;
            }

            let mut msg = MaybeUninit::<Msg>::zeroed();
            while unsafe { GetMessageW(msg.as_mut_ptr(), std::ptr::null_mut(), 0, 0) } > 0 {
                unsafe {
                    TranslateMessage(msg.as_ptr());
                    DispatchMessageW(msg.as_ptr());
                }
            }

            unsafe { DestroyWindow(hwnd) };
        })
        .ok();

    let ok = ready_rx.recv().unwrap_or(false);
    if ok {
        InputStatus {
            backend: "Windows Raw Input".to_string(),
            global_active: true,
            hint: None,
        }
    } else {
        InputStatus {
            backend: "Local window only".to_string(),
            global_active: false,
            hint: Some("Falha ao registrar Windows Raw Input.".to_string()),
        }
    }
}

fn raw_key_to_key_id(vkey: u16, make_code: u16, flags: u16) -> Option<KeyId> {
    let is_e0 = (flags & RI_KEY_E0) != 0;
    Some(match vkey {
        0x08 => KeyId::Backspace,
        0x09 => KeyId::Tab,
        0x0D => KeyId::Enter,
        0x10 => {
            if make_code == 0x36 {
                KeyId::RightShift
            } else {
                KeyId::LeftShift
            }
        }
        0x11 => {
            if is_e0 {
                KeyId::RightCtrl
            } else {
                KeyId::LeftCtrl
            }
        }
        0x12 => {
            if is_e0 {
                KeyId::RightAlt
            } else {
                KeyId::LeftAlt
            }
        }
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
        0xA0 => KeyId::LeftShift,
        0xA1 => KeyId::RightShift,
        0xA2 => KeyId::LeftCtrl,
        0xA3 => KeyId::RightCtrl,
        0xA4 => KeyId::LeftAlt,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_input_struct_layouts_match_win32_abi() {
        assert_eq!(size_of::<RawMouse>(), 24);
        assert_eq!(size_of::<RawKeyboard>(), 16);
        assert_eq!(
            size_of::<RawInputHeader>(),
            if size_of::<usize>() == 8 { 24 } else { 16 }
        );
    }

    #[test]
    fn raw_modifier_keys_split_left_and_right() {
        assert_eq!(raw_key_to_key_id(0x10, 0x2A, 0), Some(KeyId::LeftShift));
        assert_eq!(raw_key_to_key_id(0x10, 0x36, 0), Some(KeyId::RightShift));
        assert_eq!(raw_key_to_key_id(0x11, 0x1D, 0), Some(KeyId::LeftCtrl));
        assert_eq!(
            raw_key_to_key_id(0x11, 0x1D, RI_KEY_E0),
            Some(KeyId::RightCtrl)
        );
        assert_eq!(raw_key_to_key_id(0x12, 0x38, 0), Some(KeyId::LeftAlt));
        assert_eq!(
            raw_key_to_key_id(0x12, 0x38, RI_KEY_E0),
            Some(KeyId::RightAlt)
        );
    }
}
