use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use windows::Win32::System::Power::{
    ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED, EXECUTION_STATE,
    SetThreadExecutionState,
};

const REFRESH_INTERVAL: Duration = Duration::from_secs(30);
const IDLE_POLL_INTERVAL: Duration = Duration::from_millis(250);

struct PowerRequests {
    named: HashSet<&'static str>,
    guards: HashMap<u64, &'static str>,
}

impl PowerRequests {
    fn new() -> Self {
        Self {
            named: HashSet::new(),
            guards: HashMap::new(),
        }
    }

    fn is_required(&self) -> bool {
        !self.named.is_empty() || !self.guards.is_empty()
    }

    fn active_count(&self) -> usize {
        self.named.len() + self.guards.len()
    }
}

struct PowerAwakeState {
    requests: Mutex<PowerRequests>,
    changed: Condvar,
    next_guard_id: AtomicU64,
    worker_started: AtomicBool,
}

impl PowerAwakeState {
    fn new() -> Self {
        Self {
            requests: Mutex::new(PowerRequests::new()),
            changed: Condvar::new(),
            next_guard_id: AtomicU64::new(1),
            worker_started: AtomicBool::new(false),
        }
    }
}

fn state() -> &'static Arc<PowerAwakeState> {
    static STATE: OnceLock<Arc<PowerAwakeState>> = OnceLock::new();
    STATE.get_or_init(|| {
        let state = Arc::new(PowerAwakeState::new());
        let worker_state = Arc::clone(&state);
        match thread::Builder::new()
            .name("sonarpad-power-awake".to_string())
            .spawn(move || power_worker(worker_state))
        {
            Ok(_handle) => {
                state.worker_started.store(true, Ordering::Release);
                crate::log_debug("Power: persistent keep-awake worker started");
            }
            Err(error) => {
                crate::log_debug(&format!(
                    "Power: failed to start persistent keep-awake worker: {error}"
                ));
            }
        }
        state
    })
}

fn apply_execution_state(required: bool, context: &str) -> bool {
    let flags = if required {
        ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED
    } else {
        ES_CONTINUOUS
    };
    let result = unsafe { SetThreadExecutionState(flags) };
    let ok = result != EXECUTION_STATE(0);
    crate::log_debug(&format!(
        "Power: SetThreadExecutionState context={context} required={required} success={ok}"
    ));
    ok
}

fn power_worker(state: Arc<PowerAwakeState>) {
    let mut applied = false;
    let mut last_refresh = Instant::now();

    loop {
        let (required, active_count) = {
            let requests = match state.requests.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            let required = requests.is_required();
            let active_count = requests.active_count();

            if required == applied && (!required || last_refresh.elapsed() < REFRESH_INTERVAL) {
                let timeout = if required {
                    REFRESH_INTERVAL
                        .saturating_sub(last_refresh.elapsed())
                        .min(IDLE_POLL_INTERVAL)
                } else {
                    IDLE_POLL_INTERVAL
                };
                let wait_result = state.changed.wait_timeout(requests, timeout);
                match wait_result {
                    Ok((_guard, _timeout_result)) => {}
                    Err(poisoned) => {
                        let (_guard, _timeout_result) = poisoned.into_inner();
                    }
                }
                continue;
            }
            (required, active_count)
        };

        if required != applied {
            if apply_execution_state(required, "state-change") {
                applied = required;
                last_refresh = Instant::now();
                crate::log_debug(&format!(
                    "Power: keep-awake {} active_requests={active_count}",
                    if required { "enabled" } else { "disabled" }
                ));
            } else {
                thread::sleep(IDLE_POLL_INTERVAL);
            }
            continue;
        }

        if required && last_refresh.elapsed() >= REFRESH_INTERVAL {
            if apply_execution_state(true, "heartbeat") {
                last_refresh = Instant::now();
            }
            continue;
        }
    }
}

pub(crate) fn set_required(reason: &'static str, enabled: bool) -> bool {
    let state = state();
    let changed = {
        let mut requests = match state.requests.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if enabled {
            requests.named.insert(reason)
        } else {
            requests.named.remove(reason)
        }
    };
    if changed {
        crate::log_debug(&format!(
            "Power: named keep-awake reason={reason} enabled={enabled}"
        ));
        state.changed.notify_all();
    }
    state.worker_started.load(Ordering::Acquire)
}

pub(crate) struct PowerAwakeGuard {
    id: u64,
    reason: &'static str,
}

impl Drop for PowerAwakeGuard {
    fn drop(&mut self) {
        let state = state();
        let removed = {
            let mut requests = match state.requests.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            requests.guards.remove(&self.id).is_some()
        };
        if removed {
            crate::log_debug(&format!(
                "Power: activity keep-awake released reason={} id={}",
                self.reason, self.id
            ));
            state.changed.notify_all();
        }
    }
}

pub(crate) fn acquire(reason: &'static str) -> PowerAwakeGuard {
    let state = state();
    let id = state.next_guard_id.fetch_add(1, Ordering::Relaxed);
    {
        let mut requests = match state.requests.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        requests.guards.insert(id, reason);
    }
    crate::log_debug(&format!(
        "Power: activity keep-awake acquired reason={reason} id={id}"
    ));
    state.changed.notify_all();
    PowerAwakeGuard { id, reason }
}
