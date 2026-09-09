#![deny(warnings)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(let_underscore_drop)]
#![allow(non_snake_case)]
#![allow(clippy::upper_case_acronyms)]

mod buffer_notify;
use buffer_notify::{BufferOwner, IID_ITTSBUFNOTIFYSINK};

use std::collections::VecDeque;
use std::env;
use std::io::{self, BufRead, Read};
use std::path::Path;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use winapi::shared::guiddef::{GUID, REFIID};
use winapi::shared::minwindef::{DWORD, FILETIME, WORD};
use winapi::shared::winerror::{E_NOINTERFACE, S_OK};
use winapi::um::combaseapi::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL};
use winapi::um::unknwnbase::IUnknown;
use winapi::um::winuser::{DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE};

use widestring::U16CString;

use windows::core::{Interface, PCWSTR};
use windows::Win32::Foundation::VARIANT_BOOL;
use windows::Win32::Media::MediaFoundation::{
    IMFMediaType, IMFSinkWriter, IMFSourceReader, MFAudioFormat_MP3, MFAudioFormat_PCM,
    MFCreateMediaType, MFCreateSinkWriterFromURL, MFCreateSourceReaderFromURL, MFMediaType_Audio,
    MFShutdown, MFStartup, MF_MT_AUDIO_AVG_BYTES_PER_SECOND, MF_MT_AUDIO_BITS_PER_SAMPLE,
    MF_MT_AUDIO_BLOCK_ALIGNMENT, MF_MT_AUDIO_NUM_CHANNELS, MF_MT_AUDIO_SAMPLES_PER_SECOND,
    MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE, MF_SOURCE_READERF_ENDOFSTREAM,
    MF_SOURCE_READER_FIRST_AUDIO_STREAM, MF_VERSION,
};
use windows::Win32::Media::Speech::{
    ISpMMSysAudio, ISpObjectToken, ISpVoice, ISpeechFileStream, ISpeechObjectToken,
    ISpeechObjectTokenCategory, ISpeechVoice, SSFMCreateForWrite, SpFileStream, SpMMAudioOut,
    SpObjectTokenCategory, SpVoice, SpeechVoiceSpeakFlags, SPAS_PAUSE, SPAS_RUN, SPAS_STOP,
    SPF_ASYNC, SPF_IS_XML, SPF_PURGEBEFORESPEAK,
};

// =========================
// GUID constants
// =========================

// SAPI4 GUIDs
const CLSID_TTSENUMERATOR: GUID = GUID {
    Data1: 0xD67C0280,
    Data2: 0xC743,
    Data3: 0x11CD,
    Data4: [0x80, 0xE5, 0x00, 0xAA, 0x00, 0x3E, 0x4B, 0x50],
};
const IID_ITTSENUM: GUID = GUID {
    Data1: 0x6B837B20,
    Data2: 0x4A47,
    Data3: 0x101B,
    Data4: [0x93, 0x1A, 0x00, 0xAA, 0x00, 0x47, 0xBA, 0x4F],
};

// Default audio output (MMAudioDest)
const CLSID_MMAUDIODEST: GUID = GUID {
    Data1: 0xCB96B400,
    Data2: 0xC743,
    Data3: 0x11CD,
    Data4: [0x80, 0xE5, 0x00, 0xAA, 0x00, 0x3E, 0x4B, 0x50],
};
const IID_IAUDIO: GUID = GUID {
    Data1: 0xF546B340,
    Data2: 0xC743,
    Data3: 0x11CD,
    Data4: [0x80, 0xE5, 0x00, 0xAA, 0x00, 0x3E, 0x4B, 0x50],
};

const IID_IUNKNOWN: GUID = GUID {
    Data1: 0x00000000,
    Data2: 0x0000,
    Data3: 0x0000,
    Data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};

// File output (AudioDestFile)
const CLSID_AUDIODESTFILE: GUID = GUID {
    Data1: 0xD4623720,
    Data2: 0xE4B9,
    Data3: 0x11CF,
    Data4: [0x8D, 0x56, 0x00, 0xA0, 0xC9, 0x03, 0x4A, 0x7E],
};
const IID_IAUDIOFILE: GUID = GUID {
    Data1: 0xFD7C2320,
    Data2: 0x3D6D,
    Data3: 0x11B9,
    Data4: [0xC0, 0x00, 0xFE, 0xD6, 0xCB, 0xA3, 0xB1, 0xA9],
};

// Notify interfaces
const IID_ITTSNOTIFYSINKW: GUID = GUID {
    Data1: 0xC0FA8F40,
    Data2: 0x4A46,
    Data3: 0x101B,
    Data4: [0x93, 0x1A, 0x00, 0xAA, 0x00, 0x47, 0xBA, 0x4F],
};
const IID_IAUDIOFILENOTIFYSINK: GUID = GUID {
    Data1: 0x492FE490,
    Data2: 0x51E7,
    Data3: 0x11B9,
    Data4: [0xC0, 0x00, 0xFE, 0xD6, 0xCB, 0xA3, 0xB1, 0xA9],
};

const IID_ITTSATTRIBUTESW: GUID = GUID {
    Data1: 0x1287A280,
    Data2: 0x4A47,
    Data3: 0x101B,
    Data4: [0x93, 0x1A, 0x00, 0xAA, 0x00, 0x47, 0xBA, 0x4F],
};

const TTSDATAFLAG_TAGGED: DWORD = 1;

const TTSATTR_MINSPEED: DWORD = 0;
const TTSATTR_MAXSPEED: DWORD = 0xFFFF_FFFF;
const TTSATTR_MINPITCH: WORD = 0;
const TTSATTR_MAXPITCH: WORD = 0xFFFF;
const TTSATTR_MINVOLUME: DWORD = 0;
const TTSATTR_MAXVOLUME: DWORD = 0xFFFF_FFFF;

// =========================
// Commands (DEFINED ONCE)
// =========================

enum ServerCommand {
    Speak(String),
    Pause,
    Resume,
    Stop,
    Quit,
}

enum SpeakItem {
    Text(U16CString),
    Pause(u32),
}

// =========================
// Helpers / RAII
// =========================

fn hr_ok(hr: i32) -> bool {
    hr == S_OK
}

fn guid_eq(a: REFIID, b: &GUID) -> bool {
    unsafe {
        let aa = &*a;
        aa.Data1 == b.Data1 && aa.Data2 == b.Data2 && aa.Data3 == b.Data3 && aa.Data4 == b.Data4
    }
}

struct ComGuard;
impl ComGuard {
    fn init_mta() -> Result<Self, String> {
        unsafe {
            // COINIT_MULTITHREADED = 0x2
            let hr = CoInitializeEx(ptr::null_mut(), 0x2);
            // S_OK (0) or S_FALSE (1) are both success for CoInitializeEx.
            if hr != 0 && hr != 1 {
                return Err(format!("CoInitializeEx failed hr={:#x}", hr));
            }
        }
        Ok(ComGuard)
    }
}
impl Drop for ComGuard {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

/// Minimal COM smart pointer that calls Release in Drop.
/// (No `.unwrap`, no `let _ =`)
struct ComPtr<T> {
    ptr: *mut T,
}
impl<T> ComPtr<T> {
    fn new(ptr: *mut T) -> Option<Self> {
        if ptr.is_null() {
            None
        } else {
            Some(Self { ptr })
        }
    }
    fn as_ptr(&self) -> *mut T {
        self.ptr
    }
}
impl<T> Drop for ComPtr<T> {
    fn drop(&mut self) {
        unsafe {
            let unk = self.ptr as *mut IUnknown;
            if unk.is_null() {
                return;
            }
            // IUnknown vtable: QueryInterface, AddRef, Release
            let vtbl_ptr = *(unk as *mut *mut *mut usize);
            if vtbl_ptr.is_null() {
                return;
            }
            let release_fn: unsafe extern "system" fn(*mut IUnknown) -> u32 =
                std::mem::transmute(*vtbl_ptr.add(2));
            release_fn(unk);
        }
    }
}

// =========================
// SAPI4 structs / vtables
// =========================

type QWORD = u64;

#[repr(C)]
#[derive(Clone, Copy)]
struct SDATA {
    data: *mut u8,
    size: DWORD,
}

const TTSI_NAMELEN: usize = 262;
const TTSI_STYLELEN: usize = 262;
const LANG_LEN: usize = 64;

#[repr(C)]
#[derive(Clone, Copy)]
struct LANGUAGEW {
    language_id: WORD,
    dialect: [u16; LANG_LEN],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct TTSMODEINFO {
    engine_id: GUID,
    manufacturer: [u16; TTSI_NAMELEN],
    product_name: [u16; TTSI_NAMELEN],
    mode_id: GUID,
    mode_name: [u16; TTSI_NAMELEN],
    language: LANGUAGEW,
    speaker: [u16; TTSI_NAMELEN],
    style: [u16; TTSI_STYLELEN],
    gender: WORD,
    age: WORD,
    features: DWORD,
    interfaces: DWORD,
    engine_features: DWORD,
}

impl TTSMODEINFO {
    fn mode_id(&self) -> GUID {
        self.mode_id
    }
    fn mode_name(&self) -> String {
        String::from_utf16_lossy(&self.mode_name)
            .trim_end_matches('\0')
            .to_string()
    }
}

#[repr(C)]
struct ITTSEnumVtbl {
    query_interface:
        unsafe extern "system" fn(*mut ITTSEnum, REFIID, *mut *mut std::ffi::c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut ITTSEnum) -> u32,
    release: unsafe extern "system" fn(*mut ITTSEnum) -> u32,
    next: unsafe extern "system" fn(*mut ITTSEnum, u32, *mut TTSMODEINFO, *mut u32) -> i32,
    skip: unsafe extern "system" fn(*mut ITTSEnum, u32) -> i32,
    reset: unsafe extern "system" fn(*mut ITTSEnum) -> i32,
    clone: unsafe extern "system" fn(*mut ITTSEnum, *mut *mut ITTSEnum) -> i32,
    select:
        unsafe extern "system" fn(*mut ITTSEnum, GUID, *mut *mut ITTSCentral, *mut IUnknown) -> i32,
}
#[repr(C)]
struct ITTSEnum {
    lpVtbl: *const ITTSEnumVtbl,
}

#[repr(C)]
struct ITTSCentralVtbl {
    query_interface:
        unsafe extern "system" fn(*mut ITTSCentral, REFIID, *mut *mut std::ffi::c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut ITTSCentral) -> u32,
    release: unsafe extern "system" fn(*mut ITTSCentral) -> u32,
    inject: unsafe extern "system" fn(*mut ITTSCentral, *const u16) -> i32,
    mode_get: unsafe extern "system" fn(*mut ITTSCentral, *mut TTSMODEINFO) -> i32,
    phoneme: unsafe extern "system" fn(*mut ITTSCentral, u32, DWORD, SDATA, *mut SDATA) -> i32,
    posn_get: unsafe extern "system" fn(*mut ITTSCentral, *mut QWORD) -> i32,
    text_data: unsafe extern "system" fn(
        *mut ITTSCentral,
        u32,
        DWORD,
        SDATA,
        *mut std::ffi::c_void,
        GUID,
    ) -> i32,
    to_file_time: unsafe extern "system" fn(*mut ITTSCentral, *mut QWORD, *mut FILETIME) -> i32,
    audio_pause: unsafe extern "system" fn(*mut ITTSCentral) -> i32,
    audio_resume: unsafe extern "system" fn(*mut ITTSCentral) -> i32,
    audio_reset: unsafe extern "system" fn(*mut ITTSCentral) -> i32,
    register:
        unsafe extern "system" fn(*mut ITTSCentral, *mut std::ffi::c_void, GUID, *mut DWORD) -> i32,
    un_register: unsafe extern "system" fn(*mut ITTSCentral, DWORD) -> i32,
}
#[repr(C)]
struct ITTSCentral {
    lpVtbl: *const ITTSCentralVtbl,
}

#[repr(C)]
struct IAudioFileVtbl {
    query_interface:
        unsafe extern "system" fn(*mut IAudioFile, REFIID, *mut *mut std::ffi::c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut IAudioFile) -> u32,
    release: unsafe extern "system" fn(*mut IAudioFile) -> u32,
    register: unsafe extern "system" fn(*mut IAudioFile, *mut std::ffi::c_void) -> i32,
    set: unsafe extern "system" fn(*mut IAudioFile, *const u16, DWORD) -> i32,
    add: unsafe extern "system" fn(*mut IAudioFile, *const u16, DWORD) -> i32,
    flush: unsafe extern "system" fn(*mut IAudioFile) -> i32,
    real_time_set: unsafe extern "system" fn(*mut IAudioFile, WORD) -> i32,
    real_time_get: unsafe extern "system" fn(*mut IAudioFile, *mut WORD) -> i32,
}
#[repr(C)]
struct IAudioFile {
    lpVtbl: *const IAudioFileVtbl,
}

#[repr(C)]
struct ITTSAttributesVtbl {
    query_interface:
        unsafe extern "system" fn(*mut ITTSAttributes, REFIID, *mut *mut std::ffi::c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut ITTSAttributes) -> u32,
    release: unsafe extern "system" fn(*mut ITTSAttributes) -> u32,
    pitch_get: unsafe extern "system" fn(*mut ITTSAttributes, *mut WORD) -> i32,
    pitch_set: unsafe extern "system" fn(*mut ITTSAttributes, WORD) -> i32,
    real_time_get: unsafe extern "system" fn(*mut ITTSAttributes, *mut DWORD) -> i32,
    real_time_set: unsafe extern "system" fn(*mut ITTSAttributes, DWORD) -> i32,
    speed_get: unsafe extern "system" fn(*mut ITTSAttributes, *mut DWORD) -> i32,
    speed_set: unsafe extern "system" fn(*mut ITTSAttributes, DWORD) -> i32,
    volume_get: unsafe extern "system" fn(*mut ITTSAttributes, *mut DWORD) -> i32,
    volume_set: unsafe extern "system" fn(*mut ITTSAttributes, DWORD) -> i32,
}
#[repr(C)]
struct ITTSAttributes {
    lpVtbl: *const ITTSAttributesVtbl,
}

// =========================
// Notify sinks + shared state
// =========================

struct SpeakState {
    done: AtomicBool,
    buffer_completion: bool,
    generation: AtomicU32,
    current_text: Mutex<Option<U16CString>>,
    queue: Mutex<VecDeque<SpeakItem>>,
}
impl SpeakState {
    fn new() -> Self {
        Self {
            done: AtomicBool::new(true),
            buffer_completion: false,
            generation: AtomicU32::new(0),
            current_text: Mutex::new(None),
            queue: Mutex::new(VecDeque::new()),
        }
    }
    fn for_file() -> Self {
        Self {
            buffer_completion: true,
            ..Self::new()
        }
    }
    fn finish_buffer(&self, generation: u32) -> bool {
        let mut text = self.current_text.lock().unwrap_or_else(|e| e.into_inner());
        if self.generation.load(Ordering::Acquire) != generation
            || self.done.load(Ordering::Acquire)
        {
            return false;
        }
        *text = None;
        self.mark_done();
        true
    }
    fn mark_done(&self) {
        self.done.store(true, Ordering::Release);
    }
    fn mark_running(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.done.store(false, Ordering::Release);
    }
}

#[repr(C)]
struct ITTSNotifySinkVtbl {
    query_interface:
        unsafe extern "system" fn(*mut ITTSNotifySink, REFIID, *mut *mut std::ffi::c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut ITTSNotifySink) -> u32,
    release: unsafe extern "system" fn(*mut ITTSNotifySink) -> u32,
    attrib_changed: unsafe extern "system" fn(*mut ITTSNotifySink, DWORD) -> i32,
    audio_start: unsafe extern "system" fn(*mut ITTSNotifySink, QWORD) -> i32,
    audio_stop: unsafe extern "system" fn(*mut ITTSNotifySink, QWORD) -> i32,
    visual: unsafe extern "system" fn(
        *mut ITTSNotifySink,
        QWORD,
        u16,
        u16,
        DWORD,
        *mut std::ffi::c_void,
    ) -> i32,
}

#[repr(C)]
struct ITTSNotifySink {
    lpVtbl: *const ITTSNotifySinkVtbl,
    refcnt: AtomicU32,
    state: Arc<SpeakState>,
}

unsafe extern "system" fn notify_query_interface(
    this: *mut ITTSNotifySink,
    riid: REFIID,
    ppv: *mut *mut std::ffi::c_void,
) -> i32 {
    if ppv.is_null() {
        return E_NOINTERFACE;
    }
    *ppv = ptr::null_mut();
    if guid_eq(riid, &IID_IUNKNOWN) || guid_eq(riid, &IID_ITTSNOTIFYSINKW) {
        *ppv = this as *mut std::ffi::c_void;
        notify_add_ref(this);
        return S_OK;
    }
    E_NOINTERFACE
}

unsafe extern "system" fn notify_add_ref(this: *mut ITTSNotifySink) -> u32 {
    if this.is_null() {
        return 0;
    }
    let sink = &*this;
    sink.refcnt.fetch_add(1, Ordering::SeqCst) + 1
}

unsafe extern "system" fn notify_release(this: *mut ITTSNotifySink) -> u32 {
    if this.is_null() {
        return 0;
    }
    let sink = &*this;
    let prev = sink.refcnt.fetch_sub(1, Ordering::SeqCst);
    let next = prev.saturating_sub(1);
    if next == 0 {
        let _unused_box = Box::from_raw(this);
    }
    next
}

unsafe extern "system" fn notify_attrib_changed(_this: *mut ITTSNotifySink, _attr: DWORD) -> i32 {
    S_OK
}
unsafe extern "system" fn notify_audio_start(_this: *mut ITTSNotifySink, _ts: QWORD) -> i32 {
    S_OK
}
unsafe extern "system" fn notify_audio_stop(this: *mut ITTSNotifySink, _ts: QWORD) -> i32 {
    if !this.is_null() {
        let sink = &*this;
        if sink.state.buffer_completion {
            return S_OK;
        }
        sink.state.mark_done();
        let mut guard = match sink.state.current_text.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        *guard = None;
    }
    S_OK
}
unsafe extern "system" fn notify_visual(
    _this: *mut ITTSNotifySink,
    _ts: QWORD,
    _ipa: u16,
    _engine: u16,
    _hints: DWORD,
    _mouth: *mut std::ffi::c_void,
) -> i32 {
    S_OK
}

static NOTIFY_VTBL: ITTSNotifySinkVtbl = ITTSNotifySinkVtbl {
    query_interface: notify_query_interface,
    add_ref: notify_add_ref,
    release: notify_release,
    attrib_changed: notify_attrib_changed,
    audio_start: notify_audio_start,
    audio_stop: notify_audio_stop,
    visual: notify_visual,
};

// ---- Audio file notify ----

#[repr(C)]
struct IAudioFileNotifySinkVtbl {
    query_interface: unsafe extern "system" fn(
        *mut IAudioFileNotifySink,
        REFIID,
        *mut *mut std::ffi::c_void,
    ) -> i32,
    add_ref: unsafe extern "system" fn(*mut IAudioFileNotifySink) -> u32,
    release: unsafe extern "system" fn(*mut IAudioFileNotifySink) -> u32,
    file_begin: unsafe extern "system" fn(*mut IAudioFileNotifySink, DWORD) -> i32,
    file_end: unsafe extern "system" fn(*mut IAudioFileNotifySink, DWORD) -> i32,
    queue_empty: unsafe extern "system" fn(*mut IAudioFileNotifySink) -> i32,
    posn: unsafe extern "system" fn(*mut IAudioFileNotifySink, QWORD, QWORD) -> i32,
}

#[repr(C)]
struct IAudioFileNotifySink {
    lpVtbl: *const IAudioFileNotifySinkVtbl,
    refcnt: AtomicU32,
    state: Arc<SpeakState>,
}

unsafe extern "system" fn audio_file_notify_query_interface(
    this: *mut IAudioFileNotifySink,
    riid: REFIID,
    ppv: *mut *mut std::ffi::c_void,
) -> i32 {
    if ppv.is_null() {
        return E_NOINTERFACE;
    }
    *ppv = ptr::null_mut();
    if guid_eq(riid, &IID_IUNKNOWN) || guid_eq(riid, &IID_IAUDIOFILENOTIFYSINK) {
        *ppv = this as *mut std::ffi::c_void;
        audio_file_notify_add_ref(this);
        return S_OK;
    }
    E_NOINTERFACE
}

unsafe extern "system" fn audio_file_notify_add_ref(this: *mut IAudioFileNotifySink) -> u32 {
    if this.is_null() {
        return 0;
    }
    let sink = &*this;
    sink.refcnt.fetch_add(1, Ordering::SeqCst) + 1
}

unsafe extern "system" fn audio_file_notify_release(this: *mut IAudioFileNotifySink) -> u32 {
    if this.is_null() {
        return 0;
    }
    let sink = &*this;
    let prev = sink.refcnt.fetch_sub(1, Ordering::SeqCst);
    let next = prev.saturating_sub(1);
    if next == 0 {
        let _unused_box = Box::from_raw(this);
    }
    next
}

unsafe extern "system" fn audio_file_notify_file_begin(
    _this: *mut IAudioFileNotifySink,
    _id: DWORD,
) -> i32 {
    S_OK
}
unsafe extern "system" fn audio_file_notify_file_end(
    this: *mut IAudioFileNotifySink,
    _id: DWORD,
) -> i32 {
    if !this.is_null() {
        let sink = &*this;
        if !sink.state.buffer_completion {
            sink.state.mark_done();
        }
    }
    S_OK
}
unsafe extern "system" fn audio_file_notify_queue_empty(this: *mut IAudioFileNotifySink) -> i32 {
    if !this.is_null() {
        let sink = &*this;
        if !sink.state.buffer_completion {
            sink.state.mark_done();
        }
    }
    S_OK
}
unsafe extern "system" fn audio_file_notify_posn(
    _this: *mut IAudioFileNotifySink,
    _posn: QWORD,
    _time: QWORD,
) -> i32 {
    S_OK
}

static AUDIO_FILE_NOTIFY_VTBL: IAudioFileNotifySinkVtbl = IAudioFileNotifySinkVtbl {
    query_interface: audio_file_notify_query_interface,
    add_ref: audio_file_notify_add_ref,
    release: audio_file_notify_release,
    file_begin: audio_file_notify_file_begin,
    file_end: audio_file_notify_file_end,
    queue_empty: audio_file_notify_queue_empty,
    posn: audio_file_notify_posn,
};

struct SinkHandle<T> {
    ptr: *mut T,
}
impl<T> SinkHandle<T> {
    fn new(ptr: *mut T) -> Option<Self> {
        if ptr.is_null() {
            None
        } else {
            Some(Self { ptr })
        }
    }
    fn as_void_ptr(&self) -> *mut std::ffi::c_void {
        self.ptr as *mut std::ffi::c_void
    }
}
impl SinkHandle<ITTSNotifySink> {
    unsafe fn add_ref(&self) {
        notify_add_ref(self.ptr);
    }
    unsafe fn release(&self) {
        notify_release(self.ptr);
    }
}
impl SinkHandle<IAudioFileNotifySink> {
    unsafe fn add_ref(&self) {
        audio_file_notify_add_ref(self.ptr);
    }
    unsafe fn release(&self) {
        audio_file_notify_release(self.ptr);
    }
}

// =========================
// Media Foundation guard
// =========================

struct MfGuard;
impl MfGuard {
    fn start() -> Result<Self, String> {
        unsafe {
            if let Err(e) = MFStartup(MF_VERSION, 0) {
                return Err(format!("MFStartup failed: {}", e));
            }
        }
        Ok(MfGuard)
    }
}
impl Drop for MfGuard {
    fn drop(&mut self) {
        unsafe {
            if let Err(e) = MFShutdown() {
                eprintln!("MFShutdown failed: {}", e);
            }
        }
    }
}

fn encode_wav_to_mp3(wav_path: &Path, mp3_path: &Path, bitrate_kbps: u32) -> Result<(), String> {
    unsafe {
        let _mf = MfGuard::start()?;

        let wav_str = wav_path
            .to_str()
            .ok_or_else(|| "Invalid wav path".to_string())?;
        let mp3_str = mp3_path
            .to_str()
            .ok_or_else(|| "Invalid mp3 path".to_string())?;

        let wav_wide = U16CString::from_str(wav_str).map_err(|_| "Invalid wav path".to_string())?;
        let mp3_wide = U16CString::from_str(mp3_str).map_err(|_| "Invalid mp3 path".to_string())?;

        let reader: IMFSourceReader = MFCreateSourceReaderFromURL(PCWSTR(wav_wide.as_ptr()), None)
            .map_err(|e| format!("MFCreateSourceReaderFromURL failed: {}", e))?;

        let pcm_type: IMFMediaType =
            MFCreateMediaType().map_err(|e| format!("MFCreateMediaType failed: {}", e))?;
        pcm_type
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)
            .map_err(|e| format!("SetGUID major type failed: {}", e))?;
        pcm_type
            .SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM)
            .map_err(|e| format!("SetGUID subtype PCM failed: {}", e))?;

        let sample_rate = 44100u32;
        let channels = 2u32;
        let bits_per_sample = 16u32;
        let block_align = channels * (bits_per_sample / 8);
        let avg_bytes = sample_rate * block_align;

        pcm_type
            .SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, sample_rate)
            .map_err(|e| format!("Set sample rate failed: {}", e))?;
        pcm_type
            .SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, channels)
            .map_err(|e| format!("Set channels failed: {}", e))?;
        pcm_type
            .SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, bits_per_sample)
            .map_err(|e| format!("Set bits per sample failed: {}", e))?;
        pcm_type
            .SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, block_align)
            .map_err(|e| format!("Set block alignment failed: {}", e))?;
        pcm_type
            .SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, avg_bytes)
            .map_err(|e| format!("Set avg bytes failed: {}", e))?;

        reader
            .SetCurrentMediaType(
                MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32,
                None,
                &pcm_type,
            )
            .map_err(|e| format!("SetCurrentMediaType failed: {}", e))?;

        let in_type = reader
            .GetCurrentMediaType(MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32)
            .map_err(|e| format!("GetCurrentMediaType failed: {}", e))?;

        let out_type: IMFMediaType =
            MFCreateMediaType().map_err(|e| format!("MFCreateMediaType failed: {}", e))?;
        out_type
            .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)
            .map_err(|e| format!("SetGUID major type failed: {}", e))?;
        out_type
            .SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_MP3)
            .map_err(|e| format!("SetGUID subtype MP3 failed: {}", e))?;
        out_type
            .SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, channels)
            .map_err(|e| format!("Set channels failed: {}", e))?;
        out_type
            .SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, sample_rate)
            .map_err(|e| format!("Set sample rate failed: {}", e))?;

        let mp3_avg_bytes = bitrate_kbps.saturating_mul(1000) / 8;
        out_type
            .SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, mp3_avg_bytes)
            .map_err(|e| format!("Set mp3 bitrate failed: {}", e))?;

        let writer: IMFSinkWriter =
            MFCreateSinkWriterFromURL(PCWSTR(mp3_wide.as_ptr()), None, None)
                .map_err(|e| format!("MFCreateSinkWriterFromURL failed: {}", e))?;

        let stream_index = writer
            .AddStream(&out_type)
            .map_err(|e| format!("AddStream failed: {}", e))?;
        writer
            .SetInputMediaType(stream_index, &in_type, None)
            .map_err(|e| format!("SetInputMediaType failed: {}", e))?;
        writer
            .BeginWriting()
            .map_err(|e| format!("BeginWriting failed: {}", e))?;

        loop {
            let mut read_stream = 0u32;
            let mut flags = 0u32;
            let mut timestamp = 0i64;
            let mut sample = None;

            reader
                .ReadSample(
                    MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32,
                    0,
                    Some(&mut read_stream),
                    Some(&mut flags),
                    Some(&mut timestamp),
                    Some(&mut sample),
                )
                .map_err(|e| format!("ReadSample failed: {}", e))?;

            if (flags & (MF_SOURCE_READERF_ENDOFSTREAM.0 as u32)) != 0 {
                break;
            }

            if let Some(s) = sample {
                writer
                    .WriteSample(stream_index, &s)
                    .map_err(|e| format!("WriteSample failed: {}", e))?;
            }
        }

        writer
            .Finalize()
            .map_err(|e| format!("Finalize failed: {}", e))?;
        Ok(())
    }
}

// =========================
// CLI parsing
// =========================

fn arg_value(args: &[String], key: &str) -> Option<String> {
    let mut i = 0usize;
    while i < args.len() {
        if args[i] == key {
            if i + 1 < args.len() {
                return Some(args[i + 1].clone());
            }
            return None;
        }
        i = i.saturating_add(1);
    }
    None
}

fn arg_flag(args: &[String], key: &str) -> bool {
    args.iter().any(|a| a == key)
}

fn arg_u32(args: &[String], key: &str, default: u32) -> u32 {
    match arg_value(args, key) {
        Some(s) => match s.parse::<u32>() {
            Ok(v) => v,
            Err(_) => default,
        },
        None => default,
    }
}

fn arg_i32_opt(args: &[String], key: &str) -> Option<i32> {
    arg_value(args, key).and_then(|s| s.parse::<i32>().ok())
}

// =========================
// Core: enumerate / init
// =========================

fn get_mode_name(info: &TTSMODEINFO) -> String {
    info.mode_name()
}

fn list_voices() -> Result<(), String> {
    let _com = ComGuard::init_mta()?;

    let mut enum_raw: *mut ITTSEnum = ptr::null_mut();
    unsafe {
        let hr = CoCreateInstance(
            &CLSID_TTSENUMERATOR,
            ptr::null_mut(),
            CLSCTX_ALL,
            &IID_ITTSENUM,
            &mut enum_raw as *mut _ as *mut _,
        );
        if !hr_ok(hr) || enum_raw.is_null() {
            return Err(format!("Failed to create TTSEnumerator, hr={:#x}", hr));
        }
    }

    let enum_ptr = ComPtr::new(enum_raw).ok_or_else(|| "Null ITTSEnum".to_string())?;
    let vtbl = unsafe { &*(*enum_ptr.as_ptr()).lpVtbl };
    unsafe { (vtbl.reset)(enum_ptr.as_ptr()) };

    let mut idx = 1u32;
    loop {
        let mut mode_info: TTSMODEINFO = unsafe { std::mem::zeroed() };
        let mut fetched: u32 = 0;

        let hr = unsafe { (vtbl.next)(enum_ptr.as_ptr(), 1, &mut mode_info, &mut fetched) };
        if !hr_ok(hr) || fetched == 0 {
            break;
        }

        println!("VOICE:{}|{}", idx, get_mode_name(&mode_info));
        idx = idx.saturating_add(1);
    }

    Ok(())
}

fn list_sapi5_voices() -> Result<(), String> {
    let _com = ComGuard::init_mta()?;
    let voice: ISpeechVoice = unsafe {
        windows::Win32::System::Com::CoCreateInstance(
            &SpVoice,
            None,
            windows::Win32::System::Com::CLSCTX_ALL,
        )
        .map_err(|e| format!("CoCreateInstance(ISpeechVoice) failed: {}", e))?
    };
    let required = windows::core::BSTR::from("");
    let optional = windows::core::BSTR::from("");
    let tokens = unsafe { voice.GetVoices(&required, &optional) }
        .map_err(|e| format!("ISpeechVoice.GetVoices failed: {}", e))?;
    let count = unsafe { tokens.Count() }.map_err(|e| format!("tokens.Count failed: {}", e))?;
    for i in 0..count {
        if let Ok(token) = unsafe { tokens.Item(i) } {
            let desc = match unsafe { token.GetDescription(0) } {
                Ok(d) => d,
                Err(_) => continue,
            };
            let name = desc.to_string();
            if name.trim().is_empty() {
                continue;
            }
            println!("SAPI5VOICE:{}", name);
        }
    }
    Ok(())
}

fn find_sapi5_token_by_name(voice_name: &str) -> Option<ISpObjectToken> {
    let categories = [
        r"HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Speech\Voices",
        r"HKEY_CURRENT_USER\SOFTWARE\Microsoft\Speech\Voices",
        r"HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Speech_OneCore\Voices",
        r"HKEY_CURRENT_USER\SOFTWARE\Microsoft\Speech_OneCore\Voices",
    ];
    for category_id in categories {
        let category: ISpeechObjectTokenCategory = unsafe {
            windows::Win32::System::Com::CoCreateInstance(
                &SpObjectTokenCategory,
                None,
                windows::Win32::System::Com::CLSCTX_ALL,
            )
            .ok()?
        };
        let id = windows::core::BSTR::from(category_id);
        if unsafe { category.SetId(&id, VARIANT_BOOL(0)) }.is_err() {
            continue;
        }
        let required = windows::core::BSTR::from("");
        let optional = windows::core::BSTR::from("");
        let tokens = match unsafe { category.EnumerateTokens(&required, &optional) } {
            Ok(t) => t,
            Err(_) => continue,
        };
        let count = match unsafe { tokens.Count() } {
            Ok(c) => c,
            Err(_) => continue,
        };
        for i in 0..count {
            let token: ISpeechObjectToken = match unsafe { tokens.Item(i) } {
                Ok(t) => t,
                Err(_) => continue,
            };
            let desc = match unsafe { token.GetDescription(0) } {
                Ok(d) => d.to_string(),
                Err(_) => String::new(),
            };
            if desc == voice_name {
                if let Ok(sp_token) = token.cast::<ISpObjectToken>() {
                    return Some(sp_token);
                }
            }
        }
    }
    None
}

fn find_sapi5_speech_token_by_name(voice_name: &str) -> Option<ISpeechObjectToken> {
    let voice: ISpeechVoice = unsafe {
        windows::Win32::System::Com::CoCreateInstance(
            &SpVoice,
            None,
            windows::Win32::System::Com::CLSCTX_ALL,
        )
        .ok()?
    };
    let required = windows::core::BSTR::from("");
    let optional = windows::core::BSTR::from("");
    let tokens = unsafe { voice.GetVoices(&required, &optional) }.ok()?;
    let count = unsafe { tokens.Count() }.ok()?;
    for i in 0..count {
        let token = match unsafe { tokens.Item(i) } {
            Ok(t) => t,
            Err(_) => continue,
        };
        let desc = unsafe { token.GetDescription(0) }
            .map(|d| d.to_string())
            .unwrap_or_default();
        if desc == voice_name {
            return Some(token);
        }
    }
    None
}

fn map_sapi5_rate(rate_percent: i32) -> i32 {
    (rate_percent / 10).clamp(-10, 10)
}

fn map_sapi5_volume(volume: i32) -> u16 {
    volume.clamp(0, 100) as u16
}

fn escape_sapi5_xml(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

fn sapi5_xml_with_pitch(text: &str, pitch: Option<i32>) -> String {
    let escaped = escape_sapi5_xml(text);
    match pitch {
        Some(p) => format!(
            "<pitch absmiddle='{:+}'>{}</pitch>",
            p.clamp(-10, 10),
            escaped
        ),
        None => escaped,
    }
}

fn configure_sapi5_voice(
    voice: &ISpVoice,
    voice_name: &str,
    rate: Option<i32>,
    volume: Option<i32>,
) -> Result<(), String> {
    if !voice_name.trim().is_empty() {
        if let Some(token) = find_sapi5_token_by_name(voice_name) {
            unsafe { voice.SetVoice(&token) }.map_err(|e| format!("SetVoice failed: {}", e))?;
        } else {
            return Err(format!(
                "SAPI5 voice not found in 32-bit bridge: {}",
                voice_name
            ));
        }
    }
    if let Some(r) = rate {
        unsafe { voice.SetRate(map_sapi5_rate(r)) }
            .map_err(|e| format!("SetRate failed: {}", e))?;
    }
    if let Some(v) = volume {
        unsafe { voice.SetVolume(map_sapi5_volume(v)) }
            .map_err(|e| format!("SetVolume failed: {}", e))?;
    }
    Ok(())
}

fn run_sapi5_server(
    voice_name: &str,
    rate: Option<i32>,
    pitch: Option<i32>,
    volume: Option<i32>,
) -> Result<(), String> {
    let _com = ComGuard::init_mta()?;
    let voice: ISpVoice = unsafe {
        windows::Win32::System::Com::CoCreateInstance(
            &SpVoice,
            None,
            windows::Win32::System::Com::CLSCTX_ALL,
        )
        .map_err(|e| format!("CoCreateInstance(ISpVoice) failed: {}", e))?
    };
    configure_sapi5_voice(&voice, voice_name, rate, volume)?;
    let audio_output: Option<ISpMMSysAudio> = unsafe {
        match windows::Win32::System::Com::CoCreateInstance(
            &SpMMAudioOut,
            None,
            windows::Win32::System::Com::CLSCTX_ALL,
        ) {
            Ok(audio) => {
                if let Err(e) = voice.SetOutput(&audio, true) {
                    eprintln!("SAPI5 bridge audio output bind failed: {}", e);
                    None
                } else {
                    Some(audio)
                }
            }
            Err(e) => {
                eprintln!("SAPI5 bridge audio output create failed: {}", e);
                None
            }
        }
    };
    let rx = spawn_command_reader();
    loop {
        match rx.recv() {
            Ok(ServerCommand::Speak(text)) => {
                let xml = sapi5_xml_with_pitch(&text, pitch);
                let wide =
                    U16CString::from_str(xml).map_err(|e| format!("Invalid UTF-16 text: {}", e))?;
                unsafe {
                    voice.Speak(
                        PCWSTR(wide.as_ptr()),
                        (SPF_ASYNC.0 | SPF_IS_XML.0) as u32,
                        None,
                    )
                }
                .map_err(|e| format!("Speak failed: {}", e))?;
            }
            Ok(ServerCommand::Pause) => {
                if let Some(audio) = &audio_output {
                    if let Err(e) = unsafe { audio.SetState(SPAS_PAUSE, 0) } {
                        eprintln!("SAPI5 bridge audio pause failed: {}", e);
                    }
                } else if let Err(e) = unsafe { voice.Pause() } {
                    eprintln!("SAPI5 bridge voice pause failed: {}", e);
                }
            }
            Ok(ServerCommand::Resume) => {
                if let Some(audio) = &audio_output {
                    if let Err(e) = unsafe { audio.SetState(SPAS_RUN, 0) } {
                        eprintln!("SAPI5 bridge audio resume failed: {}", e);
                    }
                } else if let Err(e) = unsafe { voice.Resume() } {
                    eprintln!("SAPI5 bridge voice resume failed: {}", e);
                }
            }
            Ok(ServerCommand::Stop) | Ok(ServerCommand::Quit) | Err(_) => {
                if let Some(audio) = &audio_output {
                    if let Err(e) = unsafe { audio.SetState(SPAS_STOP, 0) } {
                        eprintln!("SAPI5 bridge audio stop failed: {}", e);
                    }
                }
                if let Err(e) =
                    unsafe { voice.Speak(PCWSTR::null(), SPF_PURGEBEFORESPEAK.0 as u32, None) }
                {
                    eprintln!("SAPI5 bridge stop failed: {}", e);
                }
                break;
            }
        }
    }
    Ok(())
}

fn speak_sapi5_to_file(
    voice_name: &str,
    out_path: &str,
    rate: Option<i32>,
    pitch: Option<i32>,
    volume: Option<i32>,
) -> Result<(), String> {
    let _com = ComGuard::init_mta()?;

    let mut text = String::new();
    io::stdin()
        .read_to_string(&mut text)
        .map_err(|e| format!("Failed to read stdin: {}", e))?;

    let voice: ISpeechVoice = unsafe {
        windows::Win32::System::Com::CoCreateInstance(
            &SpVoice,
            None,
            windows::Win32::System::Com::CLSCTX_ALL,
        )
        .map_err(|e| format!("CoCreateInstance(ISpeechVoice) failed: {}", e))?
    };
    if !voice_name.trim().is_empty() {
        let token = find_sapi5_speech_token_by_name(voice_name)
            .ok_or_else(|| format!("SAPI5 voice not found in 32-bit bridge: {}", voice_name))?;
        unsafe { voice.putref_Voice(&token) }.map_err(|e| format!("putref_Voice failed: {}", e))?;
    }
    if let Some(r) = rate {
        unsafe { voice.SetRate(map_sapi5_rate(r)) }
            .map_err(|e| format!("SetRate failed: {}", e))?;
    }
    if let Some(v) = volume {
        unsafe { voice.SetVolume(v.clamp(0, 100)) }
            .map_err(|e| format!("SetVolume failed: {}", e))?;
    }

    let stream: ISpeechFileStream = unsafe {
        windows::Win32::System::Com::CoCreateInstance(
            &SpFileStream,
            None,
            windows::Win32::System::Com::CLSCTX_ALL,
        )
        .map_err(|e| format!("CoCreateInstance(ISpeechFileStream) failed: {}", e))?
    };
    let path_bstr = windows::core::BSTR::from(out_path);
    unsafe { stream.Open(&path_bstr, SSFMCreateForWrite, VARIANT_BOOL(0)) }
        .map_err(|e| format!("ISpeechFileStream.Open failed: {}", e))?;
    unsafe { voice.putref_AudioOutputStream(&stream) }
        .map_err(|e| format!("putref_AudioOutputStream failed: {}", e))?;

    if !text.trim().is_empty() {
        let xml = sapi5_xml_with_pitch(&text, pitch);
        let text_bstr = windows::core::BSTR::from(xml);
        unsafe { voice.Speak(&text_bstr, SpeechVoiceSpeakFlags(8)) }
            .map_err(|e| format!("Speak failed: {}", e))?;
        unsafe { voice.WaitUntilDone(i32::MAX) }
            .map_err(|e| format!("WaitUntilDone failed: {}", e))?;
    }
    unsafe { stream.Close() }.map_err(|e| format!("Close stream failed: {}", e))?;
    Ok(())
}

unsafe fn init_central_with_audio(
    target_idx: u32,
    mut audio_ptr: *mut IUnknown,
) -> Result<(ComPtr<ITTSEnum>, ComPtr<ITTSCentral>), String> {
    let mut enum_raw: *mut ITTSEnum = ptr::null_mut();
    let hr = CoCreateInstance(
        &CLSID_TTSENUMERATOR,
        ptr::null_mut(),
        CLSCTX_ALL,
        &IID_ITTSENUM,
        &mut enum_raw as *mut _ as *mut _,
    );
    if !hr_ok(hr) || enum_raw.is_null() {
        return Err(format!("Failed to create TTSEnumerator, hr={:#x}", hr));
    }

    let enum_ptr = ComPtr::new(enum_raw).ok_or_else(|| "Null ITTSEnum".to_string())?;
    let vtbl = &*(*enum_ptr.as_ptr()).lpVtbl;

    (vtbl.reset)(enum_ptr.as_ptr());

    let mut target_mode: TTSMODEINFO = std::mem::zeroed();
    let mut found = false;
    let mut idx = 1u32;

    loop {
        let mut mode_info: TTSMODEINFO = std::mem::zeroed();
        let mut fetched: u32 = 0;

        let hrn = (vtbl.next)(enum_ptr.as_ptr(), 1, &mut mode_info, &mut fetched);
        if !hr_ok(hrn) || fetched == 0 {
            break;
        }

        if idx == target_idx {
            target_mode = mode_info;
            found = true;
            eprintln!("Found voice {}: {}", idx, get_mode_name(&mode_info));
            break;
        }

        idx = idx.saturating_add(1);
    }

    if !found {
        return Err(format!("Voice {} not found", target_idx));
    }

    if audio_ptr.is_null() {
        let mut audio_unknown: *mut IUnknown = ptr::null_mut();
        let hra = CoCreateInstance(
            &CLSID_MMAUDIODEST,
            ptr::null_mut(),
            CLSCTX_ALL,
            &IID_IAUDIO,
            &mut audio_unknown as *mut _ as *mut _,
        );
        if hr_ok(hra) && !audio_unknown.is_null() {
            audio_ptr = audio_unknown;
        } else {
            eprintln!(
                "Failed to create MMAudioDest, hr={:#x}. Continuing without it.",
                hra
            );
            audio_ptr = ptr::null_mut();
        }
    }

    let mut central_raw: *mut ITTSCentral = ptr::null_mut();
    let mode_guid = target_mode.mode_id();
    let hrs = (vtbl.select)(enum_ptr.as_ptr(), mode_guid, &mut central_raw, audio_ptr);
    if !hr_ok(hrs) || central_raw.is_null() {
        return Err(format!("ITTSEnum::Select failed, hr={:#x}", hrs));
    }

    let central_ptr = ComPtr::new(central_raw).ok_or_else(|| "Null ITTSCentral".to_string())?;
    Ok((enum_ptr, central_ptr))
}

unsafe fn init_central(target_idx: u32) -> Result<(ComPtr<ITTSEnum>, ComPtr<ITTSCentral>), String> {
    init_central_with_audio(target_idx, ptr::null_mut())
}

// =========================
// Speak buffer lifetime helpers
// =========================

fn sanitize_text_for_u16c(text: &str) -> String {
    if text.contains('\0') {
        text.replace('\0', " ")
    } else {
        text.to_string()
    }
}

enum ParsedSpeakSegment {
    Text(String),
    Pause(u32),
}

fn parse_pause_tag_milliseconds(tag: &str) -> Option<u32> {
    let inner = tag
        .trim()
        .strip_prefix('<')?
        .strip_suffix('>')?
        .trim()
        .trim_end_matches('/')
        .trim();
    let rest = inner.strip_prefix("pause")?.trim();
    for token in rest.split_whitespace() {
        let value = token
            .strip_prefix("ms=")
            .or_else(|| token.strip_prefix("milliseconds="))
            .unwrap_or(token)
            .trim_matches(['"', '\'']);
        if let Ok(ms) = value.parse::<u32>() {
            return Some(ms.clamp(50, 60_000));
        }
    }
    None
}

fn split_pause_tag_segments(text: &str) -> Vec<ParsedSpeakSegment> {
    let lower = text.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut cursor = 0usize;
    let mut i = 0usize;
    let bytes = lower.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'<' {
            let remaining = &lower[i..];
            if remaining.starts_with("<pause") {
                if let Some(end_rel) = remaining.find('>') {
                    let end = i + end_rel + 1;
                    if let Some(ms) = parse_pause_tag_milliseconds(&lower[i..end]) {
                        if i > cursor {
                            let part = text[cursor..i].trim();
                            if !part.is_empty() {
                                out.push(ParsedSpeakSegment::Text(part.to_string()));
                            }
                        }
                        out.push(ParsedSpeakSegment::Pause(ms));
                        cursor = end;
                        i = end;
                        continue;
                    }
                }
            }
        }
        i += 1;
    }
    if cursor < text.len() {
        let part = text[cursor..].trim();
        if !part.is_empty() {
            out.push(ParsedSpeakSegment::Text(part.to_string()));
        }
    }
    if out.is_empty() && !text.trim().is_empty() {
        out.push(ParsedSpeakSegment::Text(text.to_string()));
    }
    out
}

fn set_current_text(state: &Arc<SpeakState>, text: U16CString) {
    let mut guard = match state.current_text.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    *guard = Some(text);
}

fn enqueue_item(state: &Arc<SpeakState>, item: SpeakItem) {
    let mut q = match state.queue.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    q.push_back(item);
}

fn pop_next_item(state: &Arc<SpeakState>) -> Option<SpeakItem> {
    let mut q = match state.queue.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    q.pop_front()
}

unsafe fn speak_from_state_with_flags(
    central_ptr: *mut ITTSCentral,
    state: &Arc<SpeakState>,
    flags: DWORD,
) -> i32 {
    let guard = match state.current_text.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };

    let text_u16 = match guard.as_ref() {
        Some(t) => t,
        None => return -1,
    };

    let central_vtbl = &*(*central_ptr).lpVtbl;
    let slice = text_u16.as_slice_with_nul();
    let sdata = SDATA {
        data: slice.as_ptr() as *mut u8,
        size: (slice.len().saturating_mul(2)) as DWORD,
    };

    (central_vtbl.text_data)(
        central_ptr,
        0,
        flags,
        sdata,
        ptr::null_mut(),
        std::mem::zeroed(),
    )
}

// =========================
// TTS attributes
// =========================

unsafe fn apply_tts_attributes(
    central_ptr: *mut ITTSCentral,
    rate: Option<i32>,
    pitch: Option<i32>,
    volume: Option<i32>,
) {
    if rate.is_none() && pitch.is_none() && volume.is_none() {
        return;
    }

    let central_vtbl = &*(*central_ptr).lpVtbl;
    let mut attr_raw: *mut ITTSAttributes = ptr::null_mut();

    let hr = (central_vtbl.query_interface)(
        central_ptr,
        &IID_ITTSATTRIBUTESW,
        &mut attr_raw as *mut _ as *mut _,
    );
    if !hr_ok(hr) || attr_raw.is_null() {
        return;
    }

    let attr_ptr = match ComPtr::new(attr_raw) {
        Some(p) => p,
        None => return,
    };
    let attr_vtbl = &*(*attr_ptr.as_ptr()).lpVtbl;

    let mut speed_percent: Option<f64> = None;
    let mut pitch_percent: Option<f64> = None;
    let mut volume_percent: Option<f64> = None;

    if let Some(r) = rate {
        let r = r.clamp(-100, 100);
        speed_percent = Some(((r as f64 + 100.0) / 2.0).clamp(0.0, 100.0));
    }
    if let Some(p) = pitch {
        let p = p.clamp(-12, 12);
        pitch_percent = Some(((p as f64 + 12.0) / 24.0 * 100.0).clamp(0.0, 100.0));
    }
    if let Some(v) = volume {
        let v = v.clamp(0, 100);
        volume_percent = Some(v as f64);
    }

    let scale_with_default = |percent: f64, min: f64, max: f64, default: f64| -> f64 {
        let percent = percent.clamp(0.0, 100.0);
        if percent <= 50.0 {
            let span = (default - min).max(0.0);
            min + span * (percent / 50.0)
        } else {
            let span = (max - default).max(0.0);
            default + span * ((percent - 50.0) / 50.0)
        }
    };

    // Speed
    if let Some(percent) = speed_percent {
        let mut old_val: DWORD = 0;
        if (attr_vtbl.speed_get)(attr_ptr.as_ptr(), &mut old_val) == S_OK {
            if (attr_vtbl.speed_set)(attr_ptr.as_ptr(), TTSATTR_MINSPEED) != S_OK {
                eprintln!("Failed to set min speed");
            }
            let mut min_val: DWORD = 0;
            if (attr_vtbl.speed_get)(attr_ptr.as_ptr(), &mut min_val) == S_OK {
                if (attr_vtbl.speed_set)(attr_ptr.as_ptr(), TTSATTR_MAXSPEED) != S_OK {
                    eprintln!("Failed to set max speed");
                }
                let mut max_val: DWORD = 0;
                if (attr_vtbl.speed_get)(attr_ptr.as_ptr(), &mut max_val) == S_OK {
                    let max_val = max_val.saturating_sub(1);
                    if max_val > min_val {
                        let default_val = old_val.clamp(min_val, max_val);
                        let scaled = scale_with_default(
                            percent,
                            min_val as f64,
                            max_val as f64,
                            default_val as f64,
                        );
                        if (attr_vtbl.speed_set)(attr_ptr.as_ptr(), scaled as u32) != S_OK {
                            eprintln!("Failed to set speed");
                        }
                    } else if (attr_vtbl.speed_set)(attr_ptr.as_ptr(), old_val) != S_OK {
                        eprintln!("Failed to restore speed");
                    }
                }
            }
        }
    }

    // Pitch
    if let Some(percent) = pitch_percent {
        let mut old_val: WORD = 0;
        if (attr_vtbl.pitch_get)(attr_ptr.as_ptr(), &mut old_val) == S_OK {
            if (attr_vtbl.pitch_set)(attr_ptr.as_ptr(), TTSATTR_MINPITCH) != S_OK {
                eprintln!("Failed to set min pitch");
            }
            let mut min_val: WORD = 0;
            if (attr_vtbl.pitch_get)(attr_ptr.as_ptr(), &mut min_val) == S_OK {
                if (attr_vtbl.pitch_set)(attr_ptr.as_ptr(), TTSATTR_MAXPITCH) != S_OK {
                    eprintln!("Failed to set max pitch");
                }
                let mut max_val: WORD = 0;
                if (attr_vtbl.pitch_get)(attr_ptr.as_ptr(), &mut max_val) == S_OK {
                    if max_val > min_val {
                        let default_val = old_val.clamp(min_val, max_val);
                        let scaled = scale_with_default(
                            percent,
                            min_val as f64,
                            max_val as f64,
                            default_val as f64,
                        );
                        if (attr_vtbl.pitch_set)(attr_ptr.as_ptr(), scaled as u16) != S_OK {
                            eprintln!("Failed to set pitch");
                        }
                    } else if (attr_vtbl.pitch_set)(attr_ptr.as_ptr(), old_val) != S_OK {
                        eprintln!("Failed to restore pitch");
                    }
                }
            }
        }
    }

    // Volume
    if let Some(percent) = volume_percent {
        if (attr_vtbl.volume_set)(attr_ptr.as_ptr(), TTSATTR_MINVOLUME) != S_OK {
            eprintln!("Failed to set min volume");
        }
        let mut min_val: DWORD = 0;
        if (attr_vtbl.volume_get)(attr_ptr.as_ptr(), &mut min_val) == S_OK {
            let min_val = min_val & 0xFFFF;
            if (attr_vtbl.volume_set)(attr_ptr.as_ptr(), TTSATTR_MAXVOLUME) != S_OK {
                eprintln!("Failed to set max volume");
            }
            let mut max_val: DWORD = 0;
            if (attr_vtbl.volume_get)(attr_ptr.as_ptr(), &mut max_val) == S_OK {
                let max_val = max_val & 0xFFFF;
                if max_val > min_val {
                    let scaled = min_val as f64 + (max_val - min_val) as f64 * (percent / 100.0);
                    let val = scaled as u32;
                    let packed = (val & 0xFFFF) | (val << 16);
                    if (attr_vtbl.volume_set)(attr_ptr.as_ptr(), packed) != S_OK {
                        eprintln!("Failed to set volume");
                    }
                }
            }
        }
    }
}

// =========================
// Server: stdin reader
// =========================

fn spawn_command_reader() -> mpsc::Receiver<ServerCommand> {
    let (tx, rx) = mpsc::channel();

    std::thread::spawn(move || {
        let stdin = io::stdin();
        let mut reader = io::BufReader::new(stdin.lock());

        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    if tx.send(ServerCommand::Quit).is_err() {
                        eprintln!("Send Quit failed");
                    }
                    break;
                }
                Ok(_) => {
                    let cmd = line.trim_end_matches(&['\r', '\n'][..]);

                    if cmd.starts_with("SPEAK ") {
                        let len_str = cmd.trim_start_matches("SPEAK ").trim();
                        match len_str.parse::<usize>() {
                            Ok(len) => {
                                let mut buf = vec![0u8; len];
                                match reader.read_exact(&mut buf) {
                                    Ok(()) => {
                                        let text = String::from_utf8_lossy(&buf).to_string();
                                        if tx.send(ServerCommand::Speak(text)).is_err() {
                                            eprintln!("Send Speak failed");
                                        }
                                        continue;
                                    }
                                    Err(e) => {
                                        eprintln!("Read SPEAK bytes failed: {}", e);
                                        if tx.send(ServerCommand::Quit).is_err() {
                                            eprintln!("Send Quit failed");
                                        }
                                        break;
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("Bad SPEAK len: {}", e);
                                if tx.send(ServerCommand::Quit).is_err() {
                                    eprintln!("Send Quit failed");
                                }
                                break;
                            }
                        }
                    }

                    let res = match cmd {
                        "PAUSE" => tx.send(ServerCommand::Pause),
                        "RESUME" => tx.send(ServerCommand::Resume),
                        "STOP" => tx.send(ServerCommand::Stop),
                        "QUIT" => tx.send(ServerCommand::Quit),
                        _ => Ok(()),
                    };

                    if res.is_err() && !cmd.is_empty() {
                        eprintln!("Send command failed");
                    }

                    if cmd == "QUIT" {
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("stdin read_line failed: {}", e);
                    if tx.send(ServerCommand::Quit).is_err() {
                        eprintln!("Send Quit failed");
                    }
                    break;
                }
            }
        }
    });

    rx
}

// =========================
// Run server
// =========================

struct ServerSession {
    central_ptr: ComPtr<ITTSCentral>,
    central_vtbl: *const ITTSCentralVtbl,

    state: Arc<SpeakState>,

    notify_handle: SinkHandle<ITTSNotifySink>,
    reg_key: DWORD,
    registered: bool,
    pause_until: Option<Instant>,
}

impl ServerSession {
    unsafe fn new(
        target_idx: u32,
        rate: Option<i32>,
        pitch: Option<i32>,
        volume: Option<i32>,
    ) -> Result<Self, String> {
        let (_enum_ptr, central_ptr) = init_central(target_idx)?;

        let central_vtbl = (*central_ptr.as_ptr()).lpVtbl;

        let state = Arc::new(SpeakState::new());

        let sink_box = Box::new(ITTSNotifySink {
            lpVtbl: &NOTIFY_VTBL,
            refcnt: AtomicU32::new(1),
            state: state.clone(),
        });
        let sink_raw = Box::into_raw(sink_box);
        let notify_handle =
            SinkHandle::new(sink_raw).ok_or_else(|| "Failed to create notify sink".to_string())?;
        notify_handle.add_ref();

        let mut reg_key: DWORD = 0;
        let hr_reg = ((*central_vtbl).register)(
            central_ptr.as_ptr(),
            notify_handle.as_void_ptr(),
            IID_ITTSNOTIFYSINKW,
            &mut reg_key,
        );
        let registered = hr_ok(hr_reg);
        if !registered {
            eprintln!("ITTSCentral Register failed: {:#x}", hr_reg);
        }

        apply_tts_attributes(central_ptr.as_ptr(), rate, pitch, volume);

        Ok(Self {
            central_ptr,
            central_vtbl,
            state,
            notify_handle,
            reg_key,
            registered,
            pause_until: None,
        })
    }

    unsafe fn enqueue_and_maybe_start(&mut self, text: &str) {
        for segment in split_pause_tag_segments(text) {
            match segment {
                ParsedSpeakSegment::Text(segment_text) => {
                    let cleaned = sanitize_text_for_u16c(&segment_text);
                    match U16CString::from_str(&cleaned) {
                        Ok(u16c) => enqueue_item(&self.state, SpeakItem::Text(u16c)),
                        Err(_) => {
                            eprintln!("Failed to convert text to UTF-16 C-string (interior NUL?)");
                            return;
                        }
                    }
                }
                ParsedSpeakSegment::Pause(ms) => {
                    enqueue_item(&self.state, SpeakItem::Pause(ms));
                }
            }
        }
        self.try_start_next();
    }

    unsafe fn try_start_next(&mut self) {
        if let Some(until) = self.pause_until {
            if Instant::now() < until {
                return;
            }
            self.pause_until = None;
            self.state.mark_done();
        }
        if !self.state.done.load(Ordering::Acquire) {
            return;
        }

        let next = match pop_next_item(&self.state) {
            Some(t) => t,
            None => return,
        };

        self.state.mark_running();
        let next = match next {
            SpeakItem::Text(text) => text,
            SpeakItem::Pause(ms) => {
                self.pause_until = Some(Instant::now() + Duration::from_millis(u64::from(ms)));
                return;
            }
        };
        set_current_text(&self.state, next);

        let hr = speak_from_state_with_flags(self.central_ptr.as_ptr(), &self.state, 0);
        if !hr_ok(hr) {
            eprintln!("TextData failed, hr={:#x}", hr);
            self.state.mark_done();
            let mut guard = match self.state.current_text.lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            *guard = None;
        }
    }

    unsafe fn pause(&self) {
        let hr = ((*self.central_vtbl).audio_pause)(self.central_ptr.as_ptr());
        if !hr_ok(hr) {
            eprintln!("AudioPause failed: {:#x}", hr);
        }
    }

    unsafe fn resume(&self) {
        let hr = ((*self.central_vtbl).audio_resume)(self.central_ptr.as_ptr());
        if !hr_ok(hr) {
            eprintln!("AudioResume failed: {:#x}", hr);
        }
    }

    unsafe fn stop(&mut self) {
        let hr = ((*self.central_vtbl).audio_reset)(self.central_ptr.as_ptr());
        if !hr_ok(hr) {
            eprintln!("AudioReset failed: {:#x}", hr);
        }

        self.state.mark_done();
        self.pause_until = None;
        {
            let mut guard = match self.state.current_text.lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            *guard = None;
        }
        {
            let mut q = match self.state.queue.lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            q.clear();
        }
    }
}

impl Drop for ServerSession {
    fn drop(&mut self) {
        unsafe {
            if self.registered {
                let hr_un =
                    ((*self.central_vtbl).un_register)(self.central_ptr.as_ptr(), self.reg_key);
                if !hr_ok(hr_un) {
                    eprintln!("UnRegister failed: {:#x}", hr_un);
                }
            }
            // we added one extra ref in new()
            self.notify_handle.release();
            // release initial ref (Box::new started at 1)
            self.notify_handle.release();
        }
    }
}

fn run_server(
    target_idx: u32,
    rate: Option<i32>,
    pitch: Option<i32>,
    volume: Option<i32>,
) -> Result<(), String> {
    let _com = ComGuard::init_mta()?;

    let mut session = unsafe { ServerSession::new(target_idx, rate, pitch, volume)? };
    let rx = spawn_command_reader();
    let mut running = true;

    while running {
        let mut msg: MSG = unsafe { std::mem::zeroed() };
        unsafe {
            while PeekMessageW(&mut msg, ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        unsafe { session.try_start_next() };

        match rx.try_recv() {
            Ok(ServerCommand::Speak(text)) => unsafe { session.enqueue_and_maybe_start(&text) },
            Ok(ServerCommand::Pause) => unsafe { session.pause() },
            Ok(ServerCommand::Resume) => unsafe { session.resume() },
            Ok(ServerCommand::Stop) => {
                unsafe { session.stop() };
                running = false;
            }
            Ok(ServerCommand::Quit) => running = false,
            Err(mpsc::TryRecvError::Disconnected) => running = false,
            Err(mpsc::TryRecvError::Empty) => {}
        }

        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    Ok(())
}

// =========================
// speak_with_voice: single shot
// =========================

fn speak_with_voice(
    target_idx: u32,
    rate: Option<i32>,
    pitch: Option<i32>,
    volume: Option<i32>,
) -> Result<(), String> {
    let _com = ComGuard::init_mta()?;

    let (_enum_ptr, central_ptr) = unsafe { init_central(target_idx)? };
    let central_vtbl = unsafe { &*(*central_ptr.as_ptr()).lpVtbl };

    unsafe { apply_tts_attributes(central_ptr.as_ptr(), rate, pitch, volume) };

    let state = Arc::new(SpeakState::new());

    let sink_box = Box::new(ITTSNotifySink {
        lpVtbl: &NOTIFY_VTBL,
        refcnt: AtomicU32::new(1),
        state: state.clone(),
    });
    let sink_raw = Box::into_raw(sink_box);
    let notify_handle =
        SinkHandle::new(sink_raw).ok_or_else(|| "Failed to create notify sink".to_string())?;
    unsafe { notify_handle.add_ref() };

    let mut reg_key: DWORD = 0;
    let hr_reg = unsafe {
        (central_vtbl.register)(
            central_ptr.as_ptr(),
            notify_handle.as_void_ptr(),
            IID_ITTSNOTIFYSINKW,
            &mut reg_key,
        )
    };
    let registered = hr_ok(hr_reg);
    if !registered {
        eprintln!("ITTSCentral Register failed: {:#x}", hr_reg);
    }

    let mut text = String::new();
    match io::stdin().read_to_string(&mut text) {
        Ok(_) => {}
        Err(e) => eprintln!("Failed to read text from stdin: {}", e),
    }
    if text.is_empty() {
        text = "Test di sintesi vocale. Questa è la voce italiana.".to_string();
    }

    let cleaned = sanitize_text_for_u16c(&text);
    let u16c =
        U16CString::from_str(&cleaned).map_err(|_| "Input contains interior NUL".to_string())?;

    state.mark_running();
    set_current_text(&state, u16c);

    let hr = unsafe { speak_from_state_with_flags(central_ptr.as_ptr(), &state, 0) };
    if !hr_ok(hr) {
        eprintln!("TextData failed, hr={:#x}", hr);
        state.mark_done();
    }

    let mut msg: MSG = unsafe { std::mem::zeroed() };
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(60);

    while !state.done.load(Ordering::Acquire) && start.elapsed() < timeout {
        unsafe {
            while PeekMessageW(&mut msg, ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    if registered {
        let hr_un = unsafe { (central_vtbl.un_register)(central_ptr.as_ptr(), reg_key) };
        if !hr_ok(hr_un) {
            eprintln!("UnRegister failed: {:#x}", hr_un);
        }
    }

    unsafe {
        notify_handle.release();
        notify_handle.release();
    }

    Ok(())
}

// =========================
// speak_to_file: optional mp3
// =========================

fn split_text_for_recording(text: &str, max_chars: usize) -> Vec<String> {
    if text.len() <= max_chars {
        return vec![text.to_string()];
    }

    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut start = 0usize;

    while start < text.len() {
        let mut end = (start + max_chars.max(4)).min(text.len());
        while !text.is_char_boundary(end) {
            end -= 1;
        }

        // Taglio preferenziale: newline -> fine frase -> spazio
        let mut cut: Option<usize> = None;

        // newline
        let mut i = end;
        while i > start {
            if bytes[i - 1] == b'\n' {
                cut = Some(i);
                break;
            }
            i -= 1;
        }

        // punteggiatura
        if cut.is_none() {
            let mut j = end;
            while j > start {
                match bytes[j - 1] {
                    b'.' | b'!' | b'?' | b';' => {
                        cut = Some(j);
                        break;
                    }
                    _ => {}
                }
                j -= 1;
            }
        }

        // spazio
        if cut.is_none() {
            let mut k = end;
            while k > start {
                if bytes[k - 1] == b' ' {
                    cut = Some(k);
                    break;
                }
                k -= 1;
            }
        }

        if let Some(c) = cut {
            end = c;
        }

        let chunk = text[start..end].to_string();
        out.push(chunk);

        start = end;
        while start < text.len() && bytes[start] == b' ' {
            start += 1;
        }
    }

    out
}

unsafe fn try_start_next_recording_chunk(
    central_ptr: *mut ITTSCentral,
    state: &Arc<SpeakState>,
    flags: DWORD,
    buffers: &mut Vec<BufferOwner>,
) -> Result<(), String> {
    if !state.done.load(Ordering::Acquire) {
        return Ok(());
    }
    let Some(next) = pop_next_item(state) else {
        return Ok(());
    };
    let next = match next {
        SpeakItem::Text(text) => text,
        SpeakItem::Pause(ms) => {
            std::thread::sleep(Duration::from_millis(u64::from(ms)));
            return Ok(());
        }
    };
    state.mark_running();
    set_current_text(state, next.clone());
    let owner = BufferOwner::new(state.clone(), next);
    let data = owner.data();
    let notify = owner.as_void_ptr();
    buffers.push(owner);
    let central_vtbl = &*(*central_ptr).lpVtbl;
    eprintln!(
        "SAPI4 DIAG: TextData start chunk={} completion=TextDataDone",
        state.generation.load(Ordering::Acquire)
    );
    let hr = (central_vtbl.text_data)(central_ptr, 0, flags, data, notify, IID_ITTSBUFNOTIFYSINK);
    if !hr_ok(hr) {
        return Err(format!("SAPI4 TextData failed: hr={:#x}", hr));
    }
    Ok(())
}

fn speak_to_file(
    target_idx: u32,
    output_path: &str,
    rate: Option<i32>,
    pitch: Option<i32>,
    volume: Option<i32>,
    mp3_bitrate_kbps: u32,
) -> Result<(), String> {
    let _com = ComGuard::init_mta()?;

    unsafe {
        // Must outlive the engine: some legacy voices retain or over-release callback pointers.
        let mut buffers: Vec<BufferOwner> = Vec::new();
        let mut audio_file_raw: *mut IAudioFile = ptr::null_mut();
        let hr = CoCreateInstance(
            &CLSID_AUDIODESTFILE,
            ptr::null_mut(),
            CLSCTX_ALL,
            &IID_IAUDIOFILE,
            &mut audio_file_raw as *mut _ as *mut _,
        );
        if !hr_ok(hr) || audio_file_raw.is_null() {
            return Err(format!("Failed to create AudioDestFile, hr={:#x}", hr));
        }

        let audio_file_ptr =
            ComPtr::new(audio_file_raw).ok_or_else(|| "Null IAudioFile".to_string())?;
        let audio_vtbl = &*(*audio_file_ptr.as_ptr()).lpVtbl;

        let out_path = Path::new(output_path);
        let is_mp3 = out_path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.eq_ignore_ascii_case("mp3"))
            .unwrap_or(false);

        let wav_path = if is_mp3 {
            out_path.with_extension("wav")
        } else {
            out_path.to_path_buf()
        };

        let wav_str = wav_path
            .to_str()
            .ok_or_else(|| "Invalid output path".to_string())?;
        let out_wide = U16CString::from_str(wav_str)
            .map_err(|_| "Invalid output path (interior NUL?)".to_string())?;

        let hr_set = (audio_vtbl.set)(audio_file_ptr.as_ptr(), out_wide.as_ptr(), 1);
        if !hr_ok(hr_set) {
            return Err(format!("AudioFile::Set failed, hr={:#x}", hr_set));
        }

        // =========================
        // OTTIMIZZAZIONE: prova non-real-time SOLO in registrazione
        // (non cambia la velocità del parlato, cambia il modo di output/buffering)
        // =========================
        // =========================
        // OTTIMIZZAZIONE: prova non-real-time
        // =========================
        let hr_rt = (audio_vtbl.real_time_set)(audio_file_ptr.as_ptr(), 0);
        if hr_rt != S_OK {
            eprintln!("Warning: real_time_set failed: {hr_rt:x}");
        }

        let audio_unknown = audio_file_ptr.as_ptr() as *mut IUnknown;
        let (_enum_ptr, central_ptr) = init_central_with_audio(target_idx, audio_unknown)?;
        let central_vtbl = &*(*central_ptr.as_ptr()).lpVtbl;

        apply_tts_attributes(central_ptr.as_ptr(), rate, pitch, volume);

        let state = Arc::new(SpeakState::for_file());

        // --- TTS notify sink ---
        let tts_sink_box = Box::new(ITTSNotifySink {
            lpVtbl: &NOTIFY_VTBL,
            refcnt: AtomicU32::new(1),
            state: state.clone(),
        });
        let tts_sink_raw = Box::into_raw(tts_sink_box);
        let tts_handle = SinkHandle::new(tts_sink_raw)
            .ok_or_else(|| "Failed to create TTS notify sink".to_string())?;
        tts_handle.add_ref();

        let mut reg_key: DWORD = 0;
        let hr_reg_tts = (central_vtbl.register)(
            central_ptr.as_ptr(),
            tts_handle.as_void_ptr(),
            IID_ITTSNOTIFYSINKW,
            &mut reg_key,
        );
        let registered = hr_ok(hr_reg_tts);

        // --- AudioFile notify sink ---
        let af_sink_box = Box::new(IAudioFileNotifySink {
            lpVtbl: &AUDIO_FILE_NOTIFY_VTBL,
            refcnt: AtomicU32::new(1),
            state: state.clone(),
        });
        let af_sink_raw = Box::into_raw(af_sink_box);
        let af_handle = SinkHandle::new(af_sink_raw)
            .ok_or_else(|| "Failed to create audiofile notify sink".to_string())?;
        af_handle.add_ref();

        let hr = (audio_vtbl.register)(audio_file_ptr.as_ptr(), af_handle.as_void_ptr());
        if hr != S_OK {
            eprintln!("Warning: audio register failed: {hr:x}");
        }

        // =========================
        // READ TEXT
        // =========================
        let mut text = String::new();
        if let Err(e) = io::stdin().read_to_string(&mut text) {
            eprintln!("Failed to read from stdin: {}", e);
        }

        if text.is_empty() {
            if registered {
                let hr = (central_vtbl.un_register)(central_ptr.as_ptr(), reg_key);
                if hr != S_OK {
                    eprintln!("Warning: un_register failed: {hr:x}");
                }
            }
            tts_handle.release();
            af_handle.release();
            return Ok(());
        }

        for segment in split_pause_tag_segments(&text) {
            match segment {
                ParsedSpeakSegment::Text(segment_text) => {
                    let cleaned = sanitize_text_for_u16c(&segment_text);
                    let chunks = split_text_for_recording(&cleaned, 8000);
                    for ch in chunks {
                        if let Ok(u16c) = U16CString::from_str(&ch) {
                            enqueue_item(&state, SpeakItem::Text(u16c));
                        }
                    }
                }
                ParsedSpeakSegment::Pause(ms) => enqueue_item(&state, SpeakItem::Pause(ms)),
            }
        }

        state.mark_done();
        try_start_next_recording_chunk(
            central_ptr.as_ptr(),
            &state,
            TTSDATAFLAG_TAGGED,
            &mut buffers,
        )?;

        let mut msg: MSG = std::mem::zeroed();
        let start_wait = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(60 * 60);

        loop {
            while PeekMessageW(&mut msg, ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            try_start_next_recording_chunk(
                central_ptr.as_ptr(),
                &state,
                TTSDATAFLAG_TAGGED,
                &mut buffers,
            )?;

            if state.done.load(Ordering::Acquire) {
                let queue_empty = state.queue.lock().map(|q| q.is_empty()).unwrap_or(true);
                let no_current = state
                    .current_text
                    .lock()
                    .map(|c| c.is_none())
                    .unwrap_or(true);
                if queue_empty && no_current {
                    break;
                }
            }

            if start_wait.elapsed() >= timeout {
                let reset = (central_vtbl.audio_reset)(central_ptr.as_ptr());
                if !hr_ok(reset) {
                    eprintln!("SAPI4 timeout AudioReset failed: {reset:#x}");
                }
                return Err("SAPI4 synthesis timed out waiting for TextDataDone".to_string());
            }
            // Ridotto lo sleep per non frenare l'engine se è veloce
            std::thread::sleep(std::time::Duration::from_millis(1));
        }

        let hr_flush = (audio_vtbl.flush)(audio_file_ptr.as_ptr());
        if hr_flush != S_OK {
            return Err(format!("SAPI4 audio flush failed: {hr_flush:#x}"));
        }
        let output_size = std::fs::metadata(&wav_path)
            .map_err(|e| format!("SAPI4 output missing: {e}"))?
            .len();
        eprintln!(
            "SAPI4 DIAG: file synthesis complete chunks={} bytes={} completion=TextDataDone",
            buffers.len(),
            output_size
        );
        if output_size <= 44 {
            return Err("SAPI4 synthesis completed but produced no audio data".to_string());
        }

        if registered {
            let hr_un = (central_vtbl.un_register)(central_ptr.as_ptr(), reg_key);
            if !hr_ok(hr_un) {
                eprintln!("UnRegister failed: {:#x}", hr_un);
            }
        }

        // release handles
        tts_handle.release();
        af_handle.release();

        if is_mp3 {
            if let Err(err) = encode_wav_to_mp3(&wav_path, out_path, mp3_bitrate_kbps) {
                eprintln!("WAV->MP3 failed: {}", err);
            } else if let Err(e) = std::fs::remove_file(&wav_path) {
                eprintln!("Failed to remove temp wav: {}", e);
            }
        }

        Ok(())
    }
}

// =========================
// main
// =========================

fn main() {
    let code = match real_main() {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("{}", e);
            1
        }
    };
    std::process::exit(code);
}

fn real_main() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();

    if arg_flag(&args, "--list") {
        return list_voices();
    }
    if arg_flag(&args, "--sapi5-list") {
        return list_sapi5_voices();
    }

    let target_idx = arg_u32(&args, "--voice", 1);
    let sapi5_voice_name = arg_value(&args, "--voice-name").unwrap_or_default();

    let rate = arg_i32_opt(&args, "--rate");
    let pitch = arg_i32_opt(&args, "--pitch");
    let volume = arg_i32_opt(&args, "--volume");
    let mp3_bitrate_kbps = arg_u32(&args, "--bitrate", 128);

    if let Some(path) = arg_value(&args, "--sapi5-output") {
        return speak_sapi5_to_file(&sapi5_voice_name, &path, rate, pitch, volume);
    }

    if arg_flag(&args, "--sapi5-server") {
        return run_sapi5_server(&sapi5_voice_name, rate, pitch, volume);
    }

    if let Some(path) = arg_value(&args, "--output") {
        return speak_to_file(target_idx, &path, rate, pitch, volume, mp3_bitrate_kbps);
    }

    if arg_flag(&args, "--server") {
        return run_server(target_idx, rate, pitch, volume);
    }

    speak_with_voice(target_idx, rate, pitch, volume)
}
