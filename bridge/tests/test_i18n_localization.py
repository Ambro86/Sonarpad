import json
import re
import unittest
from pathlib import Path


I18N_DIR = Path(__file__).resolve().parents[2] / "i18n"
PLACEHOLDER_RE = re.compile(r"\{[a-z_]+\}")


class AudioDescriptionLocalizationTests(unittest.TestCase):
    def setUp(self):
        self.english = json.loads((I18N_DIR / "en.json").read_text(encoding="utf-8"))
        self.keys = sorted(
            key for key in self.english if key.startswith("audio_description.")
        )

    def test_all_18_locales_contain_every_audio_description_key(self):
        locale_files = sorted(I18N_DIR.glob("*.json"))
        self.assertEqual(len(locale_files), 18)
        for path in locale_files:
            with self.subTest(locale=path.stem):
                values = json.loads(path.read_text(encoding="utf-8"))
                self.assertEqual(
                    [], [key for key in self.keys if key not in values]
                )

    def test_all_locales_preserve_format_placeholders(self):
        for path in sorted(I18N_DIR.glob("*.json")):
            values = json.loads(path.read_text(encoding="utf-8"))
            with self.subTest(locale=path.stem):
                for key in self.keys:
                    self.assertEqual(
                        set(PLACEHOLDER_RE.findall(self.english[key])),
                        set(PLACEHOLDER_RE.findall(values[key])),
                        key,
                    )

    def test_non_english_locales_translate_primary_module_labels(self):
        primary_keys = (
            "audio_description.title",
            "audio_description.start",
            "audio_description.progress.gemini_processing",
        )
        for path in sorted(I18N_DIR.glob("*.json")):
            if path.stem == "en":
                continue
            values = json.loads(path.read_text(encoding="utf-8"))
            with self.subTest(locale=path.stem):
                for key in primary_keys:
                    self.assertNotEqual(values[key], self.english[key], key)

    def test_audio_description_menu_and_title_use_ai_wording(self):
        expected = {
            "cs": ("Vytvořit a&udiopopis pomocí AI...", "Vytvořit audiopopis pomocí AI"),
            "de": ("A&udiodeskription mit KI erstellen...", "Audiodeskription mit KI erstellen"),
            "en": ("Create audio d&escription with AI...", "Create audio description with AI"),
            "es": ("Crear &audiodescripción con IA...", "Crear audiodescripción con IA"),
            "fr": ("Créer une &audiodescription avec l'IA...", "Créer une audiodescription avec l’IA"),
            "hi": ("&AI की मदद से ऑडियो वर्णन बनाएँ...", "AI की मदद से ऑडियो वर्णन बनाएँ"),
            "it": ("Crea audiodescrizione con &IA...", "Crea audiodescrizione con IA"),
            "lt": ("Kurti garsinį vaizdavimą naudojant &DI...", "Kurti garsinį vaizdavimą naudojant DI"),
            "pl": ("&Utwórz audiodeskrypcję z pomocą AI...", "Utwórz audiodeskrypcję z pomocą AI"),
            "pt-BR": ("Criar &audiodescrição com IA...", "Criar audiodescrição com IA"),
            "pt": ("Criar &audiodescrição com IA...", "Criar audiodescrição com IA"),
            "ru": ("Создать а&удиодескрипцию с помощью ИИ...", "Создать аудиодескрипцию с помощью ИИ"),
            "sr": ("Направи аудиодескрипцију помоћу &ВИ...", "Направи аудиодескрипцију помоћу ВИ"),
            "sv": ("Skapa synto&lkning med AI...", "Skapa syntolkning med AI"),
            "uk": ("Створити аудіодескрипцію за допомогою &ШІ...", "Створити аудіодескрипцію за допомогою ШІ"),
            "vi": ("Tạo th&uyết minh hình ảnh bằng AI...", "Tạo thuyết minh hình ảnh bằng AI"),
            "zh": ("使用 &AI 创建音频描述...", "使用 AI 创建音频描述"),
        }
        for locale, (menu, title) in expected.items():
            values = json.loads((I18N_DIR / f"{locale}.json").read_text(encoding="utf-8"))
            with self.subTest(locale=locale):
                self.assertEqual(menu, values["menu.create_audio_description"])
                self.assertEqual(title, values["audio_description.title"])



if __name__ == "__main__":
    unittest.main()
