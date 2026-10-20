# Indonesian translation handoff

This branch adds Indonesian resources and the integration needed for a standalone
test build. Indonesian is registered as `id` and selectable as `Bahasa Indonesia`.
This handoff accompanies a draft upstream PR. The earlier standalone build was
tested successfully. The latest wording changes still need a fresh build and
application verification before the PR is ready for review.

## Resources

| New file | Source | Entries |
| --- | --- | ---: |
| `i18n/id.json` | `i18n/en.json` | 2,210 |
| `i18n/features/treccani_id.json` | `i18n/features/treccani_it.json` | 17 |
| `i18n/features/la7_play/id.json` | `i18n/features/la7_play/it.json` | 23 |
| `i18n/features/tv/id.json` | `i18n/features/tv/it.json` | 116 |
| `guida_id.txt` | `guida_en.txt` | User guide |
| `donations_id.txt` | `donations_en.txt` | Donation information and acknowledgements |

The source snapshot is commit `9b2f6ff7ba87b08a0fe0b0f1f5cc8706a3be20ef`,
package version `0.9.17`. The language code is `id` and the display name is
`Bahasa Indonesia`.

## Translation conventions

- Use `berkas`, `folder`, `buku audio`, `deskripsi audio`, `penanda`,
  `pembaca layar`, `papan klip`, `peramban`, and `takarir` consistently.
- Translate instructions and labels; retain product names, codec names, engine
  identifiers, model names (such as Whisper Small/Medium), URLs, and technical
  identifiers.
- Preserve every source key, its order, and all named placeholders.
- Preserve keyboard shortcuts after tabs, file-dialog filter patterns, literal
  escapes, significant outer whitespace, and line breaks.
- Localize mnemonic letters marked by `&`; retain `&&` for a literal ampersand.
  The main menu and color/size submenus use distinct mnemonic letters.
  Edit, File, and Playback action labels retain their original mnemonic letters
  to avoid introducing collisions between commands; a suffix such as `(&C)` is
  used when the Indonesian label does not contain the required letter.
- Use three dots for action-label ellipses. Source Unicode was checked directly:
  apparent replacement characters in terminal output were display artifacts,
  not damaged English resources. The English source is unchanged.

## Validation

All four JSON resources were parsed with duplicate-key rejection and checked
against their respective sources: 2,366 entries in total. Key sets and order,
placeholder multiplicity, shortcut suffixes, mnemonic counts, literal ampersands,
newlines, escaped separators, file-filter patterns, and significant outer
whitespace match. The resources contain no Unicode replacement characters.

Shared technical terms and proper names may intentionally match the source.
No translation API was used.

The standalone build from commit
`7465f08dda1f2f156a9632198c7c01d48a6c1835` passed:

- 393 Python tests.
- 557 Rust tests, with 10 additional tests ignored by the existing suite.
- Static startup, host runtime, and simulated missing-runtime dialog checks.
- A separate Windows UI test that selected Bahasa Indonesia in the real options
  dialog, checked the translated main menu, verified `language: "id"` in saved
  settings, and verified the selection and menus after restarting the process.

[Successful build](https://github.com/muhammadGagah/Sonarpad/actions/runs/39373589127)
and [successful UI test](https://github.com/muhammadGagah/Sonarpad/actions/runs/39382235658).
The UI workflow verifies that application sources and translation resources
still match the binary's source commit before reusing the artifact.

The downloaded ZIP matched GitHub's SHA-256 artifact digest:
`f761b227958ffc62bc20a35da58b4f2aa69f78dce32af7bbdef09d8a13f3e482`.
The executable's `--check-runtime` probe also returned exit code 0 on the local
Windows machine. The local test copy used separate standalone settings with
Indonesian selected and startup update checks disabled for that test.

## Developer integration

The language model, selectors, base resource loader, guide, donation text,
calendar names, and locale mappings now include Indonesian. RSS defaults follow
the existing news-language selection; the Indonesian embedded-feed fallback is
English. Podcast category names and historical changelogs use English fallbacks.
Language and territory display names also retain the existing English fallback
until an Indonesian CLDR dataset is added.

Treccani, TV, and La7 translations are provided as resources. These Italy-specific
features retain their existing language gates and loaders; their Indonesian
resources are not activated by this integration.

The `Indonesian standalone test build` workflow runs only in the contributor fork,
uploads an executable artifact, and creates no release. It deliberately does not
require the maintainer's route-service token; token-dependent services cannot be
fully tested in this build. The workflows and test scripts are retained in this draft for maintainer review.
Both workflows are restricted to the contributor fork. Their push triggers still
name the retired experimental branch. The Windows UI workflow references the
previous artifact, which was deleted during cleanup, so it cannot currently be
rerun successfully. A fresh build and an updated artifact reference are needed
before repeating that UI test. Previous run links above are historical evidence,
not links to downloadable binaries.

The Indonesian wording review simplifies status messages and technical terms,
aligns guide references with menu labels, and explains text-to-speech, API keys,
proxy servers, RSS, subtitles, and temporary storage for beginners. Guide entries
also identify the actual menus for media conversion, file execution, and batch
audiobooks. Automatic display wrapping is distinguished from inserted line breaks.

The revised resources pass validation for all 2,366 translation keys, 181 guide
lines, and 18 donation-text lines. Punctuation checks reject added unusual symbols
and bullet markers absent from the corresponding source. The 393 Python tests
also pass. These wording changes are newer than the executable tested above and
have not yet been rebuilt or checked for visual fit in the application.

Source review confirms the guide's default shortcuts: Save All is `Ctrl+Shift+S`
and Execute File is `Ctrl+Shift+F5`. The Save All menu resource still displays
`Alt+Shift+S`, matching the English resource but differing from the actual binding.
That existing source issue is left for the developer because this translation
preserves source shortcut suffixes. Execute File's raw `Shift+F5` resource suffix
is replaced at runtime with the configured shortcut.

After integration, check keyboard navigation and mnemonic uniqueness in each
actual menu/dialog, long-label layout, and speech/braille announcements with
NVDA or JAWS. Automated UI selection and restart persistence have passed on a
Windows runner. Manual keyboard navigation, screen-reader announcements, visual
layout, speech playback, and everyday editing still need user acceptance testing.

Historical changelogs and RSS feed selections are not part of this translation.
