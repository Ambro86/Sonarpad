use crate::ffmpeg_dyn::*;
use crate::log_debug;
use crate::settings::settings_dir;
use libloading::os::windows::{
    LOAD_LIBRARY_SEARCH_DEFAULT_DIRS, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, Library,
};
use rodio::Source;
use std::ffi::{CStr, CString};
use std::path::Path;
use std::ptr;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

const AV_NOPTS_VALUE_I64: i64 = i64::MIN;
const AV_DICT_IGNORE_SUFFIX: i32 = 2;

/// Metadata about an audio stream in a media file.
#[derive(Debug, Clone)]
pub struct AudioStreamInfo {
    /// Stream index (used to select this stream).
    pub index: i32,
    /// Language code (e.g., "eng", "ita") if available.
    pub language: Option<String>,
    /// Stream title/name if available.
    pub title: Option<String>,
    /// Codec name (e.g., "aac", "ac3", "opus").
    pub codec: String,
    /// Number of channels.
    pub channels: i32,
    /// Sample rate in Hz.
    pub sample_rate: i32,
    /// Whether this is the default stream.
    pub is_default: bool,
}

type AvformatNetworkInit = unsafe extern "C" fn() -> libc::c_int;
type AvformatOpenInput = unsafe extern "C" fn(
    *mut *mut AVFormatContext,
    *const libc::c_char,
    *mut AVInputFormat,
    *mut *mut AVDictionary,
) -> libc::c_int;
type AvformatFindStreamInfo =
    unsafe extern "C" fn(*mut AVFormatContext, *mut *mut AVDictionary) -> libc::c_int;
type AvformatCloseInput = unsafe extern "C" fn(*mut *mut AVFormatContext);
type AvformatAllocOutputContext2 = unsafe extern "C" fn(
    *mut *mut AVFormatContext,
    *mut AVOutputFormat,
    *const libc::c_char,
    *const libc::c_char,
) -> libc::c_int;
type AvformatNewStream =
    unsafe extern "C" fn(*mut AVFormatContext, *const AVCodec) -> *mut AVStream;
type AvformatWriteHeader =
    unsafe extern "C" fn(*mut AVFormatContext, *mut *mut AVDictionary) -> libc::c_int;
type AvWriteTrailer = unsafe extern "C" fn(*mut AVFormatContext) -> libc::c_int;
type AvformatFreeContext = unsafe extern "C" fn(*mut AVFormatContext);
type AvInterleavedWriteFrame =
    unsafe extern "C" fn(*mut AVFormatContext, *mut AVPacket) -> libc::c_int;
type AvPacketRescaleTs = unsafe extern "C" fn(*mut AVPacket, AVRational, AVRational);
type AvformatSeekFile = unsafe extern "C" fn(
    *mut AVFormatContext,
    libc::c_int,
    i64,
    i64,
    i64,
    libc::c_int,
) -> libc::c_int;
type AvReadFrame = unsafe extern "C" fn(*mut AVFormatContext, *mut AVPacket) -> libc::c_int;
type AvFindBestStream = unsafe extern "C" fn(
    *mut AVFormatContext,
    AVMediaType,
    libc::c_int,
    libc::c_int,
    *mut *const AVCodec,
    libc::c_int,
) -> libc::c_int;
type AvcodecFindDecoder = unsafe extern "C" fn(codec_id: AVCodecID) -> *const AVCodec;
type AvcodecFindEncoder = unsafe extern "C" fn(codec_id: AVCodecID) -> *const AVCodec;
type AvcodecFindEncoderByName = unsafe extern "C" fn(name: *const libc::c_char) -> *const AVCodec;
type AvcodecAllocContext3 = unsafe extern "C" fn(*const AVCodec) -> *mut AVCodecContext;
type AvcodecParametersToContext =
    unsafe extern "C" fn(*mut AVCodecContext, *const AVCodecParameters) -> libc::c_int;
type AvcodecParametersFromContext =
    unsafe extern "C" fn(*mut AVCodecParameters, *const AVCodecContext) -> libc::c_int;
type AvcodecParametersCopy =
    unsafe extern "C" fn(*mut AVCodecParameters, *const AVCodecParameters) -> libc::c_int;
type AvcodecOpen2 = unsafe extern "C" fn(
    *mut AVCodecContext,
    *const AVCodec,
    *mut *mut AVDictionary,
) -> libc::c_int;
type AvcodecSendPacket = unsafe extern "C" fn(*mut AVCodecContext, *const AVPacket) -> libc::c_int;
type AvcodecReceiveFrame = unsafe extern "C" fn(*mut AVCodecContext, *mut AVFrame) -> libc::c_int;
type AvcodecFlushBuffers = unsafe extern "C" fn(*mut AVCodecContext);
type AvcodecSendFrame = unsafe extern "C" fn(*mut AVCodecContext, *const AVFrame) -> libc::c_int;
type AvcodecReceivePacket = unsafe extern "C" fn(*mut AVCodecContext, *mut AVPacket) -> libc::c_int;
type AvcodecFreeContext = unsafe extern "C" fn(*mut *mut AVCodecContext);
type AvBsfGetByName = unsafe extern "C" fn(*const libc::c_char) -> *const AVBitStreamFilter;
type AvBsfAlloc =
    unsafe extern "C" fn(*const AVBitStreamFilter, *mut *mut AVBSFContext) -> libc::c_int;
type AvBsfInit = unsafe extern "C" fn(*mut AVBSFContext) -> libc::c_int;
type AvBsfSendPacket = unsafe extern "C" fn(*mut AVBSFContext, *mut AVPacket) -> libc::c_int;
type AvBsfReceivePacket = unsafe extern "C" fn(*mut AVBSFContext, *mut AVPacket) -> libc::c_int;
type AvBsfFree = unsafe extern "C" fn(*mut *mut AVBSFContext);
type AvPacketAlloc = unsafe extern "C" fn() -> *mut AVPacket;
type AvPacketFree = unsafe extern "C" fn(*mut *mut AVPacket);
type AvPacketUnref = unsafe extern "C" fn(*mut AVPacket);
type AvFrameAlloc = unsafe extern "C" fn() -> *mut AVFrame;
type AvFrameFree = unsafe extern "C" fn(*mut *mut AVFrame);
type AvFrameUnref = unsafe extern "C" fn(*mut AVFrame);
type AvFrameGetBuffer = unsafe extern "C" fn(*mut AVFrame, libc::c_int) -> libc::c_int;
type AvFrameMakeWritable = unsafe extern "C" fn(*mut AVFrame) -> libc::c_int;
type AvStrerror = unsafe extern "C" fn(libc::c_int, *mut libc::c_char, libc::size_t) -> libc::c_int;
type AvutilVersion = unsafe extern "C" fn() -> libc::c_uint;
type SwrAllocSetOpts2 = unsafe extern "C" fn(
    *mut *mut SwrContext,
    *const AVChannelLayout,
    AVSampleFormat,
    libc::c_int,
    *const AVChannelLayout,
    AVSampleFormat,
    libc::c_int,
    libc::c_int,
    *mut libc::c_void,
) -> libc::c_int;
type SwrInit = unsafe extern "C" fn(*mut SwrContext) -> libc::c_int;
type SwrClose = unsafe extern "C" fn(*mut SwrContext);
type SwrFree = unsafe extern "C" fn(*mut *mut SwrContext);
type SwrConvert = unsafe extern "C" fn(
    *mut SwrContext,
    *mut *mut u8,
    libc::c_int,
    *const *const u8,
    libc::c_int,
) -> libc::c_int;
type SwrGetOutSamples = unsafe extern "C" fn(*mut SwrContext, libc::c_int) -> libc::c_int;
type AvChannelLayoutCheck = unsafe extern "C" fn(*const AVChannelLayout) -> libc::c_int;
type AvChannelLayoutCopy =
    unsafe extern "C" fn(*mut AVChannelLayout, *const AVChannelLayout) -> libc::c_int;
type AvChannelLayoutDefault = unsafe extern "C" fn(*mut AVChannelLayout, libc::c_int);
type AvChannelLayoutUninit = unsafe extern "C" fn(*mut AVChannelLayout);
type AvioOpen =
    unsafe extern "C" fn(*mut *mut AVIOContext, *const libc::c_char, libc::c_int) -> libc::c_int;
type AvioClosep = unsafe extern "C" fn(*mut *mut AVIOContext) -> libc::c_int;
type AvDictGet = unsafe extern "C" fn(
    *const AVDictionary,
    *const libc::c_char,
    *const AVDictionaryEntry,
    libc::c_int,
) -> *mut AVDictionaryEntry;
type AvDictSet = unsafe extern "C" fn(
    *mut *mut AVDictionary,
    *const libc::c_char,
    *const libc::c_char,
    libc::c_int,
) -> libc::c_int;
type AvDictFree = unsafe extern "C" fn(*mut *mut AVDictionary);
type AvMallocz = unsafe extern "C" fn(libc::size_t) -> *mut libc::c_void;

pub(crate) struct FfmpegApi {
    _libs: Vec<Library>,
    pub(crate) avformat_network_init: AvformatNetworkInit,
    pub(crate) avformat_open_input: AvformatOpenInput,
    pub(crate) avformat_find_stream_info: AvformatFindStreamInfo,
    pub(crate) avformat_close_input: AvformatCloseInput,
    pub(crate) avformat_alloc_output_context2: AvformatAllocOutputContext2,
    pub(crate) avformat_new_stream: AvformatNewStream,
    pub(crate) avformat_write_header: AvformatWriteHeader,
    pub(crate) av_write_trailer: AvWriteTrailer,
    pub(crate) avformat_free_context: AvformatFreeContext,
    pub(crate) av_interleaved_write_frame: AvInterleavedWriteFrame,
    pub(crate) av_packet_rescale_ts: AvPacketRescaleTs,
    pub(crate) avformat_seek_file: AvformatSeekFile,
    pub(crate) av_read_frame: AvReadFrame,
    pub(crate) av_find_best_stream: AvFindBestStream,
    pub(crate) avcodec_find_decoder: AvcodecFindDecoder,
    pub(crate) avcodec_find_encoder: AvcodecFindEncoder,
    pub(crate) avcodec_find_encoder_by_name: AvcodecFindEncoderByName,
    pub(crate) avcodec_alloc_context3: AvcodecAllocContext3,
    pub(crate) avcodec_parameters_to_context: AvcodecParametersToContext,
    pub(crate) avcodec_parameters_from_context: AvcodecParametersFromContext,
    pub(crate) avcodec_parameters_copy: AvcodecParametersCopy,
    pub(crate) avcodec_open2: AvcodecOpen2,
    pub(crate) avcodec_send_packet: AvcodecSendPacket,
    pub(crate) avcodec_receive_frame: AvcodecReceiveFrame,
    pub(crate) avcodec_flush_buffers: AvcodecFlushBuffers,
    pub(crate) avcodec_send_frame: AvcodecSendFrame,
    pub(crate) avcodec_receive_packet: AvcodecReceivePacket,
    pub(crate) avcodec_free_context: AvcodecFreeContext,
    pub(crate) av_bsf_get_by_name: AvBsfGetByName,
    pub(crate) av_bsf_alloc: AvBsfAlloc,
    pub(crate) av_bsf_init: AvBsfInit,
    pub(crate) av_bsf_send_packet: AvBsfSendPacket,
    pub(crate) av_bsf_receive_packet: AvBsfReceivePacket,
    pub(crate) av_bsf_free: AvBsfFree,
    pub(crate) av_packet_alloc: AvPacketAlloc,
    pub(crate) av_packet_free: AvPacketFree,
    pub(crate) av_packet_unref: AvPacketUnref,
    pub(crate) av_frame_alloc: AvFrameAlloc,
    pub(crate) av_frame_free: AvFrameFree,
    pub(crate) av_frame_unref: AvFrameUnref,
    pub(crate) av_frame_get_buffer: AvFrameGetBuffer,
    pub(crate) av_frame_make_writable: AvFrameMakeWritable,
    pub(crate) av_strerror: AvStrerror,
    pub(crate) avutil_version: AvutilVersion,
    pub(crate) swr_alloc_set_opts2: SwrAllocSetOpts2,
    pub(crate) swr_init: SwrInit,
    pub(crate) swr_close: SwrClose,
    pub(crate) swr_free: SwrFree,
    pub(crate) swr_convert: SwrConvert,
    pub(crate) swr_get_out_samples: SwrGetOutSamples,
    pub(crate) av_channel_layout_check: AvChannelLayoutCheck,
    pub(crate) av_channel_layout_copy: AvChannelLayoutCopy,
    pub(crate) av_channel_layout_default: AvChannelLayoutDefault,
    pub(crate) av_channel_layout_uninit: AvChannelLayoutUninit,
    pub(crate) avio_open: AvioOpen,
    pub(crate) avio_closep: AvioClosep,
    pub(crate) av_dict_get: AvDictGet,
    pub(crate) av_dict_set: AvDictSet,
    pub(crate) av_dict_free: AvDictFree,
    pub(crate) av_mallocz: AvMallocz,
}

fn load_symbol<T: Copy>(lib: &Library, name: &[u8]) -> Result<T, String> {
    unsafe { lib.get::<T>(name).map(|s| *s) }.map_err(|e| {
        format!(
            "FFmpeg: missing symbol {}: {}",
            String::from_utf8_lossy(name),
            e
        )
    })
}

fn load_ffmpeg_api() -> Result<FfmpegApi, String> {
    let deps_dir = settings_dir();
    let vcpkg_local = Path::new(r"C:\rustnotepad\rustnotepad\vcpkg_installed\x64-windows\bin");
    let vcpkg_ci = Path::new(r"C:\vcpkg\installed\x64-windows\bin");

    let ffmpeg_root = if vcpkg_local.exists() {
        Some(vcpkg_local)
    } else if vcpkg_ci.exists() {
        Some(vcpkg_ci)
    } else {
        None
    };

    let ffmpeg_dlls = [
        "avutil-60.dll",
        "swresample-6.dll",
        "swscale-9.dll",
        "opus.dll",
        "libmp3lame.dll",
        "avcodec-62.dll",
        "avformat-62.dll",
    ];

    let mut libs = Vec::new();
    let mut avutil_lib: Option<Library> = None;
    let mut swresample_lib: Option<Library> = None;
    let mut avcodec_lib: Option<Library> = None;
    let mut avformat_lib: Option<Library> = None;
    let flags = LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS;

    for dll in ffmpeg_dlls {
        let mut path = deps_dir.join(dll);
        if let Some(root) = ffmpeg_root {
            let p = root.join(dll);
            if p.exists() {
                path = p;
            }
        }

        if !path.exists() {
            return Err(format!("FFmpeg: missing DLL {}", path.display()));
        }

        let lib = unsafe { Library::load_with_flags(&path, flags) }
            .map_err(|e| format!("FFmpeg: failed to load {}: {}", path.display(), e))?;

        match dll {
            "avutil-60.dll" => avutil_lib = Some(lib),
            "swresample-6.dll" => swresample_lib = Some(lib),
            "avcodec-62.dll" => avcodec_lib = Some(lib),
            "avformat-62.dll" => avformat_lib = Some(lib),
            _ => libs.push(lib),
        }
    }

    let avutil_lib = avutil_lib.ok_or_else(|| "FFmpeg: avutil-60.dll not loaded".to_string())?;
    let swresample_lib =
        swresample_lib.ok_or_else(|| "FFmpeg: swresample-6.dll not loaded".to_string())?;
    let avcodec_lib = avcodec_lib.ok_or_else(|| "FFmpeg: avcodec-62.dll not loaded".to_string())?;
    let avformat_lib =
        avformat_lib.ok_or_else(|| "FFmpeg: avformat-62.dll not loaded".to_string())?;

    libs.push(avutil_lib);
    libs.push(swresample_lib);
    libs.push(avcodec_lib);
    libs.push(avformat_lib);

    // Symbols are resolved from the specific libraries that export them.
    let avutil = &libs[libs.len() - 4];
    let swresample = &libs[libs.len() - 3];
    let avcodec = &libs[libs.len() - 2];
    let avformat = &libs[libs.len() - 1];

    let avformat_network_init = load_symbol(avformat, b"avformat_network_init\0")?;
    let avformat_open_input = load_symbol(avformat, b"avformat_open_input\0")?;
    let avformat_find_stream_info = load_symbol(avformat, b"avformat_find_stream_info\0")?;
    let avformat_close_input = load_symbol(avformat, b"avformat_close_input\0")?;
    let avformat_alloc_output_context2 =
        load_symbol(avformat, b"avformat_alloc_output_context2\0")?;
    let avformat_new_stream = load_symbol(avformat, b"avformat_new_stream\0")?;
    let avformat_write_header = load_symbol(avformat, b"avformat_write_header\0")?;
    let av_write_trailer = load_symbol(avformat, b"av_write_trailer\0")?;
    let avformat_free_context = load_symbol(avformat, b"avformat_free_context\0")?;
    let av_interleaved_write_frame = load_symbol(avformat, b"av_interleaved_write_frame\0")?;
    let avformat_seek_file = load_symbol(avformat, b"avformat_seek_file\0")?;
    let av_read_frame = load_symbol(avformat, b"av_read_frame\0")?;
    let av_find_best_stream = load_symbol(avformat, b"av_find_best_stream\0")?;
    let avcodec_find_decoder = load_symbol(avcodec, b"avcodec_find_decoder\0")?;
    let avcodec_find_encoder = load_symbol(avcodec, b"avcodec_find_encoder\0")?;
    let avcodec_find_encoder_by_name = load_symbol(avcodec, b"avcodec_find_encoder_by_name\0")?;
    let avcodec_alloc_context3 = load_symbol(avcodec, b"avcodec_alloc_context3\0")?;
    let avcodec_parameters_to_context = load_symbol(avcodec, b"avcodec_parameters_to_context\0")?;
    let avcodec_parameters_from_context =
        load_symbol(avcodec, b"avcodec_parameters_from_context\0")?;
    let avcodec_parameters_copy = load_symbol(avcodec, b"avcodec_parameters_copy\0")?;
    let avcodec_open2 = load_symbol(avcodec, b"avcodec_open2\0")?;
    let avcodec_send_packet = load_symbol(avcodec, b"avcodec_send_packet\0")?;
    let avcodec_receive_frame = load_symbol(avcodec, b"avcodec_receive_frame\0")?;
    let avcodec_flush_buffers = load_symbol(avcodec, b"avcodec_flush_buffers\0")?;
    let avcodec_send_frame = load_symbol(avcodec, b"avcodec_send_frame\0")?;
    let avcodec_receive_packet = load_symbol(avcodec, b"avcodec_receive_packet\0")?;
    let avcodec_free_context = load_symbol(avcodec, b"avcodec_free_context\0")?;
    let av_bsf_get_by_name = load_symbol(avcodec, b"av_bsf_get_by_name\0")?;
    let av_bsf_alloc = load_symbol(avcodec, b"av_bsf_alloc\0")?;
    let av_bsf_init = load_symbol(avcodec, b"av_bsf_init\0")?;
    let av_bsf_send_packet = load_symbol(avcodec, b"av_bsf_send_packet\0")?;
    let av_bsf_receive_packet = load_symbol(avcodec, b"av_bsf_receive_packet\0")?;
    let av_bsf_free = load_symbol(avcodec, b"av_bsf_free\0")?;
    let av_packet_alloc = load_symbol(avcodec, b"av_packet_alloc\0")?;
    let av_packet_free = load_symbol(avcodec, b"av_packet_free\0")?;
    let av_packet_unref = load_symbol(avcodec, b"av_packet_unref\0")?;
    let av_frame_alloc = load_symbol(avutil, b"av_frame_alloc\0")?;
    let av_frame_free = load_symbol(avutil, b"av_frame_free\0")?;
    let av_frame_unref = load_symbol(avutil, b"av_frame_unref\0")?;
    let av_frame_get_buffer = load_symbol(avutil, b"av_frame_get_buffer\0")?;
    let av_frame_make_writable = load_symbol(avutil, b"av_frame_make_writable\0")?;
    let av_packet_rescale_ts = load_symbol(avcodec, b"av_packet_rescale_ts\0")?;
    let avio_open = load_symbol(avformat, b"avio_open\0")?;
    let avio_closep = load_symbol(avformat, b"avio_closep\0")?;
    let av_strerror = load_symbol(avutil, b"av_strerror\0")?;
    let avutil_version = load_symbol(avutil, b"avutil_version\0")?;
    let swr_alloc_set_opts2 = load_symbol(swresample, b"swr_alloc_set_opts2\0")?;
    let swr_init = load_symbol(swresample, b"swr_init\0")?;
    let swr_close = load_symbol(swresample, b"swr_close\0")?;
    let swr_free = load_symbol(swresample, b"swr_free\0")?;
    let swr_convert = load_symbol(swresample, b"swr_convert\0")?;
    let swr_get_out_samples = load_symbol(swresample, b"swr_get_out_samples\0")?;
    let av_channel_layout_check = load_symbol(avutil, b"av_channel_layout_check\0")?;
    let av_channel_layout_copy = load_symbol(avutil, b"av_channel_layout_copy\0")?;
    let av_channel_layout_default = load_symbol(avutil, b"av_channel_layout_default\0")?;
    let av_channel_layout_uninit = load_symbol(avutil, b"av_channel_layout_uninit\0")?;
    let av_dict_get = load_symbol(avutil, b"av_dict_get\0")?;
    let av_dict_set = load_symbol(avutil, b"av_dict_set\0")?;
    let av_dict_free = load_symbol(avutil, b"av_dict_free\0")?;
    let av_mallocz = load_symbol(avutil, b"av_mallocz\0")?;

    Ok(FfmpegApi {
        _libs: libs,
        avformat_network_init,
        avformat_open_input,
        avformat_find_stream_info,
        avformat_close_input,
        avformat_alloc_output_context2,
        avformat_new_stream,
        avformat_write_header,
        av_write_trailer,
        avformat_free_context,
        av_interleaved_write_frame,
        av_packet_rescale_ts,
        avformat_seek_file,
        av_read_frame,
        av_find_best_stream,
        avcodec_find_decoder,
        avcodec_find_encoder,
        avcodec_find_encoder_by_name,
        avcodec_alloc_context3,
        avcodec_parameters_to_context,
        avcodec_parameters_from_context,
        avcodec_parameters_copy,
        avcodec_open2,
        avcodec_send_packet,
        avcodec_receive_frame,
        avcodec_flush_buffers,
        avcodec_send_frame,
        avcodec_receive_packet,
        avcodec_free_context,
        av_bsf_get_by_name,
        av_bsf_alloc,
        av_bsf_init,
        av_bsf_send_packet,
        av_bsf_receive_packet,
        av_bsf_free,
        av_packet_alloc,
        av_packet_free,
        av_packet_unref,
        av_frame_alloc,
        av_frame_free,
        av_frame_unref,
        av_frame_get_buffer,
        av_frame_make_writable,
        av_strerror,
        avutil_version,
        swr_alloc_set_opts2,
        swr_init,
        swr_close,
        swr_free,
        swr_convert,
        swr_get_out_samples,
        av_channel_layout_check,
        av_channel_layout_copy,
        av_channel_layout_default,
        av_channel_layout_uninit,
        avio_open,
        avio_closep,
        av_dict_get,
        av_dict_set,
        av_dict_free,
        av_mallocz,
    })
}

static FFMPEG_API: OnceLock<Result<FfmpegApi, String>> = OnceLock::new();

pub(crate) fn ffmpeg_api() -> Result<&'static FfmpegApi, String> {
    let res = FFMPEG_API.get_or_init(load_ffmpeg_api);
    res.as_ref().map_err(|e| e.clone())
}

pub(crate) fn avformat_close_input_safe(api: &FfmpegApi, fmt_ctx: *mut *mut AVFormatContext) {
    unsafe { (api.avformat_close_input)(fmt_ctx) }
}

pub(crate) fn avformat_open_input_safe(
    api: &FfmpegApi,
    ps: *mut *mut AVFormatContext,
    url: *const i8,
    fmt: *mut AVInputFormat,
    options: *mut *mut AVDictionary,
) -> i32 {
    unsafe { (api.avformat_open_input)(ps, url, fmt, options) }
}

pub(crate) fn avformat_find_stream_info_safe(
    api: &FfmpegApi,
    ic: *mut AVFormatContext,
    options: *mut *mut AVDictionary,
) -> i32 {
    unsafe { (api.avformat_find_stream_info)(ic, options) }
}

pub(crate) fn av_find_best_stream_safe(
    api: &FfmpegApi,
    ic: *mut AVFormatContext,
    typ: AVMediaType,
    wanted_stream_nb: i32,
    related_stream: i32,
    decoder_ret: *mut *const AVCodec,
    flags: i32,
) -> i32 {
    unsafe {
        (api.av_find_best_stream)(
            ic,
            typ,
            wanted_stream_nb,
            related_stream,
            decoder_ret,
            flags,
        )
    }
}

pub(crate) fn av_packet_alloc_safe(api: &FfmpegApi) -> *mut AVPacket {
    unsafe { (api.av_packet_alloc)() }
}

pub(crate) fn avformat_free_context_safe(api: &FfmpegApi, s: *mut AVFormatContext) {
    unsafe { (api.avformat_free_context)(s) }
}

pub(crate) fn avformat_new_stream_safe(
    api: &FfmpegApi,
    s: *mut AVFormatContext,
    c: *const AVCodec,
) -> *mut AVStream {
    unsafe { (api.avformat_new_stream)(s, c) }
}

pub(crate) fn avformat_alloc_output_context2_safe(
    api: &FfmpegApi,
    ctx: *mut *mut AVFormatContext,
    oformat: *mut AVOutputFormat,
    format_name: *const i8,
    filename: *const i8,
) -> i32 {
    unsafe { (api.avformat_alloc_output_context2)(ctx, oformat, format_name, filename) }
}

pub(crate) fn avformat_write_header_safe(
    api: &FfmpegApi,
    s: *mut AVFormatContext,
    options: *mut *mut AVDictionary,
) -> i32 {
    unsafe { (api.avformat_write_header)(s, options) }
}

pub(crate) fn av_write_trailer_safe(api: &FfmpegApi, s: *mut AVFormatContext) -> i32 {
    unsafe { (api.av_write_trailer)(s) }
}

pub(crate) fn avio_open_safe(
    api: &FfmpegApi,
    s: *mut *mut AVIOContext,
    url: *const i8,
    flags: i32,
) -> i32 {
    unsafe { (api.avio_open)(s, url, flags) }
}

pub(crate) fn av_read_frame_safe(
    api: &FfmpegApi,
    s: *mut AVFormatContext,
    pkt: *mut AVPacket,
) -> i32 {
    unsafe { (api.av_read_frame)(s, pkt) }
}

pub(crate) fn av_interleaved_write_frame_safe(
    api: &FfmpegApi,
    s: *mut AVFormatContext,
    pkt: *mut AVPacket,
) -> i32 {
    unsafe { (api.av_interleaved_write_frame)(s, pkt) }
}

pub(crate) fn avcodec_alloc_context3_safe(
    api: &FfmpegApi,
    codec: *const AVCodec,
) -> *mut AVCodecContext {
    unsafe { (api.avcodec_alloc_context3)(codec) }
}

pub(crate) fn avcodec_open2_safe(
    api: &FfmpegApi,
    avctx: *mut AVCodecContext,
    codec: *const AVCodec,
    options: *mut *mut AVDictionary,
) -> i32 {
    unsafe { (api.avcodec_open2)(avctx, codec, options) }
}

pub(crate) fn av_frame_alloc_safe(api: &FfmpegApi) -> *mut AVFrame {
    unsafe { (api.av_frame_alloc)() }
}

pub(crate) fn av_frame_nb_samples_safe(frame: *const AVFrame) -> i32 {
    unsafe { (*frame).nb_samples }
}

pub(crate) fn av_frame_data_mut_ptr_safe(frame: *mut AVFrame) -> *mut *mut u8 {
    unsafe { (*frame).data.as_mut_ptr() }
}

pub(crate) fn av_codec_context_frame_size_safe(codec_ctx: *const AVCodecContext) -> i32 {
    unsafe { (*codec_ctx).frame_size }
}

pub(crate) fn av_format_context_nb_streams_safe(fmt_ctx: *const AVFormatContext) -> u32 {
    unsafe { (*fmt_ctx).nb_streams }
}

pub(crate) fn av_format_context_streams_safe(
    fmt_ctx: *const AVFormatContext,
) -> *mut *mut AVStream {
    unsafe { (*fmt_ctx).streams }
}

pub(crate) fn av_stream_codecpar_safe(stream: *const AVStream) -> *mut AVCodecParameters {
    unsafe { (*stream).codecpar }
}

pub(crate) fn av_codecpar_codec_type_safe(codecpar: *const AVCodecParameters) -> AVMediaType {
    unsafe { (*codecpar).codec_type }
}

pub(crate) fn av_codecpar_codec_id_safe(codecpar: *const AVCodecParameters) -> AVCodecID {
    unsafe { (*codecpar).codec_id }
}

pub(crate) fn av_codecpar_channels_safe(codecpar: *const AVCodecParameters) -> i32 {
    unsafe { (*codecpar).ch_layout.nb_channels }
}

pub(crate) fn av_codecpar_sample_rate_safe(codecpar: *const AVCodecParameters) -> i32 {
    unsafe { (*codecpar).sample_rate }
}

pub(crate) fn av_codecpar_width_safe(codecpar: *const AVCodecParameters) -> i32 {
    unsafe { (*codecpar).width }
}

pub(crate) fn av_codecpar_height_safe(codecpar: *const AVCodecParameters) -> i32 {
    unsafe { (*codecpar).height }
}

pub(crate) fn av_stream_disposition_safe(stream: *const AVStream) -> i32 {
    unsafe { (*stream).disposition }
}

pub(crate) fn av_packet_stream_index_safe(pkt: *const AVPacket) -> i32 {
    unsafe { (*pkt).stream_index }
}

pub(crate) fn av_packet_set_stream_index_safe(pkt: *mut AVPacket, stream_index: i32) {
    unsafe { (*pkt).stream_index = stream_index }
}

pub(crate) fn av_stream_index_safe(stream: *const AVStream) -> i32 {
    unsafe { (*stream).index }
}

pub(crate) fn av_stream_time_base_safe(stream: *const AVStream) -> AVRational {
    unsafe { (*stream).time_base }
}

pub(crate) fn avcodec_find_encoder_safe(api: &FfmpegApi, codec_id: AVCodecID) -> *const AVCodec {
    unsafe { (api.avcodec_find_encoder)(codec_id) }
}

pub(crate) fn avcodec_find_decoder_safe(api: &FfmpegApi, codec_id: AVCodecID) -> *const AVCodec {
    unsafe { (api.avcodec_find_decoder)(codec_id) }
}

pub(crate) fn avcodec_find_encoder_by_name_safe(
    api: &FfmpegApi,
    name: *const core::ffi::c_char,
) -> *const AVCodec {
    unsafe { (api.avcodec_find_encoder_by_name)(name) }
}

pub(crate) fn avcodec_send_frame_safe(
    api: &FfmpegApi,
    avctx: *mut AVCodecContext,
    frame: *const AVFrame,
) -> i32 {
    unsafe { (api.avcodec_send_frame)(avctx, frame) }
}

pub(crate) fn avcodec_receive_packet_safe(
    api: &FfmpegApi,
    avctx: *mut AVCodecContext,
    avpkt: *mut AVPacket,
) -> i32 {
    unsafe { (api.avcodec_receive_packet)(avctx, avpkt) }
}

pub(crate) fn av_packet_unref_safe(api: &FfmpegApi, pkt: *mut AVPacket) {
    unsafe { (api.av_packet_unref)(pkt) }
}

pub(crate) fn av_packet_free_safe(api: &FfmpegApi, pkt: *mut *mut AVPacket) {
    unsafe { (api.av_packet_free)(pkt) }
}

pub(crate) fn av_packet_rescale_ts_safe(
    api: &FfmpegApi,
    pkt: *mut AVPacket,
    tb_src: AVRational,
    tb_dst: AVRational,
) {
    unsafe { (api.av_packet_rescale_ts)(pkt, tb_src, tb_dst) }
}

pub(crate) fn av_frame_unref_safe(api: &FfmpegApi, frame: *mut AVFrame) {
    unsafe { (api.av_frame_unref)(frame) }
}

pub(crate) fn swr_init_safe(api: &FfmpegApi, s: *mut SwrContext) -> i32 {
    unsafe { (api.swr_init)(s) }
}

pub(crate) fn swr_convert_safe(
    api: &FfmpegApi,
    s: *mut SwrContext,
    out: *mut *mut u8,
    out_count: i32,
    input: *const *const u8,
    in_count: i32,
) -> i32 {
    unsafe { (api.swr_convert)(s, out, out_count, input, in_count) }
}

pub(crate) fn swr_get_out_samples_safe(
    api: &FfmpegApi,
    s: *mut SwrContext,
    in_samples: i32,
) -> i32 {
    unsafe { (api.swr_get_out_samples)(s, in_samples) }
}

pub(crate) fn ffmpeg_err(api: &FfmpegApi, code: i32) -> String {
    let mut buf = [0i8; 256];
    let ret = unsafe { (api.av_strerror)(code, buf.as_mut_ptr(), buf.len()) };
    if ret == 0 {
        crate::cstr_ptr_to_lossy_string_safe(buf.as_ptr())
    } else {
        format!("ffmpeg error {}", code)
    }
}

pub(crate) fn av_dict_free_safe(api: &FfmpegApi, dict: *mut *mut AVDictionary) {
    unsafe { (api.av_dict_free)(dict) }
}

pub(crate) fn avcodec_parameters_copy_safe(
    api: &FfmpegApi,
    dst: *mut AVCodecParameters,
    src: *const AVCodecParameters,
) -> i32 {
    unsafe { (api.avcodec_parameters_copy)(dst, src) }
}

fn is_eagain(code: i32) -> bool {
    code == -(EAGAIN as i32)
}

static FFMPEG_INIT: OnceLock<()> = OnceLock::new();

const AVERROR_EOF_FALLBACK: i32 = {
    let a = b'E' as i32;
    let b = b'O' as i32;
    let c = b'F' as i32;
    let d = b' ' as i32;
    -((a) | (b << 8) | (c << 16) | (d << 24))
};

fn init_ffmpeg_once(api: &FfmpegApi) {
    FFMPEG_INIT.get_or_init(|| unsafe {
        let ret = (api.avformat_network_init)();
        if ret < 0 {
            log_debug(&format!(
                "FFmpeg: network init failed: {}",
                ffmpeg_err(api, ret)
            ));
        }
        let version = (api.avutil_version)();
        log_debug(&format!("FFmpeg: avutil version {}", version));
    });
}

/// Helper to read a string value from an AVDictionary.
fn dict_get_string(api: &FfmpegApi, dict: *mut AVDictionary, key: &str) -> Option<String> {
    unsafe {
        if dict.is_null() {
            return None;
        }
        let key_c = CString::new(key).ok()?;
        let entry = (api.av_dict_get)(dict, key_c.as_ptr(), ptr::null(), AV_DICT_IGNORE_SUFFIX);
        if entry.is_null() {
            return None;
        }
        let value_ptr = (*entry).value;
        if value_ptr.is_null() {
            return None;
        }
        Some(CStr::from_ptr(value_ptr).to_string_lossy().into_owned())
    }
}

/// Return whether the media contains a real video stream rather than only an attached cover.
pub fn has_real_video_stream(path: &Path) -> Result<bool, String> {
    const AV_DISPOSITION_ATTACHED_PIC_I32: i32 = 0x0400;

    let api = ffmpeg_api()?;
    init_ffmpeg_once(api);
    let path_c = CString::new(path.to_string_lossy().as_bytes())
        .map_err(|_| "FFmpeg: invalid path".to_string())?;
    let mut fmt_ctx: *mut AVFormatContext = ptr::null_mut();
    let open_ret = crate::ffmpeg_source::avformat_open_input_safe(
        api,
        &mut fmt_ctx,
        path_c.as_ptr(),
        ptr::null_mut(),
        ptr::null_mut(),
    );
    if open_ret < 0 || fmt_ctx.is_null() {
        return Err(format!(
            "FFmpeg: input open failed: {}",
            ffmpeg_err(api, open_ret)
        ));
    }
    let info_ret =
        crate::ffmpeg_source::avformat_find_stream_info_safe(api, fmt_ctx, ptr::null_mut());
    if info_ret < 0 {
        crate::ffmpeg_source::avformat_close_input_safe(api, &mut fmt_ctx);
        return Err(format!(
            "FFmpeg: stream info failed: {}",
            ffmpeg_err(api, info_ret)
        ));
    }

    let nb_streams = crate::ffmpeg_source::av_format_context_nb_streams_safe(fmt_ctx);
    let streams_ptr = crate::ffmpeg_source::av_format_context_streams_safe(fmt_ctx);
    let mut found = false;
    if !streams_ptr.is_null() {
        for index in 0..nb_streams {
            let stream = unsafe { *streams_ptr.add(index as usize) };
            if stream.is_null() {
                continue;
            }
            let codecpar = crate::ffmpeg_source::av_stream_codecpar_safe(stream);
            if codecpar.is_null()
                || crate::ffmpeg_source::av_codecpar_codec_type_safe(codecpar)
                    != AVMediaType_AVMEDIA_TYPE_VIDEO
            {
                continue;
            }
            let disposition = crate::ffmpeg_source::av_stream_disposition_safe(stream);
            let width = crate::ffmpeg_source::av_codecpar_width_safe(codecpar);
            let height = crate::ffmpeg_source::av_codecpar_height_safe(codecpar);
            let codec_id = crate::ffmpeg_source::av_codecpar_codec_id_safe(codecpar);
            let attached_picture = disposition & AV_DISPOSITION_ATTACHED_PIC_I32 != 0;
            log_debug(&format!(
                "FFmpeg: video stream probe index={index} codec_id={codec_id:?} width={width} height={height} disposition=0x{disposition:08x} attached_picture={attached_picture}"
            ));
            // Some valid WebM/Matroska streams expose dimensions only after the
            // decoder opens. A non-attached video stream is sufficient here;
            // the later chunking stage will report a precise decoding error if
            // the stream is actually unusable.
            if !attached_picture {
                found = true;
                break;
            }
        }
    }
    crate::ffmpeg_source::avformat_close_input_safe(api, &mut fmt_ctx);
    Ok(found)
}

/// List all audio streams in a media file.
pub fn list_audio_streams(path: &Path) -> Result<Vec<AudioStreamInfo>, String> {
    let api = ffmpeg_api()?;
    init_ffmpeg_once(api);

    let path_c = CString::new(path.to_string_lossy().as_bytes())
        .map_err(|_| "FFmpeg: invalid path".to_string())?;

    let mut fmt_ctx: *mut AVFormatContext = ptr::null_mut();
    let open_ret = crate::ffmpeg_source::avformat_open_input_safe(
        api,
        &mut fmt_ctx,
        path_c.as_ptr(),
        ptr::null_mut(),
        ptr::null_mut(),
    );
    if open_ret < 0 || fmt_ctx.is_null() {
        return Err(format!(
            "FFmpeg: input open failed: {}",
            ffmpeg_err(api, open_ret)
        ));
    }

    let info_ret =
        crate::ffmpeg_source::avformat_find_stream_info_safe(api, fmt_ctx, ptr::null_mut());
    if info_ret < 0 {
        crate::ffmpeg_source::avformat_close_input_safe(api, &mut fmt_ctx);
        return Err(format!(
            "FFmpeg: stream info failed: {}",
            ffmpeg_err(api, info_ret)
        ));
    }

    // Find the default audio stream index
    let default_stream = crate::ffmpeg_source::av_find_best_stream_safe(
        api,
        fmt_ctx,
        AVMediaType_AVMEDIA_TYPE_AUDIO,
        -1,
        -1,
        ptr::null_mut(),
        0,
    );

    let mut streams = Vec::new();
    let nb_streams = crate::ffmpeg_source::av_format_context_nb_streams_safe(fmt_ctx);
    let streams_ptr = crate::ffmpeg_source::av_format_context_streams_safe(fmt_ctx);

    if streams_ptr.is_null() {
        crate::ffmpeg_source::avformat_close_input_safe(api, &mut fmt_ctx);
        return Ok(streams);
    }

    for i in 0..nb_streams {
        let stream = unsafe { *streams_ptr.add(i as usize) };
        if stream.is_null() {
            continue;
        }

        let codecpar = crate::ffmpeg_source::av_stream_codecpar_safe(stream);
        if codecpar.is_null() {
            continue;
        }

        let codec_type = crate::ffmpeg_source::av_codecpar_codec_type_safe(codecpar);
        if codec_type != AVMediaType_AVMEDIA_TYPE_AUDIO {
            continue;
        }

        let codec_id = crate::ffmpeg_source::av_codecpar_codec_id_safe(codecpar);
        let codec = crate::ffmpeg_source::avcodec_find_decoder_safe(api, codec_id);
        let codec_name = if !codec.is_null() {
            let name_ptr = unsafe { (*codec).name };
            if !name_ptr.is_null() {
                unsafe { CStr::from_ptr(name_ptr).to_string_lossy().into_owned() }
            } else {
                format!("codec_{}", codec_id)
            }
        } else {
            format!("codec_{}", codec_id)
        };

        let channels = crate::ffmpeg_source::av_codecpar_channels_safe(codecpar);
        let sample_rate = crate::ffmpeg_source::av_codecpar_sample_rate_safe(codecpar);
        let metadata = unsafe { (*stream).metadata };

        let language = dict_get_string(api, metadata, "language");
        let title = dict_get_string(api, metadata, "title");

        let is_default = i as i32 == default_stream;

        streams.push(AudioStreamInfo {
            index: i as i32,
            language,
            title,
            codec: codec_name,
            channels,
            sample_rate,
            is_default,
        });
    }

    crate::ffmpeg_source::avformat_close_input_safe(api, &mut fmt_ctx);
    Ok(streams)
}

pub struct FfmpegSource {
    api: &'static FfmpegApi,
    fmt_ctx: *mut AVFormatContext,
    codec_ctx: *mut AVCodecContext,
    stream_index: i32,
    swr_ctx: *mut SwrContext,
    packet: *mut AVPacket,
    frame: *mut AVFrame,
    buffer: Vec<f32>,
    buffer_pos: usize,
    buffer_start_pts_us: Option<i64>,
    buffer_frame_count: usize,
    channels: u16,
    sample_rate: u32,
    time_base_num: i64,
    time_base_den: i64,
    next_pts_us: i64,
    stream_start_us: i64,
    fmt_start_us: i64,
    pts_offset_us: Option<i64>,
    // Requested media position for an exact post-demux seek trim. FFmpeg may
    // seek to an earlier packet/keyframe, so decoded samples before this point
    // must never be exposed to the player.
    seek_target_us: Option<i64>,
    pts_clock: Option<Arc<AtomicI64>>,
    total_duration: Option<Duration>,
    eof: bool,
    sent_eof: bool,
}

// SAFETY: FfmpegSource is used only on the playback thread.
unsafe impl Send for FfmpegSource {}

impl FfmpegSource {
    /// Create a new FFmpeg audio source.
    ///
    /// - `path`: Path to the media file.
    /// - `start_seconds`: Start position in seconds.
    /// - `pts_clock`: Optional atomic for PTS tracking.
    /// - `preferred_stream_index`: Optional audio stream index. If `None`, uses the default stream.
    pub fn try_new(
        path: &Path,
        start_seconds: u64,
        pts_clock: Option<Arc<AtomicI64>>,
        preferred_stream_index: Option<i32>,
    ) -> Result<Self, String> {
        Self::try_new_at_with_forced_channels(
            path,
            start_seconds as f64,
            pts_clock,
            preferred_stream_index,
            None,
        )
    }

    /// Create a new FFmpeg audio source while forcing the decoded output to a
    /// specific channel count. This is intentionally used only by conservative
    /// recovery paths; ordinary playback and exports keep the source layout.
    pub(crate) fn try_new_with_forced_channels(
        path: &Path,
        start_seconds: u64,
        pts_clock: Option<Arc<AtomicI64>>,
        preferred_stream_index: Option<i32>,
        forced_channels: u16,
    ) -> Result<Self, String> {
        Self::try_new_at_with_forced_channels(
            path,
            start_seconds as f64,
            pts_clock,
            preferred_stream_index,
            Some(forced_channels.max(1)),
        )
    }

    /// Create a new FFmpeg audio source at a precise fractional-second position.
    pub fn try_new_at(
        path: &Path,
        start_seconds: f64,
        pts_clock: Option<Arc<AtomicI64>>,
        preferred_stream_index: Option<i32>,
    ) -> Result<Self, String> {
        Self::try_new_at_with_forced_channels(
            path,
            start_seconds,
            pts_clock,
            preferred_stream_index,
            None,
        )
    }

    fn try_new_at_with_forced_channels(
        path: &Path,
        start_seconds: f64,
        pts_clock: Option<Arc<AtomicI64>>,
        preferred_stream_index: Option<i32>,
        forced_channels: Option<u16>,
    ) -> Result<Self, String> {
        let start_seconds = if start_seconds.is_finite() {
            start_seconds.max(0.0)
        } else {
            0.0
        };
        let api = ffmpeg_api()?;
        init_ffmpeg_once(api);

        let path_c = CString::new(path.to_string_lossy().as_bytes())
            .map_err(|_| "FFmpeg: invalid path".to_string())?;

        let mut fmt_ctx: *mut AVFormatContext = ptr::null_mut();
        let open_ret = crate::ffmpeg_source::avformat_open_input_safe(
            api,
            &mut fmt_ctx,
            path_c.as_ptr(),
            ptr::null_mut(),
            ptr::null_mut(),
        );
        if open_ret < 0 || fmt_ctx.is_null() {
            return Err(format!(
                "FFmpeg: input open failed: {}",
                ffmpeg_err(api, open_ret)
            ));
        }

        let info_ret =
            crate::ffmpeg_source::avformat_find_stream_info_safe(api, fmt_ctx, ptr::null_mut());
        if info_ret < 0 {
            crate::ffmpeg_source::avformat_close_input_safe(api, &mut fmt_ctx);
            return Err(format!(
                "FFmpeg: stream info failed: {}",
                ffmpeg_err(api, info_ret)
            ));
        }

        // Determine which audio stream to use
        let stream_index = if let Some(preferred) = preferred_stream_index {
            // Validate that the preferred stream exists and is an audio stream
            let nb_streams =
                crate::ffmpeg_source::av_format_context_nb_streams_safe(fmt_ctx) as i32;
            if preferred < 0 || preferred >= nb_streams {
                crate::ffmpeg_source::avformat_close_input_safe(api, &mut fmt_ctx);
                return Err(format!(
                    "FFmpeg: stream index {} out of range (0-{})",
                    preferred,
                    nb_streams - 1
                ));
            }
            let streams = crate::ffmpeg_source::av_format_context_streams_safe(fmt_ctx);
            if streams.is_null() {
                crate::ffmpeg_source::avformat_close_input_safe(api, &mut fmt_ctx);
                return Err("FFmpeg: stream list missing".to_string());
            }
            let stream = unsafe { *streams.add(preferred as usize) };
            if stream.is_null() {
                crate::ffmpeg_source::avformat_close_input_safe(api, &mut fmt_ctx);
                return Err(format!("FFmpeg: stream {} is null", preferred));
            }
            let codecpar = crate::ffmpeg_source::av_stream_codecpar_safe(stream);
            if codecpar.is_null() {
                crate::ffmpeg_source::avformat_close_input_safe(api, &mut fmt_ctx);
                return Err(format!(
                    "FFmpeg: stream {} has no codec parameters",
                    preferred
                ));
            }
            let codec_type = crate::ffmpeg_source::av_codecpar_codec_type_safe(codecpar);
            if codec_type != AVMediaType_AVMEDIA_TYPE_AUDIO {
                crate::ffmpeg_source::avformat_close_input_safe(api, &mut fmt_ctx);
                return Err(format!(
                    "FFmpeg: stream {} is not an audio stream",
                    preferred
                ));
            }
            log_debug(&format!(
                "FFmpeg: using preferred audio stream {}",
                preferred
            ));
            preferred
        } else {
            // Use the best audio stream
            let idx = crate::ffmpeg_source::av_find_best_stream_safe(
                api,
                fmt_ctx,
                AVMediaType_AVMEDIA_TYPE_AUDIO,
                -1,
                -1,
                ptr::null_mut(),
                0,
            );
            if idx < 0 {
                crate::ffmpeg_source::avformat_close_input_safe(api, &mut fmt_ctx);
                return Err("FFmpeg: audio stream not found".to_string());
            }
            idx
        };

        let stream = unsafe {
            let streams = (*fmt_ctx).streams;
            if streams.is_null() {
                (api.avformat_close_input)(&mut fmt_ctx);
                return Err("FFmpeg: stream list missing".to_string());
            }
            *streams.add(stream_index as usize)
        };
        if stream.is_null() {
            crate::ffmpeg_source::avformat_close_input_safe(api, &mut fmt_ctx);
            return Err("FFmpeg: audio stream pointer missing".to_string());
        }

        let codecpar = crate::ffmpeg_source::av_stream_codecpar_safe(stream);
        if codecpar.is_null() {
            crate::ffmpeg_source::avformat_close_input_safe(api, &mut fmt_ctx);
            return Err("FFmpeg: codec parameters missing".to_string());
        }

        let codec_id = crate::ffmpeg_source::av_codecpar_codec_id_safe(codecpar);
        log_debug(&format!("FFmpeg: codec_id={}", codec_id));
        let codec = crate::ffmpeg_source::avcodec_find_decoder_safe(api, codec_id);
        if codec.is_null() {
            crate::ffmpeg_source::avformat_close_input_safe(api, &mut fmt_ctx);
            return Err(format!(
                "FFmpeg: decoder not found for codec_id={}",
                codec_id
            ));
        }
        log_debug("FFmpeg: decoder found");

        let mut codec_ctx = crate::ffmpeg_source::avcodec_alloc_context3_safe(api, codec);
        if codec_ctx.is_null() {
            crate::ffmpeg_source::avformat_close_input_safe(api, &mut fmt_ctx);
            return Err("FFmpeg: failed to allocate codec context".to_string());
        }

        let params_ret = unsafe { (api.avcodec_parameters_to_context)(codec_ctx, codecpar) };
        if params_ret < 0 {
            unsafe {
                (api.avcodec_free_context)(&mut codec_ctx);
                (api.avformat_close_input)(&mut fmt_ctx);
            }
            return Err(format!(
                "FFmpeg: parameters to context failed: {}",
                ffmpeg_err(api, params_ret)
            ));
        }

        let open_ret =
            crate::ffmpeg_source::avcodec_open2_safe(api, codec_ctx, codec, ptr::null_mut());
        if open_ret < 0 {
            unsafe {
                (api.avcodec_free_context)(&mut codec_ctx);
                (api.avformat_close_input)(&mut fmt_ctx);
            }
            return Err(format!(
                "FFmpeg: decoder open failed: {}",
                ffmpeg_err(api, open_ret)
            ));
        }

        let (mut swr_ctx, channels, sample_rate) =
            Self::init_resampler(api, codec_ctx, codecpar, forced_channels)?;

        let mut packet = crate::ffmpeg_source::av_packet_alloc_safe(api);
        if packet.is_null() {
            unsafe {
                (api.swr_free)(&mut swr_ctx);
                (api.avcodec_free_context)(&mut codec_ctx);
                (api.avformat_close_input)(&mut fmt_ctx);
            }
            return Err("FFmpeg: packet alloc failed".to_string());
        }

        let frame = crate::ffmpeg_source::av_frame_alloc_safe(api);
        if frame.is_null() {
            unsafe {
                (api.av_packet_free)(&mut packet);
                (api.swr_free)(&mut swr_ctx);
                (api.avcodec_free_context)(&mut codec_ctx);
                (api.avformat_close_input)(&mut fmt_ctx);
            }
            return Err("FFmpeg: frame alloc failed".to_string());
        }

        let total_duration = unsafe {
            let dur = (*fmt_ctx).duration;
            if dur > 0 {
                Some(Duration::from_micros(dur as u64))
            } else {
                None
            }
        };

        let time_base = unsafe { (*stream).time_base };
        let time_base_num = if time_base.num == 0 {
            1
        } else {
            time_base.num as i64
        };
        let time_base_den = if time_base.den == 0 {
            1
        } else {
            time_base.den as i64
        };
        let start_pts_us = (start_seconds * 1_000_000.0).round() as i64;
        let stream_start_raw = unsafe { (*stream).start_time };
        let stream_start_us = if stream_start_raw != AV_NOPTS_VALUE_I64 {
            (stream_start_raw as i128)
                .saturating_mul(time_base_num as i128)
                .saturating_mul(1_000_000)
                .saturating_div(time_base_den as i128)
                .clamp(i64::MIN as i128, i64::MAX as i128) as i64
        } else {
            0
        };
        let fmt_start_raw = unsafe { (*fmt_ctx).start_time };
        let fmt_start_us = if fmt_start_raw != AV_NOPTS_VALUE_I64 {
            fmt_start_raw
        } else {
            0
        };
        if stream_start_us != 0 || fmt_start_us != 0 {
            log_debug(&format!(
                "FFmpeg: start_time stream={} us fmt={} us",
                stream_start_us, fmt_start_us
            ));
        }

        let mut source = Self {
            api,
            fmt_ctx,
            codec_ctx,
            stream_index,
            swr_ctx,
            packet,
            frame,
            buffer: Vec::new(),
            buffer_pos: 0,
            buffer_start_pts_us: None,
            buffer_frame_count: 0,
            channels,
            sample_rate,
            time_base_num,
            time_base_den,
            next_pts_us: start_pts_us,
            stream_start_us,
            fmt_start_us,
            pts_offset_us: None,
            seek_target_us: None,
            pts_clock,
            total_duration,
            eof: false,
            sent_eof: false,
        };

        if start_seconds > 0.0 {
            log_debug(&format!("FFmpeg: seeking to {}s", start_seconds));
            match source.try_seek(Duration::from_secs_f64(start_seconds)) {
                Ok(()) => log_debug(&format!("FFmpeg: seek to {}s succeeded", start_seconds)),
                Err(err) => log_debug(&format!("FFmpeg: initial seek failed: {}", err)),
            }
        }

        Ok(source)
    }

    fn init_resampler(
        api: &FfmpegApi,
        codec_ctx: *mut AVCodecContext,
        codecpar: *const AVCodecParameters,
        forced_channels: Option<u16>,
    ) -> Result<(*mut SwrContext, u16, u32), String> {
        let mut in_layout: AVChannelLayout = crate::zeroed_safe();
        let mut out_layout: AVChannelLayout = crate::zeroed_safe();

        let src_layout = unsafe { &(*codec_ctx).ch_layout };
        let mut channels = src_layout.nb_channels;
        if channels <= 0 && !codecpar.is_null() {
            channels = crate::ffmpeg_source::av_codecpar_channels_safe(codecpar);
        }
        if channels <= 0 {
            channels = 2;
        }

        let valid = unsafe { (api.av_channel_layout_check)(src_layout) } != 0;
        let copy_ret = if valid && src_layout.nb_channels > 0 {
            unsafe { (api.av_channel_layout_copy)(&mut in_layout, src_layout) }
        } else {
            unsafe { (api.av_channel_layout_default)(&mut in_layout, channels) };
            0
        };
        if copy_ret < 0 {
            unsafe { (api.av_channel_layout_uninit)(&mut in_layout) };
            return Err(format!(
                "FFmpeg: channel layout copy failed: {}",
                ffmpeg_err(api, copy_ret)
            ));
        }

        let out_copy = if let Some(forced_channels) = forced_channels {
            unsafe {
                (api.av_channel_layout_default)(&mut out_layout, forced_channels.max(1) as i32);
            }
            0
        } else {
            unsafe { (api.av_channel_layout_copy)(&mut out_layout, &in_layout) }
        };
        if out_copy < 0 {
            unsafe {
                (api.av_channel_layout_uninit)(&mut in_layout);
                (api.av_channel_layout_uninit)(&mut out_layout);
            }
            return Err(format!(
                "FFmpeg: channel layout copy failed: {}",
                ffmpeg_err(api, out_copy)
            ));
        }

        let mut sample_rate = unsafe { (*codec_ctx).sample_rate };
        if sample_rate <= 0 && !codecpar.is_null() {
            sample_rate = crate::ffmpeg_source::av_codecpar_sample_rate_safe(codecpar);
        }
        if sample_rate <= 0 {
            sample_rate = 48_000;
        }

        let mut swr_ctx: *mut SwrContext = ptr::null_mut();
        let in_sample_fmt = unsafe { (*codec_ctx).sample_fmt };
        let swr_ret = unsafe {
            (api.swr_alloc_set_opts2)(
                &mut swr_ctx,
                &out_layout,
                AVSampleFormat_AV_SAMPLE_FMT_FLT,
                sample_rate,
                &in_layout,
                in_sample_fmt,
                sample_rate,
                0,
                ptr::null_mut(),
            )
        };
        if swr_ret < 0 || swr_ctx.is_null() {
            unsafe {
                (api.av_channel_layout_uninit)(&mut in_layout);
                (api.av_channel_layout_uninit)(&mut out_layout);
            }
            return Err(format!(
                "FFmpeg: resampler alloc failed: {}",
                ffmpeg_err(api, swr_ret)
            ));
        }

        let init_ret = crate::ffmpeg_source::swr_init_safe(api, swr_ctx);
        if init_ret < 0 {
            unsafe {
                (api.swr_free)(&mut swr_ctx);
                (api.av_channel_layout_uninit)(&mut in_layout);
                (api.av_channel_layout_uninit)(&mut out_layout);
            }
            return Err(format!(
                "FFmpeg: resampler init failed: {}",
                ffmpeg_err(api, init_ret)
            ));
        }

        let out_channels = out_layout.nb_channels as u16;

        unsafe {
            (api.av_channel_layout_uninit)(&mut in_layout);
            (api.av_channel_layout_uninit)(&mut out_layout);
        }

        Ok((swr_ctx, out_channels, sample_rate as u32))
    }

    fn fill_from_frame(&mut self) -> Result<bool, String> {
        let in_samples = unsafe { (*self.frame).nb_samples };
        if in_samples <= 0 {
            crate::ffmpeg_source::av_frame_unref_safe(self.api, self.frame);
            return Ok(false);
        }

        let out_samples = unsafe { (self.api.swr_get_out_samples)(self.swr_ctx, in_samples) };
        if out_samples <= 0 {
            crate::ffmpeg_source::av_frame_unref_safe(self.api, self.frame);
            return Ok(false);
        }

        let channels = self.channels as usize;
        let mut out_buffer = vec![0.0f32; out_samples as usize * channels];
        let mut out_ptr = out_buffer.as_mut_ptr() as *mut u8;
        let out_ptrs = &mut out_ptr as *mut *mut u8;

        let in_data = unsafe {
            if !(*self.frame).extended_data.is_null() {
                (*self.frame).extended_data as *const *const u8
            } else {
                (*self.frame).data.as_ptr() as *const *const u8
            }
        };

        // Capture the real decoded timestamp before av_frame_unref(). After unref
        // AVFrame::pts is no longer reliable, which previously made an early seek
        // landing look as if it began exactly at the requested position.
        let decoded_frame_pts_us = self.frame_pts_us();
        let converted = unsafe {
            (self.api.swr_convert)(self.swr_ctx, out_ptrs, out_samples, in_data, in_samples)
        };

        crate::ffmpeg_source::av_frame_unref_safe(self.api, self.frame);

        if converted < 0 {
            return Err(format!(
                "FFmpeg: resample failed: {}",
                ffmpeg_err(self.api, converted)
            ));
        }

        let produced = converted as usize * channels;
        if produced == 0 {
            return Ok(false);
        }
        let mut pts_us = if self.seek_target_us.is_some() {
            decoded_frame_pts_us.unwrap_or(self.next_pts_us)
        } else {
            self.next_pts_us
        };
        if self.pts_offset_us.is_none() {
            let mut offset = 0i64;
            if self.stream_start_us > 0 && pts_us >= 0 && pts_us < self.stream_start_us / 2 {
                offset = self.stream_start_us;
            } else if self.fmt_start_us > 0 && pts_us >= 0 && pts_us < self.fmt_start_us / 2 {
                offset = self.fmt_start_us;
            }
            if offset != 0 {
                log_debug(&format!(
                    "FFmpeg: applying start_time offset {:.3}s",
                    offset as f64 / 1_000_000.0
                ));
            }
            self.pts_offset_us = Some(offset);
        }
        if let Some(offset) = self.pts_offset_us {
            pts_us = pts_us.saturating_add(offset);
        }
        out_buffer.truncate(produced);

        // avformat_seek_file is allowed to land before the requested position.
        // Keep decoding, but discard every sample before the exact seek target.
        if let Some(requested_target_us) = self.seek_target_us {
            // pts_us is normalized with the same stream/container offset used by
            // the playback clock, so put the requested target on that timeline too.
            let target_us = requested_target_us.saturating_add(self.pts_offset_us.unwrap_or(0));
            let frame_count = out_buffer.len() / channels;
            let frame_duration_us = (frame_count as i128)
                .saturating_mul(1_000_000)
                .saturating_div(self.sample_rate as i128)
                as i64;
            let frame_end_us = pts_us.saturating_add(frame_duration_us);

            if frame_end_us <= target_us {
                self.next_pts_us = frame_end_us;
                return Ok(false);
            }

            if pts_us < target_us {
                let delta_us = target_us.saturating_sub(pts_us) as i128;
                let skip_frames = delta_us
                    .saturating_mul(self.sample_rate as i128)
                    .saturating_add(999_999)
                    .saturating_div(1_000_000)
                    .min(frame_count as i128) as usize;
                let skip_samples = skip_frames.saturating_mul(channels);
                if skip_samples >= out_buffer.len() {
                    self.next_pts_us = frame_end_us;
                    return Ok(false);
                }
                if skip_samples > 0 {
                    out_buffer.drain(..skip_samples);
                    let skipped_us = (skip_frames as i128)
                        .saturating_mul(1_000_000)
                        .saturating_div(self.sample_rate as i128)
                        as i64;
                    pts_us = pts_us.saturating_add(skipped_us);
                    log_debug(&format!(
                        "FFmpeg: precise seek trimmed {} frame(s); first output {:.6}s, target {:.6}s",
                        skip_frames,
                        pts_us as f64 / 1_000_000.0,
                        target_us as f64 / 1_000_000.0
                    ));
                }
            }
            log_debug(&format!(
                "FFmpeg: precise seek ready; first output {:.6}s, target {:.6}s",
                pts_us as f64 / 1_000_000.0,
                target_us as f64 / 1_000_000.0
            ));
            self.seek_target_us = None;
        }

        self.buffer.extend_from_slice(&out_buffer);
        self.buffer_start_pts_us = Some(pts_us);
        self.buffer_frame_count = out_buffer.len() / channels;
        let duration_us = (self.buffer_frame_count as i128)
            .saturating_mul(1_000_000)
            .saturating_div(self.sample_rate as i128) as i64;
        self.next_pts_us = pts_us.saturating_add(duration_us);
        Ok(!out_buffer.is_empty())
    }

    fn flush_resampler(&mut self) -> bool {
        let out_samples = unsafe { (self.api.swr_get_out_samples)(self.swr_ctx, 0) };
        if out_samples <= 0 {
            return false;
        }
        let channels = self.channels as usize;
        let mut out_buffer = vec![0.0f32; out_samples as usize * channels];
        let mut out_ptr = out_buffer.as_mut_ptr() as *mut u8;
        let out_ptrs = &mut out_ptr as *mut *mut u8;

        let converted =
            unsafe { (self.api.swr_convert)(self.swr_ctx, out_ptrs, out_samples, ptr::null(), 0) };
        if converted <= 0 {
            return false;
        }
        let produced = converted as usize * channels;
        out_buffer.truncate(produced);
        self.buffer.extend_from_slice(&out_buffer);
        true
    }

    fn receive_frame(&mut self) -> Result<bool, String> {
        let ret = unsafe { (self.api.avcodec_receive_frame)(self.codec_ctx, self.frame) };
        if ret == 0 {
            return self.fill_from_frame();
        }
        if ret == AVERROR_EOF_FALLBACK {
            self.sent_eof = true;
            return Ok(false);
        }
        if is_eagain(ret) {
            return Ok(false);
        }
        Err(format!(
            "FFmpeg: receive_frame failed: {}",
            ffmpeg_err(self.api, ret)
        ))
    }

    fn refill(&mut self) -> bool {
        if self.eof {
            log_debug("FFmpeg refill: already eof");
            return false;
        }

        self.buffer.clear();
        self.buffer_pos = 0;
        self.buffer_start_pts_us = None;
        self.buffer_frame_count = 0;

        loop {
            match self.receive_frame() {
                Ok(true) => return true,
                Ok(false) => {}
                Err(err) => {
                    log_debug(&format!("FFmpeg refill: receive_frame error: {}", err));
                    return false;
                }
            }

            if self.sent_eof {
                if self.flush_resampler() {
                    return true;
                }
                self.eof = true;
                log_debug("FFmpeg refill: sent_eof, setting eof=true");
                return false;
            }

            let read_ret = unsafe { (self.api.av_read_frame)(self.fmt_ctx, self.packet) };
            if read_ret < 0 {
                log_debug(&format!(
                    "FFmpeg refill: av_read_frame returned {}",
                    read_ret
                ));
                self.sent_eof = true;
                let send_ret =
                    unsafe { (self.api.avcodec_send_packet)(self.codec_ctx, ptr::null()) };
                if send_ret < 0 {
                    log_debug(&format!(
                        "FFmpeg: send_packet EOF failed: {}",
                        ffmpeg_err(self.api, send_ret)
                    ));
                    self.eof = true;
                    return false;
                }
                continue;
            }

            let pkt_stream = unsafe { (*self.packet).stream_index };
            if pkt_stream != self.stream_index {
                crate::ffmpeg_source::av_packet_unref_safe(self.api, self.packet);
                continue;
            }

            let send_ret = unsafe { (self.api.avcodec_send_packet)(self.codec_ctx, self.packet) };
            crate::ffmpeg_source::av_packet_unref_safe(self.api, self.packet);
            if send_ret < 0 {
                log_debug(&format!(
                    "FFmpeg: send_packet failed: {}",
                    ffmpeg_err(self.api, send_ret)
                ));
            }
        }
    }
}

impl Iterator for FfmpegSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.buffer_pos >= self.buffer.len() && !self.refill() {
            return None;
        }
        if let (Some(start_pts), Some(clock)) = (self.buffer_start_pts_us, &self.pts_clock) {
            let frame_index = self.buffer_pos / self.channels as usize;
            let pts_us = (start_pts as i128).saturating_add(
                (frame_index as i128)
                    .saturating_mul(1_000_000)
                    .saturating_div(self.sample_rate as i128),
            ) as i64;
            clock.store(pts_us, Ordering::Release);
        }
        let sample = self.buffer[self.buffer_pos];
        self.buffer_pos += 1;
        Some(sample)
    }
}

impl Source for FfmpegSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        self.total_duration
    }

    fn try_seek(&mut self, pos: Duration) -> Result<(), rodio::source::SeekError> {
        let target = (pos.as_secs_f64() * AV_TIME_BASE as f64) as i64;
        log_debug(&format!(
            "FFmpeg: try_seek to {} (target={}, eof={}, sent_eof={})",
            pos.as_secs_f64(),
            target,
            self.eof,
            self.sent_eof
        ));
        let seek_ret = unsafe {
            (self.api.avformat_seek_file)(self.fmt_ctx, -1, i64::MIN, target, i64::MAX, 0)
        };
        if seek_ret < 0 {
            let err = std::io::Error::other(format!(
                "FFmpeg: seek failed: {}",
                ffmpeg_err(self.api, seek_ret)
            ));
            return Err(rodio::source::SeekError::Other(Box::new(err)));
        }
        log_debug("FFmpeg: avformat_seek_file succeeded");

        unsafe {
            (self.api.avcodec_flush_buffers)(self.codec_ctx);
            (self.api.swr_close)(self.swr_ctx);
            let reset_ret = (self.api.swr_init)(self.swr_ctx);
            if reset_ret < 0 {
                let err = std::io::Error::other(format!(
                    "FFmpeg: resampler reset failed: {}",
                    ffmpeg_err(self.api, reset_ret)
                ));
                return Err(rodio::source::SeekError::Other(Box::new(err)));
            }
        }

        self.buffer.clear();
        self.buffer_pos = 0;
        self.buffer_start_pts_us = None;
        self.buffer_frame_count = 0;
        self.eof = false;
        self.sent_eof = false;
        let pos_us = pos.as_micros().min(i64::MAX as u128) as i64;
        let mut next_pts = pos_us;
        if let Some(offset) = self.pts_offset_us {
            next_pts = next_pts.saturating_add(offset);
        }
        self.next_pts_us = next_pts;
        self.seek_target_us = Some(pos_us);
        if let Some(clock) = &self.pts_clock {
            clock.store(next_pts, Ordering::Release);
        }
        Ok(())
    }
}

impl FfmpegSource {
    fn frame_pts_us(&self) -> Option<i64> {
        let pts = unsafe { (*self.frame).pts };
        if pts == AV_NOPTS_VALUE_I64 {
            return None;
        }
        let num = self.time_base_num as i128;
        let den = self.time_base_den as i128;
        let pts_us = (pts as i128)
            .saturating_mul(num)
            .saturating_mul(1_000_000)
            .saturating_div(den);
        Some(pts_us.clamp(i64::MIN as i128, i64::MAX as i128) as i64)
    }
}

impl Drop for FfmpegSource {
    fn drop(&mut self) {
        unsafe {
            if !self.frame.is_null() {
                (self.api.av_frame_free)(&mut self.frame);
            }
            if !self.packet.is_null() {
                (self.api.av_packet_free)(&mut self.packet);
            }
            if !self.swr_ctx.is_null() {
                (self.api.swr_free)(&mut self.swr_ctx);
            }
            if !self.codec_ctx.is_null() {
                (self.api.avcodec_free_context)(&mut self.codec_ctx);
            }
            if !self.fmt_ctx.is_null() {
                (self.api.avformat_close_input)(&mut self.fmt_ctx);
            }
        }
    }
}
