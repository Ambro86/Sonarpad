"""Static checks for optional, accessible Windows Japanese OCR installation.
Real UAC/Windows integration requires Windows and is not simulated by these tests.
"""
from pathlib import Path
import json

root = Path(__file__).resolve().parents[1]
main = (root/'src/main.rs').read_text(encoding='utf-8')
module = (root/'src/japanese_ocr_installer.rs').read_text(encoding='utf-8')
progress = (root/'src/app_windows/podcast_save_window.rs').read_text(encoding='utf-8')
ocr = (root/'src/win_ocr.rs').read_text(encoding='utf-8')
assert 'mod japanese_ocr_installer;' in main
assert 'japanese_ocr_installer::start(hwnd, language)' in main
assert 'japanese_ocr_installer::handle_update(hwnd, lparam)' in main
assert main.count('offer_japanese_ocr_installation_on_request(hwnd, language);') == 1
assert 'WM_JAPANESE_OCR_INSTALL_UPDATE' in main
assert 'offer_japanese_ocr_installation' not in main.split('Ok(PdfTextResult::NoText) => {',1)[1].split('Err(message) => {',1)[0]
assert 'if choice != IDYES' in main
assert 'ShellExecuteExW(&mut exec)' in module and 'w!("runas")' in module
assert 'Language.OCR~~~ja-JP~0.0.1.0' in module
assert 'Add-WindowsCapability -Online -Name' in module and '$psi.Arguments' not in module
assert '-EncodedCommand' in module and '-WindowStyle Hidden' in module
assert 'show_indeterminate_progress(dialog)' in module
assert 'state.has_real_progress = true' in progress
assert 'state.show_percentage = false' in progress
assert 'state.show_percentage = true' in progress
assert 'if state.show_percentage' in progress
assert 'pct % 10 == 0' in module
assert 'WAIT_OBJECT_0' in module and 'CloseHandle(handle)' in module
assert 'Stop-Job -Job $job' in module
assert 'japanese_ocr_installer::cancel(' in main
assert 'japanese_ocr_installer::active_dialog()' in main
assert 'hwnd, language, labels, true, true' in module
assert 'crate::bring_modal_window_to_foreground(dialog)' in module
assert 'DONE:' in module and 'FAILED:' in module
assert 'japanese_ocr_is_available' in ocr
langs = list((root/'i18n').glob('*.json'))
assert len(langs) == 17
for p in langs:
    d = json.loads(p.read_text(encoding='utf-8'))
    for key in ('app.pdf_japanese_ocr_install_progress', 'app.pdf_japanese_ocr_install_success', 'app.pdf_japanese_ocr_install_restart', 'app.pdf_japanese_ocr_install_error', 'app.pdf_japanese_ocr_install_cancel_confirm', 'app.pdf_japanese_ocr_install_canceling', 'app.pdf_japanese_ocr_install_cancelled'):
        assert d.get(key), (p.name, key)
cl = (root/'CHANGELOG_IT.md').read_text(encoding='utf-8')
assert cl.startswith('# Changelog\n\nVersione 0.9.14 – 2026-10-12')
assert '15. L’installazione facoltativa' in cl
assert '12. Nascosta la sezione **Dirette** dal menu RaiPlay' in cl
print('OK: explicit consent, elevated PowerShell Add-WindowsCapability, modal cancel, accurate indeterminate progress, 17 languages, no PDF-open prompt')
