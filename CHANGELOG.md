# Changelog

Version 0.9.11 – 2026-09-22

File saving
1. Fixed Save As: when a document that was opened from disk is saved under a new name, the save dialog now opens in the same folder as the original file instead of starting from the configured default folder or OneDrive. New documents continue to use the configured save folder.

Google voices
1. Added a safe startup fallback for Google voices: if Chrome closes before the Google TTS runtime is ready, Sonarpad automatically retries with Microsoft Edge when available. On systems where Google voices already work, the existing Chrome path remains unchanged.

2. Improved diagnostics for rare Google TTS browser startup failures: Chrome or Edge stdout and stderr are written to Sonarpad diagnostics only when the browser exits before the Google TTS runtime is ready. Successful startups do not add browser output to diagnostics.

Version 0.9.10 – 2026-09-16

Audio descriptions
1. Improved audio descriptions by replacing the `_` (underscore) character with a space, preventing speech synthesis from pronouncing that character in audio descriptions.

Version 0.9.9 – 2026-09-15

AI Audio Description
1. Added an isolated fallback for source videos with no audio track, without changing the existing successful pipeline. Sonarpad still runs the normal export first; only if FFmpeg reports a missing audio stream and Sonarpad confirms that the source contains no audio streams does it create a silent source with the same duration and mix the synthesized descriptions onto it. When exporting on the original video, the existing video/audio mux path is then reused. Videos that already contain audio continue through the exact normal path.

2. Extended the strictly isolated no-description fallback without changing the normal audio-description pipeline. Sonarpad still runs the existing analysis first; only when it ends with no usable descriptions does it offer the Brief retry, which uses the same pipeline and keeps all silence/no-dialogue-overlap constraints active. If that second attempt also cannot place a description, Sonarpad now asks for explicit permission for one final fallback. Only after Yes, it reuses the brief Gemini descriptions already saved by the second attempt and mixes them at their visual timestamps with ducking, allowing short overlaps with dialogue. Pyannote, the Gemini bridge, the normal alignment rules, TTS scheduling and the first two analysis paths are not altered. If the user declines, or no generated description is available to reuse, Sonarpad stops cleanly with a localized message.

3. Refined the isolated no-description fallback without changing the normal audio-description pipeline. If the user’s original verbosity is already Brief and the analysis ends with no usable descriptions, Sonarpad now skips the redundant second Gemini analysis and, when reusable generated descriptions are available, asks directly whether to use the final dialogue-overlap fallback. If the original verbosity is Standard or Detailed, the Brief retry is still kept even when the available pauses are very short: in that case it also serves to generate a shorter description for the possible final overlap fallback, reducing how long narration covers dialogue. Pyannote, the Gemini bridge and the normal silence/alignment rules remain unchanged.

Podcast recording
1. Added a conservative microphone device-position fallback for problematic WASAPI capture drivers without changing recording on systems that already work. It activates only after persistent device-position mismatches (24 consecutive mismatches, or at least 80% mismatches across 64 clean packets). Real WASAPI `DATA_DISCONTINUITY` flags and timestamp errors remain authoritative, and system-audio capture is unchanged.

YouTube and streaming
1. Fixed failed YouTube downloads opened from the results context menu. Known members-only errors from yt-dlp are now converted into Sonarpad's localized message instead of exposing the raw English yt-dlp output. After dismissing the error, Sonarpad now performs a deferred focus restore after the native context-menu/modal loop unwinds, reliably returning keyboard focus to the existing results list while keeping the normal successful download pipeline unchanged.

2. Matched SonarTube mobile's preventive filtering for YouTube navigation results: videos for which YouTube/yt-dlp does not provide any view-count metadata are now hidden from search results and channel/playlist browsing, while channels and playlists remain visible and genuine videos reporting 0 views are kept. The existing localized members-only download error remains as a second fallback. Bulk playlist download is unchanged.

Version 0.9.8 – 2026-09-12

AI Audio Description
1. Strengthened multichannel audio-description export conservatively without changing the pipeline for files that already work. Sonarpad always keeps the original channel layout on the normal path; only if the multichannel staging WAV fails because a frame/sample is not aligned to the channel count does Sonarpad retry that export alone through a stereo fallback. Mono, stereo and multichannel files that already export successfully continue through the normal path unchanged.

Dark mode and accessibility
1. Fixed dark mode interfering with native Windows dialogs and confirmation messages. Sonarpad no longer applies its custom dark-theme subclass to system `#32770` dialog trees, so Save/Open dialogs, diagnostic export and MessageBox confirmations remain managed by Windows and can correctly receive keyboard/NVDA focus. Also fixed an internal issue with the default ZIP extension used by the diagnostic Save dialog.

Version 0.9.7 – 2026-09-11

AI Audio Description
1. Added the optional “Create audio description on the original video file” setting. When enabled, Sonarpad now prefers MP4: it copies the original video stream without re-encoding and inserts the mixed audio-description track. If FFmpeg reports a container/packet compatibility failure while creating the MP4, Sonarpad automatically retries the same export as MKV, still without re-encoding the video. Users can also explicitly choose MKV. Extended pauses are ignored in this mode to keep audio and video synchronized.

2. Improved segment reanalysis in saved audio-description projects. The reanalysis window now uses the AI access already configured in the main Create audio description window, so API key, Sonarpad AI credit and model controls are no longer duplicated. A reanalyzed segment now keeps the full group of project descriptions that belong to that analysis chunk instead of only the first/nearest description; every description in the group is marked as reanalyzed and “Apply segment and re-export MP3” applies the whole group at once. Extended-pause previews are synthesized directly instead of seeking inside the exported MP3, avoiding previews that start on film audio or cut the description.

3. Added conservative Gemini media recovery without changing the existing successful pipeline. HTTP 400 `INVALID_ARGUMENT` still triggers smaller MKV chunks (about 15 MB) and then MP4 compatibility chunks. In addition, if Gemini accepts an uploaded chunk but its processing ends in `FAILED`, Sonarpad retries that exact upload once and only then falls back to MP4; server Code 13 retries are capped at three before the same fallback, preventing endless waits. Network, quota, authentication and synthesis errors do not activate this path.

4. Fixed AI credential persistence when switching access modes. The personal Gemini API key and the Sonarpad AI access code are now stored independently: switching from Personal API key to Sonarpad AI, or back, no longer clears the inactive credential. The Gemini key is also saved when its field loses focus.

5. Added a real Cancel action for “Reanalyze segment” and “Apply segment and re-export MP3”. The progress bar remains active, while Cancel now interrupts FFmpeg segment preparation, the AI worker, TTS validation and the final export as soon as the current stage can stop safely. Applying a reanalyzed segment is now transactional: the existing project and output media are replaced only after a successful re-export; if the operation is cancelled, the previous files remain unchanged and the reanalyzed proposal stays available for another attempt.

6. Fixed segment reanalysis for descriptions outside the first Gemini chunk. Isolated chunks are now sent to the existing worker on a local 0-based timeline and mapped back to their absolute project position, preventing `Invalid prepared chunk timeline at chunk 1` without changing the normal audio-description pipeline.

7. Reworked saved-project segment reanalysis to use the same safety pipeline as full audio-description creation. The selected chunk now gets the same mono 16 kHz audio extraction and Pyannote dialogue/silence analysis, the same Gemini timing rules and media fallbacks, the same real TTS synthesis with the project voice, and the same final scheduler for safe placement and extended pauses. The reanalysis keeps the newly verified absolute timings and refreshed Pyannote protection for that chunk when it is applied; the previous preview-only workaround for overlong reanalyzed text has been removed.

8. Fixed structural segment replacement after full reanalysis. Sonarpad no longer requires the newly verified segment to contain exactly the same number of descriptions as the saved project: the old descriptions in that chunk are replaced by the complete safe result of the full pipeline, so a 10-description segment may correctly become 9 (or 11). The project editor immediately shows the new count and timings for preview, while the saved project and media remain unchanged until “Apply segment and re-export MP3” completes successfully.

9. Reworked segment reanalysis so the selected physical chunk is treated as a real, self-contained mini-film. Sonarpad now passes that mini-film directly to the exact same `create_audio_description` function used by normal full creation, so video and soundtrack share the same local timeline and the ordinary Pyannote, Gemini, real-TTS, scheduling and safety pipeline runs without a separate reanalysis implementation. Only after that normal pipeline finishes are the resulting local times shifted back to the original movie position.

Text editing
1. Improved “Join wrapped lines” in Edit > Text. With no selection it processes the whole document; with a selection it processes only the selected lines. It now reconstructs complete paragraphs by joining consecutive lines even when a sentence already ends with punctuation, using blank lines as paragraph separators while preserving numbered and bulleted lists.

10. Fixed segment reanalysis timing when mapping an independently analyzed mini-film back to the original movie. Sonarpad now applies the same small chunk-duration reconciliation used by the normal full analysis to descriptions, visual-evidence timestamps, and protected dialogue intervals, preventing progressive sub-second drift near the end of a chunk from moving narration onto speech.

Version 0.9.6 – 2026-09-09

1. Fixed freezes with some SAPI4 voices when cursor tracking was enabled. The bridge now waits for actual synthesis completion while keeping cursor tracking active.

2. Fixed microphone/system-audio synchronization and clicks caused by timestamp rounding in podcast recording. Corrections also apply when saving the sources separately.

3. Isolated SAPI5 audio-description synthesis in child processes. Failed synthesis is retried with reduced concurrency while retaining successful descriptions; the working limit is remembered for the voice.

Version 0.9.5 – 2026-09-07

AI Audio Description
1. Improved resilience when Gemini returns a temporary `MALFORMED_RESPONSE` with no usable content. Sonarpad now retries the exact same chunk up to three times, waiting briefly between attempts, instead of stopping the whole audio description immediately. Normal Gemini responses, safety blocks, token-limit errors and existing checkpoint behavior remain unchanged.

2. Fixed another rare `invalid Gemini chunk timeline` case with some videos whose FFmpeg-prepared chunks report a small cumulative duration drift. Sonarpad keeps the existing timeline path unchanged whenever it is valid and uses a conservative reconciliation fallback only when the normal timeline would fail and the total discrepancy is small; large or suspicious mismatches still stop with an error.

3. Added the optional Sonarpad AI service for AI audio descriptions. Users can keep using their own Gemini API key or choose the Sonarpad service and enter a personal Sonarpad code. The code is protected with Windows DPAPI. In service mode, prepared video chunks are uploaded directly from the user’s PC to Google through a temporary Google upload URL; sonarpad.com never receives or stores the video bytes. The backend only authorizes access, enforces the Gemini model/Standard tier and usage limits, accounts usage, and returns the Gemini response.

4. Improved editing of saved audio-description projects. Users can now change multiple descriptions one after another without having to apply each one immediately. The edits remain pending in memory; pressing “Apply text” validates every modified description against its saved timing range and then saves all valid changes together. If one description does not fit, Sonarpad returns focus to that description without losing the other pending edits.

5. Added “Adjust voice” immediately before “Create audio description”. The new dialog provides speech engine, voice, speed and volume controls plus “Test voice”. Pressing OK returns focus exactly to “Adjust voice” and applies those values to the audio description being created. If the dialog is never used, synthesis behavior remains exactly as before.


6. Expanded voice controls in saved audio-description projects. The project editor now also provides speed, volume and “Test voice” alongside engine and voice. Applying the voice settings rechecks every existing description with the selected synthesis parameters and rebuilds the MP3 only if all descriptions still fit their saved timing ranges. Existing speech-free intervals, Visual Evidence timestamps, Gemini descriptions and project timing analysis are reused without running AI analysis again.

Version 0.9.4 – 2026-09-04

AI Audio Description
1. Fixed an issue with Matroska/WebM videos that contain an unusually large internal start timestamp and could stop audio-description generation with “invalid Gemini chunk timeline”. Sonarpad now normalizes the source and Gemini chunk durations only when this anomalous timestamp pattern is detected, while normal videos and existing audio-description behavior remain unchanged.
2. Improved audio-description ducking for a more natural mix. The original soundtrack now lowers smoothly before the narration begins and returns more gradually afterward; closely spaced descriptions keep a stable background level instead of repeatedly raising and lowering the soundtrack.
3. Fixed a final audio-description export failure with some AVI files and other sources using 5.1/multichannel audio, where decoding could end with an incomplete interleaved audio frame. Sonarpad now safely pads only that final partial frame with the strictly necessary silent samples; already complete frames and normal mono, stereo, or multichannel files remain unchanged.


Version 0.9.3 – 2026-09-03

SAPI5 voices
1. Fixed an issue where some local SAPI5 voices could fail to speak when cursor movement was enabled, during multi-voice reading, or while creating audiobooks/MP3 files. Sonarpad now uses a SAPI5 file-synthesis path that works reliably on Windows and still allows synthesis to be cancelled, while normal direct playback remains unchanged.
2. Fixed cursor position during multi-voice dialogue reading. Voice tags inserted automatically by Sonarpad for dialogue are now treated only as playback metadata and no longer as characters in the editor, so pausing/resuming with F4 or stopping/restarting with F6 no longer moves the cursor ahead of the real text. Single-voice reading and explicit <voice> tags in documents remain unchanged.

AI Audio Description
1. Added a “Show API key” checkbox immediately after the Gemini API key field. It is disabled by default; when enabled, it temporarily reveals the full key so users can verify that it was pasted completely, while reopening the window always returns the key to hidden mode.

Podcasts and Wikipedia
1. After saving a podcast recording, Sonarpad now asks whether to open the folder containing the saved file, matching the behavior already used after saving YouTube/streaming media.
2. When another Wikipedia article is imported into an editor that already contains text, the new article is now appended at the end instead of being inserted at the top. The cursor is positioned at the beginning of the newly imported article.


Version 0.9.2 – 2026-09-02

AI Audio Description
1. Fixed an issue that could cause AI Audio Description to fail during final MP3 export with videos containing multichannel audio such as 5.1. Sonarpad now automatically downmixes multichannel audio to stereo only when required for MP3 encoding, without changing mono or stereo exports.
2. When starting AI Audio Description with a video containing multiple audio tracks, Sonarpad now asks which track to use before processing. The accessible combo box can be changed with the arrow keys; OK starts the audio description with the selected track, while Cancel closes the audio-description window and returns focus to the Sonarpad editor.

YouTube and streaming
1. Fixed an issue where starting AI Audio Description from a video on page 2 or later of a YouTube playlist or channel could reopen the YouTube selection window and steal focus from the audio-description window. Sonarpad now closes the selector cleanly without returning to previous pages.
Version 0.9.1 – 2026-09-01

YouTube downloads
• Fixed an issue where YouTube/streaming download progress windows could repeatedly return to the foreground after switching to another application with Alt+Tab. Downloads now continue in the background without stealing focus.
• Improved accessibility of download progress. When returning to the progress window, screen readers can read the current status and percentage. For playlists, Sonarpad also reports the current item number, total number of items, and title.
• Fixed false watchdog hang reports during long downloads and conversions when the progress window was still responsive.
• Added a Format combo box to playlist downloads. From the video list, press Tab to choose MP4, MP3, M4A, OPUS, OGG, WAV, or FLAC before starting the bulk download.
• Reorganized streaming media saving. Format and quality are now chosen when saving instead of in the initial streaming search window. “Save media” opens one format/quality dialog, and playlist downloads provide both Format and Quality combo boxes.

AI Audio Description
• Fixed an issue that could prevent AI Audio Description from starting with some MKV videos. Sonarpad now handles videos with irregular or missing timestamps more reliably.

Version 0.9.0 – 2026-08-31

AI Audio Description — major new feature
• Added “Create AI Audio Description” under Tools > Multimedia. Sonarpad analyzes the audio to find spaces without dialogue, generates descriptions with Gemini, and uses the speech engines already available in Sonarpad while avoiding spoken dialogue.
• Improved synchronization between what happens in the video and the generated descriptions, with automatic checks on Gemini timestamps.
• “Enable extended pauses” is disabled by default. It can be enabled for content with heavy dialogue or little available space so longer descriptions can still be inserted.
• Sonarpad can try to recognize characters and use their names. Character catalogs can be kept across episodes of a series to improve continuity.
• Projects can be saved, edited later, and exported again without generating everything again with Gemini.
• If the process is interrupted, Sonarpad keeps its progress and can continue the audio description. If the Gemini quota is exhausted, you can wait, switch model, or stop without losing completed work.
• The window lets you choose language, detail level, Gemini model, speech engine, and voice, and remembers the selected preferences.
• The module is available in all 17 Sonarpad languages. During generation the interface only exposes progress, current status, and Cancel; when finished, the MP3 can be opened directly in the internal player.

E-books and documents
• Added DRM-free Kindle import for MOBI, AZW, and AZW3, with text and chapters available in the editor and document index.
• Added DAISY 2.02 and DAISY 3 support. DAISY audiobooks use Sonarpad’s internal player and respect chapter navigation and playback limits.
• Kindle and DAISY files are imported without overwriting the original file; DRM-protected Kindle books are explicitly rejected.
• Fixed EPUB “Save As”: when TXT or another format is selected, the chosen extension is now used and the original EPUB remains associated with the open document.

RSS and articles
• Added multiple selection for RSS articles so several articles can be deleted in one operation.
• RSS now supports real folders that are preserved during OPML import and export, including empty folders.
• Feeds can be reordered inside the current folder with Move up, Move down, Move to top, Move to bottom, and Move to position.

Accessibility, guides, and interface
• Sonarpad guides have been reorganized with an index, and a complete guide to AI Audio Description has been added.
• Fixed a German translation issue that could prevent Open, Save As, and other file-selection dialogs from appearing.

Voices and languages
• The downloadable Google TTS catalog has grown from 104 to 156 packages and from 53 to 81 language variants.
• Added new Google TTS packages and localized names for additional languages across the interface.

Version 0.8.4 – 2026-07-24

EPUB document editing
• Sonarpad can now not only open EPUB documents, but also edit them and save them again in EPUB format while preserving the original formatting, table of contents, footnotes, images, style sheets, metadata and internal links.
• EPUB is available in “Save As” for documents opened from an EPUB. Saving updates only the changed text and keeps the book structure intact.

Audiobook reliability
• Fixed an intermittent problem where, after five failed Google TTS attempts, a synthesis unit was silently discarded and the final audiobook could be missing part of the text.
• Google units are now retried until they succeed or the user cancels. Worker startup is staggered to reduce temporary Chrome and file conflicts, and Sonarpad now stops instead of saving an audiobook with a missing segment.
• Edge audiobooks now retry temporary network, WebSocket, timeout, service-limit and invalid-audio responses until success or user cancellation, including mixed voices and time-based splitting. SAPI4 and SAPI5 retain adaptive finite recovery; if a segment still fails, Sonarpad stops without saving an incomplete audiobook.

Digital library navigation
• LibriVox, Internet Archive and Project Gutenberg search results now use page navigation like YouTube: “Go to previous results” appears at the top and “Go to next results” at the bottom.
• Fixed LibriVox focus transitions: opening a book or chapter no longer sends NVDA focus to the main editor before the next list or player opens.
• Added a LibriVox focus guard during searches and book loading: a localized loading dialog remains in the foreground while the request is running, preventing NVDA focus from escaping to Command Prompt, Windows Terminal or another application.

YouTube playlist downloads
• Added an accessible multi-selection command to YouTube playlists, allowing users to choose which videos to download without changing the existing “Save media” command for the currently playing item.
• Selected items are downloaded one at a time using the format and quality chosen when opening the playlist, receive numbered file names that preserve playlist order, and are saved in a dedicated folder inside the configured Media folder.
• The selection window includes Select all and Deselect all commands, announces the number of selected items, supports cancellation while keeping completed files, and reports any items that could not be downloaded.
• Playlist entries are now native check boxes: screen readers announce each title, control role and checked state automatically, without adding selection words to the visible title or using forced speech.

Version 0.8.3 – 2026-07-23

Dark mode
• Added a dark mode that can be enabled from the View menu and is saved in the user preferences.
• The dark theme is applied to the editor, menus, secondary windows and main controls, with text colors adapted to preserve readability and accessibility.

German language
• Added German as a complete interface language, selectable from Options.
• News and RSS, the spell checker, the calendar and all quotations, donations, the guide and the changelog are fully available in German.

Brazilian Portuguese and Google News
• Added Brazilian Portuguese as a complete interface language, separate from Portuguese (Portugal) and selectable from Options.
• The complete interface, calendar entries and quotations, spell checker, donations, guide and changelog are available in Brazilian Portuguese.
• Google News now supports the Brazilian localization, Brazilian categories and separate default Brazilian RSS sources.
• Related Google News sources for the same story are shown as accessible child items in the tree when the feed provides them.

LibriVox
• Optimized LibriVox searches to avoid excessive requests to the service and interface freezes. Large catalog scans were removed, attempts were reduced and shorter timeouts were introduced.

Speech synthesis
• Sequences of three or more dots are now normalized before reading, preventing some voices from saying “dot dot” or generating segments made up only of punctuation.

Related Google News articles
• For each news story, related articles are now shown when available, meaning other articles covering the same story. To read them, simply expand the main article when Sonarpad announces that related articles are available. Users who do not want to expand this section can simply press Enter on the main article and read the news story as usual.
• Related articles now use the same read/unread system as main articles, including accessible announcements, date and time, saved status, and preservation after feed updates or restarting Sonarpad.

Audiobook part announcements
• Added an “Announcement at the beginning of each part” combo box to Audio Options. For audiobooks split into multiple files, each part can begin with no announcement, the book title, the title and part number, the file name, or the file name and part number.

Version 0.8.2 – 2026-07-17

Digital libraries and audiobooks
• Added Project Gutenberg, with search by title or author and language selection.
• Project Gutenberg EPUB books are downloaded to Documents\Sonarpad\Documents; when the download finishes, Sonarpad asks whether to open the book immediately in the editor.
• Added Internet Archive for searching and listening to audio collections, including old-time radio, speeches and live music.
• Added LibriVox for searching audiobooks by title or author and playing chapters directly with the same player used for podcasts.
• The three new features are available in the Tools menu and, when menu grouping is enabled, in the Reading section.

Long audio transcriptions
• Fixed transcription of long audio files: audio is now automatically divided into 15-minute parts, transcribed one part at a time and then joined again, preventing errors that could occur with long recordings.

YouTube
• The most useful actions that were previously available only after opening a YouTube video and accessing the Playback menu are now also available directly from that same video’s context menu, such as “Transcribe current audio”, “Create audio description with AI” and “Save media”, for easier use.
• Added “Copy link”, also available with Ctrl+C, to copy the URL of the selected YouTube video, playlist or channel to the clipboard.

Version 0.8.1 – 2026-07-16

Google text-to-speech
• Fixed Google TTS startup on Windows systems where connections accepted by the internal browser server inherited non-blocking socket mode, causing error 10035 and preventing downloaded voices from speaking.
• Sonarpad now waits until the Chrome or Edge WASM engine is fully loaded before voice preview or F5 reading, preventing the “Chrome WASM TTS engine was not loaded” error.
• The hidden browser disables page translation and renderer accessibility so it cannot announce “Translate page” or interfere with reading commands.
• The “Voices in editor” panel now shows a “Manage Google voices...” button whenever the Google engine is selected, and refreshes the installed voice list immediately after the manager closes.
• Dependency warnings shown when removing Google voice packages are now localized in every interface language.

Update experience
• After an automatic update, the completion and changelog window now opens after the initial editor focus restoration and remains in the foreground instead of appearing only after pressing Tab.

PDF documents
• Fixed PDF files whose embedded text contained NUL characters and was cut off at the first occurrence when loaded into the editor.
• When pdf-extract returns embedded NULs, Sonarpad now retries with PDFium; any remaining NULs are removed before text is sent to Windows controls, so the rest of the document is preserved.

Menu accessibility
• Removed runtime mnemonic generation: access keys are now written explicitly in each of the 15 interface translations and therefore remain identical across launches.
• Reviewed every stable main-menu item and submenu, including Playback, font choices, Save image and Show EPUB index; missing or duplicate sibling mnemonics were corrected directly in the translations.
• Automated tests now only validate the translations and fail if a mnemonic is missing, invalid or duplicated; they never modify menu labels at runtime.
• In exceptionally large menus where the translated labels do not provide enough distinct characters, an explicit numeric access key is shown using the standard Windows form “(&1)”.

Version 0.8.0 – 2026-07-15

Online dictionary
• Added German to the online Wiktionary dictionary.
• German definitions and synonyms are now parsed using the structure of the German Wiktionary, rather than only adding the language to the selection list.

SAPI5 audiobook reliability
• SAPI5 audiobook creation keeps up to 12 parallel workers when the selected voice produces reliable output.
• Every generated part is now checked using file size, estimated duration and a conservative comparison with the assigned text.
• Missing or suspicious parts are regenerated automatically with progressively lower concurrency: 12, 8, 6, 4, 2 and finally 1 worker. Only problematic parts are repeated.
• The reliable worker limit is remembered separately for each SAPI5 voice, without slowing down voices that work correctly with 12 workers.
• A final integrity check prevents Sonarpad from silently accepting an MP3 that is much shorter than the generated parts.
• Detailed diagnostics are written to `sapi5_audiobook_diagnostic.log`.
• Each SAPI5 synthesis unit now runs in a separate hidden Sonarpad process. If a third-party voice crashes, only that worker closes and the main application remains open.
• During the same audiobook creation, unfinished parts are immediately retried with the next lower concurrency level; parts already validated are preserved.
• Recovery on the next launch remains as an additional safeguard only if the main application or computer is interrupted.

SAPI4 audiobook workers
• The number of SAPI4 processes selected by the user is now respected, up to a technical maximum of 64; the previous hidden limit of 16 has been removed.
• The effective number is reduced only when the audiobook contains fewer work units than requested.
• If one or more SAPI4 bridge processes fail, completed parts are preserved and only failed units are retried automatically with progressively lower concurrency.
• Sonarpad now checks the SAPI4 bridge exit status and rejects empty or invalid audio parts instead of treating them as successful.

Proxy configuration
• Added a separate field for the proxy port in Network settings.
• The port can now be entered independently from the proxy address, is validated from 1 to 65535 and correctly replaces any port already included in the URL.

Radio search by language and country
• The Language and Country filters are now updated with every available entry from the Radio Browser directory instead of being limited to a fixed list.
• Language names are now recognized even when Radio Browser supplies them in another script, as native names, abbreviations or combinations of several languages, and are displayed translated in the current interface language. Values that are not real languages, such as numbers, genres, countries or generic labels, are filtered out.
• The directory is refreshed in the background, with a fallback list that remains usable when Radio Browser cannot be reached.
• Duplicate Radio Browser language entries that become identical after translation are now merged into a single combo-box item, preventing silent steps with screen readers.

Major improvement: synchronization between speech and cursor movement
• Synchronization between speech playback and cursor movement has been significantly improved for every supported speech engine.
• When “Move Cursor During Reading” is enabled, Sonarpad now uses a common progress system for Microsoft Edge Neural, Google TTS, SAPI4, SAPI5 and OneCore.
• The cursor follows the text actually being spoken more accurately, with more consistent sentence and phrase segmentation.
• Premature movement, delays, irregular jumps and differences between speech engines have been greatly reduced.
• The correct position is now preserved more reliably after pausing, resuming, searching within a document or changing the speech engine.

Separate podcast recording tracks
• Added “Save microphone and system or application audio to separate files”.
• When the microphone and another source are recorded together, Sonarpad can create one microphone-only file and a second file containing system audio, one application or the selected applications.
• Separate source recording is available in both MP3 and WAV.
• When the option is disabled, Sonarpad continues to create one normally mixed file.
• Separate files make volume adjustment, noise removal and later editing of podcasts, interviews and tutorials easier.

Scheduled radio recordings
• Radio recordings can now be scheduled in advance.
• For each recording, users can choose the station, day, start hour and minute, and duration.
• A custom duration from 1 to 1,440 minutes is available.
• Recordings can run once, every day or every week.
• The recordings window now shows active and scheduled recordings, planned date and time, duration and remaining time before start more clearly.
• Scheduled recordings can use Windows Task Scheduler, allowing them to start automatically even when Sonarpad is not already open.

Calendar
• Added a complete keyboard-accessible calendar.
• Users can browse previous and following days, return quickly to today and check holidays and observances.
• Added the saint of the day and quote of the day, which can be read, spoken or copied.
• Reminders can be created, edited, deleted, postponed and marked as completed.
• Alerts can be shown at the exact time or in advance and can use Windows scheduling even when Sonarpad is closed.

Weather
• Added a weather forecast section.
• Users can search for a city and quickly reopen recently viewed locations.
• Current conditions, temperature, minimum and maximum values, humidity, precipitation probability and forecasts for the following days are available.
• Temperature can be shown in Celsius, Fahrenheit or selected automatically.

Movies in theaters
• Added a section for movies currently in theaters and upcoming releases.
• Title search, plot, release date and trailer playback are available.

Google text-to-speech
• Added Google TTS for document reading and audiobook creation.
• Added a voice manager to list voices, filter them by language, download them and remove voices that are no longer needed.
• Speed, volume and pitch can be adjusted.
• Google Natural voice pitch is applied directly by the engine for a more natural and stable result.
• Google TTS responsiveness and reliability have been improved, with synthesis time limits adapted to the selected speech speed.
• Unnecessary waiting when the engine does not respond has been reduced, and error and interruption handling has been improved.
• Diagnostic logging is more stable during simultaneous operations.

EPUB table of contents
• Sonarpad now recognizes the table of contents embedded in EPUB books.
• Its presence is announced and it can be opened from the View menu.
• Chapters and subchapters are displayed hierarchically.
• Pressing Enter immediately moves to the selected location in the book.

News and RSS sources
• Expanded the News section with new search and organization tools.
• Added news language selection.
• Users can search within RSS sources and read news from their city.
• Community RSS sources can be browsed, added to the personal collection and submitted to the Sonarpad community.

Podcast recording
• Users can record only the microphone, all system audio, one application, multiple selected applications, or the microphone and applications together.
• The microphone device and audio source can be selected, source volumes adjusted separately and levels monitored in real time.
• Added pause and resume, MP3 or WAV output, MP3 bitrate selection and destination folder selection.
• The computer can be kept awake during recording.
• Separate files receive distinct names so the microphone track can be immediately distinguished from system or application audio.

Radio
• The Radio section has been extensively reorganized.
• Stations can be searched by name or free text, language, country, city, music genre or category.
• Favorites management has been improved and all filters can be reset quickly.
• Stations can be submitted to the Sonarpad community.
• Added live recording, “Record and Play”, a recordings list and recording deletion and management.
• Radio recordings are stored in their own folder inside the main recordings directory.

Media playback
• Significantly improved media player stability.
• Fixed an issue that could block mpv and made communication with the player more reliable.
• Improved the opening of different media file types.
• Sonarpad now remembers the volume used during playback.
• Stream and recording handling has been improved.
• Fixed files opened from Windows through double-click or “Open with”.

PDF documents
• Added recognition of form fields in PDF documents.
• Sonarpad can find fillable fields, present them in an accessible text form, allow their values to be edited and save the entered data back to the PDF.
• Fixed cursor position calculation during speech, especially in documents containing multibyte characters or complex structures.
• The new shared synchronization system further improves cursor movement with every speech engine.

Accessibility and keyboard commands
• Improved standard editing commands throughout the program.
• Copy, cut, paste, select all, undo and redo are now correctly sent to the focused field, including secondary windows and dialogs.
• Fixed an issue that could prevent Braille displays from updating correctly.
• Improved focus handling in secondary windows.
• Fixed language selection in the Wikipedia window.
• Added an option to group Tools menu functions by category.
• Added configurable actions to open Calendar, Weather and Movies in theaters quickly.
• Improved changelog presentation after an update.

Audiobooks
• Improved audiobook creation while dialogs or other modal windows are open.
• Progress handling is more robust and ignores obsolete audio updates, reducing freezes, incorrect notifications and unresponsive windows.
• Google TTS can also be used for audiobook creation with speed, volume and pitch controls.

Artificial intelligence
• Updated the default Gemini model to `gemini-3.5-flash`.

General fixes
• Fixed several mpv playback freezes.
• Fixed the opening of some audio and video files.
• Improved commands sent to the media player.
• Fixed cursor restoration during speech playback.
• Fixed shortcuts in text fields contained in auxiliary windows.
• Improved audiobook creation stability.
• Fixed files opened externally through Windows.
• Improved the overall handling of media, RSS, radio and EPUB.

Version 0.7.1 – 2026-05-13

New features and improvements
• Created the official website sonarpad.com, a new reference point for following the latest news, downloading the latest version of the program, reading visitor comments and, in the future, listening to all Sonarpad podcasts. The Help menu now also includes “Visit sonarpad.com”, allowing users to quickly open the official website.
• Fixed an issue where files with accents or special characters caused an error when voice transcription was started.
• From now on, in the View menu, items such as Word wrap and Show video during playback will always show the correct state, enabled or disabled.
• Improved YouTube search, allowing users to return to the previous page or screen with Esc.
• Added a preliminary check to verify whether a video can be played. Playback has also been improved: Sonarpad can now play videos or playlists marked as mixes, which previously could not be played.
• Improved automatic bookmark management. Previously, if the Automatic bookmarks option was enabled and then disabled, those bookmarks remained; now the program correctly ignores them until the option is enabled again. Also, when the end of a media file is reached, the bookmark is automatically deleted.
• Improved tag handling when dialogs are enabled. Sonarpad now correctly manages both features, allowing tags to be inserted even when the dialogs option is enabled.
• Improved voice settings by clearly separating each engine, making adjustments more precise. Voice profiles now correctly keep settings for each individual engine: Edge, Sapi5 and Sapi4.
• Added a tag for inserting pauses, directly from the options or from the voice panel by pressing Tab from the editor. The available choices are: 250 ms, 500 ms, 1 second, 2 seconds or a custom duration.
• Fixed the behavior when playing a YouTube video and starting transcription. Now, when returning with Alt+Tab, focus will correctly be on the Cancel button of the active transcription.
• Transcriptions are now saved automatically when the process is completed.
• Improved Wikipedia import. You can choose whether to read only one section and then return to the search from the article by pressing Esc, or import the entire article. You can also choose the Wikipedia language to use.
• Added a worldwide radio section, where you can search for radio stations by country, language and genre. You can also add local radio stations to the Sonarpad database, so other users can listen to them too. Radio stations can also be added to favorites.
• Added a routes section for calculating routes by choosing the travel mode: walking, cycling, driving or wheelchair. You can choose whether to calculate the shortest or fastest route and whether to show the municipalities crossed. Once the route is imported, you can also save the visual map from the File menu, Save image.
• Added Print to the File menu. Sonarpad will print TXT files using its own system and will use the associated program for other files, such as DOCX, PDF and similar formats, in order to preserve the original layout as much as possible.
• Added a translation service for each document, accessible from the editor context menu. Users can use the free DeepL and Google Translate services without entering any API key; by entering a Gemini API key, they can translate using Gemini instead.
• In the translation menu, users can choose the target language. The menu automatically reorders itself: if a user first chooses English, then French and then Italian, these three options will be shown at the top of the language menu.
• If users enter their Gemini API key, they can also access the Summarize text feature, also available from the context menu, to summarize any article.
• Added a menu to the Playback menu, visible while playing a media file, to split the current media. It works with MP3, MP4 and other formats, splitting either by number of parts or by the duration of each part.

Version 0.7.0 – 2026-04-25

What's New
• Added support for the mpv player for streaming playback. Videos from YouTube and supported sites are now played instantly; if the user chooses to keep them, they are downloaded as before. When transcribing streaming content, it is first downloaded and then transcribed. The mpv player is also used to open local videos and handle subtitles, ensuring improved compatibility with many formats that were previously not fully supported.
• Improved podcast recording for system audio: you can now choose whether to record all system audio, a single application, or multiple applications at the same time. This choice is integrated into normal recording, so the microphone can still be enabled or disabled separately.
• Added Hindi language. Translated the interface and added RSS feeds, changelog, and Sonarpad guide.
• Added an option in the Editor tab to always move the cursor to the start of the line when using the Up and Down arrow keys.
• Added an option in the "Convert Audio" menu to convert audio to M4B.

Fixes
• Fixed `F10` so it once again switches to the next favorite voice during text reading.
• When a podcast recording is in progress, closing another document no longer also closes the active recording.
• In YouTube comments opened from "Play streaming audio...", Sonarpad now loads only the first 50 top-level comments at first, always including all replies for those comments, and adds a final item to load all comments on demand.
• Bookmarks are now shown and handled in position order for both text documents and media files, instead of following creation order. If a bookmark already exists at the same position, it is no longer added again.
• Added an option in the Bookmarks menu that, when enabled, allows automatic bookmark management. When playing a local or streaming file and closing it, Sonarpad automatically sets a bookmark based on the reached position and resumes from that point when the file is opened again. The same applies to text files: if a text is opened and the cursor is moved, Sonarpad will remember that position when the file is closed; if reading is started, the last sentence read will be saved and reading will resume exactly from there.
• Added an item to the View menu to show video rendering for local or streaming files. The video content is shown in an enlarged window, where all controls are hidden unless the Alt key is pressed or the mouse is moved toward the top of the window. This should make the content larger and more usable for partially sighted users.

Version 0.6.9 – 2026-04-08

Fixes
• Improved the Find in Files experience: when opening Browse Folder, focus now goes straight to the folder list; opening a result with Enter no longer breaks keyboard commands; pressing Esc returns to the previously selected result; and when returning with Alt+Tab, focus goes either to the search field or to the results list if results are open.
• F5 always started reading from the beginning. It has now been fixed and reading starts from the current caret position, while preserving `Shift+F5` and `Ctrl+F5` for previous and next sentence navigation.
• After using Go to Line, pressing Esc could move focus out of Sonarpad. It now correctly returns focus to the editor.
• The `Word Wrap` option is now applied immediately to documents that are already open, instead of only taking effect after reopening the file.

Version 0.6.8 – 2026-04-07

What's New
• Added a new item to the Play menu that lets you transcribe any audio or video file with Whisper. A new “AI and Transcription” section is now available in Options, where you can choose the model, enable optional CUDA support for NVIDIA graphics cards, preserve the original language, and enable or disable timestamps.
• Added a new Play menu action, `Transcribe current folder`, which transcribes all supported audio files in the folder of the currently open media into a single combined document, with dedicated progress, current-file status, and cancellation support. It can also be started with `Alt+Shift+C`.
• Added offline voice dictation, using the same workflow as audio transcription. By default, press `Ctrl+Shift+Space` to start dictation and press the same shortcut again to stop it; the shortcut can be customized in Options. From the second activation onward, dictation is faster because the engine stays ready in memory; this preloading and reuse are automatically disabled on PCs with less than 4 GB of RAM.
• Added a new Editor option, disabled by default, that makes `Esc` close the editor window.
• Podcast search now uses `iTunes + Spreaker` by default, with duplicate filtering when the same podcast is found on both platforms.
• Improved Apple podcast browsing and search: podcast search, category browsing, and top podcasts by category now use the selected podcast directory country. In Options > RSS / Podcast you can leave it on `Automatic` to use the system country, or choose a different country manually.
• Increased the result limit for Apple podcast categories. The first opening still loads the first 50 results as before; if you choose `Load more results`, Sonarpad loads up to 200 total results (Apple's limit) and lets you navigate through the following pages while keeping the experience smooth.
• Sonarpad is now also available on Mac with a subset of features. Project link: https://github.com/Ambro86/Sonarpad-Mac

Improvements
• Added more than 50 selectable countries for the podcast directory, so users can choose from a much wider range of national catalogs.
• "Play streaming audio..." can now also search YouTube from any text query, or accept a YouTube channel or playlist link and show its results.
• Improved how results are shown in "Play streaming audio...": YouTube entries now include title, duration, channel, and view count in a clearer format.
• "Play streaming audio..." now also supports YouTube comments: you can open them from the context menu, read replies, and expand comment threads with the Right Arrow key.
• Added YouTube favorites for channels and playlists in "Play streaming audio...": they can be added from the results with the context menu, opened directly from the Favorites list reached with Tab right after the YouTube URL/query field, and removed later from that same list using the context menu. In YouTube search results, the context menu is available only for channels and playlists.
• "Play streaming audio..." can now request credentials when a streaming site requires login. Users can enter them, save them for the site, and manage saved credentials later in Options > Audio.
• Improved focus handling during "Play streaming audio...", so the progress window stays more stable during download and conversion.
• Added two new reading navigation actions in the Voice menu: `Previous sentence` and `Next sentence`, with configurable shortcuts to jump during text reading.
• The default shortcut for `Execute file with interpreter` is now `Ctrl+Shift+F5`, so `Shift+F5` can be used by default for `Previous sentence`.
• Added voice profiles in Options > Voice: profiles can be added, renamed, and deleted.
• Expanded the playback rewind interval options in Options > Audio with additional values from 1 second up to 2 hours.
• Added Russian translation thanks to Dmitriy.
• Added a new option in Options > Audio to choose the audiobook part naming format: `Title + number`, `Number only`, or `Number + title`.
• Added RSS favorite articles: from the article context menu you can add items to a dedicated Favorites feed.
• The Favorites RSS feed can be deleted and is recreated automatically when a new article is added to favorites.
• Added RSS keyboard shortcuts to move feeds up/down: `Ctrl+Shift+Up Arrow` and `Ctrl+Shift+Down Arrow`.
• Improved the RSS window with a built-in article preview, so article text can be reviewed directly there and reached quickly with Tab before opening the full article in the editor.
• Added an explicit RSS entry “Load more news” at the end of feeds when more items are available; pressing Enter loads the next batch and moves focus to the first newly loaded article.
• In the voice dictionary, when adding or editing a replacement, there is now a “Match Case” checkbox so each substitution can either respect or ignore letter casing.
Bug fixes
• "Play streaming audio..." now respects the podcast cache limit already set in Options, and the same limit now also applies to audio descriptions playback.
• Fixed Wikipedia import so quote blocks present in pages are now imported correctly.
• Improved the web page parser for WordPress pages where list items and some section headings could be omitted.
• "Go to line" now pre-fills the field with the current line.
• Fixed OPML export for podcasts and RSS so the exported files are now accepted by iTunes.
• Added localized confirmation messages for correct OPML import and export of RSS feeds and podcasts.
• Fixed a bug where, in "Play streaming audio...", typing a search string and selecting a YouTube channel from the results could make the program appear stuck instead of opening that channel’s videos.
• Fixed a bug where the list of open files was shown in the Help menu instead of the Window menu.
• Fixed a streaming edge case where playback could start but the “Downloading stream” dialog stayed open when the downloaded file already matched the target format.
• Fixed MP3 streaming conversion behavior: when the stream is already MP3 and the user selects an explicit MP3 bitrate (for example 128 kbps), Sonarpad now re-encodes to the selected bitrate instead of skipping conversion.
• Fixed media transcription documents so closing them now asks whether to save, and the suggested file name correctly reuses the transcribed media file name instead of the first line of the text.
• Fixed the `Alt+Shift+L` shortcut: it now correctly opens the chapter list during playback.
• Fixed the `Alt+Shift+T` shortcut: it now correctly starts “Transcribe current audio” instead of opening the Tools menu.
• Fixed playback stop handling in the Play menu: pressing `.` now behaves like Stop and only stops the current track, instead of also exiting the player/episode.
• Fixed the save entry in the Play menu for media opened from Recent Files: when the file comes from a local Sonarpad cache, the localized save action is now shown correctly there as well.
• When transcription starts while audio is already playing, Sonarpad now pauses that audio automatically before starting transcription.
• Fixed a bug where importing an article from Wikipedia could succeed without showing the article text on screen.
• Added embedded podcast chapter support from local media files (e.g., MP3 chapter metadata): when feed/URL chapters are unavailable, Sonarpad now loads chapters from the downloaded file in the background, so playback starts immediately and chapter data is applied as soon as it is ready.
• Fixed chapter loading for downloaded podcast episodes opened as normal local media files: embedded chapters are now available there too, not only when playback starts from the Podcasts window.
• Fixed MP3 audiobook finalization for SAPI4 and SAPI5: final output is now finalized correctly to avoid incomplete or fragile files after long exports.
• Added an explicit finalization progress bar for all audiobook creation modes: after the creation phase, Sonarpad now announces and shows a dedicated finalization phase with visible progress.
• Fixed dialogue voice tuning: speed/pitch/volume settings are now correctly applied for both the first and second dialogue voices during synthesis.
• Improved text encoding detection for Japanese `.txt` files: added a safe Shift_JIS/CP932 fallback for mojibake cases, while preserving existing UTF/diacritics/Chinese behavior.
• Internal safety refactor: converted functions to safe implementations where possible and significantly reduced unsafe code lines.

Version 0.6.7 – 2026-03-02
Improvements
• Ora il programma riesce a gestire Sostituisci tutto in modo massivo su file grandi con un gran numero di sostituzioni.
• Updated Polish translation thanks to DJ Graco.
• Added Lithuanian translation.
• Added Chinese translation.
• From now on, frequent beta builds will be published in the project Releases section, so users can test new changes before the next stable release.
• Added shortcut `Ctrl+.` to insert an ellipsis character (…).
• Improved podcast chapter support: chapter navigation now works more reliably, including direct/streamed episodes where chapters are not embedded in the MP3 file, by using chapter metadata from feed/URL fallbacks when available. Added chapter navigation shortcuts `Ctrl+Alt+PageUp` (previous chapter) and `Ctrl+Alt+PageDown` (next chapter).
• Reorganized Sonarpad output folders under `Documents\\Sonarpad`: files are now saved in dedicated subfolders `audiobooks`, `documents`, `recordings`, and `media`, with automatic migration from legacy paths.
• Improved support for very large text files (including 60 MB): smoother opening and line-by-line navigation, especially with screen readers.
• Updated guides for all languages and refreshed localization resources across the app, including donations texts and NSIS setup translations (new Simplified Chinese and Lithuanian installer strings, plus completed Ukrainian setup translation).
• Added global network proxy support (HTTP/HTTPS and SOCKS5/SOCKS5H) for online features, with proxy validation on Options save: invalid proxies are warned and automatically removed.
• Added a new Tools action: "Play streaming audio...", allowing users to paste a URL (YouTube or direct media link), choose output format and quality/bitrate profile (including original quality/bitrate for MP3 and MP4), and play it directly in Sonarpad’s audio player.
• Added support for the system media Play/Pause key (headsets/keyboards): it now controls both media playback and text reading pause/resume (with media playback priority when both are active).
• Added a new File > Recent Files entry: "Clear recent files" to quickly wipe the recent documents list.
• Expanded audio bitrate options in Convert Audio and podcast recording settings: added lower values (64/96 kbps) and extended MP3 up to 320 kbps, with aligned validation and encoder handling.
• Extended audiobook split-by-time options up to 60 minutes.
• Improved audiobook split-by-parts: users can now enter the number of parts manually, with validation from 1 to 100.
• Added a new View > Read-only mode to lock editor text from accidental edits while keeping documents fully readable and navigable.
• Added an accessible progress bar during program updates, so screen readers can track download progress in real time.
• Added a new quiet status bar in the main window showing characters, words, and line/column (for example: "Characters (with spaces): 11. | Words: 2. | Ln 1, Col 12") without disturbing NVDA focus.
• Added a new View menu toggle for Word wrap, so line wrapping can be switched quickly without opening Options.
• Added new Edit > Text actions for indent/outdent, with shortcuts Ctrl+Shift+. (indent) and Ctrl+Shift+, (outdent), because when “Show voices in editor” is enabled the Tab key is reserved for voice-panel navigation.
• Added localized date/time in RSS articles and podcast episodes, with formatting adapted to the current interface language.
• Added a new RSS context-menu action to share the selected article by email.
• Added granular delete-confirmation options for RSS/Podcast in Options > RSS and podcast: RSS (feed/article/both/none) and Podcasts (podcast/episode/both/none).
• Added configurable quick RSS copy with Ctrl+C (Options > RSS and podcast): copy title, URL, article content, or all combined.
• Unified RSS source creation: “Add source” now accepts both direct feed URLs and keyword input (auto-generating Google News RSS), replacing the need for a separate keyword-search action.
• Pressing Ctrl+A now announces completion for clearer screen-reader feedback.
• Added Shift+F3 for "Find previous" in the Edit menu, complementing F3 "Find next".
• Improved replace feedback messages with proper singular/plural forms (e.g. “1 replacement made” vs “2 replacements made”).
• Added dictionary lookup language selection in the dictionary window, with default Auto (interface language) and optional manual override.
• Added a new Shortcuts tab in Options to customize key bindings, with conflict detection that warns when a shortcut is already assigned to another action.
• Added initial command-line switch support: `-h`/`--help` now show usage information and `--version` prints the program version.
• Improved manual speed and pitch tuning clarity: manual fields now use a 100-centered scale, where 100 corresponds to the normal value.
• Improved Microsoft voices selection in both Options > Voice and the in-editor Voice panel: added a localized language combo to filter voices by language, while keeping multilingual-only mode as a single ungrouped voice list (language combo hidden when enabled).
• Added dialogue-voice configuration in Options > Voice with full Tab navigation, using the same voice model as the main UI (engine, Edge language filter, voice, and labeled speed/pitch/volume); added optional secondary dialogue voice with the same controls (engine, Edge language filter, voice, speed/pitch/volume) for alternating dialogues; dialogue voice rules are saved in configuration `.ini`, so document text is not modified.
• Improved Undo labeling: the Edit > Undo entry now shows what action will be undone (for example, text edits, quote/unquote lines, or voice-tag insertion), while remaining disabled when no undo is available.
Bug fixes
• Fixed RTF file opening: `.rtf` documents are now parsed and displayed as plain readable text instead of raw RTF markup (e.g. `{\\rtf1...}`).
• Fixed opening of Chinese text files encoded in GB18030/GBK: Sonarpad now detects and decodes these files correctly, avoiding mojibake output.
• Improved M4B audiobook creation with chapter metadata and chapter markers; fixed the chipmunk playback issue (high pitch/speed) in generated M4B files.
• Fixed audiobook save dialog bitrate UI: removed hardcoded Italian labels and added 64 kbps to selectable bitrate options.
• Fixed Save All (Ctrl+Shift+S): all open modified documents are now detected reliably (including unsaved/new tabs), and Save All correctly saves each one or opens Save As where needed.
• Fixed Google News RSS item ordering: articles are now shown in descending publish date (newest first) when dates are available.
• Fixed NVDA label association in the dictionary window: search field and language combo now announce the correct labels.
• Fixed RSS/Podcast Properties window keyboard handling: Tab/Shift+Tab now reaches the OK button, Enter activates OK, Esc closes safely, and focus correctly returns to the RSS/Podcast list.
• Fixed RSS/Podcast undo history: Ctrl+Z now supports multi-level undo for removals (articles/episodes and sources), not just the last action.
• Improved RSS/Podcast removal feedback with explicit status announcements (RSS removed, RSS article removed, podcast episode removed).
• Improved RSS/Podcast focus behavior after delete/undo: RSS now reliably focuses the first feed when needed and avoids repeated screen-reader announcements during delayed reselection.

Version 0.6.6 – 2026-02-13
Improvements
• Added "Auto format for TTS" in the Edit menu to quickly prepare text for speech (removes markdown/quotes and reflows wrapped lines).
• Improved voice-tag insertion: when text is selected, tags are now applied correctly to both single-line and multi-line selections.
• Added a default audiobook save folder option in Audio settings (default: Documents\\Sonarpad Audiobooks).
• In the audiobook save dialog, when split mode is enabled, added a new default-on option to create a dedicated subfolder for split parts (for cleaner output organization).
• Audiobook export now saves MP3 in stereo with user-selected bitrate for Edge, SAPI5, and SAPI4 voices.
• Added support for 32-bit SAPI5 voices via bridge, so voices available only in 32-bit engines can also be used in Sonarpad.
• Reorganized voice features into a dedicated "Voice and audio" menu and added/clarified "Convert Audio", useful for converting any supported media file to MP3, AAC, OGG, Opus, FLAC, WAV, and AIFF.
• Added removal of individual RSS articles and podcast episodes (Delete key + context menu with confirmation), without removing the entire RSS/podcast source, plus undo for the last removal (single article/episode or entire RSS/podcast source).
• Added RSS feed export to OPML in the RSS window, so current RSS sources can be saved and re-imported easily.
• Added "Search RSS by keyword" in the RSS window: entering a keyword now generates a Google News RSS URL automatically and opens the add-source dialog prefilled, so keyword feeds can be created in one step.
• Added Serbian translation thanks to Mila Kuran.
• Added Ukrainian translation thanks to Ivan Shtefuriak.
• Added multi-file media opening: selecting/opening multiple media files now builds a playback queue instead of replacing the current file.
• Added variable seek shortcuts during playback: with a 1-minute base skip, Left/Right seeks 60s, Shift+Left/Right seeks 20s, and Ctrl+Left/Right seeks 3 minutes.
• Added previous/next track navigation shortcuts in the player: Ctrl+PageUp and Ctrl+PageDown.
• Added "Reset volume" and grouped reset actions into a dedicated "Reset" submenu in Playback, alongside "Reset speed" and "Reset pitch".
• Installer improvements: setup.exe now lets users choose between associating all supported file types or selecting extensions manually; MSI now exposes per-extension file-association choices in the feature tree (default remains all enabled).
• Added a new "Window" menu with "Open documents..." to quickly switch to any currently open file.
• Updated View > Font: replaced the old chooser with a quick submenu of common fonts (Arial, Calibri, Consolas, Segoe UI, Tahoma, Verdana, Times New Roman, Georgia) while preserving the current text size.
• Improved RSS/Podcast announcements with a dual status model: source nodes announce "new items" when a feed/podcast has updates, while individual RSS articles and podcast episodes announce "unread"/"unplayed"; this behavior can be disabled from Options.
Bug fixes
• Fixed EPUB text extraction for books containing inline HTML comments (<!-- ... -->): chapter text is now parsed correctly instead of being partially or fully skipped.
• Fixed Spanish Wiktionary lookups and dictionary cache handling: Spanish entries like "agua" now load correctly, and old "Word not found" cache entries are no longer reused.
• Fixed RSS article import character encoding for some Spanish sources (e.g., El Mundo): accented letters and "ñ" are now preserved correctly in the temporary editor.
• Fixed ANSI text decoding for Central European files (e.g., Czech/Polish): Sonarpad now better distinguishes UTF-8 vs ANSI and chooses the correct code page (including Windows-1250) to prevent garbled diacritics.
• Fixed RSS source persistence for feeds with URL query parameters (e.g., `rss.aspx?c=...`): these feeds are now saved and restored correctly after restarting Sonarpad.
• Fixed opening Google Drive pointer files (`.gdoc`, `.gsheet`, `.gslides`) from Explorer context menu: when direct read fails with “Incorrect function (os error 1)”, Sonarpad now falls back to shell-open so the document still opens correctly.
• Fixed legacy Excel 2010 `.xls` reading: old binary Excel files are now detected and decoded correctly instead of showing garbled text (e.g. `ÐÏ_à¡±...`).
• Fixed spellcheck announcement flow: misspellings are now announced again when reviewing text later, and the same mistake is reported again if it is deleted and retyped.
• Fixed line-based text actions (e.g. Ctrl+Q / Ctrl+Shift+Q, sort/reverse/unique/join lines): selecting a single line with Shift+Down no longer merges or truncates adjacent lines.
• Fixed multi-line behavior for line-based text actions (Ctrl+Q / Ctrl+Shift+Q and related line tools): RichEdit selections using CR-only separators are now normalized correctly, so all selected lines are processed without cutting first characters.
• Extended TTS input normalization for visible whitespace symbols (␠/U+2420, ␣/U+2423, ␉/U+2409, ␊/U+240A, ␍/U+240D, ␤/U+2424) to prevent repeated paragraph playback with multilingual voices.
• Refined Edge TTS text sanitization with a single validation pipeline: weird/invisible spaces are normalized, long punctuation runs (like "...", "!!!", "???") are compacted, and punctuation-only chunks are skipped to prevent playback loops.
• Fixed playback time announcement (Ctrl+I) for MP3/podcast streams: current time is now clamped to track duration, and playback is auto-stopped if position runs past the end.
• Improved installer localization coverage: setup.exe now includes additional installer languages (Czech, Polish, French, Serbian), while MSI is kept as a single en-US package to avoid release confusion.
• Fixed uninstall cleanup for context menu entries: "Open with Sonarpad" is now removed reliably, including legacy registry scenarios.
• Fixed SAPI5 pause/resume reliability: F4 pause now works correctly and resume returns to the expected position instead of restarting from the beginning.
• Fixed pause + seek + resume flow for media playback: after pausing and seeking with Left/Right, pressing Space now reliably resumes from the current position instead of stopping or restarting from the beginning.

Version 0.6.5 – 2026-02-07
Improvements
• Spanish translation improved thanks to Arturo Fernandez Rivas.
• Added an option to split EPUB audiobooks by chapters.
• RSS imports now use a dedicated temporary tab (localized title); Save As converts it to a normal document.
• Screen reader messages are now also sent to JAWS when available.
Bug fixes
• Reading from the caret (F5) now starts exactly at the cursor. Previously it could start a couple of lines above because the caret offset did not match CRLF/UTF-16 positions.
• Fixed a redraw issue where typing over a selection could make earlier text temporarily disappear until the selection moved.
• Fixed EPUB chapter parsing so cover or image-only pages no longer produce spoken CSS (e.g., "padding") or "Sconosciuto" titles.
• Fixed audiobook time-splitting from EPUBs with Edge TTS failing on empty/oversized chunks ("Edge audio not sent").
• RSS articles now decode HTML entities (e.g., &quot;, &amp;, &lt;, &gt;).
• Save/Save As now suggests the existing filename when saving non-overwritable formats (e.g., EPUB) instead of the first line.
• Fixed a bug where podcasts with new episodes were not announced as unplayed, and renamed "Unheard" to "Unplayed" for a more professional label.

Version 0.6.4 – 2026-02-05
Improvements
• The program has been renamed to Sonarpad to emphasize sound and audio as the key focus.
• Added audio track selection in the Playback menu for media files with multiple audio tracks (e.g., MKV files with multiple languages).
• Podcasts now clearly indicate unheard episodes with an "Unheard" prefix before the name.
• New tag-based voice switching in text. Examples:
  - Microsoft voices (Edge): <voice edge it-IT-IsabellaNeural>Hello</voice>
  - SAPI5 voices: <voice sapi5 Microsoft Helena Desktop>Hello</voice>
  - SAPI4 voices: <voice sapi4 #1>Hello</voice>
  - With speed/pitch/volume: <voice edge it-IT-ElsaNeural speed=-20 pitch=-5 volume=-10>Hello</voice>
• Enriched podcast categories.
• Improved PDF reading with automatic fallback to PDFium.
• Improved the article parser for cases where content was not read in full.
• Added pitch reset to the Playback menu.
• Added a context menu option to create an audiobook from the selected text.
• Added audiobook splitting by duration, with the ability to choose the first file name.
• Localized the author label in article reading (e.g., "by", "di", "par").
• Added indentation options (tabs/spaces with width) and Tab/Shift+Tab indent/outdent on selected lines.
• Fixed Markdown cleanup to handle '*' list bullets when bullet preservation is disabled.
• Added an option to use the legacy "Novapad" name in the window title and Start Menu shortcuts.
Bug fixes
• Fixed a bug where SAPI4 audiobooks could be created differently than expected.
• Fixed a bug where seeking past the end of a media file restarted playback from the beginning.
• Find in Files window: pressing Enter on a result now opens at the correct snippet position, and Esc returns to results.
• Options window: improved visual layout across General, Voice, Editor, and Audio tabs to prevent missing or clipped controls.
• Fixed a bookmark issue when changing playback speed.
• Fixed Podcast Index categories not displaying correctly.
• Fixed apostrophes breaking reading by removing separate dialogue reading; voice tags are used instead.

Version 0.6.3 – 2026-01-30
Improvements
• Improved microphone detection.
• Added instant playback support for all formats.
Bug fixes
• Fixed crash in podcast categories window.

Version 0.6.2 – 2026-01-30
New features
• Added file execution support (Shift+F5). Users can select an interpreter (e.g., python) in Options, search for it on the computer, and pressing Shift+F5 runs the current script. HTML files open in the browser.
• Added support for Google Docs pointer files (.gdoc, .gsheet, .gslides), which automatically open in the default browser.
• Added support for M4B audiobook format (Apple/AAC).
• Added "Show episodes" option in podcast search results context menu to browse and play episodes without subscribing.
• Added "Go to Line" feature (Edit menu or Ctrl+J) to quickly jump to a specific line number.
• Added context menu options to order RSS feeds and podcasts (alphabetically or by date).
• Added Vietnamese default RSS feeds.
• Added a microphone test box in the recording dialog to check levels before starting.
• Added "Show description" for podcast episodes in the context menu.
• Added support for extended audio/video formats via FFmpeg: mkv, avi, mov, m4v, webm, mpg, ts, wmv, flv, vob, 3gp, flac, ogg, wma, aiff.
• Added synchronized subtitle reading support (srt, vtt, ass, sub, sbv, lrc, smi) with NVDA or selected voice. The program searches for a subtitle file with the same name as the media file. Added "Import subtitles" and "Remove subtitles" options in the Playback menu for files with different names.
• Added file associations for all new supported audio/video formats in the "Open with Sonarpad" context menu.
• Added pitch adjustment setting for any file.
• Added option in General settings to enable or disable anonymous error reports. Added a menu item in Help to create a diagnostic ZIP file.
• Added option to use a different voice for dialogues, both for live reading and audiobook creation.
• Added podcast categories browser to explore podcasts by category (business, art, sport, etc.).
Improvements
• Opening an audio/video file from Explorer now opens the player view directly instead of the text editor.
• Removed the OCR prompt for inaccessible PDFs; OCR is now performed automatically to improve speed and user experience.
• Improved Accessible Terminal: NVDA reading now remembers the last read line for better continuity.
• SAPI 4: Audiobook creation is now fully parallelized and nearly instant. Added a prompt to choose the number of concurrent processes.
• SAPI 4: Eliminated the WAV-to-MP3 bottleneck by converting chunks in parallel during synthesis.
• SAPI 4: Improved error handling and automatic cleanup of temporary files.
• Find dialog: Renamed "Regex" to "Regular expression" for clarity and added missing translations for search options.
• M4B Audiobooks: Better output handling; splitting by parts/markers now produces a single M4B file with proper metadata chapters including title and author.
• Player: Fixed bookmark and time announcement precision when playback speed is not 1.0x.
• Restored Ctrl+Tab and Ctrl+Shift+Tab navigation in Options.
• Added an option in the Playback menu to instantly reset speed to Normal (1.0x).
• Updated all dependencies to the latest versions for better performance and stability.
• Integrated FFmpeg with dynamic DLL loading to ensure compatibility without blocking startup.
• Updated podcast download filters to include new audio/video formats.
• Prevented Ctrl+S from saving audio/video files to avoid corruption.
• Improved YouTube transcript import making it more robust and resilient.
• Improved audiobook part splitting robustness, ensuring no text is lost.
• Installer is now fully multilingual, supporting Italian, English, Spanish, Portuguese, Swedish, and Vietnamese based on the user's system language. English is the default for unsupported systems.
• Podcast categories: pressing Enter on a category now confirms the selection (equivalent to OK button).
• Improved hang detection system to avoid false positives when modal dialogs (error messages, "text not found") are open.
Fixes
• Fixed a bug where the changelog did not open on startup.
• Fixed a bug where the OCR prompt did not appear for inaccessible PDFs opened from Explorer.
• Fixed a startup bug that could cause loss of focus or window closure immediately after opening.
• Fixed a critical bug in regex search that prevented finding text, including issues with "Wrap around" search and "Dot matches newline" option with Windows line endings.
Localization
• Added Polish translation.
• Added French translation.
• Added Czech translation (thanks to Radek Žalud and Jiri Holzinger).

Version 0.6.1 – 2026-01-20
Fixes
• Fixed a bug where enabling “Show voices in the editor” caused podcast playback to stop.
• Fixed an issue where some podcasts could not be added via URL because the URL was being truncated.
• Fixed a bug where normal URLs could no longer be added in the RSS feed feature.
• Fixed an issue where the Wikipedia language option was shown multiple times across different settings tabs.
• Removed the creation of debug files that were incorrectly generated even in release mode.
Improvements
• Improved support for Microsoft voices, which now use a dedicated playback method with a different user agent.
• Added support for MP4 files.

Version 0.6.0 – 2026-01-20
New features
• Added spell checker. From the context menu, users can check whether the current word is correct and, if not, get spelling suggestions.
• Added podcast import and export via OPML files.
• Added Podcast Index search support in addition to iTunes. Users can enter their free API key and secret (generated using only an email address).
• Added support for SAPI4 voices, both for real-time reading and audiobook creation.
• Added automatic OCR fallback for non-accessible PDFs: when no extractable text is found, the document is recognized via OCR.
• Added dictionary support using Wiktionary. Pressing the Applications key shows definitions, and when available, synonyms and translations into other languages.
• Added Wikipedia article import with search, result selection, and direct import into the editor.
• Added Shift+Enter shortcut in the RSS module to open an article directly in the original website.
Improvements
• Microphone selection is now always respected by the application.
• In the podcast window, pressing Enter on an episode now immediately announces “loading” via NVDA to confirm the action.
• In podcast search results, pressing Enter now subscribes to the selected podcast.
• Fixed and improved labels for Ctrl+Shift+O and Podcast Ctrl+Shift+P shortcuts.
• Playback speed and volume are now saved in settings and persist across all audio files.
• Added a dedicated cache folder for podcast episodes. Users can keep episodes via “Keep podcast” in the Playback menu. The cache is automatically cleaned when exceeding the user-defined size (Options → Audio).
• Improved RSS article fetching significantly using libcurl impersonation with Chrome and iPhone profiles, ensuring compatibility with ~99% of sites.
• Added read / unread state for RSS articles, with clear indication in the RSS list.
• Replace All now reports the number of replacements performed.
• Added a Delete Podcast button when navigating the podcast library using Tab.
Fixes
• Removed the redundant “pending update” entry from the Help menu (updates are already handled automatically).
• Fixed a bug where pressing Ctrl+S on an opened MP3 file would save and corrupt the file.
• Fixed a UI issue where “Batch Audiobooks” was shown as “(B)… Ctrl+Shift+B” (removed redundant label).
• Fixed smart quotes: when enabled, normal quotes are now correctly replaced with smart quotes.
• Fixed a bug where using “Go to bookmark” reset the playback speed to 1.0.
• Fixed an issue where already-downloaded podcast episodes were re-downloaded instead of using the cached version.
Keyboard shortcuts
• F1 now opens the Help guide.
• F2 now checks for updates.
• F7 / F8 now jump to the previous or next spelling error.
• F9 / F10 now quickly switch between favorite voices.
Developer improvements
• Errors are no longer silently dropped: all let _ = patterns have been removed, and errors are now explicitly handled (propagated, logged, or handled with fallbacks as appropriate).
• The project now fails to compile if there are warnings: both cargo check and cargo clippy must pass cleanly, with lints tightened and allow removed where possible.
• Custom implementations such as strlen / wcslen-style helpers have been removed. String and UTF-16 buffer lengths are now derived from Rust-owned data instead of scanning memory.
• DLL handling has been cleaned up and consolidated around libloading, avoiding custom loader logic and PE parsing.
• Hand-rolled byte parsing helpers were removed; all byte parsing now uses standard from_le_bytes / from_be_bytes on checked slices.
These changes reduce unnecessary unsafe usage, eliminate potential undefined behavior, and make the codebase more idiomatic, robust, and maintainable.

Version 0.5.9 - 2026-01-13
New features
• Added RSS reordering from the context menu (up/down/to position) with invalid-position checks.
• Added an article context menu with open original site and share via WhatsApp, Facebook, and X.
• Added Esc shortcut to return from imported articles to the RSS list.
• Added podcast mode: search, subscribe, listen; reorder subscriptions; Esc stops playback and returns to the list; Enter on an episode starts playback.
• Added playback speed control for podcasts and MP3 files.
• Added Ctrl+T to jump to a specific time.
• Added a voice preview button after the volume combo.
• Added regex find and replace (Notepad++ style).
• Added RSS import from OPML and TXT files.
• Added an option to enable "Open with Sonarpad" in File Explorer, including portable builds.
Improvements
• Improved voice speed/pitch/volume selection, respecting TTS max limits.
• Various RSS improvements to download all articles without moving NVDA focus during updates.
• Improved audio playback with a dedicated menu, Ctrl+I time announce, and volume up to 300%.
• Added missing shortcuts for some functions.
• Reorganized the Edit menu with a text cleanup submenu.
• Reorganized Options into tabs, with Ctrl+Tab and Ctrl+Shift+Tab navigation.
• RSS reader now downloads full article content, matching the browser view.
Fixes
• Fixed Markdown cleanup removing numbers at the start of lines.
• Fixed AltGr+Z triggering undo.
• Fixed audiobook recording cancellation so it stops quickly.
Localization
• Added Vietnamese translation (thanks to Anh Đức Nguyễn).

Version 0.5.8 - 2026-01-10
New features
• Added volume control for the microphone and system audio when recording podcasts.
• Added a new feature to import articles from websites or RSS feeds, including the most important feeds for each language.
• Added a function to remove all bookmarks for the current file.
• Added a function to remove duplicate lines and duplicate consecutive lines.
• Added a function to close all tabs or windows except the current one.
• Added a Donations entry in the Help menu for all languages.
Improvements
• Improved the accessible terminal to prevent some crashes.
• Improved and fixed access keys and keyboard shortcuts across the app.
• Fixed an issue where closing the audio playback window did not stop playback.
• Added confirmation dialogs for important actions (e.g., remove duplicate lines, remove end-of-line hyphens, remove all bookmarks in the current file). No dialog is shown when the action does not apply.
• Added the ability to delete RSS feeds/sites from the library by selecting them and pressing Delete.
• Added a context menu in the RSS window to edit or delete RSS feeds/sites.
• Removed the setting to move settings to the current folder; the app now handles this automatically based on location (if the exe folder is named "sonarpad portable" or the exe is on a removable drive, settings go to the exe folder in `config`, otherwise `%APPDATA%\\Sonarpad`, with fallback to the exe `config` if the preferred folder is not writable).

Version 0.5.7 - 2026-01-05
New features
• Added Batch Audiobooks feature to convert multiple files/folders at once.
• Added support for Markdown files (.md).
• Added file encoding selection when opening text files.
• Added option in the accessible terminal to announce new lines with NVDA.
Improvements
• Audiobook recording now saves natively to MP3 when selected.
• User can now choose the position of the "unsaved changes" asterisk (*) in the window title.
• Improved the update system robustness across different scenarios.
• Added "Remove Hyphens" in Edit menu to fix OCR line-endings.

Version 0.5.6 - 2026-01-04
Fixes
  Improved Find in Files so pressing Enter opens the file exactly at the selected snippet.
Improvements
  Added PPT/PPTX support (open as text).
  Opening non-text formats now saves as .txt to avoid formatting corruption (PDF/DOC/DOCX/EPUB/HTML/PPT/PPTX).
  Added podcast recording from microphone and system audio (File menu, Ctrl+Shift+R).

Version 0.5.5 – 2026-01-03
New features
• Added an accessible terminal optimized for large output and screen readers (Ctrl+Shift+P).
• Added a setting to save user settings in the current folder (portable mode).
Fixes
• Improved Find in Files snippets so the preview stays aligned with the match.

Version 0.5.4 – 2026-01-03
Improvements
• Fixed Normalize Whitespace (Ctrl+Shift+Enter).
• Added HTML/HTM support (open as text).

Version 0.5.3 – 2026-01-02
New features
• Added Find in Files.
• Added new text tools: Normalize Whitespace, Hard Line Break, and Strip Markdown.
• Added Text Statistics (Alt+Y).
• Added new list commands in the Edit menu:
• Order Items (Alt+Shift+O)
• Keep Unique Items (Alt+Shift+K)
• Reverse Items (Alt+Shift+Z)
• Added Quote / Unquote Lines (Ctrl+Q / Ctrl+Shift+Q).
Localization
• Added Spanish localization.
• Added Portuguese localization.
Improvements
• When an EPUB file is open, Save now automatically switches to Save As and exports the content as a .txt file to prevent EPUB corruption.

## 0.5.2 - 2026-01-01
- Added a changelog.
- Added open-with-Sonarpad options and file associations for supported files during installation.
- Improved message localization (errors, dialogs, audiobook export).
- Added part selection when using "Split audiobook based on text", with a "Require the marker at line start" option.
- Added YouTube transcript import with language selection, timestamp option, and improved focus handling.

## 0.5.1 - 2025-12-31
- Automatic updates with confirmation, improved error handling and notifications.
- Audiobook export improvements (text-based split, SAPI5/Media Foundation, advanced controls).
- TTS improvements (pause/resume, replacement dictionary, favorites).
- View menu and voice/favorites panels, text color and size.
- Default language from system locale and localization improvements.
- CI and Windows packaging (artifacts, MSI/NSIS, cache).

## 0.5.0 - 2025-12-27
- Modular refactor (editor, file handler, menu, search).
- Windows build/packaging workflow and README/license updates.
- Fix TAB navigation in the Help window.

## 0.5 - 2025-12-27
- Preliminary version bump.

## 0.1.0 - 2025-12-25
- Initial release: project structure and README.
