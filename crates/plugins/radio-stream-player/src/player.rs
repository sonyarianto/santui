use libloading::Library;
use std::ffi::CString;
use std::sync::Arc;

type MpvHandle = std::ffi::c_void;

pub const MPV_EVENT_NONE: u32 = 0;
pub const MPV_EVENT_SHUTDOWN: u32 = 1;
pub const MPV_EVENT_FILE_LOADED: u32 = 6;
pub const MPV_EVENT_PLAYBACK_RESTART: u32 = 18;
pub const MPV_EVENT_PROPERTY_CHANGE: u32 = 22;
pub const MPV_EVENT_END_FILE: u32 = 25;
pub const MPV_FORMAT_NODE_OBSERVE: u32 = 6;
pub const MPV_FORMAT_STRING: u32 = 1;

pub const MPV_END_FILE_REASON_EOF: u32 = 0;
pub const MPV_END_FILE_REASON_ERROR: u32 = 3;

#[repr(C)]
pub struct MpvEventEndFile {
    pub reason: u32,
    pub error: i32,
}

#[repr(C)]
pub struct MpvEvent {
    pub event_id: u32,
    pub error: i32,
    pub reply_userdata: u64,
    pub data: *mut std::ffi::c_void,
}

#[repr(C)]
pub struct MpvEventProperty {
    pub name: *const i8,
    pub format: u32,
    pub data: *mut std::ffi::c_void,
}

type CreateFn = unsafe extern "C" fn() -> *mut MpvHandle;
type InitializeFn = unsafe extern "C" fn(*mut MpvHandle) -> i32;
type SetOptFn = unsafe extern "C" fn(*mut MpvHandle, *const i8, *const i8) -> i32;
type SetPropFn = unsafe extern "C" fn(*mut MpvHandle, *const i8, *const i8) -> i32;
type CommandFn = unsafe extern "C" fn(*mut MpvHandle, *const *const i8) -> i32;
type ObserveFn = unsafe extern "C" fn(*mut MpvHandle, u64, *const i8, u32) -> i32;
type WaitEventFn = unsafe extern "C" fn(*mut MpvHandle, f64) -> *mut MpvEvent;
type GetPropFn = unsafe extern "C" fn(*mut MpvHandle, *const i8, u32, *mut std::ffi::c_void) -> i32;
type DestroyFn = unsafe extern "C" fn(*mut MpvHandle);

struct Funcs {
    create: CreateFn,
    initialize: InitializeFn,
    set_option: SetOptFn,
    set_property: SetPropFn,
    command: CommandFn,
    observe: ObserveFn,
    wait_event: WaitEventFn,
    get_property: GetPropFn,
    destroy: DestroyFn,
}

pub struct Mpv {
    handle: *mut MpvHandle,
    _lib: Arc<Library>,
    funcs: Box<Funcs>,
}

// Mpv is Send because all access to `handle` goes through `funcs`,
// which are plain function pointers loaded from libmpv. The `_lib` Arc<Library>
// ensures the shared library stays loaded for the lifetime of every Mpv handle.
// Mpv is NOT Sync because libmpv is not thread-safe; shared references (&Mpv)
// must not cross threads. The current design keeps one Mpv per plugin process
// accessed from a single thread.
unsafe impl Send for Mpv {}

fn to_rc(rc: i32, ctx: &str) -> Result<(), Box<dyn std::error::Error>> {
    if rc >= 0 {
        Ok(())
    } else {
        Err(format!("mpv {ctx} failed: {rc}").into())
    }
}

impl Mpv {
    pub fn new() -> Result<(Self, Vec<String>), Box<dyn std::error::Error>> {
        let mut errors = Vec::new();

        let native_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("native")));

        let native_names = [
            "libmpv-2.dll",
            "libmpv.so.2",
            "libmpv.so",
            "libmpv.2.dylib",
            "libmpv.dylib",
        ];
        let native_try = native_dir.as_ref().and_then(|d| {
            native_names.iter().find_map(|name| {
                let p = d.join(name);
                unsafe { Library::new(p.as_os_str()).ok() }
            })
        });

        let lib = match unsafe {
            native_try
                .or_else(|| Library::new("libmpv-2.dll").ok())
                .or_else(|| Library::new("mpv.dll").ok())
                .or_else(|| Library::new("libmpv.so.2").ok())
                .or_else(|| Library::new("libmpv.so").ok())
                // macOS Homebrew paths (Apple Silicon & Intel)
                .or_else(|| Library::new("/opt/homebrew/lib/libmpv.2.dylib").ok())
                .or_else(|| Library::new("/opt/homebrew/lib/libmpv.dylib").ok())
                .or_else(|| Library::new("/usr/local/lib/libmpv.2.dylib").ok())
                .or_else(|| Library::new("/usr/local/lib/libmpv.dylib").ok())
                // fallback: dlopen default search path
                .or_else(|| Library::new("libmpv.2.dylib").ok())
                .or_else(|| Library::new("libmpv.dylib").ok())
        } {
            Some(l) => Arc::new(l),
            None => {
                errors.push("libmpv not found".into());
                return Err("libmpv not found".into());
            }
        };

        macro_rules! get_sym {
            ($fn_name:literal) => {
                unsafe {
                    lib.get($fn_name).map(|s| *s).map_err(|e| {
                        let name = String::from_utf8_lossy($fn_name);
                        format!("libmpv symbol {name} not found: {e}")
                    })?
                }
            };
        }
        let funcs = Box::new(Funcs {
            create: get_sym!(b"mpv_create\0"),
            initialize: get_sym!(b"mpv_initialize\0"),
            set_option: get_sym!(b"mpv_set_option_string\0"),
            set_property: get_sym!(b"mpv_set_property_string\0"),
            command: get_sym!(b"mpv_command\0"),
            observe: get_sym!(b"mpv_observe_property\0"),
            wait_event: get_sym!(b"mpv_wait_event\0"),
            get_property: get_sym!(b"mpv_get_property\0"),
            destroy: get_sym!(b"mpv_destroy\0"),
        });

        let handle = unsafe { (funcs.create)() };
        if handle.is_null() {
            return Err("mpv_create returned null".into());
        }

        let mpv = Mpv {
            handle,
            _lib: lib,
            funcs,
        };

        for (k, v) in [
            ("config", "no"),
            ("vo", "null"),
            ("audio-client-name", "santui-radio-stream-player"),
            ("stream-lavf-o", "icy=1"),
        ] {
            if let Err(e) = mpv.set_option(k, v) {
                errors.push(format!("  {k}: {e}"));
            }
        }

        if let Err(e) = mpv.initialize() {
            errors.push(format!("  init: {e}"));
        }

        Ok((mpv, errors))
    }

    fn set_option(&self, name: &str, value: &str) -> Result<(), Box<dyn std::error::Error>> {
        let n = CString::new(name)?;
        let v = CString::new(value)?;
        to_rc(
            unsafe { (self.funcs.set_option)(self.handle, n.as_ptr(), v.as_ptr()) },
            name,
        )
    }

    pub fn set_property(&self, name: &str, value: &str) -> Result<(), Box<dyn std::error::Error>> {
        let n = CString::new(name)?;
        let v = CString::new(value)?;
        to_rc(
            unsafe { (self.funcs.set_property)(self.handle, n.as_ptr(), v.as_ptr()) },
            name,
        )
    }

    pub fn initialize(&self) -> Result<(), Box<dyn std::error::Error>> {
        to_rc(
            unsafe { (self.funcs.initialize)(self.handle) },
            "initialize",
        )
    }

    pub fn command(&self, args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
        let cstrs: Vec<CString> = args
            .iter()
            .map(|a| CString::new(*a))
            .collect::<Result<Vec<_>, _>>()?;
        let mut ptrs: Vec<*const i8> = cstrs.iter().map(|c| c.as_ptr()).collect();
        ptrs.push(std::ptr::null());
        to_rc(
            unsafe { (self.funcs.command)(self.handle, ptrs.as_ptr()) },
            &format!("command {args:?}"),
        )
    }

    pub fn load_url(&self, url: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.command(&["loadfile", url, "replace"])
    }

    pub fn observe_property(&self, id: u64, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let n = CString::new(name)?;
        to_rc(
            unsafe { (self.funcs.observe)(self.handle, id, n.as_ptr(), MPV_FORMAT_NODE_OBSERVE) },
            name,
        )
    }

    pub fn stop(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.command(&["stop"])
    }

    pub fn set_volume(&self, vol: i64) -> Result<(), Box<dyn std::error::Error>> {
        self.set_property("volume", &vol.to_string())
    }

    pub fn media_title(&self) -> Result<Option<String>, Box<dyn std::error::Error>> {
        let mut ptr: *mut i8 = std::ptr::null_mut();
        let name = CString::new("media-title")?;
        let rc = unsafe {
            (self.funcs.get_property)(
                self.handle,
                name.as_ptr(),
                MPV_FORMAT_STRING,
                &mut ptr as *mut *mut i8 as *mut std::ffi::c_void,
            )
        };
        if rc < 0 || ptr.is_null() {
            return Ok(None);
        }
        let s = unsafe { std::ffi::CStr::from_ptr(ptr).to_string_lossy().to_string() };
        if s.is_empty() {
            Ok(None)
        } else {
            Ok(Some(s))
        }
    }

    pub fn metadata_title(&self) -> Result<Option<String>, Box<dyn std::error::Error>> {
        if let Some(t) = self.get_property_string("stream-title")? {
            return Ok(Some(t));
        }
        self.get_property_string("media-title")
    }

    fn get_property_string(
        &self,
        name: &str,
    ) -> Result<Option<String>, Box<dyn std::error::Error>> {
        let mut ptr: *mut i8 = std::ptr::null_mut();
        let n = CString::new(name)?;
        let rc = unsafe {
            (self.funcs.get_property)(
                self.handle,
                n.as_ptr(),
                MPV_FORMAT_STRING,
                &mut ptr as *mut *mut i8 as *mut std::ffi::c_void,
            )
        };
        if rc < 0 || ptr.is_null() {
            return Ok(None);
        }
        let s = unsafe { std::ffi::CStr::from_ptr(ptr).to_string_lossy().to_string() };
        if s.is_empty() {
            Ok(None)
        } else {
            Ok(Some(s))
        }
    }

    pub fn wait_event_raw(&self, timeout: f64) -> Option<&MpvEvent> {
        unsafe {
            let ev = (self.funcs.wait_event)(self.handle, timeout);
            if ev.is_null() || (*ev).event_id == MPV_EVENT_NONE {
                return None;
            }
            Some(&*ev)
        }
    }

    pub fn destroy(&mut self) {
        if !self.handle.is_null() {
            unsafe { (self.funcs.destroy)(self.handle) };
            self.handle = std::ptr::null_mut();
        }
    }
}

impl Drop for Mpv {
    fn drop(&mut self) {
        self.destroy();
    }
}
