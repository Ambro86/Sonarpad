//! Normalize each kept source interval to a compatible video stream using DLLs.
use super::{check_cancel, video};
use crate::media_cutter_model::MediaPart;
use crate::media_cutter_timeline::SourceClip;
use std::{path::Path, sync::atomic::AtomicBool};

fn dimensions(path: &Path) -> Result<(i32, i32), String> {
    use crate::ffmpeg_dyn::*;
    let api = crate::ffmpeg_source::ffmpeg_api()?;
    let name =
        std::ffi::CString::new(path.to_string_lossy().as_bytes()).map_err(|e| e.to_string())?;
    let mut context = std::ptr::null_mut();
    unsafe {
        let result = (|| {
            let ret = (api.avformat_open_input)(
                &mut context,
                name.as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            if ret < 0 {
                return Err(crate::ffmpeg_source::ffmpeg_err(api, ret));
            }
            let ret = (api.avformat_find_stream_info)(context, std::ptr::null_mut());
            if ret < 0 {
                return Err(crate::ffmpeg_source::ffmpeg_err(api, ret));
            }
            let index = (api.av_find_best_stream)(
                context,
                AVMediaType_AVMEDIA_TYPE_VIDEO,
                -1,
                -1,
                std::ptr::null_mut(),
                0,
            );
            if index < 0 {
                return Err("No video stream".into());
            }
            let params = (*(*(*context).streams.add(index as usize))).codecpar;
            Ok((((*params).width + 1) & !1, ((*params).height + 1) & !1))
        })();
        (api.avformat_close_input)(&mut context);
        result
    }
}

pub(super) fn prepare(
    manifest: &Path,
    clips: &[SourceClip],
    rotation: u16,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u32),
) -> Result<(), String> {
    let work = manifest.parent().ok_or("Missing video workspace")?;
    let mut videos = Vec::new();
    let mut size = None;
    for clip in clips {
        check_cancel(cancel)?;
        let has_video = crate::ffmpeg_source::has_real_video_stream(&clip.path)?;
        if has_video && size.is_none() {
            size = Some(dimensions(&clip.path)?);
        }
        videos.push(has_video);
    }
    let size = size.unwrap_or((640, 360));
    let output_size = if rotation == 90 || rotation == 270 {
        (size.1, size.0)
    } else {
        size
    };
    // Small black BMP, resized by the library. No generated executable or external process.
    let black = work.join("black.bmp");
    let mut bitmap = vec![0_u8; 70];
    bitmap[..2].copy_from_slice(b"BM");
    bitmap[2..6].copy_from_slice(&70_u32.to_le_bytes());
    bitmap[10..14].copy_from_slice(&54_u32.to_le_bytes());
    bitmap[14..18].copy_from_slice(&40_u32.to_le_bytes());
    bitmap[18..22].copy_from_slice(&2_i32.to_le_bytes());
    bitmap[22..26].copy_from_slice(&2_i32.to_le_bytes());
    bitmap[26..28].copy_from_slice(&1_u16.to_le_bytes());
    bitmap[28..30].copy_from_slice(&24_u16.to_le_bytes());
    std::fs::write(&black, bitmap).map_err(|e| e.to_string())?;
    let total: f64 = clips.iter().map(|c| c.end - c.start).sum();
    let mut done = 0.0;
    let mut list = String::from("ffconcat version 1.0\n");
    for (index, clip) in clips.iter().enumerate() {
        check_cancel(cancel)?;
        let name = format!("clip{index}.mp4");
        let output = work.join(&name);
        let length = clip.end - clip.start;
        let mut report =
            |pct| progress(((done + length * pct as f64 / 100.0) / total * 100.0) as u32);
        if videos[index] {
            video::encode_video_sized(
                &clip.path,
                &output,
                &[MediaPart {
                    start: clip.start,
                    end: clip.end,
                    keep: true,
                }],
                rotation,
                Some(size),
                cancel,
                &mut report,
            )?;
        } else {
            crate::media_conversion::encode_video_sized(
                &black,
                &output,
                Some(length),
                Some(output_size),
                cancel,
                &mut report,
            )?;
        }
        list.push_str(&format!("file '{name}'\nduration {length:.9}\n"));
        done += length;
    }
    std::fs::write(manifest, list).map_err(|e| e.to_string())?;
    Ok(())
}
