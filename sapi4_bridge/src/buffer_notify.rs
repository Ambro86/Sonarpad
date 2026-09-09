//! Per-TextData completion, independent from audio-device idle notifications.
//! ABI: SAPI 4 ITTSBufNotifySink (also used by NVDA's _sapi4 definitions).
//! https://github.com/nvaccess/nvda/blob/release-2023.3/source/synthDrivers/_sapi4.py
use super::*;

pub(super) const IID_ITTSBUFNOTIFYSINK: GUID = GUID {
    Data1: 0xE4963D40,
    Data2: 0xC743,
    Data3: 0x11CD,
    Data4: [0x80, 0xE5, 0x00, 0xAA, 0x00, 0x3E, 0x4B, 0x50],
};

#[repr(C)]
struct BufferVtbl {
    query: unsafe extern "system" fn(*mut BufferSink, REFIID, *mut *mut std::ffi::c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut BufferSink) -> u32,
    release: unsafe extern "system" fn(*mut BufferSink) -> u32,
    text_done: unsafe extern "system" fn(*mut BufferSink, QWORD, DWORD) -> i32,
    text_started: unsafe extern "system" fn(*mut BufferSink, QWORD) -> i32,
    bookmark: unsafe extern "system" fn(*mut BufferSink, QWORD, DWORD) -> i32,
    word_position: unsafe extern "system" fn(*mut BufferSink, QWORD, DWORD) -> i32,
}

#[repr(C)]
struct BufferSink {
    vtbl: *const BufferVtbl,
    references: AtomicU32,
    owner_alive: AtomicBool,
    state: Arc<SpeakState>,
    generation: u32,
    text: U16CString,
}

pub(super) struct BufferOwner(*mut BufferSink);

impl BufferOwner {
    pub fn new(state: Arc<SpeakState>, text: U16CString) -> Self {
        let generation = state.generation.load(Ordering::Acquire);
        Self(Box::into_raw(Box::new(BufferSink {
            vtbl: &BUFFER_VTBL,
            references: AtomicU32::new(1),
            owner_alive: AtomicBool::new(true),
            state,
            generation,
            text,
        })))
    }

    pub fn as_void_ptr(&self) -> *mut std::ffi::c_void {
        self.0.cast()
    }

    pub fn data(&self) -> SDATA {
        // The owner remains alive until the speech engine has been released.
        let text = unsafe { &(*self.0).text };
        SDATA {
            data: text.as_ptr() as *mut u8,
            size: (text.as_slice_with_nul().len() * 2) as DWORD,
        }
    }
}

impl Drop for BufferOwner {
    fn drop(&mut self) {
        unsafe {
            (*self.0).owner_alive.store(false, Ordering::Release);
            buffer_release(self.0);
        }
    }
}

unsafe extern "system" fn buffer_query(
    this: *mut BufferSink,
    iid: REFIID,
    out: *mut *mut std::ffi::c_void,
) -> i32 {
    if out.is_null() {
        return E_NOINTERFACE;
    }
    *out = ptr::null_mut();
    if this.is_null() || iid.is_null() {
        return E_NOINTERFACE;
    }
    if guid_eq(iid, &IID_IUNKNOWN) || guid_eq(iid, &IID_ITTSBUFNOTIFYSINK) {
        *out = this.cast();
        buffer_add_ref(this);
        S_OK
    } else {
        E_NOINTERFACE
    }
}

unsafe extern "system" fn buffer_add_ref(this: *mut BufferSink) -> u32 {
    if this.is_null() {
        return 0;
    }
    (*this).references.fetch_add(1, Ordering::SeqCst) + 1
}

unsafe extern "system" fn buffer_release(this: *mut BufferSink) -> u32 {
    if this.is_null() {
        return 0;
    }
    let sink = &*this;
    // Some legacy voices over-release callback interfaces. Retain our owner reference
    // until central/engine teardown, as NVDA also does for legacy SAPI4 engines.
    let result = sink
        .references
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
            let minimum = u32::from(sink.owner_alive.load(Ordering::Acquire));
            (current > minimum).then(|| current - 1)
        });
    match result {
        Ok(1) => {
            let _released = Box::from_raw(this);
            0
        }
        Ok(previous) => previous - 1,
        Err(current) => current,
    }
}

unsafe extern "system" fn buffer_done(this: *mut BufferSink, _time: QWORD, flags: DWORD) -> i32 {
    if !this.is_null() {
        let sink = &*this;
        if sink.state.finish_buffer(sink.generation) {
            eprintln!(
                "SAPI4 DIAG: TextDataDone chunk={} flags=0x{:X}",
                sink.generation, flags
            );
        }
    }
    S_OK
}
unsafe extern "system" fn buffer_started(_this: *mut BufferSink, _time: QWORD) -> i32 {
    S_OK
}
unsafe extern "system" fn buffer_position(
    _this: *mut BufferSink,
    _time: QWORD,
    _position: DWORD,
) -> i32 {
    S_OK
}

static BUFFER_VTBL: BufferVtbl = BufferVtbl {
    query: buffer_query,
    add_ref: buffer_add_ref,
    release: buffer_release,
    text_done: buffer_done,
    text_started: buffer_started,
    bookmark: buffer_position,
    word_position: buffer_position,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_notifications_cannot_complete_file_synthesis() {
        let state = Arc::new(SpeakState::for_file());
        state.mark_running();
        let mut tts = ITTSNotifySink {
            lpVtbl: &NOTIFY_VTBL,
            refcnt: AtomicU32::new(1),
            state: state.clone(),
        };
        let mut file = IAudioFileNotifySink {
            lpVtbl: &AUDIO_FILE_NOTIFY_VTBL,
            refcnt: AtomicU32::new(1),
            state: state.clone(),
        };
        unsafe {
            notify_audio_stop(&mut tts, 0);
            audio_file_notify_file_end(&mut file, 1);
            audio_file_notify_queue_empty(&mut file);
        }
        assert!(!state.done.load(Ordering::Acquire));
        assert!(state.finish_buffer(state.generation.load(Ordering::Acquire)));
        assert!(state.done.load(Ordering::Acquire));
    }

    #[test]
    fn old_chunk_callback_cannot_complete_the_next_chunk() {
        let state = SpeakState::for_file();
        state.mark_running();
        let old = state.generation.load(Ordering::Acquire);
        assert!(state.finish_buffer(old));
        state.mark_running();
        assert!(!state.finish_buffer(old));
        assert!(!state.done.load(Ordering::Acquire));
    }

    #[test]
    fn callback_owns_text_and_survives_engine_over_release() -> Result<(), String> {
        let state = Arc::new(SpeakState::for_file());
        state.mark_running();
        let text = U16CString::from_str("Test hlas").map_err(|e| e.to_string())?;
        set_current_text(&state, text.clone());
        let owner = BufferOwner::new(state.clone(), text);
        unsafe {
            assert_eq!(buffer_release(owner.0), 1);
            assert_eq!(buffer_add_ref(owner.0), 2);
            assert_eq!(buffer_release(owner.0), 1);
            assert_eq!(buffer_done(owner.0, 0, 0), S_OK);
            assert!(!owner.data().data.is_null());
        }
        assert!(state
            .current_text
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_none());
        assert!(state.done.load(Ordering::Acquire));
        Ok(())
    }

    #[test]
    fn direct_playback_still_completes_on_audio_stop() {
        let state = Arc::new(SpeakState::new());
        state.mark_running();
        let mut sink = ITTSNotifySink {
            lpVtbl: &NOTIFY_VTBL,
            refcnt: AtomicU32::new(1),
            state: state.clone(),
        };
        unsafe {
            notify_audio_stop(&mut sink, 0);
        }
        assert!(state.done.load(Ordering::Acquire));
    }

    #[test]
    fn recording_chunks_preserve_multibyte_text_at_the_size_limit() {
        let input = "ž".repeat(5000);
        let chunks = split_text_for_recording(&input, 7999);
        assert!(chunks.iter().all(|chunk| chunk.len() <= 7999));
        assert_eq!(chunks.concat(), input);
    }
}
