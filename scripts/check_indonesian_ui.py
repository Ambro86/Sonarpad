"""Exercise the actual standalone application with isolated settings."""
import ctypes as c, json, subprocess, time, sys, shutil, tempfile
from ctypes import wintypes as w
from pathlib import Path
source_exe = Path(sys.argv[1]).resolve()
assert source_exe.is_file(), source_exe
test_dir = Path(tempfile.mkdtemp(prefix='sonarpad-indonesian-ui-'))
exe = test_dir / 'sonarpad.exe'
shutil.copy2(source_exe, exe)
print('Isolated test folder:', test_dir, flush=True)
config = exe.parent / 'config'
config.mkdir(exist_ok=True)
settings = config / 'settings.json'
assert not settings.exists(), 'Use a fresh test directory; do not overwrite existing settings'
settings.write_text(json.dumps({'language': 'en', 'check_updates_on_startup': False, 'last_seen_changelog_version': '0.9.17', 'context_menu_open_with': False}), encoding='utf-8')
d = json.loads((Path(__file__).resolve().parents[1] / 'i18n/id.json').read_text(encoding='utf-8'))
u = c.WinDLL('user32', use_last_error=True)
callback = c.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM)
u.EnumWindows.argtypes = [callback, w.LPARAM]
u.EnumChildWindows.argtypes = [w.HWND, callback, w.LPARAM]
u.GetWindowThreadProcessId.argtypes = [w.HWND, c.POINTER(w.DWORD)]
u.GetDlgItem.argtypes = [w.HWND, c.c_int]
u.GetDlgItem.restype = w.HWND
u.GetMenu.argtypes = [w.HWND]
u.GetMenu.restype = w.HMENU
u.GetMenuItemCount.argtypes = [w.HMENU]
u.GetMenuStringW.argtypes = [w.HMENU, w.UINT, w.LPWSTR, c.c_int, w.UINT]
u.GetWindowTextW.argtypes = [w.HWND, w.LPWSTR, c.c_int]
u.SendMessageTimeoutW.argtypes = [w.HWND, w.UINT, w.WPARAM, w.LPARAM, w.UINT, w.UINT, c.POINTER(c.c_size_t)]
u.SendMessageTimeoutW.restype = w.LPARAM
u.PostMessageW.argtypes = [w.HWND, w.UINT, w.WPARAM, w.LPARAM]

def send(h, m, wp=0, lp=0):
    result = c.c_size_t()
    assert u.SendMessageTimeoutW(h, m, wp, lp, 2, 5000, c.byref(result)), f'Message {m} timed out'
    return result.value

def windows(process):
    found = []

    @callback
    def each(h, _):
        pid = w.DWORD()
        u.GetWindowThreadProcessId(h, c.byref(pid))
        if pid.value == process.pid:
            found.append(h)
        return True
    u.EnumWindows(each, 0)
    return found

def wait(fn):
    deadline = time.monotonic() + 45
    while time.monotonic() < deadline:
        result = fn()
        if result:
            return result
        time.sleep(0.15)
    raise AssertionError('Timed out waiting for application UI')

def main_window(p):
    return next((h for h in windows(p) if u.GetMenu(h)), None)

def labels(h):
    menu = u.GetMenu(h)
    out = []
    for i in range(u.GetMenuItemCount(menu)):
        b = c.create_unicode_buffer(1024)
        u.GetMenuStringW(menu, i, b, len(b), 1024)
        out.append(b.value)
    return out

def close(p, h):
    u.PostMessageW(h, 16, 0, 0)
    p.wait(timeout=20)

def launch():
    return subprocess.Popen([str(exe)], cwd=exe.parent)
p = launch()
try:
    h = wait(lambda: main_window(p))
    print('Initial menus:', labels(h), flush=True)
    u.PostMessageW(h, 273, 5001, 0)
    dialog = wait(lambda: next((x for x in windows(p) if u.GetDlgItem(x, 6001)), None))
    combo = u.GetDlgItem(dialog, 6001)
    count = send(combo, 326)
    assert count == 18, count
    text = c.create_unicode_buffer(200)
    send(combo, 328, 17, c.addressof(text))
    assert text.value == 'Bahasa Indonesia', text.value
    assert send(combo, 327) == 1, 'Expected English at startup'
    send(combo, 334, 17)
    u.PostMessageW(dialog, 273, 6005, 0)
    wait(lambda: d['menu.file'] in labels(h))
    print('Indonesian menus:', labels(h), flush=True)
    for key in ('menu.file', 'menu.edit', 'menu.view', 'menu.insert', 'menu.voice_audio', 'menu.tools', 'menu.window', 'menu.help'):
        assert d[key] in labels(h), (key, labels(h))
    wait(lambda: json.loads(settings.read_text(encoding='utf-8-sig'))['language'] == 'id')
    close(p, h)
    print('Language saved as id', flush=True)
    p = launch()
    h = wait(lambda: main_window(p))
    assert d['menu.file'] in labels(h), 'Language did not survive restart'
    u.PostMessageW(h, 273, 5001, 0)
    dialog = wait(lambda: next((x for x in windows(p) if u.GetDlgItem(x, 6001)), None))
    assert send(u.GetDlgItem(dialog, 6001), 327) == 17, 'Selected language did not survive restart'
    u.PostMessageW(dialog, 273, 6006, 0)
    wait(lambda: not u.GetDlgItem(dialog, 6001))
    close(p, h)
    print('PASS: actual UI language selection, translated menus, saved settings and restart persistence', flush=True)
finally:
    if p.poll() is None:
        p.kill()
        p.wait()
