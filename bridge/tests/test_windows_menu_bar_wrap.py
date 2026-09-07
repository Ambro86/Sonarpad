from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MAIN_RS = (ROOT / "src" / "main.rs").read_text(encoding="utf-8")


def test_menu_bar_wrap_handles_wm_nextmenu_without_system_menu_escape():
    assert "WM_NEXTMENU =>" in MAIN_RS
    assert "next_menu.hmenuIn == main_menu" in MAIN_RS
    assert "next_menu.hmenuNext = main_menu;" in MAIN_RS
    assert "next_menu.hwndNext = hwnd;" in MAIN_RS
    assert "key == u32::from(VK_RIGHT.0)" in MAIN_RS
    assert "key == u32::from(VK_LEFT.0)" not in MAIN_RS


def test_dynamic_playback_menu_shifts_fixed_top_level_indices():
    assert "let playback_offset = if playback_menu.0 != 0" in MAIN_RS
    assert "FILE_MENU_BASE_INDEX + playback_offset" in MAIN_RS
    assert "EDIT_MENU_BASE_INDEX + playback_offset" in MAIN_RS
    assert "VIEW_MENU_BASE_INDEX + playback_offset" in MAIN_RS
    assert "WINDOW_MENU_BASE_INDEX + playback_offset" in MAIN_RS
