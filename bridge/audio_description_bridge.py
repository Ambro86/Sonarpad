"""Headless Omni-derived audio-description analysis worker for Sonarpad.

Protocol
--------
The Rust host writes a UTF-8 JSON request to a temporary file and invokes:

    audio_description_bridge.exe --request <path>

Stdout contains line-oriented events. Only these prefixes are protocol data:

    STATUS:{...json...}
    PROGRESS:<0-100>
    QUOTA:{...json...}
    OVERLOAD:{...json...}
    RESULT:{...json...}

All logging goes to stderr and is merged by the Rust host into Sonarpad's main log. The worker performs Pyannote ONNX
speech segmentation and Gemini description generation. It never synthesizes,
mixes, ducks or exports audio; those operations belong to Sonarpad's Rust TTS
and FFmpeg DLL backends.
"""
from __future__ import annotations

import argparse
import json
import os
import re
import sys
import traceback
from pathlib import Path

RUNTIME_DIR = Path(__file__).resolve().parent / "audio_description_runtime"
if str(RUNTIME_DIR) not in sys.path:
    sys.path.insert(0, str(RUNTIME_DIR))

from audio_describer.core import audio_describer, speech_detector  # noqa: E402
from audio_describer.core import gemini_helpers  # noqa: E402
from audio_describer.models import config_model  # noqa: E402
from audio_describer.utils.logger import app_logger  # noqa: E402

CHUNK_DURATION_SECONDS = 180
GEMINI_FRAME_RATE_FOR_AI = 0


def _emit(prefix: str, value) -> None:
    if isinstance(value, (dict, list)):
        text = json.dumps(value, ensure_ascii=True, separators=(",", ":"))
    else:
        text = str(value)
    print(f"{prefix}:{text}", flush=True)


def _status(stage: str, message: str, progress: int | None = None) -> None:
    _emit("STATUS", {"stage": stage, "message": message})
    if progress is not None:
        _emit("PROGRESS", max(0, min(100, int(progress))))


def _read_request(path: str) -> dict:
    with open(path, "r", encoding="utf-8") as handle:
        value = json.load(handle)
    if not isinstance(value, dict):
        raise ValueError("The request root must be a JSON object.")
    return value




def _quota_decision_handler(current_model: str, exc: BaseException):
    """Ask the Rust host what to do after a real quota-exhausted response.

    The worker remains alive and blocked on the current Gemini request. The
    host replies with one JSON line on stdin:
      {"action":"switch","model":"gemini-..."}
      {"action":"wait"}
      {"action":"stop"}
    """
    _emit(
        "QUOTA",
        {
            "model": str(current_model or ""),
            "error": str(exc),
        },
    )
    while True:
        line = sys.stdin.readline()
        if not line:
            return False
        try:
            reply = json.loads(line)
        except json.JSONDecodeError:
            continue
        if not isinstance(reply, dict):
            continue
        action = str(reply.get("action") or "").strip().lower()
        if action == "stop":
            return False
        if action == "wait":
            return None
        if action == "switch":
            model = str(reply.get("model") or "").strip()
            if model:
                return model

def _overload_decision_handler(current_model: str, exc: BaseException) -> bool:
    """Ask the Rust host whether to keep retrying after repeated high-demand 503s."""
    _emit(
        "OVERLOAD",
        {
            "model": str(current_model or ""),
            "error": str(exc),
        },
    )
    while True:
        line = sys.stdin.readline()
        if not line:
            return False
        try:
            reply = json.loads(line)
        except json.JSONDecodeError:
            continue
        if not isinstance(reply, dict):
            continue
        action = str(reply.get("action") or "").strip().lower()
        if action == "stop":
            return False
        if action == "wait":
            return True


def _validate_request(request: dict) -> None:
    input_path = Path(str(request.get("input_path") or ""))
    if not input_path.is_file():
        raise FileNotFoundError(f"Input media file not found: {input_path}")
    duration = float(request.get("duration_sec") or 0.0)
    if duration <= 0.0:
        raise ValueError("Sonarpad did not provide a valid media duration.")
    chunks = request.get("chunks")
    if not isinstance(chunks, list) or not chunks:
        raise ValueError("Sonarpad did not provide prepared Gemini chunks.")
    previous_end = 0.0
    for index, chunk in enumerate(chunks, 1):
        if not isinstance(chunk, dict):
            raise ValueError(f"Invalid prepared chunk {index}.")
        chunk_path = Path(str(chunk.get("path") or ""))
        if not chunk_path.is_file():
            raise FileNotFoundError(f"Prepared Gemini chunk not found: {chunk_path}")
        start = float(chunk.get("start_sec") or 0.0)
        end = float(chunk.get("end_sec") or 0.0)
        if start < 0.0 or end <= start or abs(start - previous_end) > 2.0:
            raise ValueError(f"Invalid prepared chunk timeline at chunk {index}.")
        previous_end = end
    wav_value = str(request.get("audio_wav_path") or "").strip()
    if wav_value and not Path(wav_value).is_file():
        raise FileNotFoundError(f"Prepared Pyannote WAV not found: {wav_value}")
    api_key = str(request.get("gemini_api_key") or "").strip()
    service_url = str(request.get("sonarpad_ai_service_url") or "").strip()
    service_code = str(request.get("sonarpad_ai_access_code") or "").strip()
    service_device = str(request.get("sonarpad_ai_device_id") or "").strip()
    mode = str(request.get("ai_access_mode") or ("sonarpad" if service_url else "personal")).strip().lower()
    if mode not in {"personal", "sonarpad"}:
        raise ValueError("Unsupported AI access mode.")
    if mode == "sonarpad":
        if api_key:
            raise ValueError("Sonarpad AI mode must not receive a personal Gemini API key.")
        if not service_url.startswith("https://"):
            raise ValueError("Sonarpad AI service URL must use HTTPS.")
        if not service_code.startswith("sp_"):
            raise ValueError("Sonarpad AI access code is not configured.")
        if not service_device:
            raise ValueError("Sonarpad AI device identifier is not configured.")
    else:
        if service_url or service_code or service_device:
            raise ValueError("Personal Gemini mode must not receive Sonarpad AI service credentials.")
        if not api_key:
            raise ValueError("Gemini API key is not configured in Sonarpad.")
    verbosity = str(request.get("verbosity") or "detailed")
    if verbosity not in {"short", "standard", "detailed"}:
        raise ValueError(f"Unsupported verbosity: {verbosity}")
    initial_glossary = request.get("initial_character_glossary", [])
    if initial_glossary is not None and not isinstance(initial_glossary, list):
        raise ValueError("initial_character_glossary must be a list.")
    resume = request.get("resume")
    if resume is not None:
        if not isinstance(resume, dict):
            raise ValueError("resume must be an object.")
        completed_chunks = int(resume.get("completed_chunks") or 0)
        if completed_chunks < 0 or completed_chunks > len(chunks):
            raise ValueError("resume completed_chunks is outside the prepared chunk range.")
        if not isinstance(resume.get("descriptions", []), list):
            raise ValueError("resume descriptions must be a list.")
        if not isinstance(resume.get("character_glossary", []), list):
            raise ValueError("resume character_glossary must be a list.")


def _normalise_initial_character_glossary(value) -> list[dict]:
    if not isinstance(value, list):
        return []
    merged: dict[str, dict] = {}
    for item in value:
        if not isinstance(item, dict):
            continue
        identifier = " ".join(str(item.get("id") or "").split()).strip()
        name = " ".join(str(item.get("name") or "").split()).strip()
        description = " ".join(str(item.get("description") or "").split()).strip()
        if not name or not description:
            continue
        key = f"id:{identifier.casefold()}" if identifier else f"name:{name.casefold()}"
        candidate = {
            "id": identifier[:100],
            "name": name[:100],
            # Character catalogs are persistent identity data. Do not truncate
            # their descriptions to the short context limit used elsewhere.
            "description": description,
        }
        existing = merged.get(key)
        if existing is None:
            merged[key] = candidate
        elif len(candidate["description"]) > len(existing["description"]):
            if not candidate["id"]:
                candidate["id"] = existing["id"]
            merged[key] = candidate
    return list(merged.values())[:96]


def _configure_omni(request: dict) -> None:
    language = str(request.get("language") or "it").strip() or "it"
    model = str(request.get("gemini_model") or "gemini-3.5-flash-lite").strip()
    config_model.configure(
        {
            "user_gemini_api_key": str(request.get("gemini_api_key") or ""),
            "sonarpad_ai_service_url": str(request.get("sonarpad_ai_service_url") or "").strip(),
            "sonarpad_ai_access_code": str(request.get("sonarpad_ai_access_code") or "").strip(),
            "sonarpad_ai_device_id": str(request.get("sonarpad_ai_device_id") or "").strip(),
            "gemini_description_verbosity": str(request.get("verbosity") or "detailed"),
            "gemini_model_override": model,
            "gemini_disable_safety_block_none": True,
            "gemini_temperature": 0.3,
            "application_language": language,
            "send_silenced_video_to_ai": False,
            "enable_dialogue_protection": True,
            "description_coverage_mode": "intensive",
            "intensive_min_silence_seconds": 3.0,
            "enable_extended_audio_description": bool(
                request.get("allow_extended_pauses", True)
            ),
            "frame_rate_for_ai": GEMINI_FRAME_RATE_FOR_AI,
            "enable_video_chunking": True,
            "video_chunk_duration_seconds": CHUNK_DURATION_SECONDS,
            "enable_character_glossary": bool(
                request.get("recognize_characters", True)
            ),
            "recognize_screen_text": bool(request.get("recognize_screen_text", False)),
            "verify_chunk_timing_with_gemini": True,
        }
    )
    audio_describer.reset_gemini_client()
    if str(request.get("sonarpad_ai_service_url") or "").strip():
        # The service fixes the model and cost tier server-side. A quota/rate-limit
        # event must never invite the user to switch model; let the existing retry
        # policy retry the same request or preserve the checkpoint on final failure.
        gemini_helpers.set_quota_decision_handler(lambda _model, _exc: None)
    else:
        gemini_helpers.set_quota_decision_handler(_quota_decision_handler)
    gemini_helpers.set_overload_decision_handler(_overload_decision_handler)


_CHUNK_RE = re.compile(r"(?:chunk|segment)\D*(\d+)\s*(?:of|/)\s*(\d+)", re.I)


def _gemini_status(message: str) -> None:
    """Convert Omni's detailed English log text into stable UI status IDs."""
    message = str(message)
    match = _CHUNK_RE.search(message)
    if match:
        current = max(1, int(match.group(1)))
        total = max(current, int(match.group(2)))
        progress = 30 + round(50 * current / total)
        _status(
            "gemini_chunk",
            json.dumps({"current": current, "total": total}, separators=(",", ":")),
            progress,
        )
        return
    lowered = message.casefold()
    if "wrong language" in lowered or "language correction" in lowered:
        _status("language_correction", "")
    elif "upload" in lowered or "sending video inline" in lowered:
        _status("gemini_uploading", "")
    elif (
        "waiting for processing" in lowered
        or "waiting for gemini" in lowered
        or "still working" in lowered
        or "watching scenes" in lowered
        or "drafting descriptions" in lowered
        or "writing json" in lowered
    ):
        _status("gemini_waiting", "")
    elif "repair" in lowered or "reformat" in lowered or "invalid json" in lowered:
        _status("gemini_repair", "")
    elif "contacting gemini" in lowered or "asking gemini" in lowered:
        _status("gemini_contacting", "")
    elif (
        "response received" in lowered
        or "reading gemini response" in lowered
        or "parsing ai output" in lowered
        or "parsed" in lowered
    ):
        _status("gemini_response", "")
    elif "retry" in lowered or "attempt" in lowered or "temporary" in lowered:
        _status("gemini_retry", "")
    else:
        _status("gemini_processing", "")


def _normalise_descriptions(
    descriptions, mandatory_slots=None, source_descriptions=None
) -> list[dict]:
    result: list[dict] = []
    mandatory_slots = list(mandatory_slots or [])
    source_rows = []
    for source_index, item in enumerate(source_descriptions or []):
        if not isinstance(item, (list, tuple)) or len(item) < 3:
            continue
        try:
            source_start = max(0.0, float(item[0]))
            source_end = max(source_start, float(item[1]))
        except (TypeError, ValueError):
            continue
        source_text = str(item[2] or "").strip()
        if source_text:
            source_rows.append((source_start, source_end, source_text, source_index))
    claimed_sources = set()

    for item in descriptions or []:
        if not isinstance(item, (list, tuple)) or len(item) < 3:
            continue
        try:
            start = max(0.0, float(item[0]))
            end = max(start, float(item[1]))
        except (TypeError, ValueError):
            continue
        text = str(item[2] or "").strip()
        if not text:
            continue

        visual_start = start
        matching_sources = [
            row for row in source_rows
            if row[3] not in claimed_sources and row[2] == text
        ]
        if matching_sources:
            source_start, source_end, _source_text, source_index = min(
                matching_sources,
                key=lambda row: (
                    abs(row[0] - start),
                    abs(row[1] - end),
                    row[3],
                ),
            )
            claimed_sources.add(source_index)
            visual_start = source_start

        row = {
            "start_sec": start,
            "end_sec": end,
            "visual_start_sec": visual_start,
            "text": text,
        }
        visual_evidence = audio_describer.get_visual_evidence_time(visual_start, text)
        if visual_evidence is not None:
            row["visual_evidence_time_sec"] = visual_evidence
        result.append(row)

    # Mark exactly one representative description per mandatory slot. Extra
    # descriptions in the same silence remain optional so downstream exact-TTS
    # scheduling can distinguish the required cue from additional narration.
    claimed_rows = set()
    for slot in mandatory_slots:
        slot_start = float(slot["start"])
        slot_end = float(slot["end"])
        slot_center = (slot_start + slot_end) / 2.0
        candidates = []
        for index, row in enumerate(result):
            if index in claimed_rows:
                continue
            midpoint = (row["start_sec"] + row["end_sec"]) / 2.0
            if slot_start <= midpoint <= slot_end:
                word_count = len(re.findall(r"\w+", row["text"], flags=re.UNICODE))
                candidates.append((word_count, abs(midpoint - slot_center), index))
        if not candidates:
            continue
        _words, _distance, index = min(candidates)
        claimed_rows.add(index)
        result[index].update({
            "mandatory": True,
            "slot_id": str(slot.get("id") or ""),
            "slot_start_sec": slot_start,
            "slot_end_sec": slot_end,
        })

    result.sort(key=lambda row: (row["start_sec"], row["end_sec"]))
    return result


def run(request: dict) -> dict:
    _validate_request(request)
    _configure_omni(request)
    input_path = str(Path(request["input_path"]).resolve())
    duration = float(request["duration_sec"])
    prepared_chunks = [
        {
            "path": str(Path(chunk["path"]).resolve()),
            "start_sec": float(chunk["start_sec"]),
            "end_sec": float(chunk["end_sec"]),
        }
        for chunk in request["chunks"]
    ]

    wav_path = str(request.get("audio_wav_path") or "").strip()
    if wav_path:
        _status("pyannote_analyzing", "", 10)
        dialogue_intervals = speech_detector.detect_dialogue_intervals_from_wav(
            wav_path,
            duration,
            status_callback=lambda _text: _status("pyannote_analyzing", ""),
        )
    else:
        _status("pyannote_no_audio", "", 10)
        dialogue_intervals = []
    _status(
        "pyannote_done",
        str(len(dialogue_intervals)),
        25,
    )

    free_windows_text = speech_detector.format_intervals_for_prompt(
        dialogue_intervals, duration
    )
    _status("gemini_start", "", 30)
    initial_glossary = _normalise_initial_character_glossary(
        request.get("initial_character_glossary", [])
    )
    resume = request.get("resume") or {}

    def _checkpoint(
        completed_chunks, total_chunks, descriptions_so_far, character_glossary, gemini_model
    ):
        rows = []
        for item in descriptions_so_far:
            if not isinstance(item, (list, tuple)) or len(item) < 3:
                continue
            rows.append(
                {
                    "start_sec": float(item[0]),
                    "end_sec": float(item[1]),
                    "text": str(item[2] or ""),
                }
            )
        _emit(
            "CHECKPOINT",
            {
                "completed_chunks": int(completed_chunks),
                "total_chunks": int(total_chunks),
                "descriptions": rows,
                "character_glossary": character_glossary or [],
                "gemini_model": str(gemini_model or ""),
            },
        )

    descriptions, glossary, token_usage = audio_describer.generate_descriptions_chunked(
        input_path,
        CHUNK_DURATION_SECONDS,
        "",
        _gemini_status,
        dialogue_free_windows=free_windows_text,
        dialogue_intervals=dialogue_intervals,
        prepared_chunks=prepared_chunks,
        total_duration_override=duration,
        initial_character_glossary=initial_glossary,
        resume_completed_chunks=int(resume.get("completed_chunks") or 0),
        resume_descriptions=resume.get("descriptions") or [],
        resume_character_glossary=resume.get("character_glossary") or [],
        checkpoint_callback=_checkpoint,
    )
    if descriptions is None:
        raise RuntimeError("Gemini did not return an audio-description result.")

    _status("finalize", "", 85)
    descriptions = audio_describer._remove_consecutive_duplicates(  # noqa: SLF001
        descriptions, _gemini_status
    )
    try:
        intensive_min_silence = float(
            config_model.get_setting("intensive_min_silence_seconds") or 3.0
        )
    except (TypeError, ValueError):
        intensive_min_silence = 3.0
    mandatory_slots = []
    for index, chunk in enumerate(prepared_chunks, 1):
        mandatory_slots.extend(
            speech_detector.intensive_description_slots(
                dialogue_intervals,
                duration,
                intensive_min_silence,
                float(chunk["start_sec"]),
                float(chunk["end_sec"]),
                id_suffix=f"C{index:04d}",
            )
        )

    if bool(request.get("allow_extended_pauses", True)):
        aligned, dropped, short_gap_candidates = (
            speech_detector.align_descriptions_with_extended_pauses_prioritizing_slots(
                descriptions, dialogue_intervals, duration, mandatory_slots
            )
        )
    else:
        aligned, dropped = speech_detector.align_descriptions_prioritizing_slots(
            descriptions, dialogue_intervals, duration, mandatory_slots
        )
        short_gap_candidates = 0

    final_missing_slots = speech_detector.uncovered_intensive_slots(
        aligned, mandatory_slots
    )
    app_logger.info(
        "Final post-alignment intensive coverage audit: slots=%d covered=%d missing=%d ids=%s.",
        len(mandatory_slots),
        len(mandatory_slots) - len(final_missing_slots),
        len(final_missing_slots),
        ",".join(slot["id"] for slot in final_missing_slots) or "none",
    )
    normalized = _normalise_descriptions(aligned, mandatory_slots, descriptions)
    model = str(request.get("gemini_model") or "gemini-3.5-flash-lite").strip()
    protected = [
        {"start_sec": float(start), "end_sec": float(end)}
        for start, end in dialogue_intervals
    ]
    _status("ready_for_tts", "", 100)
    return {
        "ok": True,
        "schema_version": 2,
        "input_path": input_path,
        "duration_sec": duration,
        "chunk_duration_sec": CHUNK_DURATION_SECONDS,
        "analysis_engine": "pyannote-segmentation-onnx",
        "description_mode": "intensive",
        "safety_block_none": True,
        "allow_extended_pauses": bool(request.get("allow_extended_pauses", True)),
        "recognize_characters": bool(request.get("recognize_characters", True)),
        "protected_intervals": protected,
        "descriptions": normalized,
        "dropped_before_tts": int(dropped),
        "short_gap_candidates": int(short_gap_candidates),
        "character_glossary": glossary or [],
        "token_usage": token_usage or [],
        "gemini_model": str(
            config_model.get_setting("gemini_model_override") or model
        ),
        "log_path": "",
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--request", required=False)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        model = speech_detector.get_bundled_model_path()
        result = {
            "ok": bool(model and os.path.isfile(model)),
            "model_path": model,
            "chunk_duration_sec": CHUNK_DURATION_SECONDS,
            "exports_audio": False,
            "uses_external_ffmpeg": False,
            "expects_host_prepared_media": True,
            "contains_tts_or_playback": False,
            "interactive_quota_decisions": True,
            "interactive_overload_decisions": True,
            "optional_character_glossary": True,
            "persistent_character_catalog_seed": True,
        }
        _emit("RESULT", result)
        return 0 if result["ok"] else 1

    if not args.request:
        _emit("RESULT", {"ok": False, "error": "Missing --request argument."})
        return 2

    try:
        request = _read_request(args.request)
        app_logger.info("Starting new audio-description job; logging is merged into Sonarpad log.txt.")
        result = run(request)
        _emit("RESULT", result)
        return 0
    except KeyboardInterrupt:
        _emit("RESULT", {"ok": False, "cancelled": True, "error": "Cancelled."})
        return 130
    except gemini_helpers.GeminiRetryCancelledError as exc:
        _emit("RESULT", {"ok": False, "cancelled": True, "error": str(exc)})
        return 130
    except Exception as exc:  # process boundary: return structured diagnostics
        app_logger.error("Audio-description worker failed: %s", exc, exc_info=True)
        _emit(
            "RESULT",
            {
                "ok": False,
                "error": str(exc),
                "error_type": type(exc).__name__,
                "log_path": "",
                "traceback": traceback.format_exc(limit=12),
            },
        )
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
