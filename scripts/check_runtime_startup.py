"""Audit startup imports and simulate a missing runtime on a disposable EXE copy.

Requires pefile==2024.8.26. Does not remove or alter any installed runtime.
The renamed probes exist only in the disposable test copy, never the release.
"""
import argparse
from pathlib import Path
import subprocess
import tempfile
import time

import pefile


def check_missing_dialog(executable):
    """Exercise the actual accessible dialog and No, without opening a browser."""
    import ctypes
    from ctypes import wintypes
    user32 = ctypes.WinDLL('user32', use_last_error=True)
    callback_type = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    user32.EnumWindows.argtypes = [callback_type, wintypes.LPARAM]
    user32.EnumChildWindows.argtypes = [wintypes.HWND, callback_type, wintypes.LPARAM]
    user32.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
    user32.GetDlgItem.argtypes = [wintypes.HWND, ctypes.c_int]
    user32.GetDlgItem.restype = wintypes.HWND
    user32.GetWindowTextW.argtypes = [wintypes.HWND, wintypes.LPWSTR, ctypes.c_int]
    user32.SendMessageW.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM]
    user32.SendMessageW.restype = wintypes.LPARAM
    process = subprocess.Popen([str(executable)])
    try:
        windows = []
        @callback_type
        def find(window, _):
            owner = wintypes.DWORD()
            user32.GetWindowThreadProcessId(window, ctypes.byref(owner))
            if owner.value == process.pid and user32.GetDlgItem(window, 6) and user32.GetDlgItem(window, 7):
                windows.append(window)
            return True
        deadline = time.monotonic() + 30
        while not windows and time.monotonic() < deadline and process.poll() is None:
            user32.EnumWindows(find, 0)
            time.sleep(0.1)
        assert windows, 'Missing-runtime Yes/No dialog did not appear'
        texts = []
        @callback_type
        def read_text(window, _):
            text = ctypes.create_unicode_buffer(4096)
            user32.GetWindowTextW(window, text, len(text))
            texts.append(text.value)
            return True
        user32.EnumChildWindows(windows[0], read_text, 0)
        assert any('Microsoft Visual C++ Redistributable x64' in text for text in texts), texts
        # Native IDNO=7, BM_CLICK=0xF5. No mouse/keyboard input is stolen.
        user32.SendMessageW(user32.GetDlgItem(windows[0], 7), 0xF5, 0, 0)
        assert process.wait(timeout=15) == 0
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()


def check(executable, allow_missing_host_runtime=False):
    executable = executable.resolve()
    pe = pefile.PE(str(executable), fast_load=True)
    pe.parse_data_directories(directories=[1])
    assert pe.FILE_HEADER.Machine == 0x8664, 'Expected an x64 executable'
    imports = [entry.dll.decode('ascii').lower() for entry in pe.DIRECTORY_ENTRY_IMPORT]
    forbidden = [name for name in imports if name.startswith(('vcruntime', 'msvcp'))
                 or (name.startswith('msvcr') and name != 'msvcrt.dll')]
    assert not forbidden, f'Startup still depends on the redistributable: {forbidden}'
    # Every direct import must be a Windows component, so another app-local
    # startup DLL cannot hide a transitive VC++ dependency from this audit.
    import os
    system = Path(os.environ['SystemRoot']) / 'System32'
    unexpected = [name for name in imports if not name.startswith(('api-ms-', 'ext-ms-'))
                  and not (system / name).is_file()]
    assert not unexpected, f'Unexpected non-system startup dependencies: {unexpected}'
    result = subprocess.run([str(executable), '--check-runtime'], timeout=30)
    allowed = (0, 2) if allow_missing_host_runtime else (0,)
    assert result.returncode in allowed, f'Host-runtime check failed: {result.returncode}'
    if result.returncode == 2:
        print('Host runtime is missing/incomplete; testing the original executable dialog too.')
        check_missing_dialog(executable)
    data = bytearray(executable.read_bytes())
    # Rename only the probe's strings in .rdata, keeping PE layout and all
    # imports intact. Windows then really fails these LoadLibraryExW calls.
    section = next(s for s in pe.sections if s.Name.rstrip(b'\0') == b'.rdata')
    start, size = section.PointerToRawData, section.SizeOfRawData
    content = bytes(data[start:start + size])
    for original in (b'vcruntime140.dll', b'vcruntime140_1.dll', b'msvcp140.dll'):
        replacement = b'z' + original[1:]
        assert original in content, f'Missing probe literal: {original}'
        content = content.replace(original, replacement)
    data[start:start + size] = content
    pe.close()
    with tempfile.TemporaryDirectory(prefix='sonarpad-runtime-test-') as folder:
        simulated = Path(folder) / 'sonarpad-runtime-missing-test.exe'
        simulated.write_bytes(data)
        result = subprocess.run([str(simulated), '--check-runtime'], timeout=30)
        assert result.returncode == 2, f'Missing-runtime path failed: {result.returncode}'
        check_missing_dialog(simulated)
    print(f'{executable.name}: static startup, host-runtime and simulated missing-runtime/dialog checks passed.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--allow-missing-host-runtime', action='store_true')
    parser.add_argument('executable', type=Path, nargs='+')
    args = parser.parse_args()
    for path in args.executable:
        check(path, args.allow_missing_host_runtime)
