use anyhow::Result;

use crate::event::DeviceMatcher;

pub trait InputSuppressor: Send {
    fn notify_button_press(&self);
}

pub fn log_startup_permission_status(matcher: &DeviceMatcher) {
    platform::log_permission_status(matcher);
}

pub fn ensure_startup_permissions(matcher: &DeviceMatcher) -> Result<()> {
    platform::ensure_permissions(matcher)
}

pub fn activate_input_suppressor(matcher: &DeviceMatcher) -> Result<Box<dyn InputSuppressor>> {
    platform::activate(matcher)
}

#[cfg(target_os = "linux")]
mod platform {
    use anyhow::{Context, Result, anyhow, bail};
    use std::fs::{File, OpenOptions, read_dir};
    use std::mem::size_of;
    use std::os::fd::AsRawFd;

    use crate::event::DeviceMatcher;
    use crate::suppress::InputSuppressor;

    const IOC_NRBITS: u32 = 8;
    const IOC_TYPEBITS: u32 = 8;
    const IOC_SIZEBITS: u32 = 14;

    const IOC_NRSHIFT: u32 = 0;
    const IOC_TYPESHIFT: u32 = IOC_NRSHIFT + IOC_NRBITS;
    const IOC_SIZESHIFT: u32 = IOC_TYPESHIFT + IOC_TYPEBITS;
    const IOC_DIRSHIFT: u32 = IOC_SIZESHIFT + IOC_SIZEBITS;

    const IOC_WRITE: u32 = 1;
    const IOC_READ: u32 = 2;

    const fn ioc(dir: u32, ty: u32, nr: u32, size: u32) -> libc::c_ulong {
        ((dir << IOC_DIRSHIFT)
            | (ty << IOC_TYPESHIFT)
            | (nr << IOC_NRSHIFT)
            | (size << IOC_SIZESHIFT)) as libc::c_ulong
    }

    const fn ior<T>(ty: u32, nr: u32) -> libc::c_ulong {
        ioc(IOC_READ, ty, nr, size_of::<T>() as u32)
    }

    const fn iow<T>(ty: u32, nr: u32) -> libc::c_ulong {
        ioc(IOC_WRITE, ty, nr, size_of::<T>() as u32)
    }

    const EVIOCGID: libc::c_ulong = ior::<InputId>(b'E' as u32, 0x02);
    const EVIOCGRAB: libc::c_ulong = iow::<libc::c_int>(b'E' as u32, 0x90);

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct InputId {
        bustype: u16,
        vendor: u16,
        product: u16,
        version: u16,
    }

    pub fn activate(matcher: &DeviceMatcher) -> Result<Box<dyn InputSuppressor>> {
        let grabs = grab_matching_event_devices(matcher)?;
        if grabs.is_empty() {
            bail!(
                "failed to enable Enter suppression: no /dev/input event device matched {:04x}:{:04x}",
                matcher.vendor_id,
                matcher.product_id
            );
        }
        Ok(Box::new(LinuxSuppressor { _grabs: grabs }))
    }

    pub fn ensure_permissions(matcher: &DeviceMatcher) -> Result<()> {
        let mut saw_match = false;
        for entry in read_dir("/dev/input").context("failed to read /dev/input")? {
            let entry = entry?;
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if !name.starts_with("event") {
                continue;
            }
            let file = match OpenOptions::new().read(true).open(&path) {
                Ok(file) => file,
                Err(_) => continue,
            };
            let id = match query_input_id(&file) {
                Ok(id) => id,
                Err(_) => continue,
            };
            if id.vendor == matcher.vendor_id && id.product == matcher.product_id {
                saw_match = true;
                break;
            }
        }

        if !saw_match {
            bail!(
                "no readable /dev/input event device matched {:04x}:{:04x}; run with access to /dev/input (for example root or the input group)",
                matcher.vendor_id,
                matcher.product_id
            );
        }
        Ok(())
    }

    pub fn log_permission_status(matcher: &DeviceMatcher) {
        println!(
            "Linux input access check for {:04x}:{:04x} will require readable /dev/input/event* access.",
            matcher.vendor_id, matcher.product_id
        );
    }

    struct LinuxSuppressor {
        _grabs: Vec<GrabbedDevice>,
    }

    impl InputSuppressor for LinuxSuppressor {
        fn notify_button_press(&self) {}
    }

    struct GrabbedDevice {
        file: File,
    }

    impl Drop for GrabbedDevice {
        fn drop(&mut self) {
            let release: libc::c_int = 0;
            unsafe {
                libc::ioctl(self.file.as_raw_fd(), EVIOCGRAB, release);
            }
        }
    }

    fn grab_matching_event_devices(matcher: &DeviceMatcher) -> Result<Vec<GrabbedDevice>> {
        let mut grabs = Vec::new();
        for entry in read_dir("/dev/input").context("failed to read /dev/input")? {
            let entry = entry?;
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if !name.starts_with("event") {
                continue;
            }

            let file = match OpenOptions::new().read(true).open(&path) {
                Ok(file) => file,
                Err(_) => continue,
            };
            let id = match query_input_id(&file) {
                Ok(id) => id,
                Err(_) => continue,
            };

            if id.vendor != matcher.vendor_id || id.product != matcher.product_id {
                continue;
            }

            grab_device(&file).with_context(|| {
                format!(
                    "failed to grab Linux input device {} for {:04x}:{:04x}",
                    path.display(),
                    matcher.vendor_id,
                    matcher.product_id
                )
            })?;
            grabs.push(GrabbedDevice { file });
        }
        Ok(grabs)
    }

    fn query_input_id(file: &File) -> Result<InputId> {
        let mut id = InputId::default();
        let rc = unsafe { libc::ioctl(file.as_raw_fd(), EVIOCGID, &mut id) };
        if rc < 0 {
            return Err(anyhow!(std::io::Error::last_os_error()));
        }
        Ok(id)
    }

    fn grab_device(file: &File) -> Result<()> {
        let enable: libc::c_int = 1;
        let rc = unsafe { libc::ioctl(file.as_raw_fd(), EVIOCGRAB, enable) };
        if rc < 0 {
            return Err(anyhow!(std::io::Error::last_os_error()));
        }
        Ok(())
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use std::ffi::c_void;
    use std::ptr;
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use anyhow::{Result, bail};
    use core_foundation_sys::base::{CFAllocatorRef, CFRelease, CFTypeRef, kCFAllocatorDefault};
    use core_foundation_sys::dictionary::{
        CFDictionaryCreate, CFDictionaryRef, kCFTypeDictionaryKeyCallBacks,
        kCFTypeDictionaryValueCallBacks,
    };
    use core_foundation_sys::mach_port::CFMachPortRef;
    use core_foundation_sys::number::kCFBooleanTrue;
    use core_foundation_sys::runloop::{
        CFRunLoopAddSource, CFRunLoopGetCurrent, CFRunLoopRun, CFRunLoopSourceRef,
        kCFRunLoopCommonModes,
    };
    use core_foundation_sys::string::{CFStringCreateWithCString, kCFStringEncodingUTF8};

    use crate::event::DeviceMatcher;
    use crate::suppress::InputSuppressor;

    type CGEventMask = u64;
    type CGEventRef = *mut c_void;
    type CGEventTapProxy = *mut c_void;
    type CFMachPortCallBack = unsafe extern "C" fn(
        proxy: CGEventTapProxy,
        type_: u32,
        event: CGEventRef,
        user_info: *mut c_void,
    ) -> CGEventRef;

    const KCG_EVENT_KEY_DOWN: u32 = 10;
    const KCG_EVENT_KEY_UP: u32 = 11;
    const KCG_HEAD_INSERT_EVENT_TAP: u32 = 0;
    const KCG_SESSION_EVENT_TAP: u32 = 1;
    const KCG_EVENT_TAP_OPTION_DEFAULT: u32 = 0;
    const KCG_KEYBOARD_EVENT_KEYCODE: u32 = 9;
    const RETURN_KEYCODE: i64 = 36;
    const AX_PROMPT_KEY: &[u8] = b"AXTrustedCheckOptionPrompt\0";
    const SUPPRESSION_WINDOW: Duration = Duration::from_millis(250);

    static SUPPRESS_UNTIL_MS: AtomicU64 = AtomicU64::new(0);
    static START_RESULT: OnceLock<std::result::Result<(), String>> = OnceLock::new();

    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXIsProcessTrusted() -> bool;
        fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> bool;
        fn CGEventTapCreate(
            tap: u32,
            place: u32,
            options: u32,
            events_of_interest: CGEventMask,
            callback: CFMachPortCallBack,
            user_info: *mut c_void,
        ) -> CFMachPortRef;
        fn CFMachPortCreateRunLoopSource(
            allocator: CFAllocatorRef,
            port: CFMachPortRef,
            order: isize,
        ) -> CFRunLoopSourceRef;
        fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
    }

    pub fn ensure_permissions(_matcher: &DeviceMatcher) -> Result<()> {
        unsafe {
            if AXIsProcessTrusted() {
                return Ok(());
            }

            let prompt_key = CFStringCreateWithCString(
                kCFAllocatorDefault,
                AX_PROMPT_KEY.as_ptr().cast(),
                kCFStringEncodingUTF8,
            );
            if prompt_key.is_null() {
                bail!("failed to create macOS Accessibility permission prompt");
            }

            let keys = [prompt_key as *const c_void];
            let values = [kCFBooleanTrue as *const c_void];
            let options = CFDictionaryCreate(
                kCFAllocatorDefault,
                keys.as_ptr(),
                values.as_ptr(),
                1,
                &kCFTypeDictionaryKeyCallBacks,
                &kCFTypeDictionaryValueCallBacks,
            );
            if options.is_null() {
                CFRelease(prompt_key as CFTypeRef);
                bail!("failed to build macOS Accessibility permission request");
            }

            let trusted = AXIsProcessTrustedWithOptions(options);
            CFRelease(options as CFTypeRef);
            CFRelease(prompt_key as CFTypeRef);

            if trusted {
                Ok(())
            } else {
                bail!(
                    "macOS Accessibility permission is required to suppress Return; approve the prompt in System Settings and restart the app"
                )
            }
        }
    }

    pub fn log_permission_status(_matcher: &DeviceMatcher) {
        let trusted = unsafe { AXIsProcessTrusted() };
        println!(
            "macOS Accessibility permission trusted: {}",
            if trusted { "yes" } else { "no" }
        );
    }

    pub fn activate(_matcher: &DeviceMatcher) -> Result<Box<dyn InputSuppressor>> {
        let startup = START_RESULT.get_or_init(start_event_tap_thread);
        if let Err(error) = startup {
            bail!("failed to enable Enter suppression on macOS: {error}");
        }
        Ok(Box::new(MacSuppressor))
    }

    struct MacSuppressor;

    impl InputSuppressor for MacSuppressor {
        fn notify_button_press(&self) {
            let now = current_time_ms();
            let deadline = now.saturating_add(SUPPRESSION_WINDOW.as_millis() as u64);
            SUPPRESS_UNTIL_MS.store(deadline, Ordering::SeqCst);
        }
    }

    fn start_event_tap_thread() -> std::result::Result<(), String> {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || unsafe {
            let mask = (1_u64 << KCG_EVENT_KEY_DOWN) | (1_u64 << KCG_EVENT_KEY_UP);
            let tap = CGEventTapCreate(
                KCG_SESSION_EVENT_TAP,
                KCG_HEAD_INSERT_EVENT_TAP,
                KCG_EVENT_TAP_OPTION_DEFAULT,
                mask,
                event_tap_callback,
                ptr::null_mut(),
            );
            if tap.is_null() {
                let _ = sender.send(Err(
                    "CGEventTapCreate returned null; grant Accessibility permission to this app/terminal"
                        .to_string(),
                ));
                return;
            }

            let source = CFMachPortCreateRunLoopSource(ptr::null(), tap, 0);
            if source.is_null() {
                let _ = sender.send(Err("failed to create macOS run-loop source".to_string()));
                return;
            }

            let run_loop = CFRunLoopGetCurrent();
            CFRunLoopAddSource(run_loop, source, kCFRunLoopCommonModes);
            let _ = sender.send(Ok(()));
            CFRunLoopRun();
        });

        receiver
            .recv()
            .map_err(|_| "macOS event-tap startup channel closed".to_string())?
    }

    unsafe extern "C" fn event_tap_callback(
        _proxy: CGEventTapProxy,
        type_: u32,
        event: CGEventRef,
        _user_info: *mut c_void,
    ) -> CGEventRef {
        if (type_ == KCG_EVENT_KEY_DOWN || type_ == KCG_EVENT_KEY_UP)
            && current_time_ms() <= SUPPRESS_UNTIL_MS.load(Ordering::SeqCst)
            && unsafe { CGEventGetIntegerValueField(event, KCG_KEYBOARD_EVENT_KEYCODE) }
                == RETURN_KEYCODE
        {
            return ptr::null_mut();
        }

        event
    }

    fn current_time_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use std::ptr;
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use anyhow::{Result, bail};
    use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::VK_RETURN;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetMessageW, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, MSG,
        SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, WH_KEYBOARD_LL,
    };

    use crate::event::DeviceMatcher;
    use crate::suppress::InputSuppressor;

    static SUPPRESS_UNTIL_MS: AtomicU64 = AtomicU64::new(0);
    static START_RESULT: OnceLock<std::result::Result<(), String>> = OnceLock::new();

    const SUPPRESSION_WINDOW: Duration = Duration::from_millis(250);

    pub fn activate(_matcher: &DeviceMatcher) -> Result<Box<dyn InputSuppressor>> {
        let startup = START_RESULT.get_or_init(start_hook_thread);
        if let Err(error) = startup {
            bail!("failed to enable Enter suppression on Windows: {error}");
        }
        Ok(Box::new(WindowsSuppressor))
    }

    pub fn ensure_permissions(_matcher: &DeviceMatcher) -> Result<()> {
        Ok(())
    }

    pub fn log_permission_status(_matcher: &DeviceMatcher) {
        println!("Windows keyboard suppression active: no OS permission prompt is required.");
    }

    struct WindowsSuppressor;

    impl InputSuppressor for WindowsSuppressor {
        fn notify_button_press(&self) {
            let now = current_time_ms();
            let deadline = now.saturating_add(SUPPRESSION_WINDOW.as_millis() as u64);
            SUPPRESS_UNTIL_MS.store(deadline, Ordering::SeqCst);
        }
    }

    fn start_hook_thread() -> std::result::Result<(), String> {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || unsafe {
            let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), ptr::null_mut(), 0);
            if hook.is_null() {
                let _ = sender.send(Err(std::io::Error::last_os_error().to_string()));
                return;
            }

            let _ = sender.send(Ok(()));
            let mut message = MSG::default();
            loop {
                let status = GetMessageW(&mut message, ptr::null_mut(), 0, 0);
                if status == -1 {
                    break;
                }
                if status == 0 {
                    break;
                }
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }

            UnhookWindowsHookEx(hook);
        });

        receiver
            .recv()
            .map_err(|_| "keyboard hook startup channel closed".to_string())?
    }

    unsafe extern "system" fn keyboard_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code == HC_ACTION as i32 {
            let data = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
            let _ = wparam;
            if data.vkCode == VK_RETURN as u32
                && current_time_ms() <= SUPPRESS_UNTIL_MS.load(Ordering::SeqCst)
            {
                return 1;
            }
        }

        unsafe { CallNextHookEx(ptr::null_mut() as HHOOK, code, wparam, lparam) }
    }

    fn current_time_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
mod platform {
    use anyhow::Result;

    use crate::event::DeviceMatcher;
    use crate::suppress::InputSuppressor;

    pub fn ensure_permissions(_matcher: &DeviceMatcher) -> Result<()> {
        Ok(())
    }

    pub fn log_permission_status(_matcher: &DeviceMatcher) {}

    pub fn activate(_matcher: &DeviceMatcher) -> Result<Box<dyn InputSuppressor>> {
        Ok(Box::new(NoopSuppressor))
    }

    struct NoopSuppressor;

    impl InputSuppressor for NoopSuppressor {
        fn notify_button_press(&self) {}
    }
}
