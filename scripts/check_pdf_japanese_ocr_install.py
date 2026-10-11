"""Static regressions for explicit, non-invasive Japanese OCR installation.

Windows GUI/UAC integration must still be tested on Windows.
"""
from pathlib import Path
import json

root = Path(__file__).resolve().parents[1]
main = (root / 'src/main.rs').read_text(encoding='utf-8')
menu = (root / 'src/menu.rs').read_text(encoding='utf-8')
ocr = (root / 'src/win_ocr.rs').read_text(encoding='utf-8')
assert 'fn offer_japanese_ocr_installation_on_request(hwnd: HWND, language: Language)' in main
assert 'static JAPANESE_OCR_INSTALL_OFFERED' not in main
assert 'fn maybe_offer_japanese_ocr_installation' not in main
assert 'if win_ocr::japanese_ocr_is_available()' in main
assert 'app.pdf_japanese_ocr_installed' in main
assert 'if choice != IDYES' in main
assert 'w!("runas")' in main
assert "Language.OCR~~~ja-JP~0.0.1.0" in main
assert 'pub(crate) fn japanese_ocr_is_available() -> bool' in ocr
assert 'create_supported_engine("ja-JP")' in ocr
assert 'IDM_TOOLS_INSTALL_JAPANESE_OCR => {' in main
assert main.count('offer_japanese_ocr_installation_on_request(hwnd, language);') == 1

# The only NoText action must be continuing preexisting OCR; never interrupt
# scanned document opening with the language installer.
handler = main.split('fn handle_pdf_loaded(hwnd: HWND, payload: PdfLoadResult) {', 1)[1]
no_text = handler.split('Ok(PdfTextResult::NoText) => {', 1)[1].split('Err(message) => {', 1)[0]
assert 'start_ocr_for_pdf(hwnd, hwnd_edit, path, from_copydata, language);' in no_text
assert 'offer_japanese_ocr_installation_on_request' not in no_text
assert 'maybe_offer_japanese_ocr_installation' not in no_text

# Both grouped and flat Tools menu modes expose a user-initiated entry.
assert 'pub const IDM_TOOLS_INSTALL_JAPANESE_OCR: usize = 5029;' in menu
assert menu.count('IDM_TOOLS_INSTALL_JAPANESE_OCR,') == 2
assert menu.count('&i18n::tr(language, "menu.install_japanese_pdf_ocr")') == 2
assert main.count('IDM_TOOLS_INSTALL_JAPANESE_OCR => {') == 1

paths = list((root / 'i18n').glob('*.json'))
assert len(paths) == 17
for p in paths:
    d = json.loads(p.read_text(encoding='utf-8'))
    for key in ['menu.install_japanese_pdf_ocr',
                'app.pdf_japanese_ocr_install_title',
                'app.pdf_japanese_ocr_install_prompt',
                'app.pdf_japanese_ocr_installed',
                'app.pdf_japanese_ocr_install_failed']:
        assert d.get(key), (p, key)
    assert 'PDF' in d['menu.install_japanese_pdf_ocr'].upper(), p

for p in [*root.glob('CHANGELOG*.md'), root / 'CHANGELOG-HI.txt']:
    intro = p.read_text(encoding='utf-8').split('Version', 1)[0]
    text = p.read_text(encoding='utf-8')
    assert '0.9.13' in text[:120], p
    assert '2026-10-10' in text[:120], p
    assert '13. ' in text[:9000], p
print('OK: no automatic OCR installation prompts; optional menu action in two menu modes; explicit consent; 17 languages')
