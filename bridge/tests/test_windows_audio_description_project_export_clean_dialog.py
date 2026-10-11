"""Source-level regression checks for the accessible MP3 re-export dialog."""
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PROJECT = (ROOT / 'src/app_windows/audio_description_project_window.rs').read_text('utf-8')
SAVE = (ROOT / 'src/app_windows/podcast_save_window.rs').read_text('utf-8')

class ProjectCleanExportDialogTests(unittest.TestCase):
    def test_standalone_progress_has_cancel_and_status(self):
        export = PROJECT.split('fn start_export(',1)[1].split('fn window_proc_inner(',1)[0]
        self.assertIn('open_with_labels_and_status_field(', export)
        self.assertIn('language_labels.export.clone()', export)
        self.assertIn('language_labels.cancel.clone()', export)
        self.assertIn('audio_description.project.export_cancel_confirm', export)
        self.assertIn('podcast_save_window::enable_accessible_progress_context(dialog)', export)
        self.assertIn('podcast_save_window::focus_primary_control(dialog)', export)
        self.assertIn('set_controls_enabled(state, false)',export)
    def test_message_routing_and_real_cancel(self):
        self.assertIn('podcast_save_window::handle_navigation(export_dialog, msg)', PROJECT)
        self.assertIn('podcast_save_window::WM_PODCAST_SAVE_CANCEL => {', PROJECT)
        self.assertIn('state.export_progress_dialog == HWND(lparam.0)', PROJECT)
        self.assertIn('cancel.store(true, Ordering::SeqCst)',PROJECT)
        self.assertIn('podcast_save_window::set_status_text(',PROJECT)
        self.assertIn('podcast_save_window::WM_PODCAST_SAVE_PROGRESS',PROJECT)
    def test_modal_closes_before_success_or_error(self):
        done=PROJECT.split('WM_PROJECT_DONE => {',1)[1].split('WM_SETFOCUS => {',1)[0]
        self.assertLess(done.index('DestroyWindow(export_dialog)'),done.index('set_controls_enabled(state, true)'))
        self.assertLess(done.index('DestroyWindow(export_dialog)'),done.index('match result'))
    def test_no_fake_progress_and_dialog_is_genuine(self):
        self.assertIn('podcast_save_window::disable_fake_progress(dialog)',PROJECT)
        self.assertIn('EnableWindow(parent, false)',SAVE)
        self.assertIn('SAVE_ID_CANCEL',SAVE)
        self.assertIn('WM_PODCAST_SAVE_CANCEL',SAVE)

if __name__=='__main__':
    unittest.main()
