use reqwest::blocking::Client;
use reqwest::header::USER_AGENT;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};
#[cfg(feature = "whisper_cuda")]
use zip::ZipArchive;

#[cfg(feature = "whisper_cuda")]
const CUDA_RUNTIME_ZIP_URL: &str = "https://raw.githubusercontent.com/Ambro86/Sonarpad/master/dll/whisper-cuda-runtime-win64-v13.1.zip";
#[cfg(feature = "whisper_cuda")]
const CUDA_RUNTIME_ZIP_NAME: &str = "whisper-cuda-runtime-win64-v13.1.zip";
#[cfg(feature = "whisper_cuda")]
const CUDA_RUNTIME_DLLS: [&str; 3] = ["cudart64_13.dll", "cublas64_13.dll", "cublasLt64_13.dll"];

#[derive(Clone, Copy)]
pub enum WhisperModel {
    SmallQ51,
    MediumQ50,
    LargeV3TurboQ50,
}

struct ModelSpec {
    file_name: &'static str,
    url: &'static str,
    sha256: &'static str,
}

impl WhisperModel {
    fn spec(self) -> ModelSpec {
        match self {
            WhisperModel::SmallQ51 => ModelSpec {
                file_name: "ggml-small-q5_1.bin",
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small-q5_1.bin",
                sha256: "ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb",
            },
            WhisperModel::MediumQ50 => ModelSpec {
                file_name: "ggml-medium-q5_0.bin",
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium-q5_0.bin",
                sha256: "8c7decb0f438f48735481f5e5ba2fdfdbc8d36d495a8f03ec0bbdffcf7f8b022",
            },
            WhisperModel::LargeV3TurboQ50 => ModelSpec {
                file_name: "ggml-large-v3-turbo-q5_0.bin",
                url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin",
                sha256: "3942e8ae52f576f1e20ca34f5b17032916f6cfeb32f60ce157894bce7ce6f268",
            },
        }
    }
}

fn compute_sha256(path: &Path) -> Result<String, String> {
    let file = fs::File::open(path).map_err(|e| format!("open hash file failed: {e}"))?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("hash read failed: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn verify_model_hash(path: &Path, expected: &str) -> Result<bool, String> {
    if !path.exists() {
        return Ok(false);
    }
    let actual = compute_sha256(path)?;
    Ok(actual.eq_ignore_ascii_case(expected))
}

fn download_to_part_file(url: &str, target: &Path, cancel: &Arc<AtomicBool>) -> Result<(), String> {
    let _power_awake = crate::power_awake::acquire("download");
    let client = Client::builder()
        .connect_timeout(std::time::Duration::from_secs(30))
        .timeout(std::time::Duration::from_secs(60 * 30))
        .build()
        .map_err(|e| format!("http client build failed: {e}"))?;

    let mut response = client
        .get(url)
        .header(USER_AGENT, "Sonarpad/whisper")
        .send()
        .map_err(|e| format!("download request failed: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("download failed: HTTP {}", response.status()));
    }

    let mut part_path = target.to_path_buf();
    let part_ext = match part_path.extension().and_then(|e| e.to_str()) {
        Some(ext) if !ext.is_empty() => format!("{ext}.part"),
        _ => "part".to_string(),
    };
    part_path.set_extension(part_ext);

    let mut out = fs::File::create(&part_path).map_err(|e| format!("create .part failed: {e}"))?;
    let mut buf = vec![0u8; 128 * 1024];
    loop {
        if cancel.load(Ordering::Relaxed) {
            let _unused = fs::remove_file(&part_path);
            return Err("cancelled".to_string());
        }
        let read = response
            .read(&mut buf)
            .map_err(|e| format!("download read failed: {e}"))?;
        if read == 0 {
            break;
        }
        out.write_all(&buf[..read])
            .map_err(|e| format!("write .part failed: {e}"))?;
    }
    out.flush()
        .map_err(|e| format!("flush .part failed: {e}"))?;
    fs::rename(&part_path, target).map_err(|e| format!("atomic rename failed: {e}"))?;
    Ok(())
}

pub fn ensure_model(
    models_dir: &Path,
    model: WhisperModel,
    cancel: &Arc<AtomicBool>,
) -> Result<PathBuf, String> {
    let spec = model.spec();
    fs::create_dir_all(models_dir).map_err(|e| format!("create models dir failed: {e}"))?;
    let model_path = models_dir.join(spec.file_name);

    if verify_model_hash(&model_path, spec.sha256)? {
        return Ok(model_path);
    }

    if model_path.exists() {
        let _unused = fs::remove_file(&model_path);
    }
    download_to_part_file(spec.url, &model_path, cancel)?;
    if !verify_model_hash(&model_path, spec.sha256)? {
        let _unused = fs::remove_file(&model_path);
        download_to_part_file(spec.url, &model_path, cancel)?;
        if !verify_model_hash(&model_path, spec.sha256)? {
            let actual = compute_sha256(&model_path).unwrap_or_else(|_| "unknown".to_string());
            let _unused = fs::remove_file(&model_path);
            return Err(format!(
                "hash mismatch after download (expected {}, got {})",
                spec.sha256, actual
            ));
        }
    }
    Ok(model_path)
}

#[cfg(feature = "whisper_cuda")]
fn has_nvidia_driver() -> bool {
    let windir = std::env::var_os("WINDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    windir.join("System32").join("nvcuda.dll").exists()
}

#[cfg(feature = "whisper_cuda")]
fn copy_cuda_dlls_from_dir(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| format!("create cuda target dir failed: {e}"))?;
    for name in CUDA_RUNTIME_DLLS {
        let from = src.join(name);
        let to = dst.join(name);
        if !from.exists() {
            return Err(format!(
                "missing runtime dll after extraction: {}",
                from.display()
            ));
        }
        fs::copy(&from, &to).map_err(|e| {
            format!(
                "copy cuda dll failed ({} -> {}): {e}",
                from.display(),
                to.display()
            )
        })?;
    }
    Ok(())
}

#[cfg(feature = "whisper_cuda")]
fn cuda_runtime_present(dir: &Path) -> bool {
    CUDA_RUNTIME_DLLS.iter().all(|name| dir.join(name).exists())
}

#[cfg(feature = "whisper_cuda")]
fn extract_cuda_runtime_zip(zip_path: &Path, out_dir: &Path) -> Result<(), String> {
    let file = fs::File::open(zip_path).map_err(|e| format!("open cuda zip failed: {e}"))?;
    let mut zip = ZipArchive::new(file).map_err(|e| format!("read cuda zip failed: {e}"))?;
    fs::create_dir_all(out_dir).map_err(|e| format!("create cuda dir failed: {e}"))?;

    for dll in CUDA_RUNTIME_DLLS {
        let mut entry = zip
            .by_name(dll)
            .map_err(|e| format!("cuda zip missing {dll}: {e}"))?;
        let target = out_dir.join(dll);
        let tmp = out_dir.join(format!("{dll}.part"));
        {
            let mut out =
                fs::File::create(&tmp).map_err(|e| format!("create extracted dll failed: {e}"))?;
            std::io::copy(&mut entry, &mut out).map_err(|e| format!("extract dll failed: {e}"))?;
            out.flush()
                .map_err(|e| format!("flush extracted dll failed: {e}"))?;
        }
        if target.exists() {
            let _unused = fs::remove_file(&target);
        }
        fs::rename(&tmp, &target).map_err(|e| format!("finalize extracted dll failed: {e}"))?;
    }
    Ok(())
}

#[cfg(feature = "whisper_cuda")]
pub fn ensure_cuda_runtime(cuda_dir: &Path, cancel: &Arc<AtomicBool>) -> Result<(), String> {
    if !has_nvidia_driver() {
        crate::log_debug("CUDA runtime: no NVIDIA driver detected, skipping runtime download");
        return Ok(());
    }

    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .ok_or_else(|| "cannot resolve executable directory".to_string())?;

    if cuda_runtime_present(&exe_dir) {
        crate::log_debug("CUDA runtime: required DLLs already present next to executable");
        return Ok(());
    }

    fs::create_dir_all(cuda_dir).map_err(|e| format!("create cuda cache dir failed: {e}"))?;
    let zip_path = cuda_dir.join(CUDA_RUNTIME_ZIP_NAME);
    if !cuda_runtime_present(cuda_dir) {
        crate::log_debug(&format!(
            "CUDA runtime: downloading runtime zip from {}",
            CUDA_RUNTIME_ZIP_URL
        ));
        download_to_part_file(CUDA_RUNTIME_ZIP_URL, &zip_path, cancel)?;
        extract_cuda_runtime_zip(&zip_path, cuda_dir)?;
    }

    copy_cuda_dlls_from_dir(cuda_dir, &exe_dir)?;
    crate::log_debug(&format!(
        "CUDA runtime: runtime DLLs ready in {}",
        exe_dir.display()
    ));
    Ok(())
}

fn read_wav_as_mono_f32_16k(path: &Path) -> Result<Vec<f32>, String> {
    let reader = hound::WavReader::open(path).map_err(|e| format!("wav open failed: {e}"))?;
    let spec = reader.spec();
    let channels = spec.channels.max(1) as usize;
    let src_rate = spec.sample_rate.max(1);

    let mut interleaved = Vec::<f32>::new();
    match spec.sample_format {
        hound::SampleFormat::Float => {
            let samples = reader.into_samples::<f32>();
            for s in samples {
                interleaved.push(s.map_err(|e| format!("wav sample failed: {e}"))?);
            }
        }
        hound::SampleFormat::Int => {
            if spec.bits_per_sample <= 16 {
                let reader =
                    hound::WavReader::open(path).map_err(|e| format!("wav reopen failed: {e}"))?;
                let samples = reader.into_samples::<i16>();
                for s in samples {
                    let v = s.map_err(|e| format!("wav sample failed: {e}"))? as f32 / 32768.0;
                    interleaved.push(v);
                }
            } else {
                let reader =
                    hound::WavReader::open(path).map_err(|e| format!("wav reopen failed: {e}"))?;
                let samples = reader.into_samples::<i32>();
                let denom = ((1_i64 << (spec.bits_per_sample - 1)) - 1) as f32;
                for s in samples {
                    let v = s.map_err(|e| format!("wav sample failed: {e}"))? as f32 / denom;
                    interleaved.push(v);
                }
            }
        }
    }

    let mut mono = Vec::<f32>::new();
    if channels == 1 {
        mono = interleaved;
    } else {
        for frame in interleaved.chunks(channels) {
            let sum: f32 = frame.iter().copied().sum();
            mono.push(sum / channels as f32);
        }
    }

    if src_rate == 16_000 {
        return Ok(mono);
    }

    let out_len = ((mono.len() as u64) * 16_000u64 / src_rate as u64) as usize;
    let mut out = Vec::with_capacity(out_len.max(1));
    for i in 0..out_len {
        let src_pos = (i as f64) * (src_rate as f64) / 16_000.0;
        let p0 = src_pos.floor() as usize;
        let p1 = (p0 + 1).min(mono.len().saturating_sub(1));
        let frac = (src_pos - p0 as f64) as f32;
        let a = *mono.get(p0).unwrap_or(&0.0);
        let b = *mono.get(p1).unwrap_or(&a);
        out.push(a + (b - a) * frac);
    }
    Ok(out)
}

fn create_whisper_context(model_path: &Path) -> Result<WhisperContext, String> {
    let path = model_path
        .to_str()
        .ok_or_else(|| "invalid model path".to_string())?;

    let mut params = WhisperContextParameters::default();
    #[cfg(feature = "whisper_cuda")]
    {
        params.use_gpu(true);
        crate::log_debug("Whisper backend: CUDA feature enabled, trying GPU first");
    }
    #[cfg(not(feature = "whisper_cuda"))]
    {
        params.use_gpu(false);
        crate::log_debug("Whisper backend: CUDA feature not enabled in this build, using CPU");
    }

    match WhisperContext::new_with_params(path, params) {
        Ok(ctx) => {
            #[cfg(feature = "whisper_cuda")]
            crate::log_debug("Whisper backend: GPU context initialized");
            #[cfg(not(feature = "whisper_cuda"))]
            crate::log_debug("Whisper backend: CPU context initialized");
            Ok(ctx)
        }
        Err(err) => {
            #[cfg(feature = "whisper_cuda")]
            {
                crate::log_debug(&format!(
                    "Whisper backend: GPU initialization failed, falling back to CPU: {err}"
                ));
                let mut cpu_params = WhisperContextParameters::default();
                cpu_params.use_gpu(false);
                match WhisperContext::new_with_params(path, cpu_params) {
                    Ok(ctx) => {
                        crate::log_debug("Whisper backend: CPU fallback context initialized");
                        Ok(ctx)
                    }
                    Err(cpu_err) => Err(format!(
                        "load model failed (gpu: {err}; cpu fallback: {cpu_err})"
                    )),
                }
            }
            #[cfg(not(feature = "whisper_cuda"))]
            {
                Err(format!("load model failed: {err}"))
            }
        }
    }
}

pub fn transcribe_wav(
    model_path: &Path,
    wav_path: &Path,
    include_timestamps: bool,
    cancel: &Arc<AtomicBool>,
    progress_callback: Option<Box<dyn FnMut(i32)>>,
) -> Result<String, String> {
    let _power_awake = crate::power_awake::acquire("media-transcription");
    if cancel.load(Ordering::Relaxed) {
        return Err("cancelled".to_string());
    }

    let input = read_wav_as_mono_f32_16k(wav_path)?;
    if input.is_empty() {
        return Err("empty audio input".to_string());
    }

    let ctx = create_whisper_context(model_path)?;
    let mut state = ctx
        .create_state()
        .map_err(|e| format!("create whisper state failed: {e}"))?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    let threads = std::thread::available_parallelism()
        .map(|n| n.get() as i32)
        .unwrap_or(4)
        .clamp(1, 8);
    params.set_n_threads(threads);
    params.set_print_progress(false);
    params.set_print_special(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_translate(false);
    params.set_no_timestamps(!include_timestamps);
    params.set_progress_callback_safe::<Option<Box<dyn FnMut(i32)>>, Box<dyn FnMut(i32)>>(
        progress_callback,
    );
    let cancel_for_abort = Arc::clone(cancel);
    let abort_callback: Box<dyn FnMut() -> bool> =
        Box::new(move || cancel_for_abort.load(Ordering::Relaxed));
    params.set_abort_callback_safe::<Option<Box<dyn FnMut() -> bool>>, Box<dyn FnMut() -> bool>>(
        Some(abort_callback),
    );

    state
        .full(params, &input)
        .map_err(|e| format!("whisper inference failed: {e}"))?;

    if cancel.load(Ordering::Relaxed) {
        return Err("cancelled".to_string());
    }

    let count = state
        .full_n_segments()
        .map_err(|e| format!("read segment count failed: {e}"))?;
    let mut out = String::new();
    for i in 0..count {
        let txt = state
            .full_get_segment_text(i)
            .map_err(|e| format!("read segment text failed: {e}"))?;
        let line = if include_timestamps {
            let t0 = state
                .full_get_segment_t0(i)
                .map_err(|e| format!("read segment t0 failed: {e}"))?;
            let t1 = state
                .full_get_segment_t1(i)
                .map_err(|e| format!("read segment t1 failed: {e}"))?;
            format!("[{} - {}] {}", format_ts(t0), format_ts(t1), txt.trim())
        } else {
            txt.trim().to_string()
        };
        if !line.is_empty() {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&line);
        }
    }
    Ok(out)
}

fn format_ts(tick_10ms: i64) -> String {
    let total_ms = tick_10ms.max(0) as u64 * 10;
    let h = total_ms / 3_600_000;
    let m = (total_ms % 3_600_000) / 60_000;
    let s = (total_ms % 60_000) / 1_000;
    let ms = total_ms % 1_000;
    format!("{h:02}:{m:02}:{s:02}.{ms:03}")
}
