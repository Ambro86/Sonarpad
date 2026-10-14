# Optional non-verbal voice audio-description mode

The Windows Create audio description window includes a remembered checkbox, disabled by default. With the checkbox OFF, the original Pyannote >0 speaker logic, original Gemini prompts and original protected intervals are retained.

When enabled, the same Pyannote ONNX inference computes low-confidence candidates (`p50 < 0.75`, `p90 < 0.80`, at least 0.25 s). This does not indicate a yawn, groan or cry; actual words can yield low scores too. The existing Gemini video request is given candidates with IDs and must return only IDs whose *entire interval* it verifies as word-free vocalizations. The final protected intervals are adjusted only for IDs confirmed by Gemini, preserving 50 ms on each boundary. Rust's existing scheduling and TTS flow receives these updated protected intervals without a separate model or audio analysis pass. On rejection or missing field, candidates remain protected. Full protection is not guaranteed when Gemini misclassifies a candidate, so this is strictly opt-in.

The option is stored in global settings, in saved AD project metadata, and in interrupted-job checkpoints, along with the verified candidate IDs needed for resume.

Windows build: compile the Rust application as usual. Rebuild the audio-description Python bridge using:

    powershell -ExecutionPolicy Bypass -File .\bridge\build_audio_description_bridge.ps1

Use the rebuilt bridge executable in `dll/` and ensure Sonarpad's remote bridge asset/cache is refreshed for production users. A source ZIP alone does not update the packaged Windows executable.

Tests: `pytest bridge/tests/test_speech_detector.py bridge/tests/test_optional_nonverbal_vocals.py bridge/tests/test_chunk_timestamps.py bridge/tests/test_audio_description_preferences.py`.
