use crate::cloud_sync::{self, Session};
use crate::settings::{Language, save_settings, settings_dir};
use crate::{log_debug, with_state};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    MB_ICONINFORMATION, MB_OK, MessageBoxW, PostMessageW, WM_APP,
};
use windows::core::PCWSTR;

pub const COMPLETE: u32 = WM_APP + 160;
static BUSY: AtomicBool = AtomicBool::new(false);
static UI_OPEN: AtomicBool = AtomicBool::new(false);
static AUTO: std::sync::OnceLock<std::sync::Mutex<cloud_sync::AutoSync>> =
    std::sync::OnceLock::new();
pub const TIMER: usize = 0x5359;
fn scheduler() -> &'static std::sync::Mutex<cloud_sync::AutoSync> {
    AUTO.get_or_init(|| std::sync::Mutex::new(cloud_sync::AutoSync::default()))
}
struct DialogGuard;
impl Drop for DialogGuard {
    fn drop(&mut self) {
        UI_OPEN.store(false, Ordering::SeqCst);
    }
}
fn load_sync_settings() -> Result<Value, String> {
    load_sync_settings_from(&settings_dir().join("settings.json"))
}

fn load_sync_settings_from(path: &std::path::Path) -> Result<Value, String> {
    let raw = std::fs::read_to_string(path).map_err(|_| "storage_error")?;
    let parsed: crate::settings::AppSettings =
        serde_json::from_str(&raw).map_err(|_| "storage_error")?;
    let loaded =
        serde_json::to_value(crate::settings::parse_settings(&raw).map_err(|_| "storage_error")?)
            .map_err(|_| "storage_error")?;
    // Never export fallback defaults after a failed or racing settings read.
    if std::fs::read_to_string(path).map_err(|_| "storage_error")? != raw {
        log_debug("CloudSync settings_read file changed during read; sync blocked");
        return Err("storage_error".into());
    }
    let parsed = serde_json::to_value(parsed).map_err(|_| "storage_error")?;
    let collections = json!({"sync_preferences":false});
    if cloud_sync::selected(&collections, &cloud_sync::export(&parsed, false))
        != cloud_sync::selected(&collections, &cloud_sync::export(&loaded, false))
    {
        log_debug("CloudSync settings_read inconsistent collections; sync blocked");
        return Err("storage_error".into());
    }
    Ok(loaded)
}

pub fn tick(parent: HWND) {
    if BUSY.load(Ordering::SeqCst) || UI_OPEN.load(Ordering::SeqCst) {
        return;
    }
    let Ok(meta) = Session::metadata(&settings_dir()) else {
        return;
    };
    if !meta["group_id"].is_string() {
        return;
    }
    let Ok(settings) = load_sync_settings() else {
        return;
    };
    let dirty = cloud_sync::local_changes(&meta, &settings, false).unwrap_or(true);
    let due = scheduler()
        .lock()
        .map(|mut s| s.due(&meta, dirty, cloud_sync::now_seconds()))
        .unwrap_or(false);
    if due {
        let language = with_state(parent, |s| s.settings.language).unwrap_or_default();
        start(parent, language, settings, 2, None, true);
    }
}
struct Outcome {
    session: Session,
    remote: Option<Value>,
    code: Option<String>,
}
struct Completion {
    result: Result<Outcome, String>,
    language: Language,
    action: usize,
    automatic: bool,
}

fn label(language: Language, it: &'static str, en: &'static str) -> &'static str {
    if language == Language::Italian {
        it
    } else {
        en
    }
}
pub fn open(parent: HWND) {
    if UI_OPEN.swap(true, Ordering::SeqCst) {
        return;
    }
    let _guard = DialogGuard;
    let Some((language, settings)) =
        with_state(parent, |s| (s.settings.language, load_sync_settings()))
    else {
        return;
    };
    let settings = match settings {
        Ok(settings) => settings,
        Err(error) => {
            log_debug(&format!("CloudSync open blocked: {error}"));
            crate::show_error(
                parent,
                language,
                label(
                    language,
                    "Non riesco a leggere le impostazioni in modo sicuro. La sincronizzazione è stata bloccata per proteggere i tuoi dati. Riprova.",
                    "Unable to read settings safely. Sync was blocked to protect your data. Please try again.",
                ),
            );
            return;
        }
    };
    let title = label(language, "Sincronizzazione cloud", "Cloud sync");
    let meta = match Session::metadata(&settings_dir()) {
        Ok(meta) => meta,
        Err(_) => {
            crate::show_error(
                parent,
                language,
                label(
                    language,
                    "Non riesco a leggere lo stato della sincronizzazione. Riprova.",
                    "Unable to read sync status. Please try again.",
                ),
            );
            return;
        }
    };
    let connected = meta["group_id"].is_string();
    let status = cloud_sync::status(
        &meta,
        cloud_sync::local_changes(&meta, &settings, false).unwrap_or(true),
        BUSY.load(Ordering::SeqCst),
        scheduler().lock().map(|s| s.failed()).unwrap_or(false),
        language == Language::Italian,
    );
    if BUSY.load(Ordering::SeqCst) {
        super::help_window::open_readonly_text_modal(parent, title, &status);
        return;
    }
    let mut advanced = false;
    let action = loop {
        let (help, entries) = if advanced {
            (
                label(
                    language,
                    "Scegli cosa sincronizzare o scollega il dispositivo. Le opzioni iniziali vanno bene per la maggior parte degli utenti.",
                    "Choose what to sync or disconnect this device. Usually these options need no changes.",
                ),
                vec![
                    (
                        8,
                        label(language, "Torna alla sincronizzazione", "Back to sync"),
                    ),
                    (
                        5,
                        if meta["sync_preferences"] == json!(false) {
                            label(
                                language,
                                "Attiva sincronizzazione preferenze",
                                "Enable preference sync",
                            )
                        } else {
                            label(
                                language,
                                "Disattiva sincronizzazione preferenze",
                                "Disable preference sync",
                            )
                        },
                    ),
                    (
                        6,
                        if meta["sync_collections"] == json!(false) {
                            label(
                                language,
                                "Attiva sincronizzazione preferiti e raccolte",
                                "Enable favorites and collections sync",
                            )
                        } else {
                            label(
                                language,
                                "Disattiva sincronizzazione preferiti e raccolte",
                                "Disable favorites and collections sync",
                            )
                        },
                    ),
                    (
                        4,
                        label(
                            language,
                            "Scollega questo dispositivo",
                            "Disconnect this device",
                        ),
                    ),
                ],
            )
        } else if connected {
            (
                label(
                    language,
                    "Sei collegato. Sincronizza ora aggiorna preferiti e preferenze. Per aggiungere un dispositivo scegli Collega un altro dispositivo.",
                    "You are connected. Sync now updates favorites and preferences. Choose Connect another device to add a device.",
                ),
                vec![
                    (2, label(language, "Sincronizza ora", "Sync now")),
                    (
                        3,
                        label(
                            language,
                            "Collega un altro dispositivo",
                            "Connect another device",
                        ),
                    ),
                    (
                        7,
                        label(language, "Opzioni di sincronizzazione", "Sync options"),
                    ),
                ],
            )
        } else {
            (
                label(
                    language,
                    "Primo dispositivo: scegli Inizia da questo dispositivo. Sugli altri scegli Ho già un codice e inserisci il codice ricevuto.",
                    "First device: choose Start on this device. On your other devices choose I already have a code and enter the code you receive.",
                ),
                vec![
                    (
                        0,
                        label(
                            language,
                            "Inizia da questo dispositivo",
                            "Start on this device",
                        ),
                    ),
                    (
                        1,
                        label(language, "Ho già un codice", "I already have a code"),
                    ),
                ],
            )
        };
        let help = if connected && !advanced {
            status.as_str()
        } else {
            help
        };
        let options: Vec<String> = entries.iter().map(|(_, name)| name.to_string()).collect();
        let Some(choice) = super::prompt_window::prompt_user_choice(
            parent,
            title,
            help,
            &options,
            &options[0],
            language,
        ) else {
            return;
        };
        let Some(index) = options.iter().position(|v| v == &choice) else {
            return;
        };
        match entries[index].0 {
            7 => advanced = true,
            8 => advanced = false,
            action => break action,
        }
    };
    let code = if action == 1 {
        let Some(v) = super::prompt_window::prompt_user(
            parent,
            title,
            label(language, "Codice di 8 cifre", "8-digit code"),
            "",
            language,
        ) else {
            return;
        };
        Some(v)
    } else {
        None
    };
    if action == 4 {
        let choices = [
            label(language, "Annulla", "Cancel").into(),
            label(
                language,
                "Scollega questo dispositivo",
                "Disconnect this device",
            )
            .into(),
        ];
        if super::prompt_window::prompt_user_choice(
            parent,
            title,
            label(
                language,
                "I preferiti rimangono sul dispositivo. Confermi lo scollegamento?",
                "Favorites stay on this device. Confirm disconnection?",
            ),
            &choices,
            &choices[0],
            language,
        ) != Some(choices[1].clone())
        {
            return;
        }
    }
    start(parent, language, settings, action, code, false);
}
fn start(
    parent: HWND,
    language: Language,
    settings: Value,
    action: usize,
    code: Option<String>,
    automatic: bool,
) {
    if BUSY.swap(true, Ordering::SeqCst) {
        return;
    }
    let progress = label(language, "Sincronizzazione in corso…", "Syncing…");
    if !automatic {
        crate::set_main_status_message(parent, progress);
        crate::accessibility::screen_reader_speak(progress);
    }
    let parent_value = parent.0 as usize;
    std::thread::spawn(move || {
        let result = (|| {
            let mut session = Session::load(&settings_dir())?;
            if action >= 5 {
                let enabled = session.toggle_scope(action == 5)?;
                log_debug(if enabled {
                    "CloudSync scope enabled"
                } else {
                    "CloudSync scope disabled"
                });
                return Ok(Outcome {
                    session,
                    remote: None,
                    code: None,
                });
            }
            if action == 4 {
                session.disconnect(log_debug)?;
                return Ok(Outcome {
                    session,
                    remote: None,
                    code: None,
                });
            }
            if action <= 1 {
                session.connect(code.as_deref(), log_debug)?;
            }
            if action == 3 {
                let response = session.request("code", &json!({}), log_debug)?;
                let code = cloud_sync::text(&response, "code").to_string();
                return Ok(Outcome {
                    session,
                    remote: None,
                    code: Some(code),
                });
            }
            let remote = session.sync(&settings, false, log_debug)?;
            let invite = if action == 0 {
                Some(
                    cloud_sync::text(&session.request("code", &json!({}), log_debug)?, "code")
                        .to_string(),
                )
            } else {
                None
            };
            Ok(Outcome {
                session,
                remote: Some(remote),
                code: invite,
            })
        })();
        let ptr = Box::into_raw(Box::new(Completion {
            result,
            language,
            action,
            automatic,
        }));
        if unsafe {
            PostMessageW(
                HWND(parent_value as isize),
                COMPLETE,
                WPARAM(0),
                LPARAM(ptr as isize),
            )
        }
        .is_err()
        {
            unsafe {
                let _completion = Box::from_raw(ptr);
            }
            BUSY.store(false, Ordering::SeqCst);
        }
    });
}

pub fn complete(parent: HWND, param: LPARAM) {
    let completion = unsafe { Box::from_raw(param.0 as *mut Completion) };
    let language = completion.language;
    let action = completion.action;
    let automatic = completion.automatic;
    let result = completion.result.and_then(|mut outcome| {
        if let Some(mut remote) = outcome.remote {
            let mut activation_changed = false;
            let applied = with_state(parent, |state| -> Result<(), String> {
                let mut settings = load_sync_settings().map_err(|_| "storage_error")?;
                let current = outcome
                    .session
                    .select(&cloud_sync::export(&settings, false));
                let merged =
                    cloud_sync::merge(&remote, &outcome.session.meta["pending"]["state"], &current);
                outcome
                    .session
                    .guard_deletions("incoming", &current, &merged)?;
                let old_language = cloud_sync::language(&settings, false);
                cloud_sync::apply(&mut settings, &merged, false)?;
                if old_language != cloud_sync::language(&settings, false)
                    && outcome.session.meta["pending"]["state"]
                        .get("news_sources")
                        .is_some()
                {
                    cloud_sync::reconcile_news_language(
                        &outcome.session,
                        &mut settings,
                        &mut remote,
                        false,
                    )?;
                }
                let updated: crate::settings::AppSettings =
                    serde_json::from_value(settings).map_err(|_| "invalid_response")?;
                save_settings(updated.clone());
                let persisted = load_sync_settings().map_err(|_| "storage_error")?;
                if cloud_sync::export(&persisted, false)
                    != cloud_sync::export(
                        &serde_json::to_value(&updated).map_err(|_| "storage_error")?,
                        false,
                    )
                {
                    return Err("storage_error".into());
                }
                cloud_sync::log_collection_reductions(
                    &current,
                    &outcome
                        .session
                        .select(&cloud_sync::export(&persisted, false)),
                    "write_verified",
                    crate::log_debug,
                );
                activation_changed = state.settings.rai_luce_code != updated.rai_luce_code;
                state.settings = updated;
                Ok(())
            })
            .ok_or("storage_error")?;
            applied?;
            if activation_changed {
                crate::rebuild_menus(parent);
            }
            outcome.session.finish(remote)?;
        }
        Ok((
            outcome.code,
            outcome.session.meta["sync_preferences"] != json!(false),
            outcome.session.meta["sync_collections"] != json!(false),
        ))
    });
    BUSY.store(false, Ordering::SeqCst);
    if action <= 2
        && let Ok(mut schedule) = scheduler().lock()
    {
        schedule.completed(result.is_ok(), cloud_sync::now_seconds());
    }
    if automatic {
        if let Err(error) = result {
            log_debug(&format!("CloudSync automatic completion error={error}"));
        }
        return;
    }
    if result
        .as_ref()
        .is_err_and(|error| error == "deletion_confirmation_required")
    {
        if let Ok(session) = Session::load(&settings_dir()) {
            crate::show_error(
                parent,
                language,
                &cloud_sync::deletion_review_text(
                    &session.meta["deletion_review"],
                    language == Language::Italian,
                ),
            );
        }
        return;
    }
    crate::update_main_status_bar(parent);
    let title = label(language, "Sincronizzazione cloud", "Cloud sync");
    match result {
        Ok((code, preferences, collections)) => {
            let has_code = code.is_some();
            let message = if let Some(code) = code {
                format!(
                    "{}\r\n{code}\r\n{}",
                    label(language, "Codice di collegamento:", "Connection code:"),
                    label(
                        language,
                        "Sull’altro dispositivo apri Sincronizzazione cloud, scegli Ho già un codice e inserisci questo codice. È monouso e vale 10 minuti.",
                        "On your other device, open Cloud sync, choose I already have a code and enter this code. Single-use, valid for 10 minutes."
                    )
                )
            } else if action == 4 {
                label(
                    language,
                    "Dispositivo scollegato. I dati locali sono conservati.",
                    "Device disconnected. Local data is preserved.",
                )
                .into()
            } else if action < 5 {
                label(language, "Sincronizzazione completata.", "Sync completed.").into()
            } else {
                format!(
                    "{}\r\n{}: {}\r\n{}: {}",
                    label(language, "Operazione completata.", "Operation completed."),
                    label(language, "Preferenze", "Preferences"),
                    if preferences {
                        label(language, "Attivate", "On")
                    } else {
                        label(language, "Disattivate", "Off")
                    },
                    label(language, "Raccolte", "Collections"),
                    if collections {
                        label(language, "Attivate", "On")
                    } else {
                        label(language, "Disattivate", "Off")
                    }
                )
            };
            if has_code {
                super::help_window::open_readonly_text_modal(parent, title, &message);
                return;
            }
            let title = crate::accessibility::to_wide(title);
            let message = crate::accessibility::to_wide(&message);
            unsafe {
                MessageBoxW(
                    parent,
                    PCWSTR(message.as_ptr()),
                    PCWSTR(title.as_ptr()),
                    MB_OK | MB_ICONINFORMATION,
                );
            }
        }
        Err(error) => {
            log_debug(&format!("CloudSync completion error={error}"));
            crate::show_error(
                parent,
                language,
                label(
                    language,
                    "Sincronizzazione non riuscita. Controlla connessione e codice e riprova. I dati locali sono conservati.",
                    "Sync failed. Check your connection and code and retry. Local data is preserved.",
                ),
            );
        }
    }
}

#[cfg(test)]
mod settings_read_tests {
    use super::*;

    #[test]
    fn sync_read_preserves_file_with_encrypted_credentials() {
        let path = std::env::temp_dir().join(format!(
            "sonarpad-sync-read-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let settings = crate::settings::AppSettings {
            rai_luce_code: crate::settings::encrypt_rai_luce_code("test-only-code"),
            ..Default::default()
        };
        let raw = serde_json::to_string(&settings).unwrap();
        std::fs::write(&path, &raw).unwrap();
        for _ in 0..2 {
            let loaded = load_sync_settings_from(&path).unwrap();
            assert_eq!(loaded["rai_luce_code"], "test-only-code");
            assert_eq!(std::fs::read_to_string(&path).unwrap(), raw);
        }
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn sync_read_rejects_missing_or_corrupt_settings_without_defaults() {
        let path = std::env::temp_dir().join(format!(
            "sonarpad-sync-read-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        assert!(load_sync_settings_from(&path).is_err());
        assert!(!path.exists());
        std::fs::write(&path, "{invalid").unwrap();
        assert!(load_sync_settings_from(&path).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{invalid");
        std::fs::remove_file(&path).unwrap();
    }
}
