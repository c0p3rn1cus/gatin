use super::{InputEvent, InputStatus};
use crate::keyboard::{KeyId, MouseButton};
use std::ffi::c_void;
use std::sync::mpsc::Sender;

type CFMachPortRef = *mut c_void;
type CFRunLoopSourceRef = *mut c_void;
type CFRunLoopRef = *mut c_void;
type CFStringRef = *const c_void;
type CGEventTapProxy = *mut c_void;
type CGEventRef = *mut c_void;

const K_CG_HID_EVENT_TAP: u32 = 0;
const K_CG_HEAD_INSERT_EVENT_TAP: u32 = 0;
const K_CG_EVENT_TAP_OPTION_LISTEN_ONLY: u32 = 1;

const K_CG_EVENT_LEFT_MOUSE_DOWN: u32 = 1;
const K_CG_EVENT_LEFT_MOUSE_UP: u32 = 2;
const K_CG_EVENT_RIGHT_MOUSE_DOWN: u32 = 3;
const K_CG_EVENT_RIGHT_MOUSE_UP: u32 = 4;
const K_CG_EVENT_MOUSE_MOVED: u32 = 5;
const K_CG_EVENT_LEFT_MOUSE_DRAGGED: u32 = 6;
const K_CG_EVENT_RIGHT_MOUSE_DRAGGED: u32 = 7;
const K_CG_EVENT_KEY_DOWN: u32 = 10;
const K_CG_EVENT_KEY_UP: u32 = 11;
const K_CG_EVENT_FLAGS_CHANGED: u32 = 12;
const K_CG_EVENT_OTHER_MOUSE_DOWN: u32 = 25;
const K_CG_EVENT_OTHER_MOUSE_UP: u32 = 26;
const K_CG_EVENT_TAP_DISABLED_BY_TIMEOUT: u32 = 0xFFFF_FFFE;

const K_CG_KEYBOARD_EVENT_KEYCODE: u32 = 9;

const FLAG_CAPS: u64 = 1 << 16;
const FLAG_SHIFT: u64 = 1 << 17;
const FLAG_CTRL: u64 = 1 << 18;
const FLAG_ALT: u64 = 1 << 19;
const FLAG_CMD: u64 = 1 << 20;

#[repr(C)]
#[derive(Clone, Copy)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CGSize {
    width: f64,
    height: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CGRect {
    origin: CGPoint,
    size: CGSize,
}

type CGEventTapCallBack = unsafe extern "C" fn(
    proxy: CGEventTapProxy,
    event_type: u32,
    event: CGEventRef,
    user_info: *mut c_void,
) -> CGEventRef;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events_of_interest: u64,
        callback: CGEventTapCallBack,
        user_info: *mut c_void,
    ) -> CFMachPortRef;
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
    fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
    fn CGEventGetFlags(event: CGEventRef) -> u64;
    fn CGEventGetLocation(event: CGEventRef) -> CGPoint;
    fn CGMainDisplayID() -> u32;
    fn CGDisplayBounds(display: u32) -> CGRect;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFRunLoopCommonModes: CFStringRef;
    fn CFMachPortCreateRunLoopSource(
        allocator: *const c_void,
        port: CFMachPortRef,
        order: isize,
    ) -> CFRunLoopSourceRef;
    fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
    fn CFRunLoopRun();
}

struct TapContext {
    tx: Sender<InputEvent>,
    tap_port: CFMachPortRef,
    prev_flags: u64,
    screen_w: f64,
    screen_h: f64,
}

unsafe extern "C" fn event_tap_callback(
    _proxy: CGEventTapProxy,
    event_type: u32,
    event: CGEventRef,
    user_info: *mut c_void,
) -> CGEventRef {
    if user_info.is_null() {
        return event;
    }
    let ctx = unsafe { &mut *(user_info as *mut TapContext) };

    if event_type == K_CG_EVENT_TAP_DISABLED_BY_TIMEOUT {
        if !ctx.tap_port.is_null() {
            unsafe { CGEventTapEnable(ctx.tap_port, true) };
        }
        return event;
    }

    match event_type {
        K_CG_EVENT_KEY_DOWN => {
            let code = unsafe { CGEventGetIntegerValueField(event, K_CG_KEYBOARD_EVENT_KEYCODE) };
            if let Some(key) = mac_keycode_to_key_id(code as u16) {
                let _ = ctx.tx.send(InputEvent::KeyDown(key));
            }
        }
        K_CG_EVENT_KEY_UP => {
            let code = unsafe { CGEventGetIntegerValueField(event, K_CG_KEYBOARD_EVENT_KEYCODE) };
            if let Some(key) = mac_keycode_to_key_id(code as u16) {
                let _ = ctx.tx.send(InputEvent::KeyUp(key));
            }
        }
        K_CG_EVENT_FLAGS_CHANGED => {
            let flags = unsafe { CGEventGetFlags(event) };
            let diff = flags ^ ctx.prev_flags;
            for (mask, key) in [
                (FLAG_CAPS, KeyId::CapsLock),
                (FLAG_SHIFT, KeyId::LeftShift),
                (FLAG_CTRL, KeyId::LeftCtrl),
                (FLAG_ALT, KeyId::LeftAlt),
                (FLAG_CMD, KeyId::LeftMeta),
            ] {
                if (diff & mask) != 0 {
                    if (flags & mask) != 0 {
                        let _ = ctx.tx.send(InputEvent::KeyDown(key));
                    } else {
                        let _ = ctx.tx.send(InputEvent::KeyUp(key));
                    }
                }
            }
            ctx.prev_flags = flags;
        }
        K_CG_EVENT_MOUSE_MOVED | K_CG_EVENT_LEFT_MOUSE_DRAGGED | K_CG_EVENT_RIGHT_MOUSE_DRAGGED => {
            let loc = unsafe { CGEventGetLocation(event) };
            let _ = ctx.tx.send(InputEvent::MouseMove {
                norm_x: (loc.x / ctx.screen_w.max(1.0)).clamp(0.0, 1.0) as f32,
                norm_y: (loc.y / ctx.screen_h.max(1.0)).clamp(0.0, 1.0) as f32,
                screen_px: Some([loc.x as f32, loc.y as f32]),
                look_dir: None,
            });
        }
        K_CG_EVENT_LEFT_MOUSE_DOWN => {
            let _ = ctx.tx.send(InputEvent::MouseDown(MouseButton::Left));
        }
        K_CG_EVENT_LEFT_MOUSE_UP => {
            let _ = ctx.tx.send(InputEvent::MouseUp(MouseButton::Left));
        }
        K_CG_EVENT_RIGHT_MOUSE_DOWN => {
            let _ = ctx.tx.send(InputEvent::MouseDown(MouseButton::Right));
        }
        K_CG_EVENT_RIGHT_MOUSE_UP => {
            let _ = ctx.tx.send(InputEvent::MouseUp(MouseButton::Right));
        }
        K_CG_EVENT_OTHER_MOUSE_DOWN => {
            let _ = ctx.tx.send(InputEvent::MouseDown(MouseButton::Middle));
        }
        K_CG_EVENT_OTHER_MOUSE_UP => {
            let _ = ctx.tx.send(InputEvent::MouseUp(MouseButton::Middle));
        }
        _ => {}
    }

    event
}

pub fn start(tx: Sender<InputEvent>) -> InputStatus {
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();

    std::thread::Builder::new()
        .name("gatin-macos-tap".into())
        .spawn(move || {
            let bounds = unsafe { CGDisplayBounds(CGMainDisplayID()) };
            let ctx = Box::new(TapContext {
                tx,
                tap_port: std::ptr::null_mut(),
                prev_flags: 0,
                screen_w: bounds.size.width.max(1.0),
                screen_h: bounds.size.height.max(1.0),
            });
            let ctx_ptr = Box::into_raw(ctx);

            let mask: u64 = (1 << K_CG_EVENT_LEFT_MOUSE_DOWN)
                | (1 << K_CG_EVENT_LEFT_MOUSE_UP)
                | (1 << K_CG_EVENT_RIGHT_MOUSE_DOWN)
                | (1 << K_CG_EVENT_RIGHT_MOUSE_UP)
                | (1 << K_CG_EVENT_MOUSE_MOVED)
                | (1 << K_CG_EVENT_LEFT_MOUSE_DRAGGED)
                | (1 << K_CG_EVENT_RIGHT_MOUSE_DRAGGED)
                | (1 << K_CG_EVENT_KEY_DOWN)
                | (1 << K_CG_EVENT_KEY_UP)
                | (1 << K_CG_EVENT_FLAGS_CHANGED)
                | (1 << K_CG_EVENT_OTHER_MOUSE_DOWN)
                | (1 << K_CG_EVENT_OTHER_MOUSE_UP);

            let tap = unsafe {
                CGEventTapCreate(
                    K_CG_HID_EVENT_TAP,
                    K_CG_HEAD_INSERT_EVENT_TAP,
                    K_CG_EVENT_TAP_OPTION_LISTEN_ONLY,
                    mask,
                    event_tap_callback,
                    ctx_ptr as *mut c_void,
                )
            };

            if tap.is_null() {
                unsafe { drop(Box::from_raw(ctx_ptr)) };
                let _ = ready_tx.send(false);
                return;
            }

            unsafe {
                (*ctx_ptr).tap_port = tap;
                let source = CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
                let rl = CFRunLoopGetCurrent();
                CFRunLoopAddSource(rl, source, kCFRunLoopCommonModes);
                CGEventTapEnable(tap, true);
            }

            let _ = ready_tx.send(true);
            unsafe { CFRunLoopRun() };
        })
        .ok();

    let ok = ready_rx.recv().unwrap_or(false);
    if ok {
        InputStatus {
            backend: "macOS CoreGraphics EventTap".to_string(),
            global_active: true,
            hint: None,
        }
    } else {
        InputStatus {
            backend: "Local window only".to_string(),
            global_active: false,
            hint: Some(
                "No macOS, conceda permissão em Ajustes do Sistema → Privacidade e Segurança → Monitoramento de Entrada (Input Monitoring) / Acessibilidade e reabra o app."
                    .to_string(),
            ),
        }
    }
}

fn mac_keycode_to_key_id(code: u16) -> Option<KeyId> {
    Some(match code {
        0x00 => KeyId::A,
        0x01 => KeyId::S,
        0x02 => KeyId::D,
        0x03 => KeyId::F,
        0x04 => KeyId::H,
        0x05 => KeyId::G,
        0x06 => KeyId::Z,
        0x07 => KeyId::X,
        0x08 => KeyId::C,
        0x09 => KeyId::V,
        0x0B => KeyId::B,
        0x0C => KeyId::Q,
        0x0D => KeyId::W,
        0x0E => KeyId::E,
        0x0F => KeyId::R,
        0x10 => KeyId::Y,
        0x11 => KeyId::T,
        0x12 => KeyId::Digit1,
        0x13 => KeyId::Digit2,
        0x14 => KeyId::Digit3,
        0x15 => KeyId::Digit4,
        0x16 => KeyId::Digit6,
        0x17 => KeyId::Digit5,
        0x18 => KeyId::Equal,
        0x19 => KeyId::Digit9,
        0x1A => KeyId::Digit7,
        0x1B => KeyId::Minus,
        0x1C => KeyId::Digit8,
        0x1D => KeyId::Digit0,
        0x1E => KeyId::RightBracket,
        0x1F => KeyId::O,
        0x20 => KeyId::U,
        0x21 => KeyId::LeftBracket,
        0x22 => KeyId::I,
        0x23 => KeyId::P,
        0x24 => KeyId::Enter,
        0x25 => KeyId::L,
        0x26 => KeyId::J,
        0x27 => KeyId::Quote,
        0x28 => KeyId::K,
        0x29 => KeyId::Semicolon,
        0x2A => KeyId::Backslash,
        0x2B => KeyId::Comma,
        0x2C => KeyId::Slash,
        0x2D => KeyId::N,
        0x2E => KeyId::M,
        0x2F => KeyId::Period,
        0x30 => KeyId::Tab,
        0x31 => KeyId::Space,
        0x32 => KeyId::Grave,
        0x33 => KeyId::Backspace,
        0x35 => KeyId::Escape,
        0x37 => KeyId::LeftMeta,
        0x38 => KeyId::LeftShift,
        0x39 => KeyId::CapsLock,
        0x3A => KeyId::LeftAlt,
        0x3B => KeyId::LeftCtrl,
        0x3C => KeyId::RightShift,
        0x3D => KeyId::RightAlt,
        0x3E => KeyId::RightCtrl,
        0x75 => KeyId::Delete,
        0x7A => KeyId::F1,
        0x78 => KeyId::F2,
        0x63 => KeyId::F3,
        0x76 => KeyId::F4,
        0x60 => KeyId::F5,
        0x61 => KeyId::F6,
        0x62 => KeyId::F7,
        0x64 => KeyId::F8,
        0x65 => KeyId::F9,
        0x6D => KeyId::F10,
        0x67 => KeyId::F11,
        0x6F => KeyId::F12,
        0x7B => KeyId::ArrowLeft,
        0x7C => KeyId::ArrowRight,
        0x7D => KeyId::ArrowDown,
        0x7E => KeyId::ArrowUp,
        _ => return None,
    })
}
