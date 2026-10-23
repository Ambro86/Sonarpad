use crate::{
    accessibility::{screen_reader_speak, to_wide},
    media_cutter_export::{self, AddedTrack, ExportPlan},
    media_cutter_model::*,
    settings::Language,
};
use std::{
    os::windows::process::CommandExt,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
use windows::{
    Win32::{
        Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM},
        Graphics::Gdi::{COLOR_WINDOW, HBRUSH},
        UI::{
            Controls::{Dialogs::*, PBM_SETPOS, PBM_SETRANGE, PROGRESS_CLASSW},
            Input::KeyboardAndMouse::{
                EnableWindow, GetFocus, IsWindowEnabled, SetFocus, VK_CONTROL, VK_DOWN, VK_ESCAPE,
                VK_LEFT, VK_MENU, VK_RIGHT, VK_SHIFT, VK_SPACE, VK_UP,
            },
            WindowsAndMessaging::*,
        },
    },
    core::{PCWSTR, PWSTR},
};
const CLASS: &str = "SonarpadMediaCutter";
const OPEN: usize = 9801;
const MODE: usize = 9802;
const MOVE: usize = 9803;
const BACK: usize = 9804;
const PLAY: usize = 9805;
const NEXT: usize = 9806;
const POSITION: usize = 9807;
const GO: usize = 9808;
const PRIMARY: usize = 9809;
const LISTEN: usize = 9810;
const PARTS: usize = 9811;
const SPLIT: usize = 9812;
const DELETE: usize = 9813;
const RESTORE: usize = 9814;
const START: usize = 9815;
const END: usize = 9816;
const PRECISION: usize = 9818;
const START_BACK: usize = 9819;
const START_FORWARD: usize = 9820;
const END_BACK: usize = 9821;
const END_FORWARD: usize = 9822;
const ROTATE: usize = 9823;
const VIDEO: usize = 9824;
const TRACK: usize = 9825;
const REMOVE_TRACK: usize = 9826;
const ORIGINAL_GAIN: usize = 9827;
const ADDED_GAIN: usize = 9828;
const LOOP: usize = 9829;
const SAVE: usize = 9830;
const CLOSE: usize = 9831;
const CANCEL: usize = 9832;
const SHORTCUT_START: usize = 9833;
const SHORTCUT_END: usize = 9834;
const SHORTCUT_APPLY: usize = 9835;
const PART_PREVIOUS: usize = 9836;
const PART_NEXT: usize = 9837;
const COMMAND_AREA: usize = 9838;
const COMMAND_HELP: usize = 9839;
const BACK_FIVE: usize = 9840;
const NEXT_FIVE: usize = 9841;
const BACK_TEN: usize = 9842;
const NEXT_TEN: usize = 9843;
const FILE_HOME: usize = 9844;
const FILE_END: usize = 9845;
const INSERT_MEDIA: usize = 9846;
const TICK: usize = 1;
const WM_RESTORE_CUTTER_FOCUS: u32 = WM_APP + 212;
#[derive(Default)]
struct Job {
    percent: u32,
    result: Option<Result<(), String>>,
}
struct Player {
    child: Child,
    stop: Arc<AtomicBool>,
    position: Arc<AtomicU64>,
    ranges: Vec<MediaPart>,
}
impl Player {
    fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Err(e) = self.child.kill() {
            crate::log_debug(&format!("Cutter player stop: {e}"));
        }
        if let Err(e) = self.child.wait() {
            crate::log_debug(&format!("Cutter player wait: {e}"));
        }
    }
    fn position(&self) -> f64 {
        kept_position(
            &self.ranges,
            f64::from_bits(self.position.load(Ordering::Relaxed)),
        )
    }
}
impl Drop for Player {
    fn drop(&mut self) {
        self.stop();
    }
}
struct State {
    hwnd: HWND,
    parent: HWND,
    language: Language,
    controls: Vec<HWND>,
    last_focus: HWND,
    advanced_controls: Vec<HWND>,
    track_controls: Vec<HWND>,
    status: HWND,
    progress: HWND,
    cancel_button: HWND,
    input: Option<PathBuf>,
    duration: f64,
    has_video: bool,
    parts: Vec<MediaPart>,
    sources: Vec<crate::media_cutter_timeline::MediaSpan>,
    position: f64,
    start: Option<f64>,
    end: Option<f64>,
    dirty: bool,
    track: Option<PathBuf>,
    player: Option<Player>,
    job: Option<Arc<Mutex<Job>>>,
    cancel: Arc<AtomicBool>,
    last_percent: u32,
    close_after_export: bool,
}
impl State {
    fn remember_focus(&mut self) {
        unsafe {
            let mut focus = GetFocus();
            while focus.0 != 0 && IsChild(self.hwnd, focus).as_bool() {
                if GetWindowLongW(focus, GWL_STYLE) as u32 & WS_TABSTOP.0 != 0 {
                    self.last_focus = focus;
                    break;
                }
                focus = GetParent(focus);
            }
        }
    }
    fn restore_focus(&self) {
        unsafe {
            if GetForegroundWindow() != self.hwnd || !IsWindowEnabled(self.hwnd).as_bool() {
                return;
            }
            let target = [
                self.last_focus,
                self.cancel_button,
                self.ctrl(OPEN),
                self.ctrl(CLOSE),
            ]
            .into_iter()
            .find(|&control| {
                control.0 != 0
                    && IsChild(self.hwnd, control).as_bool()
                    && IsWindowVisible(control).as_bool()
                    && IsWindowEnabled(control).as_bool()
            });
            if let Some(target) = target {
                SetFocus(target);
                crate::log_debug(&format!(
                    "Media cutter focus restored: control={}",
                    GetDlgCtrlID(target)
                ));
            }
        }
    }
    fn tr(&self, key: &str) -> String {
        crate::i18n::tr(self.language, &format!("media_cutter.{key}"))
    }
    fn ctrl(&self, id: usize) -> HWND {
        unsafe { GetDlgItem(self.hwnd, id as i32) }
    }
    fn text(&self, id: usize) -> String {
        unsafe {
            let h = self.ctrl(id);
            let n = GetWindowTextLengthW(h);
            let mut b = vec![0u16; n as usize + 1];
            let n = GetWindowTextW(h, &mut b);
            String::from_utf16_lossy(&b[..n as usize])
        }
    }
    fn set(&self, id: usize, text: &str) {
        unsafe {
            crate::log_if_err!(SetWindowTextW(
                self.ctrl(id),
                PCWSTR(to_wide(text).as_ptr())
            ));
        }
    }
    fn selected(&self, id: usize) -> usize {
        unsafe {
            SendMessageW(self.ctrl(id), CB_GETCURSEL, WPARAM(0), LPARAM(0))
                .0
                .max(0) as usize
        }
    }
    fn checked(&self, id: usize) -> bool {
        unsafe { SendMessageW(self.ctrl(id), BM_GETCHECK, WPARAM(0), LPARAM(0)).0 == 1 }
    }
    fn say(&self, text: &str) {
        screen_reader_speak(text);
    }
    fn number(&self, id: usize) -> Result<f64, String> {
        self.text(id)
            .trim()
            .replace(',', ".")
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
            .ok_or_else(|| self.tr("invalid_cut"))
    }
    fn stop(&mut self) {
        if let Some(p) = self.player.as_ref() {
            self.position = p.position().min(self.duration);
        }
        self.player = None;
        self.set(PLAY, &self.tr("play"));
    }
    fn current(&self) -> f64 {
        self.player
            .as_ref()
            .map_or(self.position, Player::position)
            .min(self.duration)
    }
    fn announce_part(&self) {
        let index = self.selected(PARTS);
        if let Some(p) = self.parts.get(index) {
            self.say(&format!(
                "{} {}: {} – {} {}",
                self.tr("part_singular"),
                index + 1,
                spoken_time(p.start, self.language),
                spoken_time(p.end, self.language),
                if p.keep {
                    String::new()
                } else {
                    self.tr("part_deleted_marker")
                }
            ));
        }
    }
    fn refresh_parts(&self) {
        unsafe {
            SendMessageW(self.ctrl(PARTS), CB_RESETCONTENT, WPARAM(0), LPARAM(0));
            for (i, p) in self.parts.iter().enumerate() {
                let label = format!(
                    "{} {}: {} – {} {}",
                    self.tr("part_singular"),
                    i + 1,
                    spoken_time(p.start, self.language),
                    spoken_time(p.end, self.language),
                    if p.keep {
                        String::new()
                    } else {
                        self.tr("part_deleted_marker")
                    }
                );
                SendMessageW(
                    self.ctrl(PARTS),
                    CB_ADDSTRING,
                    WPARAM(0),
                    LPARAM(to_wide(&label).as_ptr() as isize),
                );
            }
            SendMessageW(self.ctrl(PARTS), CB_SETCURSEL, WPARAM(0), LPARAM(0));
        }
        self.show_bounds();
    }
    fn show_bounds(&self) {
        let bounds = if self.selected(MODE) == 0 {
            self.start.zip(self.end)
        } else {
            self.parts
                .get(self.selected(PARTS))
                .map(|p| (p.start, p.end))
        };
        if let Some((a, b)) = bounds {
            self.set(START, &format!("{a:.3}"));
            self.set(END, &format!("{b:.3}"));
        }
    }
    fn visibility(&self) {
        unsafe {
            for h in &self.track_controls {
                ShowWindow(
                    *h,
                    if self.track.is_some() && self.job.is_none() {
                        SW_SHOW
                    } else {
                        SW_HIDE
                    },
                );
            }
            let advanced = self.selected(MODE) == 1;
            let pending_cut =
                !advanced && self.start.is_some() && self.end.is_some() && self.job.is_none();
            for id in [START_BACK, START_FORWARD, END_BACK, END_FORWARD] {
                ShowWindow(self.ctrl(id), if pending_cut { SW_SHOW } else { SW_HIDE });
            }
            for h in &self.advanced_controls {
                ShowWindow(*h, if advanced { SW_SHOW } else { SW_HIDE });
            }
            ShowWindow(self.ctrl(PRIMARY), if advanced { SW_HIDE } else { SW_SHOW });
            EnableWindow(self.ctrl(ROTATE), self.has_video);
            EnableWindow(self.ctrl(VIDEO), self.has_video);
        }
    }
    fn audition_bounds(&self) -> Result<(f64, f64), String> {
        let bounds = if self.selected(MODE) == 0 {
            self.start
                .zip(self.end)
                .ok_or_else(|| self.tr("invalid_cut"))?
        } else {
            let p = self
                .parts
                .get(self.selected(PARTS))
                .ok_or_else(|| self.tr("invalid_cut"))?;
            if !p.keep {
                return Err(self.tr("part_deleted"));
            }
            (p.start, p.end)
        };
        Ok(bounds)
    }
    fn playback_ranges(&self, audition: bool) -> Result<Vec<MediaPart>, String> {
        Ok(if audition {
            let (a, b) = self.audition_bounds()?;
            vec![MediaPart {
                start: a,
                end: b,
                keep: true,
            }]
        } else {
            let mut ranges = remaining_kept_ranges(&self.parts, self.position);
            if ranges.is_empty() {
                ranges = remaining_kept_ranges(&self.parts, 0.0);
            }
            ranges
        })
    }
    fn play(&mut self, audition: bool) -> Result<(), String> {
        self.stop();
        let path = self.input.as_ref().ok_or_else(|| self.tr("no_file"))?;
        let ranges = self.playback_ranges(audition)?;
        if ranges.is_empty() {
            return Err(self.tr("nothing_to_save"));
        }
        let mpv = crate::ensure_mpv_runtime_available(self.hwnd)?;
        let ipc = PathBuf::from(format!(
            r"\\.\pipe\sonarpad-cutter-{}",
            uuid::Uuid::new_v4()
        ));
        let child = Command::new(mpv)
            .args([
                "--no-config",
                "--no-terminal",
                "--keep-open=no",
                "--audio-display=no",
            ])
            .arg(if self.checked(VIDEO) {
                "--force-window=yes"
            } else {
                "--vid=no"
            })
            .arg(format!(
                "--video-rotate={}",
                [0, 90, 270, 180][self.selected(ROTATE).min(3)]
            ))
            // Mixed EDLs may start with audio before the first video frame. Do not discard it.
            .args(if self.sources.is_empty() {
                vec![]
            } else {
                vec!["--initial-audio-sync=no"]
            })
            .arg(format!("--input-ipc-server={}", ipc.display()))
            .arg(crate::media_cutter_timeline::playback_edl(
                path,
                &self.sources,
                &ranges,
            )?)
            .creation_flags(0x08000000)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        let stop = Arc::new(AtomicBool::new(false));
        let position = Arc::new(AtomicU64::new(0.0_f64.to_bits()));
        let stop_worker = stop.clone();
        let pos_worker = position.clone();
        std::thread::spawn(move || {
            while !stop_worker.load(Ordering::Relaxed) {
                if let Ok(mut pipe) = crate::open_mpv_ipc_pipe(&ipc)
                    && let Ok(value) = crate::send_mpv_ipc_request_with_id(
                        &ipc,
                        &mut pipe,
                        r#"{"command":["get_property","time-pos"]}"#,
                        1,
                    )
                    && let Some(p) = value.get("data").and_then(|v| v.as_f64())
                {
                    pos_worker.store(p.to_bits(), Ordering::Relaxed);
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        });
        self.position = ranges[0].start;
        self.player = Some(Player {
            child,
            stop,
            position,
            ranges,
        });
        self.set(PLAY, &self.tr("pause"));
        Ok(())
    }
    fn confirm(&self, key: &str) -> bool {
        crate::watchdog::enter_modal_dialog();
        let accepted = unsafe {
            MessageBoxW(
                self.hwnd,
                PCWSTR(to_wide(&self.tr(key)).as_ptr()),
                PCWSTR(to_wide(&self.tr("unsaved_title")).as_ptr()),
                MB_YESNO | MB_ICONQUESTION | MB_DEFBUTTON2,
            ) == IDYES
        };
        crate::watchdog::exit_modal_dialog();
        accepted
    }
    fn accept_close(&mut self, confirmed: bool) -> bool {
        if !confirmed {
            return false;
        }
        if self.job.is_some() {
            self.close_after_export = true;
            self.cancel.store(true, Ordering::Relaxed);
            unsafe {
                EnableWindow(self.cancel_button, false);
            }
            crate::log_debug("Media cutter close confirmed: waiting for export cancellation");
            return false;
        }
        true
    }
    fn insert_media(&mut self, path: PathBuf) -> Result<(), String> {
        let input = self.input.clone().ok_or_else(|| self.tr("no_file"))?;
        let index = self.selected(PARTS);
        if index >= self.parts.len() {
            return Err(self.tr("invalid_cut"));
        }
        let length = crate::ffmpeg_export::media_duration_seconds(&path)
            .filter(|d| d.is_finite() && *d >= MIN_PART_SECONDS)
            .ok_or_else(|| self.tr("invalid_media"))?;
        let video = crate::ffmpeg_source::has_real_video_stream(&path)?;
        if !video && crate::ffmpeg_source::list_audio_streams(&path)?.is_empty() {
            return Err(self.tr("invalid_media"));
        }
        let mut sources = self.sources.clone();
        if sources.is_empty() {
            sources.push(crate::media_cutter_timeline::MediaSpan {
                path: input,
                start: 0.0,
                end: self.duration,
                source_start: 0.0,
            });
        }
        let duration = crate::media_cutter_timeline::insert_before(
            &mut self.parts,
            &mut sources,
            index,
            path,
            length,
        )?;
        self.sources = sources;
        self.duration = duration;
        self.has_video |= video;
        self.position = self.parts[index].start;
        self.dirty = true;
        self.start = None;
        self.end = None;
        self.refresh_parts();
        unsafe {
            SendMessageW(self.ctrl(PARTS), CB_SETCURSEL, WPARAM(index), LPARAM(0));
            SetFocus(self.ctrl(PARTS));
        }
        self.set(POSITION, &format!("{:.3}", self.position));
        self.show_bounds();
        self.visibility();
        self.say(&format!(
            "{} {} {}: {} – {}",
            self.tr("media_inserted"),
            self.tr("part_singular"),
            index + 1,
            spoken_time(self.parts[index].start, self.language),
            spoken_time(self.parts[index].end, self.language)
        ));
        Ok(())
    }
    fn command(&mut self, id: usize, notification: u32) -> Result<(), String> {
        if self.job.is_some() {
            if id == CANCEL {
                self.cancel.store(true, Ordering::Relaxed);
                unsafe {
                    EnableWindow(self.cancel_button, false);
                }
            }
            return Ok(());
        }
        match id {
            SHORTCUT_START | SHORTCUT_END | SHORTCUT_APPLY => {
                let correct_step = match id {
                    SHORTCUT_START => self.start.is_none(),
                    SHORTCUT_END => self.start.is_some() && self.end.is_none(),
                    SHORTCUT_APPLY => self.start.is_some() && self.end.is_some(),
                    _ => false,
                };
                // Each shortcut has one meaning: repeated Alt+1 must never set the end or apply a cut.
                if self.selected(MODE) == 0 && correct_step {
                    self.command(PRIMARY, BN_CLICKED)?;
                }
            }
            OPEN => {
                self.stop();
                if (self.dirty || self.start.is_some()) && !self.confirm("unsaved_message") {
                    return Ok(());
                }
                if let Some(path) = choose(self.hwnd, false, false, &self.tr("open_file"), None) {
                    let duration = crate::ffmpeg_export::media_duration_seconds(&path)
                        .filter(|d| d.is_finite() && *d > 0.0)
                        .ok_or_else(|| self.tr("invalid_media"))?;
                    let has_video = crate::ffmpeg_source::has_real_video_stream(&path)?;
                    if !has_video && crate::ffmpeg_source::list_audio_streams(&path)?.is_empty() {
                        return Err(self.tr("invalid_media"));
                    }
                    let loaded = format!(
                        "{}: {} — {duration:.3} s",
                        self.tr("file_loaded"),
                        path.file_name().unwrap_or_default().to_string_lossy()
                    );
                    self.sources.clear();
                    self.input = Some(path);
                    self.duration = duration;
                    self.has_video = has_video;
                    crate::log_debug(&format!(
                        "Media cutter input probe: extension={:?} has_video={has_video}",
                        self.input.as_ref().and_then(|p| p.extension())
                    ));
                    self.parts = vec![MediaPart {
                        start: 0.0,
                        end: duration,
                        keep: true,
                    }];
                    self.position = 0.0;
                    self.start = None;
                    self.end = None;
                    self.track = None;
                    self.dirty = false;
                    self.set(PRIMARY, &self.tr("set_start"));
                    self.set(TRACK, &self.tr("choose_track"));
                    self.refresh_parts();
                    self.visibility();
                    self.set(POSITION, "0.000");
                    self.set(START, "0.000");
                    self.set(END, "0.000");
                    unsafe {
                        crate::log_if_err!(SetWindowTextW(
                            self.status,
                            PCWSTR(to_wide(&loaded).as_ptr())
                        ));
                    }
                    self.say(&format!("{loaded}. {}", self.tr("guide_start")));
                }
            }
            MODE if notification == CBN_SELCHANGE => {
                if self.start.is_some() && !self.confirm("discard_pending_cut") {
                    unsafe {
                        SendMessageW(self.ctrl(MODE), CB_SETCURSEL, WPARAM(0), LPARAM(0));
                    }
                    return Ok(());
                }
                self.start = None;
                self.end = None;
                self.set(PRIMARY, &self.tr("set_start"));
                self.visibility();
                self.show_bounds();
            }
            PLAY => {
                if self.player.is_some() {
                    self.stop();
                } else {
                    self.play(false)?;
                }
            }
            BACK | NEXT | BACK_FIVE | NEXT_FIVE | BACK_TEN | NEXT_TEN => {
                let running = self.player.is_some();
                self.stop();
                let step = match id {
                    BACK_FIVE | NEXT_FIVE => 5.0,
                    BACK_TEN | NEXT_TEN => 10.0,
                    _ => [1.0, 5.0, 10.0, 30.0, 60.0][self.selected(MOVE).min(4)],
                };
                self.position = kept_seek_position(
                    &self.parts,
                    self.position,
                    if matches!(id, BACK | BACK_FIVE | BACK_TEN) {
                        -step
                    } else {
                        step
                    },
                );
                self.set(POSITION, &format!("{:.3}", self.position));
                if running {
                    self.play(false)?;
                }
                self.say(&spoken_time(self.position, self.language));
            }
            FILE_HOME | FILE_END => {
                let running = self.player.is_some();
                self.stop();
                self.position = if id == FILE_HOME { 0.0 } else { self.duration };
                self.set(POSITION, &format!("{:.3}", self.position));
                if running && id == FILE_HOME && self.parts.iter().any(|p| p.keep) {
                    self.play(false)?;
                }
                self.say(&spoken_time(self.position, self.language));
            }
            GO => {
                let position = self.number(POSITION)?;
                let running = self.player.is_some();
                self.stop();
                self.position = position.clamp(0.0, self.duration);
                if running {
                    self.play(false)?;
                }
            }
            PRIMARY => {
                if self.input.is_none() {
                    return Err(self.tr("no_file"));
                }
                let pos = self.current();
                if self.start.is_none() {
                    self.start = Some(pos);
                    self.set(PRIMARY, &self.tr("set_end"));
                    self.say(&self.tr("guide_end"));
                } else if self.end.is_none() {
                    let first = self.start.unwrap_or(pos);
                    let (a, b) = (first.min(pos), first.max(pos));
                    if b - a < MIN_PART_SECONDS {
                        return Err(self.tr("invalid_cut"));
                    }
                    self.stop();
                    self.start = Some(a);
                    self.end = Some(b);
                    self.set(PRIMARY, &self.tr("apply_cut"));
                    self.show_bounds();
                    self.say(&self.tr("guide_review"));
                } else {
                    self.stop();
                    delete_range(
                        &mut self.parts,
                        self.start.unwrap_or(0.0),
                        self.end.unwrap_or(0.0),
                        self.duration,
                    );
                    self.start = None;
                    self.end = None;
                    self.dirty = true;
                    self.set(PRIMARY, &self.tr("set_start"));
                    self.refresh_parts();
                    self.say(&format!(
                        "{}. {}",
                        self.tr("cut_applied"),
                        self.tr("guide_start")
                    ));
                }
                self.visibility();
            }
            COMMAND_HELP => {
                let delete_keys = format!("D / {}", self.tr("delete_key"));
                let mut commands =
                    vec![("←", "back"), ("→", "forward"), ("3 / Enter", "listen_cut")];
                if self.selected(MODE) == 1 {
                    commands.extend([
                        ("↑ / ↓", "part_singular"),
                        ("S", "split_here"),
                        (delete_keys.as_str(), "delete"),
                        ("R", "restore"),
                    ]);
                } else {
                    commands.extend([("1", "set_start"), ("2", "set_end"), ("4", "apply_cut")]);
                }
                let play = self.tr("play");
                let space = play
                    .rsplit_once("(Ctrl+")
                    .map(|(_, key)| key.trim_end_matches(')'))
                    .unwrap_or("Space");
                let mut lines = vec![format!(
                    "{space}: {} / {}",
                    play.split(" (").next().unwrap_or(&play),
                    self.tr("pause").split(" (").next().unwrap_or("")
                )];
                for (key, label) in commands {
                    let label = self.tr(label);
                    lines.push(format!(
                        "{key}: {}",
                        label.split(" (").next().unwrap_or(&label)
                    ));
                }
                for (modifier, seconds) in [(self.tr("shift_key"), 5.0), ("Ctrl".into(), 10.0)] {
                    for (arrow, action) in [("←", "back"), ("→", "forward")] {
                        let action = self.tr(action);
                        lines.push(format!(
                            "{modifier}+{arrow}: {} {}",
                            action.split(" (").next().unwrap_or(&action),
                            spoken_time(seconds, self.language)
                        ));
                    }
                }
                lines.push(self.tr("home_end_help"));
                lines.push(self.tr("save"));
                self.say(&lines.join(". "));
            }
            COMMAND_AREA => self.command(LISTEN, BN_CLICKED)?,
            LISTEN => {
                if self.selected(MODE) == 1
                    && self
                        .parts
                        .get(self.selected(PARTS))
                        .is_some_and(|p| !p.keep)
                {
                    self.stop();
                    self.say(&self.tr("part_deleted"));
                } else {
                    self.play(true)?;
                }
            }
            INSERT_MEDIA if self.selected(MODE) == 1 => {
                self.stop();
                if self.input.is_none() {
                    return Err(self.tr("no_file"));
                }
                if let Some(path) = choose(self.hwnd, false, true, &self.tr("insert_media"), None) {
                    self.insert_media(path)?;
                }
            }
            SPLIT if self.selected(MODE) == 1 => {
                self.stop();
                if let Some(index) = split_part_at(&mut self.parts, self.position) {
                    self.dirty = true;
                    self.refresh_parts();
                    unsafe {
                        SendMessageW(self.ctrl(PARTS), CB_SETCURSEL, WPARAM(index), LPARAM(0));
                    }
                    self.show_bounds();
                    self.say(&self.tr("split_added"));
                }
            }
            PARTS if notification == CBN_SELCHANGE => self.show_bounds(),
            PART_PREVIOUS | PART_NEXT if self.selected(MODE) == 1 => {
                if !self.parts.is_empty() {
                    let current = self.selected(PARTS);
                    let index = if id == PART_PREVIOUS {
                        current.saturating_sub(1)
                    } else {
                        (current + 1).min(self.parts.len() - 1)
                    };
                    unsafe {
                        SendMessageW(self.ctrl(PARTS), CB_SETCURSEL, WPARAM(index), LPARAM(0));
                    }
                    self.show_bounds();
                    self.announce_part();
                }
            }
            DELETE | RESTORE if self.selected(MODE) == 1 => {
                self.stop();
                let index = self.selected(PARTS);
                if let Some(p) = self.parts.get_mut(index) {
                    p.keep = id == RESTORE;
                    self.dirty = true;
                    self.refresh_parts();
                    unsafe {
                        SendMessageW(self.ctrl(PARTS), CB_SETCURSEL, WPARAM(index), LPARAM(0));
                    }
                    self.show_bounds();
                    self.say(&self.tr(if id == RESTORE {
                        "part_restored"
                    } else {
                        "part_deleted"
                    }));
                }
            }
            START | END | START_BACK | START_FORWARD | END_BACK | END_FORWARD => {
                let automatic = id == START || id == END;
                let (mut a, mut b) = (self.number(START)?, self.number(END)?);
                let step = [0.01, 0.1, 0.5, 1.0][self.selected(PRECISION).min(3)];
                match id {
                    START_BACK => a -= step,
                    START_FORWARD => a += step,
                    END_BACK => b -= step,
                    END_FORWARD => b += step,
                    _ => {}
                }
                if a < 0.0 || b > self.duration || b - a < MIN_PART_SECONDS {
                    return Err(self.tr("invalid_cut"));
                }
                if self.selected(MODE) == 0 {
                    if self.start.is_none() || self.end.is_none() {
                        return Err(self.tr("invalid_cut"));
                    }
                    self.stop();
                    self.start = Some(a);
                    self.end = Some(b);
                    self.set(PRIMARY, &self.tr("apply_cut"));
                } else {
                    let index = self.selected(PARTS);
                    let mut proposed = self.parts.clone();
                    let p = proposed
                        .get(index)
                        .ok_or_else(|| self.tr("invalid_cut"))?
                        .clone();
                    if (a - p.start).abs() > 0.000001 {
                        adjust_part_boundary(
                            &mut proposed,
                            index,
                            true,
                            a - p.start,
                            self.duration,
                        );
                    }
                    let end = proposed[index].end;
                    if (b - end).abs() > 0.000001 {
                        adjust_part_boundary(&mut proposed, index, false, b - end, self.duration);
                    }
                    if (proposed[index].start - a).abs() > 0.001
                        || (proposed[index].end - b).abs() > 0.001
                    {
                        return Err(self.tr("invalid_cut"));
                    }
                    self.stop();
                    self.parts = proposed;
                    self.dirty = true;
                    if !automatic {
                        self.refresh_parts();
                        unsafe {
                            SendMessageW(self.ctrl(PARTS), CB_SETCURSEL, WPARAM(index), LPARAM(0));
                        }
                    }
                }
                if !automatic {
                    self.show_bounds();
                    self.say(&format!("{}: {a:.3} – {b:.3}", self.tr("adjusted")));
                }
                self.visibility();
            }
            ROTATE if notification == CBN_SELCHANGE => {
                self.stop();
                self.dirty = true;
            }
            VIDEO => {
                let running = self.player.is_some();
                self.stop();
                if running {
                    self.play(false)?;
                }
            }
            TRACK => {
                self.stop();
                if let Some(path) = choose(self.hwnd, false, false, &self.tr("choose_track"), None)
                {
                    if crate::ffmpeg_source::list_audio_streams(&path)?.is_empty() {
                        return Err(self.tr("invalid_media"));
                    }
                    let announcement = format!(
                        "{}: {}",
                        self.tr("track_added"),
                        path.file_name().unwrap_or_default().to_string_lossy()
                    );
                    self.track = Some(path);
                    self.dirty = true;
                    self.visibility();
                    self.say(&announcement);
                }
            }
            LOOP => {
                self.dirty |= self.track.is_some();
            }
            REMOVE_TRACK => {
                self.track = None;
                self.dirty = true;
                self.set(TRACK, &self.tr("choose_track"));
                unsafe {
                    SetFocus(self.ctrl(TRACK));
                }
                self.visibility();
            }
            SAVE => {
                self.stop();
                let input = self.input.clone().ok_or_else(|| self.tr("no_file"))?;
                if self.start.is_some() {
                    return Err(self.tr("guide_review"));
                }
                if kept_duration(&self.parts) < MIN_PART_SECONDS {
                    return Err(self.tr("nothing_to_save"));
                }
                let track = if let Some(path) = self.track.clone() {
                    let original_volume = self.number(ORIGINAL_GAIN)?;
                    let new_volume = self.number(ADDED_GAIN)?;
                    if !(0.0..=200.0).contains(&original_volume)
                        || !(0.0..=200.0).contains(&new_volume)
                    {
                        return Err(self.tr("invalid_cut"));
                    }
                    Some(AddedTrack {
                        path,
                        original_volume: original_volume as f32 / 100.0,
                        new_volume: new_volume as f32 / 100.0,
                        loop_track: self.checked(LOOP),
                    })
                } else {
                    None
                };
                let stem = input.file_stem().unwrap_or_default().to_string_lossy();
                let suggested =
                    input.with_file_name(format!("{stem} {}", self.tr("output_suffix")));
                if let Some(output) = choose(
                    self.hwnd,
                    true,
                    self.has_video,
                    &self.tr("save"),
                    Some(&suggested),
                ) {
                    let plan = ExportPlan {
                        input,
                        output,
                        parts: self.parts.clone(),
                        sources: self.sources.clone(),
                        rotation: [0, 90, 270, 180][self.selected(ROTATE).min(3)],
                        track,
                    };
                    self.cancel = Arc::new(AtomicBool::new(false));
                    let cancel = self.cancel.clone();
                    let job = Arc::new(Mutex::new(Job::default()));
                    self.job = Some(job.clone());
                    self.last_percent = u32::MAX;
                    unsafe {
                        // Transfer focus before hiding the focused Save button, so Windows
                        // does not activate the owner while the export is running.
                        ShowWindow(self.progress, SW_SHOW);
                        ShowWindow(self.cancel_button, SW_SHOW);
                        EnableWindow(self.cancel_button, true);
                        SetFocus(self.cancel_button);
                        for h in &self.controls {
                            ShowWindow(*h, SW_HIDE);
                        }
                        SendMessageW(self.progress, PBM_SETPOS, WPARAM(0), LPARAM(0));
                    }
                    crate::log_debug(&format!(
                        "Media cutter export started: kept_parts={} duration={:.3}s video={}",
                        plan.parts.iter().filter(|p| p.keep).count(),
                        kept_duration(&plan.parts),
                        self.has_video
                    ));
                    self.say(&self.tr("processing_started"));
                    std::thread::spawn(move || {
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            media_cutter_export::export(&plan, cancel, &mut |p| {
                                if let Ok(mut state) = job.lock() {
                                    state.percent = p;
                                }
                            })
                        }))
                        .unwrap_or_else(|_| Err("Media cutter export panicked".into()));
                        crate::log_debug(&format!("Media cutter export finished: {result:?}"));
                        if let Ok(mut state) = job.lock() {
                            state.result = Some(result);
                        }
                    });
                }
            }
            CLOSE => unsafe {
                crate::log_if_err!(PostMessageW(self.hwnd, WM_CLOSE, WPARAM(0), LPARAM(0)));
            },
            _ => {}
        }
        Ok(())
    }
    fn tick(&mut self) {
        if let Some(player) = self.player.as_mut() {
            match player.child.try_wait() {
                Ok(Some(_)) => {
                    self.position = player.ranges.last().map_or(self.duration, |p| p.end);
                    self.player = None;
                    self.set(PLAY, &self.tr("play"));
                }
                Err(e) => {
                    crate::log_debug(&format!("Cutter player: {e}"));
                    self.stop();
                }
                _ => {}
            }
        }
        if self.player.is_some()
            && unsafe { windows::Win32::UI::Input::KeyboardAndMouse::GetFocus() }
                != self.ctrl(POSITION)
        {
            self.set(POSITION, &format!("{:.3}", self.current()));
        }
        let snapshot = self
            .job
            .as_ref()
            .and_then(|j| j.lock().ok().map(|mut j| (j.percent, j.result.take())));
        if let Some((pct, result)) = snapshot {
            if pct != self.last_percent {
                self.last_percent = pct;
                unsafe {
                    SendMessageW(self.progress, PBM_SETPOS, WPARAM(pct as usize), LPARAM(0));
                    crate::log_if_err!(SetWindowTextW(
                        self.status,
                        PCWSTR(to_wide(&format!("{}: {pct}%", self.tr("processing"))).as_ptr())
                    ));
                }
            }
            if let Some(result) = result {
                self.job = None;
                if self.close_after_export {
                    self.dirty = false;
                    self.start = None;
                    self.end = None;
                    unsafe {
                        crate::log_if_err!(PostMessageW(self.hwnd, WM_CLOSE, WPARAM(0), LPARAM(0)));
                    }
                    return;
                }
                unsafe {
                    ShowWindow(self.progress, SW_HIDE);
                    ShowWindow(self.cancel_button, SW_HIDE);
                    for h in &self.controls {
                        ShowWindow(*h, SW_SHOW);
                    }
                    SetFocus(self.ctrl(SAVE));
                }
                self.visibility();
                match result {
                    Ok(()) => {
                        self.dirty = false;
                        self.say(&self.tr("saved"));
                        crate::show_info(self.hwnd, self.language, &self.tr("saved"));
                    }
                    Err(e) if self.cancel.load(Ordering::Relaxed) => {
                        crate::log_debug(&e);
                        self.say(&self.tr("processing_cancelled"));
                    }
                    Err(e) => crate::show_error(
                        self.hwnd,
                        self.language,
                        &format!("{}: {e}", self.tr("save_failed")),
                    ),
                }
                unsafe {
                    crate::log_if_err!(SetWindowTextW(
                        self.status,
                        PCWSTR(to_wide(&self.tr("ready")).as_ptr())
                    ));
                }
            }
        }
    }
}
fn choose(
    hwnd: HWND,
    save: bool,
    video: bool,
    title: &str,
    suggested: Option<&std::path::Path>,
) -> Option<PathBuf> {
    unsafe {
        let mut path = vec![0u16; 32768];
        if let Some(suggested) = suggested {
            let initial = to_wide(&suggested.to_string_lossy());
            let count = initial.len().min(path.len() - 1);
            path[..count].copy_from_slice(&initial[..count]);
        }
        let mut formats = Vec::new();
        if video {
            formats.extend_from_slice(media_cutter_export::VIDEO_EXTENSIONS);
        }
        formats.extend_from_slice(media_cutter_export::AUDIO_EXTENSIONS);
        let filter_text = if save {
            let mut text = String::new();
            for ext in &formats {
                text.push_str(&format!("{}\0*.{}\0", ext.to_uppercase(), ext));
            }
            text.push('\0');
            text
        } else {
            "Media\0*.mp3;*.m4a;*.wav;*.flac;*.ogg;*.opus;*.aac;*.mp4;*.mkv;*.avi;*.mov;*.webm;*.ts;*.m4v;*.wma;*.aiff\0All\0*.*\0\0".into()
        };
        let filter = to_wide(&filter_text);
        let title = to_wide(title);
        let ext = to_wide(if video { "mp4" } else { "m4a" });
        let mut ofn = OPENFILENAMEW {
            lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
            hwndOwner: hwnd,
            lpstrFilter: PCWSTR(filter.as_ptr()),
            nFilterIndex: 1,
            lpstrFile: PWSTR(path.as_mut_ptr()),
            nMaxFile: path.len() as u32,
            lpstrTitle: PCWSTR(title.as_ptr()),
            lpstrDefExt: PCWSTR(ext.as_ptr()),
            Flags: OFN_EXPLORER
                | OFN_PATHMUSTEXIST
                | OFN_NOCHANGEDIR
                | if save {
                    OFN_OVERWRITEPROMPT
                } else {
                    OFN_FILEMUSTEXIST
                },
            ..Default::default()
        };
        crate::watchdog::enter_modal_dialog();
        let accepted = (if save {
            GetSaveFileNameW(&mut ofn)
        } else {
            GetOpenFileNameW(&mut ofn)
        })
        .as_bool();
        crate::watchdog::exit_modal_dialog();
        if accepted {
            Some(PathBuf::from(String::from_utf16_lossy(
                &path[..path.iter().position(|v| *v == 0).unwrap_or(0)],
            )))
        } else {
            None
        }
    }
}
fn control(
    s: &mut State,
    class: &str,
    label: &str,
    id: usize,
    bounds: [i32; 4],
    style: WINDOW_STYLE,
) -> HWND {
    let [x, y, w, h] = bounds;
    unsafe {
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(to_wide(class).as_ptr()),
            PCWSTR(to_wide(label).as_ptr()),
            WS_CHILD | WS_VISIBLE | style,
            x,
            y,
            w,
            h,
            s.hwnd,
            HMENU(id as isize),
            HINSTANCE(crate::get_module_handle_raw_default()),
            None,
        );
        SendMessageW(
            hwnd,
            WM_SETFONT,
            WPARAM(
                windows::Win32::Graphics::Gdi::GetStockObject(
                    windows::Win32::Graphics::Gdi::DEFAULT_GUI_FONT,
                )
                .0 as usize,
            ),
            LPARAM(1),
        );
        s.controls.push(hwnd);
        hwnd
    }
}
fn button(s: &mut State, key: &str, id: usize, x: i32, y: i32, w: i32) -> HWND {
    let shortcut = matches!(id, BACK | PLAY | NEXT | PRIMARY | LISTEN | SAVE);
    control(
        s,
        "BUTTON",
        &s.tr(key),
        id,
        [x, y, w, if shortcut { 36 } else { 28 }],
        WS_TABSTOP
            | if shortcut {
                WINDOW_STYLE(BS_MULTILINE as u32)
            } else {
                WINDOW_STYLE(0)
            },
    )
}
fn combo(s: &mut State, key: &str, id: usize, values: &[String], x: i32, y: i32, w: i32) {
    control(s, "STATIC", &s.tr(key), 0, [x, y, w, 20], WINDOW_STYLE(0));
    let h = control(
        s,
        "COMBOBOX",
        "",
        id,
        [x, y + 22, w, 240],
        WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32) | WS_VSCROLL,
    );
    unsafe {
        for v in values {
            SendMessageW(
                h,
                CB_ADDSTRING,
                WPARAM(0),
                LPARAM(to_wide(v).as_ptr() as isize),
            );
        }
        SendMessageW(h, CB_SETCURSEL, WPARAM(0), LPARAM(0));
    }
}
fn edit(s: &mut State, key: &str, id: usize, value: &str, x: i32, y: i32, w: i32) {
    control(s, "STATIC", &s.tr(key), 0, [x, y, w, 20], WINDOW_STYLE(0));
    control(
        s,
        "EDIT",
        value,
        id,
        [x, y + 22, w, 24],
        WS_TABSTOP | WS_BORDER | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
    );
}
fn create(hwnd: HWND, parent: HWND) -> State {
    let language = crate::with_state(parent, |s| s.settings.language).unwrap_or_default();
    let mut s = State {
        last_focus: HWND(0),
        hwnd,
        parent,
        language,
        controls: vec![],
        advanced_controls: vec![],
        track_controls: vec![],
        status: HWND(0),
        progress: HWND(0),
        cancel_button: HWND(0),
        input: None,
        duration: 0.0,
        has_video: false,
        parts: vec![],
        sources: vec![],
        position: 0.0,
        start: None,
        end: None,
        dirty: false,
        track: None,
        player: None,
        job: None,
        cancel: Arc::new(AtomicBool::new(false)),
        last_percent: 0,
        close_after_export: false,
    };
    button(&mut s, "open_file", OPEN, 12, 12, 230);
    let modes = [s.tr("guided"), s.tr("advanced")];
    combo(&mut s, "mode", MODE, &modes, 12, 48, 250);
    combo(
        &mut s,
        "movement",
        MOVE,
        &[
            "1 s".into(),
            "5 s".into(),
            "10 s".into(),
            "30 s".into(),
            "60 s".into(),
        ],
        280,
        48,
        130,
    );
    button(&mut s, "back", BACK, 12, 105, 120);
    button(&mut s, "play", PLAY, 140, 105, 120);
    button(&mut s, "forward", NEXT, 268, 105, 120);
    let command_label = s.tr("command_area");
    control(
        &mut s,
        "BUTTON",
        &command_label,
        COMMAND_AREA,
        [430, 105, 345, 76],
        WS_TABSTOP | WINDOW_STYLE(BS_MULTILINE as u32),
    );
    edit(&mut s, "position", POSITION, "0.000", 430, 48, 180);
    button(&mut s, "go", GO, 620, 70, 100);
    button(&mut s, "set_start", PRIMARY, 12, 145, 200);
    button(&mut s, "listen_cut", LISTEN, 230, 145, 180);
    let first = s.controls.len();
    button(&mut s, "split_here", SPLIT, 12, 145, 200);
    let values: Vec<String> = vec![];
    combo(&mut s, "part_selection", PARTS, &values, 12, 183, 460);
    button(&mut s, "delete", DELETE, 490, 203, 105);
    button(&mut s, "restore", RESTORE, 605, 203, 170);
    button(&mut s, "insert_media", INSERT_MEDIA, 12, 240, 760);
    s.advanced_controls = s.controls[first..].to_vec();
    edit(&mut s, "start_value", START, "0.000", 12, 282, 180);
    edit(&mut s, "end_value", END, "0.000", 205, 282, 180);
    combo(
        &mut s,
        "precision",
        PRECISION,
        &[
            "0.01 s".into(),
            "0.1 s".into(),
            "0.5 s".into(),
            "1 s".into(),
        ],
        620,
        282,
        140,
    );
    for (key, id, x, width) in [
        ("start_back", START_BACK, 12, 180),
        ("start_forward", START_FORWARD, 205, 180),
        ("end_back", END_BACK, 400, 175),
        ("end_forward", END_FORWARD, 585, 175),
    ] {
        let label = s.tr(key);
        control(
            &mut s,
            "BUTTON",
            &label,
            id,
            [x, 340, width, 36],
            WS_TABSTOP | WINDOW_STYLE(BS_MULTILINE as u32),
        );
    }
    let rotations = [
        s.tr("rotation_none"),
        s.tr("rotation_right"),
        s.tr("rotation_left"),
        "180°".into(),
    ];
    combo(&mut s, "rotation", ROTATE, &rotations, 12, 380, 220);
    let label = s.tr("show_video");
    control(
        &mut s,
        "BUTTON",
        &label,
        VIDEO,
        [250, 402, 400, 25],
        WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
    );
    button(&mut s, "choose_track", TRACK, 12, 440, 470);
    let track_start = s.controls.len();
    button(&mut s, "track_remove", REMOVE_TRACK, 495, 440, 270);
    edit(
        &mut s,
        "original_volume",
        ORIGINAL_GAIN,
        "100",
        12,
        478,
        220,
    );
    edit(&mut s, "new_volume", ADDED_GAIN, "100", 245, 478, 220);
    let label = s.tr("loop_track");
    control(
        &mut s,
        "BUTTON",
        &label,
        LOOP,
        [12, 534, 750, 25],
        WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
    );
    s.track_controls = s.controls[track_start..].to_vec();
    button(&mut s, "save", SAVE, 12, 580, 200);
    button(&mut s, "close", CLOSE, 230, 580, 180);
    let label = s.tr("ready");
    s.status = control(
        &mut s,
        "STATIC",
        &label,
        0,
        [12, 625, 750, 24],
        WINDOW_STYLE(0),
    );
    s.controls.pop();
    unsafe {
        s.progress = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PROGRESS_CLASSW,
            PCWSTR::null(),
            WS_CHILD,
            12,
            70,
            750,
            28,
            hwnd,
            HMENU(0),
            HINSTANCE(crate::get_module_handle_raw_default()),
            None,
        );
        SendMessageW(s.progress, PBM_SETRANGE, WPARAM(0), LPARAM(100 << 16));
    }
    s.cancel_button = button(&mut s, "cancel_processing", CANCEL, 12, 110, 230);
    s.controls.pop();
    unsafe {
        ShowWindow(s.cancel_button, SW_HIDE);
    }
    s.visibility();
    s
}
fn spoken_time(seconds: f64, language: Language) -> String {
    // Same whole-second announcements and translated units as Sonarpad Mobile.
    let total = if seconds.is_finite() && seconds > 0.0 {
        (seconds as u64).max(1)
    } else {
        0
    };
    let mut units = Vec::new();
    for (unit, value) in [
        ("hour", total / 3600),
        ("minute", total % 3600 / 60),
        ("second", total % 60),
    ] {
        if value == 0 && (unit != "second" || !units.is_empty()) {
            continue;
        }
        let form = match language {
            Language::Polish => {
                if value == 1 {
                    "one"
                } else if (2..=4).contains(&(value % 10)) && !(12..=14).contains(&(value % 100)) {
                    "few"
                } else {
                    "many"
                }
            }
            Language::Czech => {
                if value == 1 {
                    "one"
                } else if (2..=4).contains(&value) {
                    "few"
                } else {
                    "many"
                }
            }
            Language::Russian | Language::Ukrainian | Language::Serbian => {
                if value % 10 == 1 && value % 100 != 11 {
                    "one"
                } else if (2..=4).contains(&(value % 10)) && !(12..=14).contains(&(value % 100)) {
                    "few"
                } else {
                    "many"
                }
            }
            Language::Lithuanian => {
                if value % 10 == 1 && value % 100 != 11 {
                    "one"
                } else if (2..=9).contains(&(value % 10)) && !(11..=19).contains(&(value % 100)) {
                    "few"
                } else {
                    "many"
                }
            }
            _ => {
                if value == 1 {
                    "one"
                } else {
                    "many"
                }
            }
        };
        units.push(format!(
            "{value} {}",
            crate::i18n::tr(language, &format!("media_cutter.duration_{unit}_{form}"))
        ));
    }
    if units.len() == 1 {
        return units.remove(0);
    }
    let last = units.pop().unwrap_or_default();
    format!(
        "{} {} {last}",
        units.join(", "),
        crate::i18n::tr(language, "media_cutter.duration_and")
    )
}

fn shortcut_command(
    key: u32,
    ctrl: bool,
    alt: bool,
    shift: bool,
    letter_reserved: bool,
) -> Option<usize> {
    if shift {
        return None;
    }
    if alt && !ctrl {
        return match key {
            0x31 => Some(SHORTCUT_START),
            0x32 => Some(SHORTCUT_END),
            0x33 => Some(LISTEN),
            0x34 => Some(SHORTCUT_APPLY),
            0x44 => Some(DELETE),
            0x52 => Some(RESTORE),
            key if key == VK_UP.0 as u32 => Some(PART_PREVIOUS),
            key if key == VK_DOWN.0 as u32 => Some(PART_NEXT),
            key if key == VK_LEFT.0 as u32 => Some(BACK),
            key if key == VK_RIGHT.0 as u32 => Some(NEXT),
            _ => None,
        };
    }
    if ctrl && !alt && key == 'S' as u32 {
        return Some(SAVE);
    }
    if ctrl && !alt && key == VK_SPACE.0 as u32 {
        return Some(PLAY);
    }
    if !ctrl && !alt && key == 'S' as u32 && !letter_reserved {
        return Some(SPLIT);
    }
    None
}

fn modified_area_command(
    key: u32,
    advanced: bool,
    ctrl: bool,
    alt: bool,
    shift: bool,
) -> Option<usize> {
    if alt || (ctrl && shift) {
        return None;
    }
    if ctrl || shift {
        return match (key, ctrl) {
            (0x25, false) => Some(BACK_FIVE),
            (0x27, false) => Some(NEXT_FIVE),
            (0x25, true) => Some(BACK_TEN),
            (0x27, true) => Some(NEXT_TEN),
            _ => None,
        };
    }
    area_command(key, advanced)
}

fn area_command(key: u32, advanced: bool) -> Option<usize> {
    match key {
        0x20 => Some(PLAY),
        0x25 => Some(BACK),
        0x27 => Some(NEXT),
        0x0d | 0x33 => Some(LISTEN),
        0x70 => Some(COMMAND_HELP),
        0x24 => Some(FILE_HOME),
        0x23 => Some(FILE_END),
        0x26 if advanced => Some(PART_PREVIOUS),
        0x28 if advanced => Some(PART_NEXT),
        0x53 if advanced => Some(SPLIT),
        0x44 | 0x2e if advanced => Some(DELETE),
        0x52 if advanced => Some(RESTORE),
        0x31 if !advanced => Some(SHORTCUT_START),
        0x32 if !advanced => Some(SHORTCUT_END),
        0x34 if !advanced => Some(SHORTCUT_APPLY),
        _ => None,
    }
}

fn reserves_letters(target: HWND) -> bool {
    let mut name = [0_u16; 128];
    let count = unsafe { GetClassNameW(target, &mut name) };
    let name = String::from_utf16_lossy(&name[..count.max(0) as usize]).to_ascii_lowercase();
    name == "edit" || name.starts_with("richedit") || name.starts_with("combo") || name == "listbox"
}

pub fn handle_navigation(hwnd: HWND, msg: &MSG) -> bool {
    if msg.hwnd != hwnd && !crate::is_child_safe(hwnd, msg.hwnd) {
        return false;
    }
    if msg.message == WM_KEYDOWN || msg.message == WM_SYSKEYDOWN {
        let down = |key: u16| crate::get_key_state_safe(key as i32) & (0x8000_u16 as i16) != 0;
        let area = if msg.hwnd == unsafe { GetDlgItem(hwnd, COMMAND_AREA as i32) } {
            modified_area_command(
                msg.wParam.0 as u32,
                unsafe {
                    SendMessageW(
                        GetDlgItem(hwnd, MODE as i32),
                        CB_GETCURSEL,
                        WPARAM(0),
                        LPARAM(0),
                    )
                    .0 == 1
                },
                down(VK_CONTROL.0),
                down(VK_MENU.0),
                down(VK_SHIFT.0),
            )
        } else {
            None
        };
        if let Some(command) = area.or_else(|| {
            shortcut_command(
                msg.wParam.0 as u32,
                down(VK_CONTROL.0),
                down(VK_MENU.0),
                down(VK_SHIFT.0),
                reserves_letters(msg.hwnd)
                    || unsafe {
                        SendMessageW(
                            GetDlgItem(hwnd, MODE as i32),
                            CB_GETCURSEL,
                            WPARAM(0),
                            LPARAM(0),
                        )
                        .0 != 1
                    },
            )
        }) {
            if matches!(command, DELETE | PART_PREVIOUS | PART_NEXT)
                && unsafe {
                    SendMessageW(
                        GetDlgItem(hwnd, MODE as i32),
                        CB_GETCURSEL,
                        WPARAM(0),
                        LPARAM(0),
                    )
                    .0 != 1
                }
            {
                return false;
            }
            // Allow held arrows for navigation, but never repeat a cut or toggle playback repeatedly.
            let repeated = msg.lParam.0 & (1_isize << 30) != 0;
            if !repeated
                || matches!(
                    command,
                    BACK | NEXT
                        | BACK_FIVE
                        | NEXT_FIVE
                        | BACK_TEN
                        | NEXT_TEN
                        | PART_PREVIOUS
                        | PART_NEXT
                )
            {
                unsafe {
                    crate::log_if_err!(PostMessageW(hwnd, WM_COMMAND, WPARAM(command), LPARAM(0)));
                }
            }
            return true;
        }
    }
    if msg.message == WM_KEYDOWN && msg.wParam.0 == VK_ESCAPE.0 as usize {
        unsafe {
            crate::log_if_err!(PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0)));
        }
        return true;
    }
    crate::accessibility::handle_accessibility(hwnd, msg)
}
pub(crate) fn restore_on_parent_activation(parent: HWND) -> bool {
    let window = crate::with_state(parent, |s| s.media_cutter_window).unwrap_or(HWND(0));
    unsafe {
        if !IsWindow(window).as_bool() {
            return false;
        }
        let foreground = GetForegroundWindow();
        if foreground == window {
            return true;
        }
        if foreground != parent {
            return false;
        }
        let popup = GetLastActivePopup(window);
        if popup != window && IsWindowVisible(popup).as_bool() {
            SetForegroundWindow(popup);
            return true;
        }
        ShowWindow(window, SW_RESTORE);
        SetForegroundWindow(window);
        crate::log_if_err!(PostMessageW(
            window,
            WM_RESTORE_CUTTER_FOCUS,
            WPARAM(0),
            LPARAM(0)
        ));
        crate::log_debug("Media cutter restored after parent activation");
    }
    true
}

pub fn open(parent: HWND) {
    unsafe {
        let existing = crate::with_state(parent, |s| s.media_cutter_window).unwrap_or(HWND(0));
        if IsWindow(existing).as_bool() {
            SetForegroundWindow(existing);
            return;
        }
        let language = crate::with_state(parent, |s| s.settings.language).unwrap_or_default();
        let class = to_wide(CLASS);
        let instance = HINSTANCE(crate::get_module_handle_raw_default());
        let wc = WNDCLASSW {
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hInstance: instance,
            lpszClassName: PCWSTR(class.as_ptr()),
            lpfnWndProc: Some(wndproc),
            hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as isize),
            ..Default::default()
        };
        crate::register_class_w_safe(&wc);
        let hwnd = CreateWindowExW(
            WS_EX_CONTROLPARENT | WS_EX_DLGMODALFRAME,
            PCWSTR(class.as_ptr()),
            PCWSTR(to_wide(&crate::i18n::tr(language, "media_cutter.title")).as_ptr()),
            WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
            80,
            30,
            805,
            700,
            parent,
            HMENU(0),
            instance,
            None,
        );
        if hwnd.0 != 0 {
            crate::with_state(parent, |s| s.media_cutter_window = hwnd);
            SetFocus(GetDlgItem(hwnd, OPEN as i32));
        }
    }
}
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe {
        crate::panic_guard::guard(
            "media_cutter_wndproc",
            || DefWindowProcW(hwnd, msg, w, l),
            || inner(hwnd, msg, w, l),
        )
    }
}
fn inner(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe {
        if msg == WM_CREATE {
            let parent = (*(l.0 as *const CREATESTRUCTW)).hwndParent;
            let s = Box::new(create(hwnd, parent));
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(s) as isize);
            SetTimer(hwnd, TICK, 150, None);
            return LRESULT(0);
        }
        if ![
            WM_COMMAND,
            WM_TIMER,
            WM_CLOSE,
            WM_NCDESTROY,
            WM_ACTIVATE,
            WM_SETFOCUS,
            WM_RESTORE_CUTTER_FOCUS,
        ]
        .contains(&msg)
        {
            return DefWindowProcW(hwnd, msg, w, l);
        }
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
        if ptr.is_null() {
            // A confirmation/file dialog may dispatch nested close messages while State is detached.
            if msg == WM_CLOSE {
                return LRESULT(0);
            }
            return DefWindowProcW(hwnd, msg, w, l);
        }
        // Detach while calling native controls/dialogs: nested notifications must not alias State.
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        let s = &mut *ptr;
        s.remember_focus();
        match msg {
            WM_ACTIVATE => {
                if w.0 & 0xffff != WA_INACTIVE as usize {
                    crate::log_if_err!(PostMessageW(
                        hwnd,
                        WM_RESTORE_CUTTER_FOCUS,
                        WPARAM(0),
                        LPARAM(0)
                    ));
                }
                // State remains detached while Windows processes activation and nested focus messages.
                DefWindowProcW(hwnd, msg, w, l);
            }
            WM_SETFOCUS => {
                crate::log_if_err!(PostMessageW(
                    hwnd,
                    WM_RESTORE_CUTTER_FOCUS,
                    WPARAM(0),
                    LPARAM(0)
                ));
            }
            WM_RESTORE_CUTTER_FOCUS => s.restore_focus(),
            WM_COMMAND => {
                let notification = (w.0 >> 16) as u32;
                if notification == EN_CHANGE
                    && [ORIGINAL_GAIN, ADDED_GAIN].contains(&(w.0 & 0xffff))
                {
                    s.dirty |= s.track.is_some();
                }
                let id = w.0 & 0xffff;
                if [START, END].contains(&id)
                    && [EN_CHANGE, EN_KILLFOCUS].contains(&notification)
                    && s.job.is_none()
                    && (s.selected(MODE) == 1 || (s.start.is_some() && s.end.is_some()))
                {
                    // Apply valid edits immediately, without rewriting text or moving the caret.
                    // Incomplete numbers are allowed while typing; restore valid bounds on leaving.
                    let result = s.command(id, notification);
                    if notification == EN_KILLFOCUS {
                        if result.is_err() {
                            s.say(&s.tr("invalid_cut"));
                        }
                        let index = s.selected(PARTS);
                        s.refresh_parts();
                        SendMessageW(s.ctrl(PARTS), CB_SETCURSEL, WPARAM(index), LPARAM(0));
                        s.show_bounds();
                    }
                }
                if (notification == BN_CLICKED || notification == CBN_SELCHANGE)
                    && let Err(e) = s.command(w.0 & 0xffff, notification)
                {
                    crate::show_error(hwnd, s.language, &e);
                }
            }
            WM_TIMER => s.tick(),
            WM_CLOSE => {
                if s.close_after_export && s.job.is_some() {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, ptr as isize);
                    return LRESULT(0);
                }
                let confirmed = if s.job.is_some() {
                    s.confirm("export_close_message")
                } else if s.dirty || s.start.is_some() {
                    s.confirm("unsaved_message")
                } else {
                    true
                };
                if !s.accept_close(confirmed) {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, ptr as isize);
                    return LRESULT(0);
                }
                s.stop();
                let parent = s.parent;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, ptr as isize);
                match DestroyWindow(hwnd) {
                    Ok(()) => {
                        // WM_NCDESTROY has cleared media_cutter_window: restoring now
                        // cannot redirect focus back to the cutter being closed.
                        if IsWindow(parent).as_bool() {
                            crate::log_debug(
                                "Media cutter closed: restoring editor focus with retries",
                            );
                            crate::restore_editor_focus(parent);
                        }
                    }
                    Err(error) => crate::log_debug(&format!("Media cutter close failed: {error}")),
                }
                return LRESULT(0);
            }
            WM_NCDESTROY => {
                crate::log_if_err!(KillTimer(hwnd, TICK));
                let mut state = Box::from_raw(ptr);
                crate::log_debug(&format!(
                    "Media cutter destroyed: export_running={} dirty={}",
                    state.job.is_some(),
                    state.dirty
                ));
                state.cancel.store(true, Ordering::Relaxed);
                state.stop();
                crate::with_state(state.parent, |s| s.media_cutter_window = HWND(0));
                return DefWindowProcW(hwnd, msg, w, l);
            }
            _ => {}
        }
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, ptr as isize);
        LRESULT(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn command_area_keys_keep_navigation_and_modes_separate() {
        for advanced in [false, true] {
            assert_eq!(
                modified_area_command(0x25, advanced, false, false, true),
                Some(BACK_FIVE)
            );
            assert_eq!(
                modified_area_command(0x27, advanced, false, false, true),
                Some(NEXT_FIVE)
            );
            assert_eq!(
                modified_area_command(0x25, advanced, true, false, false),
                Some(BACK_TEN)
            );
            assert_eq!(
                modified_area_command(0x27, advanced, true, false, false),
                Some(NEXT_TEN)
            );
            assert_eq!(
                modified_area_command(0x27, advanced, true, true, false),
                None
            );
            assert_eq!(
                modified_area_command(0x27, advanced, true, false, true),
                None
            );
            assert_eq!(
                modified_area_command(0x09, advanced, false, false, true),
                None
            );
            assert_eq!(area_command(0x24, advanced), Some(FILE_HOME));
            assert_eq!(area_command(0x23, advanced), Some(FILE_END));
            for key in [0x24, 0x23] {
                assert_eq!(shortcut_command(key, false, false, false, false), None);
                assert_eq!(
                    modified_area_command(key, advanced, true, false, false),
                    None
                );
            }
            assert_eq!(area_command(0x20, advanced), Some(PLAY));
            assert_eq!(area_command(0x33, advanced), Some(LISTEN));
            assert_eq!(area_command(0x0d, advanced), Some(LISTEN));
            assert_eq!(area_command(0x09, advanced), None); // Tab must leave the area.
            assert_eq!(area_command(0x70, advanced), Some(COMMAND_HELP));
        }
        assert_eq!(area_command(0x52, true), Some(RESTORE));
        assert_eq!(area_command(0x52, false), None);
        assert_eq!(shortcut_command(0x52, false, false, false, true), None);
        assert_eq!(area_command(0x44, true), Some(DELETE));
        assert_eq!(area_command(0x44, false), None);
        assert_eq!(area_command(0x2e, true), Some(DELETE));
        assert_eq!(area_command(0x2e, false), None);
        assert_eq!(shortcut_command(0x2e, false, false, false, true), None);
        assert_eq!(area_command(0x53, true), Some(SPLIT));
        assert_eq!(area_command(0x31, false), Some(SHORTCUT_START));
        assert_eq!(area_command(0x31, true), None);
        assert_eq!(area_command(0x26, true), Some(PART_PREVIOUS));
        assert_eq!(area_command(0x28, true), Some(PART_NEXT));
    }
    #[test]
    fn spoken_positions_match_mobile() {
        assert_eq!(spoken_time(0.0, Language::Italian), "0 secondi");
        assert_eq!(
            spoken_time(80.0, Language::Italian),
            "1 minuto e 20 secondi"
        );
        assert_eq!(
            spoken_time(5410.0, Language::Italian),
            "1 ora, 30 minuti e 10 secondi"
        );
        assert_eq!(spoken_time(3600.0, Language::Italian), "1 ora");
        assert_eq!(
            spoken_time(80.0, Language::English),
            "1 minute and 20 seconds"
        );
    }
    #[test]
    fn international_shortcuts_preserve_control_navigation() {
        for (key, command) in [
            ('1' as u32, SHORTCUT_START),
            ('2' as u32, SHORTCUT_END),
            ('3' as u32, LISTEN),
            ('4' as u32, SHORTCUT_APPLY),
            ('D' as u32, DELETE),
            ('R' as u32, RESTORE),
            (VK_UP.0 as u32, PART_PREVIOUS),
            (VK_DOWN.0 as u32, PART_NEXT),
            (VK_LEFT.0 as u32, BACK),
            (VK_RIGHT.0 as u32, NEXT),
        ] {
            assert_eq!(
                shortcut_command(key, false, true, false, true),
                Some(command)
            );
            assert_eq!(shortcut_command(key, true, true, false, true), None); // AltGr
            assert_eq!(shortcut_command(key, false, true, true, true), None);
        }
        assert_eq!(
            shortcut_command('S' as u32, true, false, false, true),
            Some(SAVE)
        );
        assert_eq!(
            shortcut_command(VK_SPACE.0 as u32, true, false, false, false),
            Some(PLAY)
        );
        assert_eq!(
            shortcut_command(VK_SPACE.0 as u32, false, false, false, true),
            None
        );
        assert_eq!(
            shortcut_command(VK_SPACE.0 as u32, true, false, false, true),
            Some(PLAY)
        );
        assert_eq!(
            shortcut_command(VK_SPACE.0 as u32, false, false, false, false),
            None
        );
        assert_eq!(
            shortcut_command('S' as u32, false, false, false, false),
            Some(SPLIT)
        );
        assert_eq!(
            shortcut_command('S' as u32, false, false, false, true),
            None
        );
        for key in [VK_LEFT.0 as u32, VK_RIGHT.0 as u32, '1' as u32, 'S' as u32] {
            assert_eq!(shortcut_command(key, false, false, false, true), None);
        }
    }
    #[test]
    fn native_controls_modes_tab_and_progress() {
        unsafe {
            let class = to_wide("SonarpadCutterWindowTest");
            let instance = HINSTANCE(crate::get_module_handle_raw_default());
            crate::register_class_w_safe(&WNDCLASSW {
                hInstance: instance,
                lpszClassName: PCWSTR(class.as_ptr()),
                lpfnWndProc: Some(wndproc),
                ..Default::default()
            });
            let hwnd = CreateWindowExW(
                WS_EX_CONTROLPARENT,
                PCWSTR(class.as_ptr()),
                PCWSTR::null(),
                WS_CAPTION,
                0,
                0,
                805,
                660,
                HWND(0),
                HMENU(0),
                instance,
                None,
            );
            assert_ne!(hwnd.0, 0);
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
            assert!(!ptr.is_null());
            let visible = |h| GetWindowLongPtrW(h, GWL_STYLE) & WS_VISIBLE.0 as isize != 0;
            assert!(visible((*ptr).ctrl(PRIMARY)));
            assert!(!visible((*ptr).ctrl(PARTS)));
            assert!(!visible((*ptr).cancel_button));
            assert!(!reserves_letters((*ptr).ctrl(OPEN)));
            assert!(reserves_letters((*ptr).ctrl(START)));
            assert!(reserves_letters((*ptr).ctrl(MODE)));
            assert!(!reserves_letters(hwnd));
            let first = GetNextDlgTabItem(hwnd, HWND(0), false);
            assert_eq!(first, (*ptr).ctrl(OPEN));
            let mut tab = first;
            let mut seen = vec![tab];
            for _ in 0..60 {
                tab = GetNextDlgTabItem(hwnd, tab, false);
                if tab == first {
                    break;
                }
                assert!(!seen.contains(&tab), "tab navigation loop");
                seen.push(tab);
            }
            let area = (*ptr).ctrl(COMMAND_AREA);
            assert!(seen.contains(&area));
            let after_area = GetNextDlgTabItem(hwnd, area, false);
            assert_ne!(after_area, area);
            assert_eq!(GetNextDlgTabItem(hwnd, after_area, true), area);
            assert!(seen.contains(&(*ptr).ctrl(SAVE)));
            assert!(seen.contains(&(*ptr).ctrl(CLOSE)));
            assert!((*ptr).track_controls.iter().all(|h| !visible(*h)));
            for id in [REMOVE_TRACK, ORIGINAL_GAIN, ADDED_GAIN, LOOP] {
                assert!(!seen.contains(&(*ptr).ctrl(id)));
            }
            (*ptr).track = Some(PathBuf::from("selected-audio.wav"));
            (*ptr).visibility();
            assert!((*ptr).track_controls.iter().all(|h| visible(*h)));
            SendMessageW(hwnd, WM_COMMAND, WPARAM(REMOVE_TRACK), LPARAM(0));
            assert!((*ptr).track.is_none());
            assert!((*ptr).track_controls.iter().all(|h| !visible(*h)));
            (*ptr).track = Some(PathBuf::from("selected-audio.wav"));
            (*ptr).visibility();
            assert!((*ptr).track_controls.iter().all(|h| visible(*h)));
            let adjustment_ids = [START_BACK, START_FORWARD, END_BACK, END_FORWARD];
            assert!(adjustment_ids.iter().all(|id| !visible((*ptr).ctrl(*id))));
            assert!(
                adjustment_ids
                    .iter()
                    .all(|id| !seen.contains(&(*ptr).ctrl(*id)))
            );
            (*ptr).input = Some(PathBuf::from("selected-media.wav"));
            (*ptr).duration = 6.0;
            (*ptr).parts = vec![MediaPart {
                start: 0.0,
                end: 6.0,
                keep: true,
            }];
            (*ptr).position = 1.0;
            SendMessageW(hwnd, WM_COMMAND, WPARAM(SHORTCUT_APPLY), LPARAM(0));
            assert!((*ptr).start.is_none());
            SendMessageW(hwnd, WM_COMMAND, WPARAM(SHORTCUT_START), LPARAM(0));
            assert!(adjustment_ids.iter().all(|id| !visible((*ptr).ctrl(*id))));
            SendMessageW(hwnd, WM_COMMAND, WPARAM(SHORTCUT_START), LPARAM(0));
            assert!((*ptr).end.is_none());
            (*ptr).position = 3.0;
            SendMessageW(hwnd, WM_COMMAND, WPARAM(SHORTCUT_END), LPARAM(0));
            SendMessageW(hwnd, WM_COMMAND, WPARAM(SHORTCUT_END), LPARAM(0));
            assert!((*ptr).start.is_some());
            assert!(adjustment_ids.iter().all(|id| visible((*ptr).ctrl(*id))));
            (*ptr).set(START, "1.250");
            SendMessageW(
                hwnd,
                WM_COMMAND,
                WPARAM(START | ((EN_CHANGE as usize) << 16)),
                LPARAM(0),
            );
            assert_eq!((*ptr).start, Some(1.25));
            assert_eq!((*ptr).text(START), "1.250");
            (*ptr).set(START, "");
            SendMessageW(
                hwnd,
                WM_COMMAND,
                WPARAM(START | ((EN_CHANGE as usize) << 16)),
                LPARAM(0),
            );
            assert_eq!((*ptr).start, Some(1.25));
            SendMessageW(
                hwnd,
                WM_COMMAND,
                WPARAM(START | ((EN_KILLFOCUS as usize) << 16)),
                LPARAM(0),
            );
            assert_eq!((*ptr).text(START), "1.250");
            SendMessageW(hwnd, WM_COMMAND, WPARAM(SHORTCUT_APPLY), LPARAM(0));
            SendMessageW(hwnd, WM_COMMAND, WPARAM(SHORTCUT_APPLY), LPARAM(0));
            assert!(adjustment_ids.iter().all(|id| !visible((*ptr).ctrl(*id))));
            assert!((*ptr).start.is_none() && (*ptr).end.is_none());
            (*ptr).parts = vec![MediaPart {
                start: 0.0,
                end: 6.0,
                keep: true,
            }];
            (*ptr).duration = 6.0;
            (*ptr).refresh_parts();
            SendMessageW((*ptr).ctrl(MODE), CB_SETCURSEL, WPARAM(1), LPARAM(0));
            SendMessageW(
                hwnd,
                WM_COMMAND,
                WPARAM(MODE | ((CBN_SELCHANGE as usize) << 16)),
                LPARAM(0),
            );
            assert!(!visible((*ptr).ctrl(PRIMARY)));
            assert!(visible((*ptr).ctrl(PARTS)));
            assert!(visible((*ptr).ctrl(INSERT_MEDIA)));
            (*ptr).set(START, "0.25");
            (*ptr).set(END, "5.75");
            assert_eq!(GetDlgItem(hwnd, 9817), HWND(0));
            SendMessageW(
                hwnd,
                WM_COMMAND,
                WPARAM(END | ((EN_CHANGE as usize) << 16)),
                LPARAM(0),
            );
            assert_eq!((&(*ptr).parts)[0].start, 0.25);
            assert_eq!((&(*ptr).parts)[0].end, 5.75);
            // S selects the newly created right-hand part, which Alt+3 auditions.
            (*ptr).position = 2.0;
            SendMessageW(hwnd, WM_COMMAND, WPARAM(SPLIT), LPARAM(0));
            assert_eq!((*ptr).selected(PARTS), 1);
            assert_eq!((*ptr).audition_bounds().unwrap(), (2.0, 5.75));
            (*ptr).position = 4.0;
            SendMessageW(hwnd, WM_COMMAND, WPARAM(SPLIT), LPARAM(0));
            assert_eq!((*ptr).selected(PARTS), 2);
            assert_eq!((*ptr).audition_bounds().unwrap(), (4.0, 5.75));
            SendMessageW((*ptr).ctrl(PARTS), CB_SETCURSEL, WPARAM(1), LPARAM(0));
            assert_eq!((*ptr).audition_bounds().unwrap(), (2.0, 4.0));
            SendMessageW(hwnd, WM_COMMAND, WPARAM(PART_NEXT), LPARAM(0));
            assert_eq!((*ptr).selected(PARTS), 2);
            SendMessageW(hwnd, WM_COMMAND, WPARAM(PART_NEXT), LPARAM(0));
            assert_eq!((*ptr).selected(PARTS), 2);
            SendMessageW(hwnd, WM_COMMAND, WPARAM(DELETE), LPARAM(0));
            assert!(!(&(*ptr).parts)[2].keep);
            assert_eq!((*ptr).selected(PARTS), 2);
            assert!((*ptr).playback_ranges(true).is_err());
            SendMessageW(hwnd, WM_COMMAND, WPARAM(LISTEN), LPARAM(0));
            assert!((*ptr).player.is_none());
            (*ptr).position = 5.0;
            SendMessageW(hwnd, WM_COMMAND, WPARAM(BACK), LPARAM(0));
            let ranges = (*ptr).playback_ranges(false).unwrap();
            assert!(!ranges.is_empty());
            assert!(ranges.iter().all(|p| p.end <= 4.0));

            SendMessageW(hwnd, WM_COMMAND, WPARAM(PART_PREVIOUS), LPARAM(0));
            assert_eq!((*ptr).audition_bounds().unwrap(), (2.0, 4.0));
            SendMessageW(hwnd, WM_COMMAND, WPARAM(PART_PREVIOUS), LPARAM(0));
            SendMessageW(hwnd, WM_COMMAND, WPARAM(PART_PREVIOUS), LPARAM(0));
            assert_eq!((*ptr).selected(PARTS), 0);
            // Insert through the same handler used after the file picker, then audition/delete/restore.
            let inserted =
                std::env::temp_dir().join(format!("cutter-ui-insert-{}.wav", uuid::Uuid::new_v4()));
            let mut writer = hound::WavWriter::create(
                &inserted,
                hound::WavSpec {
                    channels: 1,
                    sample_rate: 8000,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                },
            )
            .unwrap();
            for _ in 0..8000 {
                writer.write_sample(1000_i16).unwrap();
            }
            writer.finalize().unwrap();
            SendMessageW((*ptr).ctrl(PARTS), CB_SETCURSEL, WPARAM(1), LPARAM(0));
            (*ptr).insert_media(inserted.clone()).unwrap();
            assert_eq!((*ptr).selected(PARTS), 1);
            assert_eq!((*ptr).duration, 7.0);
            assert_eq!((*ptr).position, 2.0);
            assert!((*ptr).dirty);
            assert_eq!((*ptr).audition_bounds().unwrap(), (2.0, 3.0));
            assert!(!(&(*ptr).parts)[3].keep);
            let clips = crate::media_cutter_timeline::resolve(
                &(*ptr).sources,
                &(*ptr).playback_ranges(true).unwrap(),
            )
            .unwrap();
            assert_eq!(clips.len(), 1);
            assert_eq!(clips[0].path, inserted);
            assert_eq!((clips[0].start, clips[0].end), (0.0, 1.0));
            SendMessageW(hwnd, WM_COMMAND, WPARAM(DELETE), LPARAM(0));
            assert!((*ptr).playback_ranges(true).is_err());
            SendMessageW(hwnd, WM_COMMAND, WPARAM(RESTORE), LPARAM(0));
            assert_eq!((*ptr).audition_bounds().unwrap(), (2.0, 3.0));
            std::fs::remove_file(inserted).unwrap();
            let job = Arc::new(Mutex::new(Job {
                percent: 37,
                result: None,
            }));
            (*ptr).job = Some(job);
            for h in &(*ptr).controls {
                ShowWindow(*h, SW_HIDE);
            }
            ShowWindow((*ptr).progress, SW_SHOW);
            ShowWindow((*ptr).cancel_button, SW_SHOW);
            SendMessageW(hwnd, WM_TIMER, WPARAM(TICK), LPARAM(0));
            assert!(!visible((*ptr).ctrl(OPEN)));
            assert!(!visible((*ptr).ctrl(SAVE)));
            assert!(visible((*ptr).progress));
            assert_eq!((*ptr).last_percent, 37);
            let mut text = [0_u16; 200];
            let n = GetWindowTextW((*ptr).status, &mut text);
            assert!(String::from_utf16_lossy(&text[..n as usize]).contains("37%"));
            assert!(!(*ptr).accept_close(false));
            assert!(!(*ptr).close_after_export);
            assert!(IsWindow(hwnd).as_bool());
            assert!((*ptr).job.is_some());
            assert!(!(*ptr).cancel.load(Ordering::Relaxed));
            assert!(!(*ptr).accept_close(true));
            assert!((*ptr).close_after_export);
            assert!((*ptr).job.is_some());
            assert!((*ptr).cancel.load(Ordering::Relaxed));
            (*ptr).job.as_ref().unwrap().lock().unwrap().result = Some(Err("__CANCELLED__".into()));
            SendMessageW(hwnd, WM_TIMER, WPARAM(TICK), LPARAM(0));
            assert!((*ptr).job.is_none());
            assert!(!(*ptr).dirty);
            assert!((*ptr).start.is_none());
            // The same close path used by Escape runs after cancellation completes.
            SendMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0));
            assert!(!IsWindow(hwnd).as_bool());
        }
    }
}
