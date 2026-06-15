use libloading::Library;
use std::ffi::CString;
use std::sync::Arc;

type MpvHandle = std::ffi::c_void;

pub const MPV_EVENT_NONE: u32 = 0;
pub const MPV_EVENT_SHUTDOWN: u32 = 1;
pub const MPV_EVENT_PROPERTY_CHANGE: u32 = 13;
pub const MPV_EVENT_END_FILE: u32 = 25;
pub const MPV_FORMAT_NODE: u32 = 73;

pub const MPV_END_FILE_REASON_EOF: u32 = 0;
pub const MPV_END_FILE_REASON_ERROR: u32 = 3;

const MPV_NODE_STRING: i32 = 1;
const MPV_NODE_MAP: i32 = 5;

#[repr(C)]
struct MpvNodeList {
    num: i32,
    keys: *mut *const i8,
    values: *mut MpvNode,
}

#[repr(C)]
union MpvNodeData {
    string: *const i8,
    int64: i64,
    double_: f64,
    flag: bool,
    list: *mut MpvNodeList,
}

#[repr(C)]
pub(crate) struct MpvNode {
    data: MpvNodeData,
    format: i32,
}

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
pub(crate) struct MpvEventProperty {
    pub name: *const i8,
    pub format: u32,
    pub data: *mut MpvNode,
}

type CreateFn = unsafe extern "C" fn() -> *mut MpvHandle;
type InitializeFn = unsafe extern "C" fn(*mut MpvHandle) -> i32;
type SetOptFn = unsafe extern "C" fn(*mut MpvHandle, *const i8, *const i8) -> i32;
type SetPropFn = unsafe extern "C" fn(*mut MpvHandle, *const i8, *const i8) -> i32;
type CommandFn = unsafe extern "C" fn(*mut MpvHandle, *const *const i8) -> i32;
type ObserveFn = unsafe extern "C" fn(*mut MpvHandle, u64, *const i8, u32) -> i32;
type WaitEventFn = unsafe extern "C" fn(*mut MpvHandle, f64) -> *mut MpvEvent;
type GetPropFn = unsafe extern "C" fn(*mut MpvHandle, *const i8, u32, *mut *mut MpvNode) -> i32;
type FreeNodeFn = unsafe extern "C" fn(*mut MpvNode);
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
    free_node: FreeNodeFn,
    destroy: DestroyFn,
}

pub struct Mpv {
    handle: *mut MpvHandle,
    _lib: Arc<Library>,
    funcs: &'static Funcs,
}

unsafe impl Send for Mpv {}
unsafe impl Sync for Mpv {}

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

        let native_path = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("native").join("mpv-1.dll")));

        let lib = match unsafe {
            native_path
                .as_ref()
                .and_then(|p| Library::new(p.as_os_str()).ok())
                .or_else(|| Library::new("mpv-1.dll").ok())
                .or_else(|| Library::new("libmpv-2.dll").ok())
                .or_else(|| Library::new("mpv.dll").ok())
                .or_else(|| Library::new("libmpv.so.2").ok())
                .or_else(|| Library::new("libmpv.so").ok())
                .or_else(|| Library::new("libmpv.2.dylib").ok())
                .or_else(|| Library::new("libmpv.dylib").ok())
        } {
            Some(l) => Arc::new(l),
            None => {
                errors.push("libmpv not found".into());
                return Err("libmpv not found".into());
            }
        };

        let funcs = Box::leak(Box::new(Funcs {
            create: unsafe { *lib.get(b"mpv_create\0").unwrap() },
            initialize: unsafe { *lib.get(b"mpv_initialize\0").unwrap() },
            set_option: unsafe { *lib.get(b"mpv_set_option_string\0").unwrap() },
            set_property: unsafe { *lib.get(b"mpv_set_property_string\0").unwrap() },
            command: unsafe { *lib.get(b"mpv_command\0").unwrap() },
            observe: unsafe { *lib.get(b"mpv_observe_property\0").unwrap() },
            wait_event: unsafe { *lib.get(b"mpv_wait_event\0").unwrap() },
            get_property: unsafe { *lib.get(b"mpv_get_property\0").unwrap() },
            free_node: unsafe { *lib.get(b"mpv_free_node_contents\0").unwrap() },
            destroy: unsafe { *lib.get(b"mpv_destroy\0").unwrap() },
        }));

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
            ("audio-client-name", "santui-radio"),
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

    pub fn initialize(&self) -> Result<(), Box<dyn std::error::Error>> {
        to_rc(
            unsafe { (self.funcs.initialize)(self.handle) },
            "initialize",
        )
    }

    pub fn command(&self, args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
        let cstrs: Vec<CString> = args.iter().map(|a| CString::new(*a).unwrap()).collect();
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
            unsafe { (self.funcs.observe)(self.handle, id, n.as_ptr(), MPV_FORMAT_NODE) },
            name,
        )
    }

    pub fn stop(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.command(&["stop"])
    }

    pub fn set_volume(&self, vol: i64) -> Result<(), Box<dyn std::error::Error>> {
        let v = CString::new(format!("{vol}"))?;
        to_rc(
            unsafe { (self.funcs.set_property)(self.handle, c"volume".as_ptr(), v.as_ptr()) },
            "volume",
        )
    }

    pub fn metadata_title(&self) -> Result<Option<String>, Box<dyn std::error::Error>> {
        let mut node: *mut MpvNode = std::ptr::null_mut();
        let rc = unsafe {
            (self.funcs.get_property)(
                self.handle,
                c"metadata".as_ptr(),
                MPV_FORMAT_NODE,
                &mut node,
            )
        };
        if rc < 0 || node.is_null() {
            return Ok(None);
        }
        let title = unsafe { extract_title_from_node(&*node) };
        unsafe { (self.funcs.free_node)(node) };
        Ok(title)
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

    pub fn destroy(&self) {
        unsafe { (self.funcs.destroy)(self.handle) };
    }
}

unsafe fn extract_title_from_node(node: &MpvNode) -> Option<String> {
    if node.format != MPV_NODE_MAP {
        return None;
    }
    let raw_list = unsafe { node.data.list };
    if raw_list.is_null() {
        return None;
    }
    let list = unsafe { &*raw_list };
    for i in 0..list.num {
        let key = unsafe { &*list.keys.offset(i as isize) };
        let key_str = unsafe { std::ffi::CStr::from_ptr(*key) }.to_string_lossy();
        let val = unsafe { &*list.values.offset(i as isize) };
        if val.format == MPV_NODE_STRING
            && (key_str.eq_ignore_ascii_case("icy-title") || key_str.eq_ignore_ascii_case("title"))
        {
            let s = unsafe { std::ffi::CStr::from_ptr(val.data.string) }
                .to_string_lossy()
                .to_string();
            if !s.is_empty() {
                return Some(s);
            }
        }
    }
    None
}
