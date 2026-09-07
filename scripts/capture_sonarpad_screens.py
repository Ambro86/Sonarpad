#!/usr/bin/env python3
"""Create a screenshot catalogue of SonarPad's native Windows UI.

The application is launched with an isolated APPDATA profile.  Windows are
opened through their WM_COMMAND identifiers, captured, closed, and documented
in a Markdown manifest.  Requires Windows and Pillow.
"""

from __future__ import annotations

import argparse
import ctypes
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
import zipfile
from ctypes import wintypes
from dataclasses import dataclass
from pathlib import Path

from PIL import Image, ImageChops, ImageDraw, ImageFont, ImageGrab


if os.name != "nt":
    raise SystemExit("This capture utility requires Windows.")


user32 = ctypes.WinDLL("user32", use_last_error=True)
dwmapi = ctypes.WinDLL("dwmapi", use_last_error=True)

WM_COMMAND = 0x0111
WM_CLOSE = 0x0010
WM_VSCROLL = 0x0115
SB_TOP = 6
SB_PAGEDOWN = 3
SW_RESTORE = 9
SW_MAXIMIZE = 3
GW_OWNER = 4
VK_CONTROL = 0x11
VK_TAB = 0x09
KEYEVENTF_KEYUP = 0x0002
DWMWA_EXTENDED_FRAME_BOUNDS = 9

EnumWindowsProc = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)

user32.EnumWindows.argtypes = [EnumWindowsProc, wintypes.LPARAM]
user32.EnumWindows.restype = wintypes.BOOL
user32.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
user32.GetWindowThreadProcessId.restype = wintypes.DWORD
user32.IsWindowVisible.argtypes = [wintypes.HWND]
user32.IsWindowVisible.restype = wintypes.BOOL
user32.GetWindowTextLengthW.argtypes = [wintypes.HWND]
user32.GetWindowTextLengthW.restype = ctypes.c_int
user32.GetWindowTextW.argtypes = [wintypes.HWND, wintypes.LPWSTR, ctypes.c_int]
user32.GetWindowTextW.restype = ctypes.c_int
user32.GetClassNameW.argtypes = [wintypes.HWND, wintypes.LPWSTR, ctypes.c_int]
user32.GetClassNameW.restype = ctypes.c_int
user32.GetWindowRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
user32.GetWindowRect.restype = wintypes.BOOL
user32.ShowWindow.argtypes = [wintypes.HWND, ctypes.c_int]
user32.ShowWindow.restype = wintypes.BOOL
user32.SetForegroundWindow.argtypes = [wintypes.HWND]
user32.SetForegroundWindow.restype = wintypes.BOOL
user32.BringWindowToTop.argtypes = [wintypes.HWND]
user32.BringWindowToTop.restype = wintypes.BOOL
user32.PostMessageW.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM]
user32.PostMessageW.restype = wintypes.BOOL
user32.GetWindow.argtypes = [wintypes.HWND, wintypes.UINT]
user32.GetWindow.restype = wintypes.HWND
user32.keybd_event.argtypes = [wintypes.BYTE, wintypes.BYTE, wintypes.DWORD, ctypes.c_void_p]
user32.keybd_event.restype = None


@dataclass(frozen=True)
class ScreenCommand:
    slug: str
    label: str
    command_id: int
    wait: float = 1.4


# Commands that open a window without requiring a destructive confirmation.
SCREEN_COMMANDS = [
    ScreenCommand("modifica-trova", "Editor - Trova", 2006),
    ScreenCommand("modifica-sostituisci", "Editor - Sostituisci", 2008),
    ScreenCommand("modifica-trova-nei-file", "Editor - Trova nei file", 2009),
    ScreenCommand("modifica-vai-alla-riga", "Editor - Vai alla riga", 2025),
    ScreenCommand("segnalibri", "Gestione segnalibri", 2102),
    ScreenCommand("audiolibri-batch", "Audiolibri multipli", 1013),
    ScreenCommand("registra-podcast", "Registrazione podcast", 1012),
    ScreenCommand("converti-audio", "Conversione audio", 1016),
    ScreenCommand("dizionario", "Dizionario", 5002),
    ScreenCommand("wikipedia", "Importazione Wikipedia", 5008),
    ScreenCommand("gutenberg", "Project Gutenberg", 5023, 2.5),
    ScreenCommand("internet-archive", "Internet Archive", 5024, 2.5),
    ScreenCommand("librivox", "LibriVox", 5025, 2.5),
    ScreenCommand("treccani", "Treccani", 5022, 2.0),
    ScreenCommand("prompt", "Prompt", 5004),
    ScreenCommand("rss", "RSS", 5005, 2.0),
    ScreenCommand("podcast", "Podcast", 5006, 2.0),
    ScreenCommand("bdciechi", "bdCiechi", 5010, 2.0),
    ScreenCommand("rai-audiodescrizioni", "Rai audiodescrizioni", 5012, 2.5),
    ScreenCommand("raiplay-sound", "RaiPlay Sound", 5013, 2.5),
    ScreenCommand("raiplay", "RaiPlay", 5014, 2.5),
    ScreenCommand("la7-play", "LA7 Play", 5026, 2.5),
    ScreenCommand("radio", "Radio", 5016, 2.5),
    ScreenCommand("tv", "TV", 5018, 2.5),
    ScreenCommand("italiaonline", "Elenchi Italiaonline", 5015, 2.0),
    ScreenCommand("calendario", "Calendario", 5021),
    ScreenCommand("meteo", "Meteo", 5019, 2.0),
    ScreenCommand("cinema", "Cinema", 5020, 2.0),
    ScreenCommand("percorsi", "Percorsi e navigazione", 5017, 2.0),
    ScreenCommand("guida", "Guida", 7001),
    ScreenCommand("changelog", "Novita / Changelog", 7004),
    ScreenCommand("feedback", "Invia feedback", 7005),
    ScreenCommand("donazioni", "Donazioni", 7006),
    ScreenCommand("informazioni", "Informazioni su SonarPad", 7002),
]

SETTINGS_TABS = [
    "generali",
    "voce",
    "editor",
    "audio",
    "rss-podcast",
    "ia-trascrizione",
    "scorciatoie",
]


def window_text(hwnd: int) -> str:
    length = user32.GetWindowTextLengthW(hwnd)
    buf = ctypes.create_unicode_buffer(max(length + 1, 2))
    user32.GetWindowTextW(hwnd, buf, len(buf))
    return buf.value


def class_name(hwnd: int) -> str:
    buf = ctypes.create_unicode_buffer(256)
    user32.GetClassNameW(hwnd, buf, len(buf))
    return buf.value


def process_windows(pid: int, visible_only: bool = True) -> list[int]:
    found: list[int] = []

    @EnumWindowsProc
    def callback(hwnd: int, _lparam: int) -> bool:
        window_pid = wintypes.DWORD()
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(window_pid))
        if window_pid.value == pid and (not visible_only or user32.IsWindowVisible(hwnd)):
            found.append(int(hwnd))
        return True

    user32.EnumWindows(callback, 0)
    return found


def wait_for_main_window(pid: int, timeout: float = 25.0) -> int:
    deadline = time.time() + timeout
    while time.time() < deadline:
        candidates = process_windows(pid)
        for hwnd in candidates:
            if class_name(hwnd) in {"SonarpadWin32", "SonarpadWindow"}:
                return hwnd
        if candidates:
            # The main window title contains SonarPad even if the class changes.
            titled = [h for h in candidates if "sonarpad" in window_text(h).lower()]
            if titled:
                return max(titled, key=lambda h: window_area(h))
        time.sleep(0.2)
    raise RuntimeError("The SonarPad main window did not appear.")


def window_rect(hwnd: int) -> tuple[int, int, int, int]:
    rect = wintypes.RECT()
    hr = dwmapi.DwmGetWindowAttribute(
        wintypes.HWND(hwnd),
        wintypes.DWORD(DWMWA_EXTENDED_FRAME_BOUNDS),
        ctypes.byref(rect),
        ctypes.sizeof(rect),
    )
    if hr != 0 or rect.right <= rect.left or rect.bottom <= rect.top:
        if not user32.GetWindowRect(hwnd, ctypes.byref(rect)):
            raise ctypes.WinError(ctypes.get_last_error())
    return rect.left, rect.top, rect.right, rect.bottom


def window_area(hwnd: int) -> int:
    try:
        left, top, right, bottom = window_rect(hwnd)
        return max(0, right - left) * max(0, bottom - top)
    except OSError:
        return 0


def foreground(hwnd: int, maximize: bool = False) -> None:
    user32.ShowWindow(hwnd, SW_MAXIMIZE if maximize else SW_RESTORE)
    user32.BringWindowToTop(hwnd)
    user32.SetForegroundWindow(hwnd)
    time.sleep(0.45)


def sanitize_image(image: Image.Image) -> Image.Image:
    # Password edit controls are masked by SonarPad itself. This hook also strips
    # metadata so PNG files cannot retain machine-specific information.
    clean = Image.new("RGB", image.size, "white")
    clean.paste(image.convert("RGB"))
    return clean


def capture_window(hwnd: int, path: Path, maximize: bool = False) -> str:
    foreground(hwnd, maximize=maximize)
    left, top, right, bottom = window_rect(hwnd)
    image = ImageGrab.grab(bbox=(left, top, right, bottom), all_screens=True)
    image = sanitize_image(image)
    path.parent.mkdir(parents=True, exist_ok=True)
    image.save(path, "PNG", optimize=True)
    return hashlib.sha256(image.tobytes()).hexdigest()


def post_command(main_hwnd: int, command_id: int) -> None:
    if not user32.PostMessageW(main_hwnd, WM_COMMAND, command_id, 0):
        raise ctypes.WinError(ctypes.get_last_error())


def newest_dialog(pid: int, main_hwnd: int, previous: set[int]) -> int | None:
    windows = process_windows(pid)
    new = [h for h in windows if h != main_hwnd and h not in previous]
    if new:
        return max(new, key=window_area)
    owned = [
        h
        for h in windows
        if h != main_hwnd and int(user32.GetWindow(h, GW_OWNER) or 0) == main_hwnd
    ]
    return max(owned, key=window_area) if owned else None


def wait_for_dialog(pid: int, main_hwnd: int, previous: set[int], timeout: float) -> int | None:
    deadline = time.time() + timeout
    while time.time() < deadline:
        dialog = newest_dialog(pid, main_hwnd, previous)
        if dialog and window_area(dialog) > 10_000:
            return dialog
        time.sleep(0.15)
    return None


def close_secondary_windows(pid: int, main_hwnd: int) -> None:
    for _attempt in range(2):
        for hwnd in process_windows(pid):
            # Never close an editor window. Some Win32 common dialogs can cause the
            # application to rebuild or change its active top-level handle.
            if hwnd != main_hwnd and class_name(hwnd) not in {"SonarpadWin32", "SonarpadWindow"}:
                # Modal Yes/No and OK/Cancel message boxes can ignore WM_CLOSE.
                # Select No/Cancel first, then request a normal close.
                user32.PostMessageW(hwnd, WM_COMMAND, 7, 0)  # IDNO
                user32.PostMessageW(hwnd, WM_COMMAND, 2, 0)  # IDCANCEL
                user32.PostMessageW(hwnd, WM_CLOSE, 0, 0)
        time.sleep(0.45)


def ctrl_tab(hwnd: int) -> None:
    foreground(hwnd)
    user32.keybd_event(VK_CONTROL, 0, 0, None)
    user32.keybd_event(VK_TAB, 0, 0, None)
    user32.keybd_event(VK_TAB, 0, KEYEVENTF_KEYUP, None)
    user32.keybd_event(VK_CONTROL, 0, KEYEVENTF_KEYUP, None)
    time.sleep(0.5)


def reset_scroll(hwnd: int) -> None:
    user32.PostMessageW(hwnd, WM_VSCROLL, SB_TOP, 0)
    time.sleep(0.35)


def page_scroll(hwnd: int) -> None:
    user32.PostMessageW(hwnd, WM_VSCROLL, SB_PAGEDOWN, 0)
    time.sleep(0.4)


def image_difference(a: Path, b: Path) -> float:
    with Image.open(a).convert("RGB") as left, Image.open(b).convert("RGB") as right:
        if left.size != right.size:
            return 1.0
        diff = ImageChops.difference(left, right).convert("L")
        histogram = diff.histogram()
        changed = sum(count for value, count in enumerate(histogram) if value > 3)
        return changed / float(left.width * left.height)


def make_contact_sheet(images: list[Path], target: Path) -> None:
    thumbs: list[tuple[Path, Image.Image]] = []
    for path in images:
        with Image.open(path) as image:
            thumb = image.convert("RGB")
            thumb.thumbnail((420, 260))
            thumbs.append((path, thumb.copy()))
    if not thumbs:
        return
    cell_w, cell_h = 450, 310
    columns = 3
    rows = (len(thumbs) + columns - 1) // columns
    sheet = Image.new("RGB", (cell_w * columns, cell_h * rows), "#eeeeee")
    draw = ImageDraw.Draw(sheet)
    font = ImageFont.load_default()
    for index, (path, thumb) in enumerate(thumbs):
        x = (index % columns) * cell_w
        y = (index // columns) * cell_h
        sheet.paste(thumb, (x + (cell_w - thumb.width) // 2, y + 8))
        label = path.stem[:64]
        draw.text((x + 12, y + 278), label, fill="black", font=font)
    sheet.save(target, "PNG", optimize=True)


def safe_slug(value: str) -> str:
    return "".join(c if c.isalnum() or c in "-_" else "-" for c in value.lower()).strip("-")


def create_manifest(output: Path, rows: list[tuple[str, Path, str]], skipped: list[str]) -> None:
    lines = [
        "# Catalogo schermate SonarPad",
        "",
        "Acquisizione automatica eseguita con un profilo temporaneo isolato.",
        "Le immagini non contengono metadati e i campi password sono mascherati dall'applicazione.",
        "",
        f"Schermate acquisite: **{len(rows)}**",
        "",
        "| # | Schermata | File | Titolo finestra |",
        "|---:|---|---|---|",
    ]
    for index, (label, path, title) in enumerate(rows, 1):
        rel = path.relative_to(output).as_posix()
        clean_title = title.replace("|", "\\|") or "(senza titolo)"
        lines.append(f"| {index} | {label} | [{rel}]({rel}) | {clean_title} |")
    if skipped:
        lines += ["", "## Schermate non acquisite", ""]
        lines += [f"- {item}" for item in skipped]
    lines += [
        "",
        "## Note per l'analisi con GPT",
        "",
        "Controllare coerenza visiva, testi troncati, sovrapposizioni, allineamento,",
        "contrasto, chiarezza delle etichette, gerarchia dei controlli e facilità d'uso.",
    ]
    (output / "INDICE.md").write_text("\n".join(lines) + "\n", encoding="utf-8")


def zip_output(output: Path, zip_path: Path) -> None:
    if zip_path.exists():
        zip_path.unlink()
    with zipfile.ZipFile(zip_path, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for path in sorted(output.rglob("*")):
            if path.is_file():
                archive.write(path, path.relative_to(output.parent))


def run_capture(
    executable: Path,
    output: Path,
    zip_path: Path,
    use_saved_rai_code: bool = False,
) -> None:
    if output.exists():
        shutil.rmtree(output)
    output.mkdir(parents=True)
    isolated = Path(tempfile.mkdtemp(prefix="sonarpad-screens-"))
    env = os.environ.copy()
    env["APPDATA"] = str(isolated / "AppData" / "Roaming")
    env["LOCALAPPDATA"] = str(isolated / "AppData" / "Local")
    Path(env["APPDATA"]).mkdir(parents=True, exist_ok=True)
    Path(env["LOCALAPPDATA"]).mkdir(parents=True, exist_ok=True)

    if use_saved_rai_code:
        current_appdata = os.environ.get("APPDATA", "").strip()
        source_settings = Path(current_appdata) / "Sonarpad" / "settings.json"
        if not source_settings.is_file():
            raise RuntimeError(f"Current SonarPad settings not found: {source_settings}")
        source_data = json.loads(source_settings.read_text(encoding="utf-8"))
        encrypted_code = str(source_data.get("rai_luce_code", "")).strip()
        if not encrypted_code:
            raise RuntimeError("No saved Rai/SonarPad code was found in current settings.")
        isolated_settings_dir = Path(env["APPDATA"]) / "Sonarpad"
        isolated_settings_dir.mkdir(parents=True, exist_ok=True)
        # Copy only the encrypted code. Personal paths, API keys, history and
        # credentials from the real profile never enter the capture profile.
        (isolated_settings_dir / "settings.json").write_text(
            json.dumps({"rai_luce_code": encrypted_code}, ensure_ascii=False, indent=2),
            encoding="utf-8",
        )

    rows: list[tuple[str, Path, str]] = []
    skipped: list[str] = []
    captured_code_prompts: set[str] = set()
    process: subprocess.Popen[bytes] | None = None
    counter = 1

    def save(hwnd: int, folder: str, slug: str, label: str, maximize: bool = False) -> Path:
        nonlocal counter
        path = output / folder / f"{counter:02d}-{safe_slug(slug)}.png"
        capture_window(hwnd, path, maximize=maximize)
        rows.append((label, path, window_text(hwnd)))
        counter += 1
        print(f"CAPTURED {path.relative_to(output)}", flush=True)
        return path

    try:
        process = subprocess.Popen(
            [str(executable)],
            cwd=str(executable.parent),
            env=env,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        main_hwnd = wait_for_main_window(process.pid)
        # Close a possible first-run changelog, then capture the clean editor.
        time.sleep(1.2)
        for hwnd in process_windows(process.pid):
            if hwnd != main_hwnd and class_name(hwnd) != "SonarpadOptions":
                user32.PostMessageW(hwnd, WM_CLOSE, 0, 0)
        time.sleep(0.6)
        save(main_hwnd, "editor", "editor-avvio", "Editor - schermata iniziale", maximize=True)

        # Useful editor layout variants, confined to the isolated profile.
        post_command(main_hwnd, 6101)
        time.sleep(0.8)
        save(main_hwnd, "editor", "editor-pannello-voci", "Editor - pannello voci", maximize=True)
        post_command(main_hwnd, 6102)
        time.sleep(0.8)
        save(main_hwnd, "editor", "editor-voci-preferite", "Editor - voci e preferiti", maximize=True)
        post_command(main_hwnd, 6107)
        time.sleep(0.9)
        save(main_hwnd, "editor", "editor-tema-scuro", "Editor - tema scuro", maximize=True)
        post_command(main_hwnd, 6107)
        post_command(main_hwnd, 6102)
        post_command(main_hwnd, 6101)
        time.sleep(0.7)

        # Settings: capture every tab and, where available, multiple scroll pages.
        main_hwnd = wait_for_main_window(process.pid, timeout=4.0)
        previous = set(process_windows(process.pid))
        post_command(main_hwnd, 5001)
        settings_hwnd = wait_for_dialog(process.pid, main_hwnd, previous, 8.0)
        if settings_hwnd:
            for tab_index, tab in enumerate(SETTINGS_TABS):
                if tab_index:
                    ctrl_tab(settings_hwnd)
                reset_scroll(settings_hwnd)
                top = save(
                    settings_hwnd,
                    "impostazioni",
                    f"impostazioni-{tab}-parte-1",
                    f"Impostazioni - {tab} (parte 1)",
                )
                prior = top
                # Long pages need more than one image; stop when scrolling no longer changes content.
                for part in range(2, 5):
                    page_scroll(settings_hwnd)
                    candidate = output / "impostazioni" / f"{counter:02d}-impostazioni-{tab}-parte-{part}.png"
                    capture_window(settings_hwnd, candidate)
                    if image_difference(prior, candidate) < 0.004 or candidate.stat().st_size < 50 * 1024:
                        candidate.unlink(missing_ok=True)
                        break
                    rows.append((f"Impostazioni - {tab} (parte {part})", candidate, window_text(settings_hwnd)))
                    print(f"CAPTURED {candidate.relative_to(output)}", flush=True)
                    counter += 1
                    prior = candidate
            user32.PostMessageW(settings_hwnd, WM_CLOSE, 0, 0)
            time.sleep(0.8)
        else:
            skipped.append("Impostazioni: finestra non rilevata")

        for command in SCREEN_COMMANDS:
            if process.poll() is not None:
                raise RuntimeError("SonarPad terminated before capture completed.")
            # Refresh the handle before every command: closing standard Win32
            # Find/Replace windows may change which editor window is active.
            main_hwnd = wait_for_main_window(process.pid, timeout=4.0)
            close_secondary_windows(process.pid, main_hwnd)
            foreground(main_hwnd)
            previous = set(process_windows(process.pid))
            post_command(main_hwnd, command.command_id)
            dialog = wait_for_dialog(process.pid, main_hwnd, previous, max(5.0, command.wait + 2.0))
            if not dialog:
                skipped.append(f"{command.label}: nessuna finestra rilevata")
                print(f"SKIPPED {command.label}", flush=True)
                continue
            time.sleep(command.wait)
            title = window_text(dialog).strip()
            if title.casefold().startswith("codice "):
                prompt_key = title.casefold()
                if prompt_key not in captured_code_prompts:
                    save(
                        dialog,
                        "finestre",
                        f"configurazione-{safe_slug(title)}",
                        f"Configurazione richiesta - {title}",
                    )
                    captured_code_prompts.add(prompt_key)
                skipped.append(f"{command.label}: richiede la configurazione '{title}'")
                print(f"BLOCKED {command.label} ({title})", flush=True)
            else:
                save(dialog, "finestre", command.slug, command.label)
            close_secondary_windows(process.pid, main_hwnd)

    finally:
        if process and process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
        shutil.rmtree(isolated, ignore_errors=True)

    pngs = sorted(p for p in output.rglob("*.png") if p.name != "PANORAMICA.png")
    make_contact_sheet(pngs, output / "PANORAMICA.png")
    create_manifest(output, rows, skipped)
    zip_output(output, zip_path)
    print(f"DONE {len(rows)} screenshots -> {zip_path}", flush=True)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--exe",
        type=Path,
        default=Path(__file__).resolve().parents[1] / "target" / "debug" / "sonarpad.exe",
        help="Path to sonarpad.exe",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=Path(__file__).resolve().parents[1] / "artifacts" / "sonarpad-schermate",
        help="Screenshot directory",
    )
    parser.add_argument(
        "--zip",
        dest="zip_path",
        type=Path,
        default=Path(__file__).resolve().parents[1] / "artifacts" / "sonarpad-schermate.zip",
        help="Final ZIP path",
    )
    parser.add_argument(
        "--use-saved-rai-code",
        action="store_true",
        help="Copy only the encrypted Rai/SonarPad code from the current profile",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    executable = args.exe.resolve()
    output = args.output.resolve()
    zip_path = args.zip_path.resolve()
    if not executable.is_file():
        print(f"Executable not found: {executable}", file=sys.stderr)
        return 2
    zip_path.parent.mkdir(parents=True, exist_ok=True)
    run_capture(
        executable,
        output,
        zip_path,
        use_saved_rai_code=args.use_saved_rai_code,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
