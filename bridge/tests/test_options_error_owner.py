"""Guard against interpreting OptionsDialogState as the main AppState."""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]
SOURCE = (ROOT / "src/app_windows/options_window.rs").read_text(encoding="utf-8")


class OptionsErrorOwnerTests(unittest.TestCase):
    def test_options_never_passes_its_owner_to_editor_error_helpers(self):
        # These helpers assume GWLP_USERDATA points to AppState and can corrupt
        # OptionsDialogState both before the message box and on focus restoration.
        self.assertNotRegex(SOURCE, r"crate::show_error(?:_with_id)?\s*\(")

    def test_error_dialog_preserves_options_focus_without_editor_state(self):
        start = SOURCE.index("fn show_options_error(")
        end = SOURCE.index("fn move_options_focus_tab(", start)
        helper = SOURCE[start:end]
        self.assertIn("crate::message_box_modal(", helper)
        self.assertNotIn("with_state(", helper)
        self.assertNotIn("restore_editor_focus(", helper)
        self.assertIn("IsChild(hwnd, previous_focus)", helper)
        self.assertIn("crate::set_focus_safe(previous_focus)", helper)

    def test_both_shortcut_conflicts_use_options_error_dialog(self):
        conflicts = re.findall(
            r"if let Some\(\(language, message\)\) = conflict \{\s*([^;]+);",
            SOURCE,
        )
        self.assertEqual(len(conflicts), 2)
        for call in conflicts:
            self.assertEqual(call, "show_options_error(hwnd, language, &message)")


if __name__ == "__main__":
    unittest.main()
