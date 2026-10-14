"""Prevent unrelated options controls from sharing WM_COMMAND notifications."""

import collections
import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


class OptionsControlIdTests(unittest.TestCase):
    def test_options_control_ids_are_unique(self):
        source = (ROOT / 'src/app_windows/options_window.rs').read_text('utf-8')
        declarations = re.findall(
            r'const (OPTIONS_ID_\w+): usize = (\d+);', source
        )
        self.assertTrue(declarations, 'No options control IDs found')
        controls = collections.defaultdict(list)
        for name, value in declarations:
            controls[int(value)].append(name)
        collisions = {
            value: names for value, names in controls.items() if len(names) > 1
        }
        self.assertEqual(collisions, {}, 'Options controls must have unique IDs')


if __name__ == '__main__':
    unittest.main()
