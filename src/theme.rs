use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Once, OnceLock};

const WH_CALLWNDPROC: i32 = 4;
const HC_ACTION: i32 = 0;
const WM_CREATE: u32 = 0x0001;
const WM_ERASEBKGND: u32 = 0x0014;
const WM_SHOWWINDOW: u32 = 0x0018;
const WM_NCDESTROY: u32 = 0x0082;
const WM_INITDIALOG: u32 = 0x0110;
const WM_CTLCOLORMSGBOX: u32 = 0x0132;
const WM_CTLCOLOREDIT: u32 = 0x0133;
const WM_CTLCOLORLISTBOX: u32 = 0x0134;
const WM_CTLCOLORBTN: u32 = 0x0135;
const WM_CTLCOLORDLG: u32 = 0x0136;
const WM_CTLCOLORSCROLLBAR: u32 = 0x0137;
const WM_CTLCOLORSTATIC: u32 = 0x0138;
const EM_SETBKGNDCOLOR: u32 = 0x0443;
const LVM_SETBKCOLOR: u32 = 0x1001;
const LVM_SETTEXTCOLOR: u32 = 0x1024;
const LVM_SETTEXTBKCOLOR: u32 = 0x1026;
const TVM_SETBKCOLOR: u32 = 0x111D;
const TVM_SETTEXTCOLOR: u32 = 0x111E;

const COLOR_WINDOW: i32 = 5;
const COLOR_WINDOWTEXT: i32 = 8;
const TRANSPARENT: i32 = 1;
const GA_ROOT: u32 = 2;

const RDW_INVALIDATE: u32 = 0x0001;
const RDW_ERASE: u32 = 0x0004;
const RDW_ALLCHILDREN: u32 = 0x0080;
const RDW_FRAME: u32 = 0x0400;

const DARK_BACKGROUND: u32 = 0x001E1E1E;
const DARK_SURFACE: u32 = 0x00262626;
const DARK_TEXT: u32 = 0x00F2F2F2;
const THEME_SUBCLASS_ID: usize = 0x534F_4E41;

const PREFERRED_APP_MODE_FORCE_DARK: i32 = 2;
const PREFERRED_APP_MODE_FORCE_LIGHT: i32 = 3;
const UXTHEME_REFRESH_IMMERSIVE_COLOR_POLICY_STATE: usize = 104;
const UXTHEME_ALLOW_DARK_MODE_FOR_WINDOW: usize = 133;
const UXTHEME_SET_PREFERRED_APP_MODE: usize = 135;
const UXTHEME_FLUSH_MENU_THEMES: usize = 136;
const DWMWA_USE_IMMERSIVE_DARK_MODE_BEFORE_20H1: u32 = 19;
const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;

#[repr(C)]
struct RawRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
struct CallWndProcMessage {
    _lparam: isize,
    _wparam: usize,
    message: u32,
    hwnd: isize,
}

type HookProc = Option<unsafe extern "system" fn(i32, usize, isize) -> isize>;
type EnumWindowProc = Option<unsafe extern "system" fn(isize, isize) -> i32>;
type SubclassProc =
    Option<unsafe extern "system" fn(isize, u32, usize, isize, usize, usize) -> isize>;
type SetPreferredAppModeFn = unsafe extern "system" fn(i32) -> i32;
type AllowDarkModeForWindowFn = unsafe extern "system" fn(isize, i32) -> i32;
type RefreshImmersiveColorPolicyStateFn = unsafe extern "system" fn();
type FlushMenuThemesFn = unsafe extern "system" fn();

#[link(name = "user32")]
unsafe extern "system" {
    #[link_name = "SetWindowsHookExW"]
    fn set_windows_hook_ex_w(
        id_hook: i32,
        hook_proc: HookProc,
        module: isize,
        thread_id: u32,
    ) -> isize;
    #[link_name = "CallNextHookEx"]
    fn call_next_hook_ex(hook: isize, code: i32, wparam: usize, lparam: isize) -> isize;
    #[link_name = "EnumChildWindows"]
    fn enum_child_windows(parent: isize, callback: EnumWindowProc, lparam: isize) -> i32;
    #[link_name = "EnumThreadWindows"]
    fn enum_thread_windows(thread_id: u32, callback: EnumWindowProc, lparam: isize) -> i32;
    #[link_name = "GetClassNameW"]
    fn get_class_name_w(hwnd: isize, class_name: *mut u16, max_count: i32) -> i32;
    #[link_name = "GetClientRect"]
    fn get_client_rect(hwnd: isize, rect: *mut RawRect) -> i32;
    #[link_name = "GetAncestor"]
    fn get_ancestor(hwnd: isize, flags: u32) -> isize;
    #[link_name = "RedrawWindow"]
    fn redraw_window(hwnd: isize, rect: *const RawRect, region: isize, flags: u32) -> i32;
    #[link_name = "DrawMenuBar"]
    fn draw_menu_bar(hwnd: isize) -> i32;
    #[link_name = "SendMessageW"]
    fn send_message_w(hwnd: isize, message: u32, wparam: usize, lparam: isize) -> isize;
    #[link_name = "FillRect"]
    fn fill_rect(hdc: isize, rect: *const RawRect, brush: isize) -> i32;
    #[link_name = "GetSysColor"]
    fn get_sys_color(index: i32) -> u32;
}

#[link(name = "gdi32")]
unsafe extern "system" {
    #[link_name = "CreateSolidBrush"]
    fn create_solid_brush(color: u32) -> isize;
    #[link_name = "SetBkColor"]
    fn set_bk_color(hdc: isize, color: u32) -> u32;
    #[link_name = "SetBkMode"]
    fn set_bk_mode(hdc: isize, mode: i32) -> i32;
    #[link_name = "SetTextColor"]
    fn set_text_color(hdc: isize, color: u32) -> u32;
}

#[link(name = "comctl32")]
unsafe extern "system" {
    #[link_name = "SetWindowSubclass"]
    fn set_window_subclass(
        hwnd: isize,
        subclass_proc: SubclassProc,
        subclass_id: usize,
        reference_data: usize,
    ) -> i32;
    #[link_name = "DefSubclassProc"]
    fn def_subclass_proc(hwnd: isize, message: u32, wparam: usize, lparam: isize) -> isize;
    #[link_name = "RemoveWindowSubclass"]
    fn remove_window_subclass(hwnd: isize, subclass_proc: SubclassProc, subclass_id: usize) -> i32;
}

#[link(name = "uxtheme")]
unsafe extern "system" {
    #[link_name = "SetWindowTheme"]
    fn set_window_theme(hwnd: isize, sub_app_name: *const u16, sub_id_list: *const u16) -> i32;
}

#[link(name = "dwmapi")]
unsafe extern "system" {
    #[link_name = "DwmSetWindowAttribute"]
    fn dwm_set_window_attribute(
        hwnd: isize,
        attribute: u32,
        attribute_value: *const c_void,
        attribute_size: u32,
    ) -> i32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    #[link_name = "GetCurrentThreadId"]
    fn get_current_thread_id() -> u32;
    #[link_name = "LoadLibraryW"]
    fn load_library_w(file_name: *const u16) -> isize;
    #[link_name = "GetProcAddress"]
    fn get_proc_address(module: isize, proc_name: *const u8) -> *mut c_void;
}

static DARK_MODE: AtomicBool = AtomicBool::new(false);
static INSTALL_HOOK: Once = Once::new();
static DARK_BRUSH: OnceLock<usize> = OnceLock::new();
static UXTHEME_MODULE: OnceLock<usize> = OnceLock::new();

pub fn initialize(enabled: bool) {
    DARK_MODE.store(enabled, Ordering::Relaxed);
    set_preferred_app_mode(enabled);
    INSTALL_HOOK.call_once(|| unsafe {
        let thread_id = get_current_thread_id();
        let hook = set_windows_hook_ex_w(WH_CALLWNDPROC, Some(call_window_proc_hook), 0, thread_id);
        if hook == 0 {
            crate::log_debug("Dark mode: failed to install the UI creation hook");
        }
    });
}

pub fn set_dark_mode(enabled: bool) {
    DARK_MODE.store(enabled, Ordering::Relaxed);
    set_preferred_app_mode(enabled);
    flush_menu_themes();
    unsafe {
        let thread_id = get_current_thread_id();
        let enumerated = enum_thread_windows(thread_id, Some(apply_top_level_callback), 0);
        if enumerated == 0 {
            crate::log_debug("Dark mode: no top-level windows were available for refresh");
        }
    }
}

pub fn is_dark_mode() -> bool {
    DARK_MODE.load(Ordering::Relaxed)
}

pub fn effective_editor_text_color(text_color: u32) -> u32 {
    if !is_dark_mode() {
        return text_color;
    }
    match text_color {
        0x000000 => 0xE6E6E6,
        0x800000 => 0xFFCC99,
        0x006400 => 0x99CC99,
        0x002850 => 0x99B2CC,
        0x404040 => 0xC0C0C0,
        _ => text_color,
    }
}

pub fn apply_to_window(hwnd: windows::Win32::Foundation::HWND) {
    attach_window_tree(hwnd.0);
}

unsafe extern "system" fn call_window_proc_hook(code: i32, wparam: usize, lparam: isize) -> isize {
    if code >= HC_ACTION && lparam != 0 {
        let message = unsafe { &*(lparam as *const CallWndProcMessage) };
        if matches!(message.message, WM_CREATE | WM_INITDIALOG | WM_SHOWWINDOW) {
            attach_window(message.hwnd);
        }
    }
    unsafe { call_next_hook_ex(0, code, wparam, lparam) }
}

unsafe extern "system" fn apply_top_level_callback(hwnd: isize, _lparam: isize) -> i32 {
    attach_window_tree(hwnd);
    1
}

unsafe extern "system" fn apply_child_callback(hwnd: isize, _lparam: isize) -> i32 {
    attach_window(hwnd);
    1
}

fn attach_window_tree(hwnd: isize) {
    if is_system_dialog_tree(hwnd) {
        return;
    }
    attach_window(hwnd);
    unsafe {
        enum_child_windows(hwnd, Some(apply_child_callback), 0);
        draw_menu_bar(hwnd);
    }
}

fn attach_window(hwnd: isize) {
    if hwnd == 0 || is_system_dialog_tree(hwnd) {
        return;
    }
    let class_name = window_class_name(hwnd).to_ascii_lowercase();
    if should_subclass_window(&class_name) {
        unsafe {
            let attached =
                set_window_subclass(hwnd, Some(theme_subclass_proc), THEME_SUBCLASS_ID, 0);
            if attached == 0 {
                crate::log_debug("Dark mode: SetWindowSubclass failed for a window");
            }
        }
    }
    apply_window_appearance_with_class(hwnd, &class_name);
}

fn should_subclass_window(class_name: &str) -> bool {
    class_name
        .get(..8)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("sonarpad"))
}

/// Windows common dialogs and MessageBox windows use the system `#32770`
/// dialog class. Keep their entire window tree native: forcing our custom
/// subclass/theme while they are being created can leave the dialog alive but
/// unusable/invisible to keyboard focus and assistive technology. Sonarpad's
/// own top-level windows use dedicated `Sonarpad...` classes and continue to
/// receive the dark theme normally.
fn is_system_dialog_tree(hwnd: isize) -> bool {
    if hwnd == 0 {
        return false;
    }
    let root = unsafe { get_ancestor(hwnd, GA_ROOT) };
    let root = if root == 0 { hwnd } else { root };
    window_class_name(root).eq_ignore_ascii_case("#32770")
}

unsafe extern "system" fn theme_subclass_proc(
    hwnd: isize,
    message: u32,
    wparam: usize,
    lparam: isize,
    _subclass_id: usize,
    _reference_data: usize,
) -> isize {
    if message == WM_NCDESTROY {
        unsafe {
            remove_window_subclass(hwnd, Some(theme_subclass_proc), THEME_SUBCLASS_ID);
            return def_subclass_proc(hwnd, message, wparam, lparam);
        }
    }

    if is_dark_mode() {
        match message {
            WM_ERASEBKGND => {
                let mut rect = RawRect {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                };
                unsafe {
                    if get_client_rect(hwnd, &mut rect) != 0 {
                        fill_rect(wparam as isize, &rect, dark_brush());
                        return 1;
                    }
                }
            }
            WM_CTLCOLORMSGBOX | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX | WM_CTLCOLORBTN
            | WM_CTLCOLORDLG | WM_CTLCOLORSCROLLBAR | WM_CTLCOLORSTATIC => {
                let hdc = wparam as isize;
                unsafe {
                    set_text_color(hdc, DARK_TEXT);
                    set_bk_color(hdc, DARK_BACKGROUND);
                    if matches!(message, WM_CTLCOLORBTN | WM_CTLCOLORSTATIC) {
                        set_bk_mode(hdc, TRANSPARENT);
                    }
                }
                return dark_brush();
            }
            _ => {}
        }
    }

    unsafe { def_subclass_proc(hwnd, message, wparam, lparam) }
}

fn apply_window_appearance_with_class(hwnd: isize, class_name: &str) {
    let dark = is_dark_mode();
    allow_dark_mode_for_window(hwnd, dark);
    apply_title_bar(hwnd, dark);

    unsafe {
        if dark {
            let theme_name = wide("DarkMode_Explorer");
            set_window_theme(hwnd, theme_name.as_ptr(), std::ptr::null());
        } else {
            set_window_theme(hwnd, std::ptr::null(), std::ptr::null());
        }

        if class_name.contains("richedit") {
            if dark {
                send_message_w(hwnd, EM_SETBKGNDCOLOR, 0, DARK_BACKGROUND as isize);
            } else {
                send_message_w(hwnd, EM_SETBKGNDCOLOR, 1, 0);
            }
        } else if class_name == "syslistview32" {
            let background = if dark {
                DARK_SURFACE
            } else {
                get_sys_color(COLOR_WINDOW)
            };
            let text = if dark {
                DARK_TEXT
            } else {
                get_sys_color(COLOR_WINDOWTEXT)
            };
            send_message_w(hwnd, LVM_SETBKCOLOR, 0, background as isize);
            send_message_w(hwnd, LVM_SETTEXTBKCOLOR, 0, background as isize);
            send_message_w(hwnd, LVM_SETTEXTCOLOR, 0, text as isize);
        } else if class_name == "systreeview32" {
            let background = if dark {
                DARK_SURFACE
            } else {
                get_sys_color(COLOR_WINDOW)
            };
            let text = if dark {
                DARK_TEXT
            } else {
                get_sys_color(COLOR_WINDOWTEXT)
            };
            send_message_w(hwnd, TVM_SETBKCOLOR, 0, background as isize);
            send_message_w(hwnd, TVM_SETTEXTCOLOR, 0, text as isize);
        }

        redraw_window(
            hwnd,
            std::ptr::null(),
            0,
            RDW_INVALIDATE | RDW_ERASE | RDW_FRAME | RDW_ALLCHILDREN,
        );
    }
}

fn apply_title_bar(hwnd: isize, dark: bool) {
    unsafe {
        if get_ancestor(hwnd, GA_ROOT) != hwnd {
            return;
        }
        let enabled = if dark { 1 } else { 0 };
        let value = &enabled as *const i32 as *const c_void;
        let size = std::mem::size_of::<i32>() as u32;
        let result = dwm_set_window_attribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, value, size);
        if result < 0 {
            dwm_set_window_attribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE_BEFORE_20H1, value, size);
        }
    }
}

fn window_class_name(hwnd: isize) -> String {
    let mut buffer = [0u16; 128];
    let length = unsafe { get_class_name_w(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) };
    if length <= 0 {
        String::new()
    } else {
        String::from_utf16_lossy(&buffer[..length as usize])
    }
}

fn dark_brush() -> isize {
    *DARK_BRUSH.get_or_init(|| unsafe {
        let brush = create_solid_brush(DARK_BACKGROUND);
        if brush == 0 {
            (COLOR_WINDOW + 1) as usize
        } else {
            brush as usize
        }
    }) as isize
}

fn set_preferred_app_mode(dark: bool) {
    if let Some(set_mode) = set_preferred_app_mode_function() {
        let mode = if dark {
            PREFERRED_APP_MODE_FORCE_DARK
        } else {
            PREFERRED_APP_MODE_FORCE_LIGHT
        };
        unsafe {
            set_mode(mode);
        }
    }
    if let Some(refresh) = refresh_immersive_color_policy_state() {
        unsafe { refresh() };
    }
}

fn allow_dark_mode_for_window(hwnd: isize, dark: bool) {
    if let Some(allow) = allow_dark_mode_for_window_function() {
        unsafe {
            allow(hwnd, if dark { 1 } else { 0 });
        }
    }
}

fn flush_menu_themes() {
    if let Some(flush) = flush_menu_themes_function() {
        unsafe { flush() };
    }
}

fn uxtheme_module() -> isize {
    *UXTHEME_MODULE.get_or_init(|| {
        let name = wide("uxtheme.dll");
        unsafe { load_library_w(name.as_ptr()) as usize }
    }) as isize
}

fn uxtheme_ordinal(ordinal: usize) -> *mut c_void {
    let module = uxtheme_module();
    if module == 0 {
        return std::ptr::null_mut();
    }
    let ordinal_pointer = std::ptr::without_provenance::<u8>(ordinal);
    unsafe { get_proc_address(module, ordinal_pointer) }
}

fn set_preferred_app_mode_function() -> Option<SetPreferredAppModeFn> {
    let pointer = uxtheme_ordinal(UXTHEME_SET_PREFERRED_APP_MODE);
    if pointer.is_null() {
        None
    } else {
        Some(unsafe { std::mem::transmute::<*mut c_void, SetPreferredAppModeFn>(pointer) })
    }
}

fn allow_dark_mode_for_window_function() -> Option<AllowDarkModeForWindowFn> {
    let pointer = uxtheme_ordinal(UXTHEME_ALLOW_DARK_MODE_FOR_WINDOW);
    if pointer.is_null() {
        None
    } else {
        Some(unsafe { std::mem::transmute::<*mut c_void, AllowDarkModeForWindowFn>(pointer) })
    }
}

fn refresh_immersive_color_policy_state() -> Option<RefreshImmersiveColorPolicyStateFn> {
    let pointer = uxtheme_ordinal(UXTHEME_REFRESH_IMMERSIVE_COLOR_POLICY_STATE);
    if pointer.is_null() {
        None
    } else {
        Some(unsafe {
            std::mem::transmute::<*mut c_void, RefreshImmersiveColorPolicyStateFn>(pointer)
        })
    }
}

fn flush_menu_themes_function() -> Option<FlushMenuThemesFn> {
    let pointer = uxtheme_ordinal(UXTHEME_FLUSH_MENU_THEMES);
    if pointer.is_null() {
        None
    } else {
        Some(unsafe { std::mem::transmute::<*mut c_void, FlushMenuThemesFn>(pointer) })
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::should_subclass_window;

    #[test]
    fn native_system_dialogs_are_not_subclassed_by_dark_theme() {
        assert!(!should_subclass_window("#32770"));
        assert!(should_subclass_window("SonarpadWin32"));
        assert!(should_subclass_window("SonarpadCreateAudioDescription"));
    }
}
