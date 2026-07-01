//! Global hotkey via CGEventTap — intercepts a configured key combo and simulates Cmd+V.

use std::ffi::c_void;
use std::ptr;

type CGEventRef = *mut c_void;
type CGEventTapProxy = *mut c_void;
type CFMachPortRef = *mut c_void;
type CFRunLoopSourceRef = *mut c_void;
type CFRunLoopRef = *mut c_void;
type CFStringRef = *const c_void;

const EVENT_KEY_DOWN: u32 = 10;
const TAP_DISABLED: u32 = 0xFFFFFFFE;
const MOD_MASK: u64 = 0x1E0000;
const MOD_CMD: u64 = 0x100000;
const V_KEY: u16 = 0x09;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventTapCreate(tap: u32, place: u32, opts: u32, mask: u64,
        cb: extern "C" fn(CGEventTapProxy, u32, CGEventRef, *mut c_void) -> CGEventRef,
        info: *mut c_void) -> CFMachPortRef;
    fn CGEventTapEnable(tap: CFMachPortRef, en: bool);
    fn CGEventGetFlags(e: CGEventRef) -> u64;
    fn CGEventGetIntegerValueField(e: CGEventRef, f: u32) -> i64;
    fn CGEventCreateKeyboardEvent(src: *const c_void, k: u16, down: bool) -> CGEventRef;
    fn CGEventSetFlags(e: CGEventRef, f: u64);
    fn CGEventPost(tap: u32, e: CGEventRef);
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFMachPortCreateRunLoopSource(a: *const c_void, p: CFMachPortRef, o: i64) -> CFRunLoopSourceRef;
    fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    fn CFRunLoopAddSource(rl: CFRunLoopRef, s: CFRunLoopSourceRef, m: CFStringRef);
    fn CFRunLoopRun();
    fn CFRelease(cf: *const c_void);
    static kCFRunLoopCommonModes: CFStringRef;
}

struct Ctx { mods: u64, key: u16, tap: CFMachPortRef }

extern "C" fn callback(_p: CGEventTapProxy, ty: u32, ev: CGEventRef, info: *mut c_void) -> CGEventRef {
    unsafe {
        let ctx = &*(info as *const Ctx);
        if ty == TAP_DISABLED { CGEventTapEnable(ctx.tap, true); return ev; }
        if ty != EVENT_KEY_DOWN { return ev; }
        if CGEventGetFlags(ev) & MOD_MASK == ctx.mods
            && CGEventGetIntegerValueField(ev, 9) as u16 == ctx.key
        {
            let d = CGEventCreateKeyboardEvent(ptr::null(), V_KEY, true);
            let u = CGEventCreateKeyboardEvent(ptr::null(), V_KEY, false);
            if !d.is_null() && !u.is_null() {
                CGEventSetFlags(d, MOD_CMD);
                CGEventSetFlags(u, MOD_CMD);
                CGEventPost(2, d); // kCGAnnotatedSessionEventTap
                CGEventPost(2, u);
                CFRelease(d); CFRelease(u);
            }
            return ptr::null_mut();
        }
        ev
    }
}

pub fn start(mods: u64, key: u16) {
    if mods == MOD_CMD && key == V_KEY { return; }
    std::thread::spawn(move || unsafe {
        let ctx = Box::into_raw(Box::new(Ctx { mods, key, tap: ptr::null_mut() }));
        let tap = CGEventTapCreate(1, 0, 0, 1 << EVENT_KEY_DOWN, callback, ctx as *mut c_void);
        if tap.is_null() {
            crate::common::log("hotkey: event tap failed (Accessibility permission needed)");
            let _ = Box::from_raw(ctx); return;
        }
        (*ctx).tap = tap;
        let src = CFMachPortCreateRunLoopSource(ptr::null(), tap, 0);
        if src.is_null() { let _ = Box::from_raw(ctx); return; }
        CFRunLoopAddSource(CFRunLoopGetCurrent(), src, kCFRunLoopCommonModes);
        CGEventTapEnable(tap, true);
        crate::common::log("hotkey: registered");
        CFRunLoopRun();
    });
}
