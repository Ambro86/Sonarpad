use crate::embedded_deps;
use crate::log_debug;
use libloading::Library;
use std::ffi::c_void;
use std::path::Path;
use std::sync::OnceLock;

pub type Bool = i32;
pub type Dword = u32;
pub type Qword = u64;
pub type Hstream = u32;
pub type Hplugin = u32;

pub const BASS_UNICODE: Dword = 0x8000_0000;
pub const BASS_SAMPLE_FLOAT: Dword = 0x0000_0100;
pub const BASS_STREAM_DECODE: Dword = 0x0020_0000;
pub const BASS_STREAM_PRESCAN: Dword = 0x0002_0000;
pub const BASS_FX_FREESOURCE: Dword = 0x0001_0000;
pub const BASS_POS_BYTE: Dword = 0;
pub const BASS_ATTRIB_VOL: Dword = 2;
pub const BASS_ATTRIB_TEMPO: Dword = 0x0001_0000;
pub const BASS_ATTRIB_TEMPO_PITCH: Dword = 0x0001_0001;

/// BASS_StreamCreate callback: return number of bytes written, or BASS_STREAMPROC_END
pub type StreamProc = unsafe extern "C" fn(
    handle: Hstream,
    buffer: *mut c_void,
    length: Dword,
    user: *mut c_void,
) -> Dword;

/// Return value to signal end of stream
pub const BASS_STREAMPROC_END: Dword = 0x8000_0000;
type BassInit = unsafe extern "C" fn(
    device: i32,
    freq: Dword,
    flags: Dword,
    win: *mut c_void,
    clsid: *const c_void,
) -> Bool;
type BassStart = unsafe extern "C" fn() -> Bool;
type BassErrorGetCode = unsafe extern "C" fn() -> i32;
type BassPluginLoad = unsafe extern "C" fn(file: *const c_void, flags: Dword) -> Hplugin;
type BassStreamCreateFile = unsafe extern "C" fn(
    mem: Bool,
    file: *const c_void,
    offset: Qword,
    length: Qword,
    flags: Dword,
) -> Hstream;
type BassStreamFree = unsafe extern "C" fn(handle: Hstream) -> Bool;
type BassChannelPlay = unsafe extern "C" fn(handle: Dword, restart: Bool) -> Bool;
type BassChannelPause = unsafe extern "C" fn(handle: Dword) -> Bool;
type BassChannelStop = unsafe extern "C" fn(handle: Dword) -> Bool;
type BassChannelGetPosition = unsafe extern "C" fn(handle: Dword, mode: Dword) -> Qword;
type BassChannelGetLength = unsafe extern "C" fn(handle: Dword, mode: Dword) -> Qword;
type BassChannelSetPosition = unsafe extern "C" fn(handle: Dword, pos: Qword, mode: Dword) -> Bool;
type BassChannelBytes2Seconds = unsafe extern "C" fn(handle: Dword, pos: Qword) -> f64;
type BassChannelSeconds2Bytes = unsafe extern "C" fn(handle: Dword, pos: f64) -> Qword;
type BassChannelSetAttribute =
    unsafe extern "C" fn(handle: Dword, attrib: Dword, value: f32) -> Bool;
type BassChannelIsActive = unsafe extern "C" fn(handle: Dword) -> Dword;

type BassStreamCreate = unsafe extern "C" fn(
    freq: Dword,
    chans: Dword,
    flags: Dword,
    proc_: StreamProc,
    user: *mut c_void,
) -> Hstream;

type BassFxTempoCreate = unsafe extern "C" fn(handle: Dword, flags: Dword) -> Hstream;

pub struct BassApi {
    _lib: Library,
    pub init: BassInit,
    pub start: BassStart,
    pub error_get_code: BassErrorGetCode,
    pub plugin_load: BassPluginLoad,
    pub stream_create_file: BassStreamCreateFile,
    pub stream_create: BassStreamCreate,
    pub stream_free: BassStreamFree,
    pub channel_play: BassChannelPlay,
    pub channel_pause: BassChannelPause,
    pub channel_stop: BassChannelStop,
    pub channel_get_position: BassChannelGetPosition,
    pub channel_get_length: BassChannelGetLength,
    pub channel_set_position: BassChannelSetPosition,
    pub channel_bytes2seconds: BassChannelBytes2Seconds,
    pub channel_seconds2bytes: BassChannelSeconds2Bytes,
    pub channel_set_attribute: BassChannelSetAttribute,
    pub channel_is_active: BassChannelIsActive,
    pub channel_get_data: unsafe extern "C" fn(Dword, *mut c_void, Dword) -> Dword,
}

pub struct BassFxApi {
    _lib: Library,
    pub tempo_create: BassFxTempoCreate,
}

fn load_symbol<T: Copy>(lib: &Library, name: &[u8]) -> Result<T, String> {
    unsafe {
        lib.get::<T>(name).map(|s| *s).map_err(|e| {
            format!(
                "BASS: missing symbol {}: {}",
                String::from_utf8_lossy(name),
                e
            )
        })
    }
}

fn load_library(path: &Path) -> Result<Library, libloading::Error> {
    unsafe { Library::new(path) }
}

fn load_bass_api(path: &Path) -> Result<BassApi, String> {
    let lib = load_library(path)
        .map_err(|e| format!("BASS: failed to load {}: {}", path.display(), e))?;
    Ok(BassApi {
        init: load_symbol(&lib, b"BASS_Init\0")?,
        start: load_symbol(&lib, b"BASS_Start\0")?,
        error_get_code: load_symbol(&lib, b"BASS_ErrorGetCode\0")?,
        plugin_load: load_symbol(&lib, b"BASS_PluginLoad\0")?,
        stream_create_file: load_symbol(&lib, b"BASS_StreamCreateFile\0")?,
        stream_create: load_symbol(&lib, b"BASS_StreamCreate\0")?,
        stream_free: load_symbol(&lib, b"BASS_StreamFree\0")?,
        channel_play: load_symbol(&lib, b"BASS_ChannelPlay\0")?,
        channel_pause: load_symbol(&lib, b"BASS_ChannelPause\0")?,
        channel_stop: load_symbol(&lib, b"BASS_ChannelStop\0")?,
        channel_get_position: load_symbol(&lib, b"BASS_ChannelGetPosition\0")?,
        channel_get_length: load_symbol(&lib, b"BASS_ChannelGetLength\0")?,
        channel_set_position: load_symbol(&lib, b"BASS_ChannelSetPosition\0")?,
        channel_bytes2seconds: load_symbol(&lib, b"BASS_ChannelBytes2Seconds\0")?,
        channel_seconds2bytes: load_symbol(&lib, b"BASS_ChannelSeconds2Bytes\0")?,
        channel_set_attribute: load_symbol(&lib, b"BASS_ChannelSetAttribute\0")?,
        channel_is_active: load_symbol(&lib, b"BASS_ChannelIsActive\0")?,
        channel_get_data: load_symbol(&lib, b"BASS_ChannelGetData\0")?,
        _lib: lib,
    })
}

fn load_bass_fx_api(path: &Path) -> Result<BassFxApi, String> {
    let lib = load_library(path)
        .map_err(|e| format!("BASS: failed to load {}: {}", path.display(), e))?;
    Ok(BassFxApi {
        tempo_create: load_symbol(&lib, b"BASS_FX_TempoCreate\0")?,
        _lib: lib,
    })
}

static BASS_API: OnceLock<Result<BassApi, String>> = OnceLock::new();
static BASS_FX_API: OnceLock<Result<BassFxApi, String>> = OnceLock::new();

pub fn bass_api() -> Result<&'static BassApi, String> {
    BASS_API
        .get_or_init(|| {
            let path = embedded_deps::get_dep_path("bass.dll");
            load_bass_api(&path)
        })
        .as_ref()
        .map_err(|e| e.clone())
}

pub fn bass_fx_api() -> Result<&'static BassFxApi, String> {
    BASS_FX_API
        .get_or_init(|| {
            let path = embedded_deps::get_dep_path("bass_fx.dll");
            load_bass_fx_api(&path)
        })
        .as_ref()
        .map_err(|e| e.clone())
}

pub fn bass_error(api: &BassApi) -> i32 {
    unsafe { (api.error_get_code)() }
}

pub fn log_bass_error(api: &BassApi, context: &str) {
    let code = bass_error(api);
    log_debug(&format!("BASS: {} failed (error {})", context, code));
}
