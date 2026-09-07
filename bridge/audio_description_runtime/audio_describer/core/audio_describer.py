# audio_describer/core/audio_describer.py
from ..i18n_setup import _
import os
import re
import time
import json
import math
import tempfile
from concurrent.futures import ThreadPoolExecutor

from .. import config
from ..utils.logger import app_logger
from ..models import config_model
from . import speech_detector, language_detector
from . import gemini_helpers as gemini
from .gemini_helpers import ContentBlockedError, GeminiAPIError, TokenLimitError

# --- CONSTANTS ---
GLOSSARY_MAX_DURATION_SEC = 3000  # 50 minutes, typical limit for raw video processing
_UPLOAD_POLL_MAX_ATTEMPTS = 100
_UPLOAD_POLL_INTERVAL_SEC = 5
# Prefer inline video for smaller files (avoids Files API processing failures).
# Gemini allows ~100MB inline; keep headroom for prompt/base64 overhead in the request.
_INLINE_VIDEO_MAX_BYTES = 48 * 1024 * 1024
# Still try inline as a fallback after Files API FAILED or a permission-denied
# generateContent call that references an uploaded file.
_INLINE_FALLBACK_MAX_BYTES = 50 * 1024 * 1024
# Gemini Files API rejects media at or above 2 GiB. Large movies are extracted
# and uploaded one analysis chunk at a time instead of uploading the whole file.
_FILES_API_MEDIA_MAX_BYTES = 2 * 1024 * 1024 * 1024
# How many times to ask Gemini to re-emit valid JSON after a parse failure.
_JSON_REPAIR_MAX_ATTEMPTS = 1
# Cap size of broken fragment sent back for repair (token safety).
_JSON_REPAIR_FRAGMENT_MAX_CHARS = 40000
# A STOP response can still abandon the tail of a chunk without being invalid.
# In high-accuracy mode, explicitly ask Gemini to cover gaps this large.
_CHUNK_COVERAGE_GAP_SEC = 90.0
# Gemini timestamp accuracy can drift badly near the tail of a long video
# slice, so chunked generation never analyzes more than three minutes at once.
_MAX_CHUNK_DURATION_SEC = 180.0
_BLOCKED_CHUNK_FALLBACK_DURATION_SEC = 60.0
# Ignore sub-half-second floating-point tails when a blocked chunk is split into
# one-minute Gemini fallback ranges. Such tails cannot contain a useful speech-free
# window and previously produced malformed videoMetadata offsets (for example
# 180.06900000000041s).
_BLOCKED_CHUNK_FALLBACK_MIN_DURATION_SEC = 0.5
_REPEATED_SUBJECT_NAME_MAX_GAP_SEC = 20.0
_LANGUAGE_DETECTION_MIN_CONFIDENCE = 0.75
_AUDIO_CONTEXT_ONLY_RULE = (
    "The soundtrack and dialogue are context only. You may use spoken names, titles, "
    "relationships, and other dialogue privately to identify or disambiguate visible "
    "characters and understand the scene. Never put spoken content itself into "
    "`description_text`: do not transcribe, quote, translate, paraphrase, summarize, "
    "complete, echo, answer, or restate any spoken line, even partially. A spoken name "
    "or title may be used only as an identity label when it is confidently linked to the "
    "visible person. Do not narrate facts learned only from speech as though they were "
    "visible. Never describe that someone talks, asks, answers, shouts, whispers, or says "
    "something; describe only new visual information. "
)
# Diagnostic temporal grounding returned by Gemini.  This metadata never
# changes scheduling; it is retained so Sonarpad can compare the requested
# description timestamp with the exact video instant Gemini claims as visual
# evidence for the sentence.
_LAST_PARSED_VISUAL_EVIDENCE_RAW = []
_VISUAL_EVIDENCE_RECORDS = []


def _reset_visual_evidence_registry():
    _LAST_PARSED_VISUAL_EVIDENCE_RAW.clear()
    _VISUAL_EVIDENCE_RECORDS.clear()


def _parse_visual_evidence_seconds(value):
    if value is None or isinstance(value, bool):
        return None
    try:
        seconds = float(value)
    except (TypeError, ValueError):
        try:
            seconds = _mmss_to_total_seconds(str(value))
        except (TypeError, ValueError):
            return None
    if not math.isfinite(seconds) or seconds < 0:
        return None
    return seconds


def _capture_visual_evidence_item(item):
    if not isinstance(item, dict):
        return
    start = item.get("start_time_mmss")
    end = item.get("end_time_mmss")
    text = str(item.get("description_text") or "").strip()
    evidence = _parse_visual_evidence_seconds(
        item.get("visual_evidence_time_seconds")
    )
    if start is None or end is None or not text or evidence is None:
        return
    try:
        start_sec = _mmss_to_total_seconds(str(start))
        end_sec = _mmss_to_total_seconds(str(end))
    except ValueError:
        return
    _LAST_PARSED_VISUAL_EVIDENCE_RAW.append(
        (start_sec, end_sec, text, evidence)
    )


def _register_visual_evidence_records(records, timeline_offset_sec=0.0):
    offset = float(timeline_offset_sec or 0.0)
    for start, end, text, evidence in records or []:
        absolute_start = float(start) + offset
        absolute_end = float(end) + offset
        absolute_evidence = float(evidence) + offset
        _VISUAL_EVIDENCE_RECORDS.append(
            (absolute_start, absolute_end, str(text or "").strip(), absolute_evidence)
        )
        app_logger.info(
            "Gemini visual evidence: description=%.3f-%.3f evidence=%.3f delta=%+.3fs text=%r",
            absolute_start, absolute_end, absolute_evidence,
            absolute_evidence - absolute_start, str(text or "").strip()[:120],
        )


def _retime_visual_evidence_records(records, descriptions, timeline_offset_sec=0.0):
    """Keep evidence metadata aligned after contextual timestamp repair."""
    offset = float(timeline_offset_sec or 0.0)
    remaining = list(descriptions or [])
    retimed = []
    for raw_start, raw_end, text, evidence in records or []:
        normalized_text = " ".join(str(text or "").split()).casefold()
        match_index = next(
            (
                index for index, item in enumerate(remaining)
                if " ".join(str(item[2] or "").split()).casefold() == normalized_text
            ),
            None,
        )
        if match_index is None:
            retimed.append((raw_start, raw_end, text, evidence))
            continue
        start, end, corrected_text = remaining.pop(match_index)
        retimed.append(
            (float(start) - offset, float(end) - offset, corrected_text, evidence)
        )
    return retimed


def get_visual_evidence_time(start_sec, text=""):
    if not _VISUAL_EVIDENCE_RECORDS:
        return None
    start = float(start_sec)
    normalized_text = " ".join(str(text or "").split()).casefold()
    candidates = sorted(
        _VISUAL_EVIDENCE_RECORDS,
        key=lambda row: (
            0 if normalized_text and " ".join(row[2].split()).casefold() == normalized_text else 1,
            abs(row[0] - start),
        ),
    )
    if not candidates:
        return None
    best = candidates[0]
    if abs(best[0] - start) > 0.35:
        return None
    return float(best[3])


_LANGUAGE_NAMES = {
    "ar": "Arabic",
    "cs": "Czech",
    "de": "German",
    "en": "English",
    "es": "Spanish",
    "fr": "French",
    "hi": "Hindi",
    "it": "Italian",
    "lt": "Lithuanian",
    "pl": "Polish",
    "pt": "Portuguese",
    "pt-br": "Brazilian Portuguese",
    "ru": "Russian",
    "sr": "Serbian",
    "sv": "Swedish",
    "tr": "Turkish",
    "uk": "Ukrainian",
    "vi": "Vietnamese",
    "zh": "Chinese",
}

_LANGUAGE_EXAMPLES = {
    "cs": ("Po ulici se řítí auto.", "Vysoký muž v tmavém obleku."),
    "de": ("Ein Auto rast die Straße entlang.", "Ein großer Mann in einem dunklen Anzug."),
    "en": ("A car speeds down the street.", "A tall man in a dark suit."),
    "es": ("Un coche avanza a toda velocidad por la calle.", "Un hombre alto con traje oscuro."),
    "fr": ("Une voiture file dans la rue.", "Un homme grand en costume sombre."),
    "hi": ("एक कार सड़क पर तेज़ी से दौड़ती है।", "गहरे सूट में एक लंबा आदमी।"),
    "it": ("Un'auto sfreccia lungo la strada.", "Un uomo alto con un abito scuro."),
    "lt": ("Automobilis lekia gatve.", "Aukštas vyras su tamsiu kostiumu."),
    "pl": ("Samochód pędzi ulicą.", "Wysoki mężczyzna w ciemnym garniturze."),
    "pt": ("Um carro avança em alta velocidade pela rua.", "Um homem alto de fato escuro."),
    "pt-br": ("Um carro corre pela rua.", "Um homem alto de terno escuro."),
    "ru": ("Машина мчится по улице.", "Высокий мужчина в тёмном костюме."),
    "sr": ("Аутомобил јури улицом.", "Висок мушкарац у тамном оделу."),
    "sv": ("En bil rusar längs gatan.", "En lång man i mörk kostym."),
    "uk": ("Автомобіль мчить вулицею.", "Високий чоловік у темному костюмі."),
    "vi": ("Một chiếc ô tô lao nhanh trên phố.", "Một người đàn ông cao mặc bộ vest tối màu."),
    "zh": ("一辆汽车沿街疾驰。", "一名身穿深色西装的高个男子。"),
}


def _target_language_details():
    raw = str(config_model.get_setting("application_language") or "en")
    canonical = raw.strip().lower().replace("_", "-") or "en"
    prompt_code = canonical if canonical in _LANGUAGE_NAMES else canonical.split("-", 1)[0]
    detection_code = language_detector.normalize_language_code(canonical) or "en"
    name = _LANGUAGE_NAMES.get(prompt_code, _LANGUAGE_NAMES.get(detection_code, "English"))
    examples = _LANGUAGE_EXAMPLES.get(prompt_code, _LANGUAGE_EXAMPLES.get(detection_code, _LANGUAGE_EXAMPLES["en"]))
    return detection_code, name, examples


def _is_permission_denied_error(exc):
    """Return True for Gemini HTTP 403 / PERMISSION_DENIED errors."""
    seen = set()
    current = exc
    while current is not None and id(current) not in seen:
        seen.add(id(current))
        code = getattr(current, "code", None)
        status_code = getattr(current, "status_code", None)
        response_code = getattr(getattr(current, "response", None), "status_code", None)
        for candidate in (code, status_code, response_code):
            try:
                if int(candidate) == 403:
                    return True
            except (TypeError, ValueError):
                pass
        text = str(current).upper()
        if "403" in text and "PERMISSION_DENIED" in text:
            return True
        current = current.__cause__ or current.__context__
    return False


class _TransientGeminiFileProcessingError(GeminiAPIError):
    """Gemini accepted an upload but transiently failed to process it."""

    def __init__(self, message, video_file_obj):
        super().__init__(message)
        self.video_file_obj = video_file_obj

# --- PUBLIC API ---

def reset_gemini_client():
    """Proxy function to reset the Gemini client in the helper module."""
    gemini.reset_gemini_client()


def _file_state_name(file_obj):
    """Return the processing state name for a Gemini File object (e.g. ACTIVE, FAILED)."""
    state = getattr(file_obj, "state", None)
    if state is None:
        return "UNKNOWN"
    return getattr(state, "name", str(state))


def _file_error_detail(file_obj):
    """Extract a human-readable server error string from a Gemini File object, if any."""
    err = getattr(file_obj, "error", None)
    if not err:
        return ""
    code = getattr(err, "code", None)
    message = getattr(err, "message", None)
    if message is None and isinstance(err, dict):
        code = err.get("code", code)
        message = err.get("message", "")
    if code is None and message is None:
        return f" {err!r}"
    return f" Server error: Code={code!s}, Message='{message or 'N/A'}'"


def _file_error_code(file_obj):
    err = getattr(file_obj, "error", None)
    if not err:
        return None
    if isinstance(err, dict):
        return err.get("code")
    return getattr(err, "code", None)


_MIME_BY_EXT = {
    ".mp4": "video/mp4",
    ".webm": "video/webm",
    ".mov": "video/quicktime",
    ".avi": "video/x-msvideo",
    ".mkv": "video/x-matroska",
    ".mpeg": "video/mpeg",
    ".mpg": "video/mpeg",
    ".flv": "video/x-flv",
    ".wmv": "video/x-ms-wmv",
    ".3gp": "video/3gpp",
    ".m4v": "video/mp4",
}


def _guess_video_mime_type(path):
    ext = os.path.splitext(path)[1].lower()
    return _MIME_BY_EXT.get(ext, "video/mp4")


def _requires_per_chunk_upload(video_path):
    return os.path.getsize(video_path) >= _FILES_API_MEDIA_MAX_BYTES


def _should_use_per_chunk_uploads(video_path, num_chunks):
    """Use physical clips for long videos and as a Files API safety fallback."""
    return int(num_chunks) > 1 or _requires_per_chunk_upload(video_path)


def _extract_upload_chunk(*_args, **_kwargs):
    raise GeminiAPIError(_(
        "Gemini chunks must be prepared by Sonarpad's Rust FFmpeg backend."
    ))


def _upload_and_wait_for_active_once(client, video_path, status_callback=None):
    """Uploads a video to Gemini Files API and polls until it becomes ACTIVE.

    Returns the active file object. Raises GeminiAPIError on timeout or failure.
    Aborts immediately if the server reports FAILED (does not keep polling).
    """
    def _status(msg):
        if status_callback:
            status_callback(msg)
        app_logger.info(f"Upload: {msg}")

    _status(_("Uploading video '%s'...") % os.path.basename(video_path))
    # Work around httpx ASCII header encoding error when the file path
    # contains non-ASCII characters (e.g. Windows username with ö, ü, etc.)
    upload_path = video_path
    temp_copy = None
    try:
        os.fspath(video_path).encode("ascii")
    except (UnicodeEncodeError, UnicodeDecodeError):
        import tempfile
        import shutil
        suffix = os.path.splitext(video_path)[1]
        temp_copy = tempfile.NamedTemporaryFile(
            suffix=suffix, prefix="omni_upload_", delete=False
        )
        temp_copy.close()
        shutil.copy2(video_path, temp_copy.name)
        upload_path = temp_copy.name
        app_logger.info("Upload: copied to ASCII-safe temp path for upload")
    mime_type = _guess_video_mime_type(upload_path)
    try:
        app_logger.info(f"Upload: posting file with mime_type={mime_type}")

        def _do_upload():
            try:
                return client.files.upload(
                    file=upload_path,
                    config={"mime_type": mime_type},
                )
            except TypeError:
                # Older SDK signatures may not accept config=
                return client.files.upload(file=upload_path)

        # Retry upload on transient network errors (e.g. WinError 10060).
        video_file_obj = gemini.run_with_retry(
            _do_upload,
            status_callback=status_callback,
            operation_label=_("video upload"),
        )
    finally:
        if temp_copy is not None:
            try:
                os.unlink(temp_copy.name)
            except OSError:
                pass
    _status(_("Video upload initiated: %s. Waiting for processing...") % video_file_obj.name)
    app_logger.info(
        "Upload: file name=%s mime=%s uri=%s initial_state=%s",
        getattr(video_file_obj, "name", None),
        getattr(video_file_obj, "mime_type", None),
        getattr(video_file_obj, "uri", None),
        _file_state_name(video_file_obj),
    )

    for attempt in range(1, _UPLOAD_POLL_MAX_ATTEMPTS + 1):
        state_name = _file_state_name(video_file_obj)
        if state_name == "ACTIVE":
            _status(_("Video is ready (ACTIVE)."))
            return video_file_obj
        if state_name == "FAILED":
            err_msg = _("Video processing failed on Gemini's servers. Final state: %s") % state_name
            err_msg += _file_error_detail(video_file_obj)
            app_logger.error(
                "Upload: Gemini file processing FAILED for %s.%s Full object: %r",
                getattr(video_file_obj, "name", "?"),
                _file_error_detail(video_file_obj),
                video_file_obj,
            )
            _status(err_msg)
            if str(_file_error_code(video_file_obj)) == "13":
                raise _TransientGeminiFileProcessingError(
                    err_msg, video_file_obj
                )
            raise GeminiAPIError(err_msg)
        if state_name not in ("PROCESSING", "STATE_UNSPECIFIED", "UNKNOWN"):
            # Unexpected terminal state — do not spin for minutes.
            err_msg = _("Video processing failed. Unexpected state: %s") % state_name
            err_msg += _file_error_detail(video_file_obj)
            _status(err_msg)
            raise GeminiAPIError(err_msg)

        # Friendly wait message (not "attempt/retry" — this is normal server-side processing).
        elapsed = attempt * _UPLOAD_POLL_INTERVAL_SEC
        _status(
            _("Waiting for Gemini to finish processing the video... %d s elapsed") % elapsed
        )
        time.sleep(_UPLOAD_POLL_INTERVAL_SEC)
        # Poll status with retry — a single WinError 10060 must not abort the whole job.
        file_name = video_file_obj.name
        video_file_obj = gemini.run_with_retry(
            lambda name=file_name: client.files.get(name=name),
            status_callback=status_callback,
            operation_label=_("upload status check"),
        )


    state_name = _file_state_name(video_file_obj)
    err_msg = _("Video processing timed out or failed. Final state: %s") % state_name
    err_msg += _file_error_detail(video_file_obj)
    _status(err_msg)
    raise GeminiAPIError(err_msg)


def _upload_and_wait_for_active(client, video_path, status_callback=None):
    """Upload until ACTIVE, retrying Gemini's transient Code=13 forever."""
    retry_number = 0
    while True:
        try:
            return _upload_and_wait_for_active_once(
                client, video_path, status_callback
            )
        except _TransientGeminiFileProcessingError as exc:
            retry_number += 1
            _cleanup_uploaded_file(
                client, exc.video_file_obj, status_callback
            )
            message = _(
                "Gemini could not process this video chunk (server Code 13). "
                "Retrying upload indefinitely; retry %d…"
            ) % retry_number
            app_logger.warning(
                "Transient Gemini file-processing failure for %s; "
                "retrying indefinitely (retry=%d).",
                os.path.basename(video_path), retry_number,
            )
            if status_callback:
                status_callback(message)
            time.sleep(_UPLOAD_POLL_INTERVAL_SEC)


def _format_gemini_duration_seconds(value):
    """Serialize a non-negative second offset with at most nanosecond precision."""
    seconds = float(value)
    if not math.isfinite(seconds) or seconds < 0.0:
        raise GeminiAPIError(_("Gemini video time offset is invalid."))

    # Protobuf Duration accepts at most nine fractional digits. Formatting the
    # binary float first also removes artifacts such as 180.06900000000041.
    text = f"{seconds:.9f}".rstrip("0").rstrip(".")
    if "." not in text:
        text += ".0"
    return f"{text}s"


def _video_metadata_kwargs(start_offset_sec=None, end_offset_sec=None):
    """Build kwargs for types.VideoMetadata from settings / chunk offsets."""
    metadata_kwargs = {}
    target_fps = config_model.get_setting("frame_rate_for_ai")
    if target_fps and target_fps > 0:
        metadata_kwargs["fps"] = target_fps
        app_logger.info(f"Setting Gemini videoMetadata fps={target_fps}")
    if start_offset_sec is not None:
        start_offset = _format_gemini_duration_seconds(start_offset_sec)
        metadata_kwargs["start_offset"] = start_offset
        app_logger.info(
            "Setting Gemini videoMetadata start_offset=%s", start_offset
        )
    if end_offset_sec is not None:
        end_offset = _format_gemini_duration_seconds(end_offset_sec)
        metadata_kwargs["end_offset"] = end_offset
        app_logger.info(
            "Setting Gemini videoMetadata end_offset=%s", end_offset
        )
    return metadata_kwargs


def _build_video_part(video_file_obj, start_offset_sec=None, end_offset_sec=None):
    """Builds a video Part with optional videoMetadata for FPS and time offsets.

    Uses the Gemini API's native videoMetadata to control frame sampling rate
    and time clipping, avoiding the need for local FFmpeg re-encoding or splitting.
    """
    gemini._lazy_import_gemini_sdk()
    types = gemini.types

    metadata_kwargs = _video_metadata_kwargs(start_offset_sec, end_offset_sec)

    if metadata_kwargs:
        video_metadata = types.VideoMetadata(**metadata_kwargs)
        video_part = types.Part(
            file_data=types.FileData(
                file_uri=video_file_obj.uri,
                mime_type=video_file_obj.mime_type
            ),
            video_metadata=video_metadata
        )
        return video_part

    # The Google SDK can convert its own File object automatically. The
    # Sonarpad service deliberately uses a tiny provider-neutral file object,
    # so build an explicit FileData Part while preserving the personal-key path.
    if gemini.using_sonarpad_service():
        return types.Part(
            file_data=types.FileData(
                file_uri=video_file_obj.uri,
                mime_type=video_file_obj.mime_type,
            )
        )
    return video_file_obj


def _build_inline_video_part(video_path, start_offset_sec=None, end_offset_sec=None):
    """Build a Part from local file bytes (bypasses Files API processing)."""
    gemini._lazy_import_gemini_sdk()
    types = gemini.types
    mime_type = _guess_video_mime_type(video_path)
    size = os.path.getsize(video_path)
    app_logger.info(
        "Building inline video Part from '%s' (%d bytes, mime=%s)",
        os.path.basename(video_path), size, mime_type,
    )
    with open(video_path, "rb") as f:
        video_bytes = f.read()

    metadata_kwargs = _video_metadata_kwargs(start_offset_sec, end_offset_sec)
    if hasattr(types.Part, "from_bytes"):
        part = types.Part.from_bytes(data=video_bytes, mime_type=mime_type)
        if metadata_kwargs:
            # from_bytes may not accept video_metadata; attach if possible
            try:
                part.video_metadata = types.VideoMetadata(**metadata_kwargs)
            except Exception as exc:
                raise GeminiAPIError(_(
                    "Gemini inline video metadata could not be applied safely."
                )) from exc
        return part

    blob = types.Blob(data=video_bytes, mime_type=mime_type)
    if metadata_kwargs:
        return types.Part(inline_data=blob, video_metadata=types.VideoMetadata(**metadata_kwargs))
    return types.Part(inline_data=blob)


def _prepare_video_for_gemini(client, video_path, status_callback=None,
                              start_offset_sec=None, end_offset_sec=None,
                              trusted_prepared_video=False):
    """Prepare a Gemini video Part, preferring a reliable delivery method.

    Strategy:
    1. Reject audio-only / no-video files early.
    2. Small files → inline bytes first (skips Files API which often returns
       FAILED code 13 even for valid short clips).
    3. Larger files → Files API; on FAILED, fall back to inline if size allows.

    Returns (video_part, uploaded_file_obj_or_None).
    """
    def _status(msg):
        if status_callback:
            status_callback(msg)
        app_logger.info(f"PrepareVideo: {msg}")

    if not trusted_prepared_video:
        raise GeminiAPIError(_(
            "The audio-description worker accepts only media prepared by Sonarpad."
        ))

    size = os.path.getsize(video_path)
    app_logger.info(
        "PrepareVideo: path=%s size=%d bytes mime=%s",
        os.path.basename(video_path), size, _guess_video_mime_type(video_path),
    )

    has_time_offsets = start_offset_sec is not None or end_offset_sec is not None

    # Prefer inline for small full chunks. Time-sliced fallback requests use the
    # Files API so Gemini's videoMetadata offsets are guaranteed to be honored.
    if (
        size <= _INLINE_VIDEO_MAX_BYTES
        and not has_time_offsets
        and not gemini.using_sonarpad_service()
    ):
        _status(_("Sending video inline to AI (%s MB)...") % f"{size / (1024 * 1024):.1f}")
        try:
            part = _build_inline_video_part(video_path, start_offset_sec, end_offset_sec)
            return part, None
        except Exception as e:
            app_logger.warning(
                "Inline video Part failed (%s); falling back to Files API.", e, exc_info=True
            )

    # Files API path
    try:
        video_file_obj = _upload_and_wait_for_active(client, video_path, status_callback)
        part = _build_video_part(video_file_obj, start_offset_sec, end_offset_sec)
        return part, video_file_obj
    except GeminiAPIError as e:
        if (
            size <= _INLINE_FALLBACK_MAX_BYTES
            and not has_time_offsets
            and not gemini.using_sonarpad_service()
        ):
            _status(
                _("Gemini file processing failed; retrying with inline video (%s MB)...")
                % f"{size / (1024 * 1024):.1f}"
            )
            app_logger.warning(
                "Files API failed (%s). Retrying with inline video (%d bytes).", e, size
            )
            part = _build_inline_video_part(video_path, start_offset_sec, end_offset_sec)
            return part, None
        raise


def _cleanup_uploaded_file(client, video_file_obj, status_callback=None):
    """Safely deletes an uploaded file from Gemini."""
    if video_file_obj and hasattr(video_file_obj, 'name') and client:
        try:
            client.files.delete(name=video_file_obj.name)
            if status_callback:
                status_callback(_("Cleaned up uploaded video file: %s") % video_file_obj.name)
        except Exception as del_e:
            app_logger.error(f"Failed to delete file {video_file_obj.name}: {del_e}")


def generate_descriptions_and_glossary(*_args, **_kwargs):
    raise GeminiAPIError(_(
        "Use generate_descriptions_chunked with Sonarpad-prepared media."
    ))

def _character_identity_key(identifier, name):
    identifier = str(identifier or "").strip()
    if identifier:
        return "id:" + identifier.casefold()
    return "name:" + str(name or "").strip().casefold()


def _character_name_tokens(name):
    tokens = []
    for raw_token in str(name or "").casefold().split():
        token = "".join(character for character in raw_token if character.isalnum())
        if token:
            tokens.append(token)
    return tokens


def _find_character_continuity_key(known_characters, identifier, name):
    """Find an established identity without guessing from a first name alone.

    Exact IDs win, followed by exact names.  The only shortened-name fallback
    accepted is a unique ID-prefix + name-token match such as
    anna -> anna_robinson / Anna -> Anna Robinson.  This deliberately avoids
    merging ambiguous identities such as eric_capretto and eric_beths merely
    because both contain the token Eric.
    """
    identifier_key = str(identifier or "").strip().casefold()
    name_key = str(name or "").strip().casefold()

    if identifier_key:
        id_matches = [
            key for key, value in known_characters.items()
            if str(value.get("id") or "").strip().casefold() == identifier_key
        ]
        if len(id_matches) == 1:
            return id_matches[0]

    if name_key:
        name_matches = [
            key for key, value in known_characters.items()
            if str(value.get("name") or "").strip().casefold() == name_key
        ]
        if len(name_matches) == 1:
            return name_matches[0]

    candidate_tokens = _character_name_tokens(name)
    if identifier_key and len(candidate_tokens) == 1 and len(candidate_tokens[0]) >= 3:
        token = candidate_tokens[0]
        alias_matches = []
        for key, value in known_characters.items():
            established_id = str(value.get("id") or "").strip().casefold()
            established_tokens = _character_name_tokens(value.get("name"))
            if (
                established_id.startswith(identifier_key + "_")
                and token in established_tokens
            ):
                alias_matches.append(key)
        if len(alias_matches) == 1:
            return alias_matches[0]

    return None


def _description_tokens(text):
    tokens = []
    current = []
    for character in str(text or "").casefold():
        if character.isalnum():
            current.append(character)
        elif current:
            token = "".join(current)
            if token not in tokens:
                tokens.append(token)
            current = []
    if current:
        token = "".join(current)
        if token not in tokens:
            tokens.append(token)
    return tokens


def _description_coverage(candidate, established):
    candidate_tokens = _description_tokens(candidate)
    if not candidate_tokens:
        return 1.0
    established_tokens = set(_description_tokens(established))
    if not established_tokens:
        return 0.0
    return sum(token in established_tokens for token in candidate_tokens) / len(candidate_tokens)


def _description_sentences(text):
    text = " ".join(str(text or "").split()).strip()
    if not text:
        return []
    sentences = []
    start = 0
    for index, character in enumerate(text):
        if character in ".!?":
            sentence = text[start:index + 1].strip()
            if sentence:
                sentences.append(sentence)
            start = index + 1
    tail = text[start:].strip()
    if tail:
        sentences.append(tail)
    return sentences


def _merge_character_descriptions(existing, observed, max_chars=2000):
    """Preserve authoritative detail and append only genuinely novel sentences."""
    existing = " ".join(str(existing or "").split()).strip()
    observed = " ".join(str(observed or "").split()).strip()
    if not existing:
        return observed
    if not observed:
        return existing

    merged = existing
    for sentence in _description_sentences(observed):
        if len(_description_tokens(sentence)) <= 2:
            continue
        # The established catalog wins over restatements. This also rejects a
        # near-duplicate sentence with one hallucinated/corrupted word, while a
        # genuinely new visual observation (different clothing, a bandage, etc.)
        # still has low lexical coverage and can be appended.
        if _description_coverage(sentence, merged) >= 0.65:
            continue
        separator = " " if merged.endswith((".", "!", "?")) else ". "
        combined = f"{merged}{separator}{sentence}"
        if len(combined) > max_chars:
            break
        merged = combined
    return merged


def _update_character_continuity(known_characters, chunk_glossary,
                                 max_characters=32):
    """Merge named characters while preserving established catalog identities."""
    for item in chunk_glossary or []:
        if not isinstance(item, dict):
            continue
        identifier = item.get("id")
        name = item.get("name")
        description = item.get("description")
        if not isinstance(name, str) or not name.strip():
            continue
        if not isinstance(description, str) or not description.strip():
            continue
        identifier = (
            " ".join(identifier.split())[:100]
            if isinstance(identifier, str) and identifier.strip()
            else ""
        )
        name = " ".join(name.split())[:100]
        # Character descriptions are persistent catalog data and may legitimately
        # be several hundred characters long. Keep the complete normalized text.
        description = " ".join(description.split())

        existing_key = _find_character_continuity_key(
            known_characters, identifier, name
        )
        existing = (
            known_characters.pop(existing_key)
            if existing_key is not None else None
        )
        if existing:
            # The saved/previously established identity is authoritative. Gemini
            # may call Anna Robinson simply "Anna", but that must not manufacture
            # a new `anna` identity or rename the established catalog entry.
            identifier = str(existing.get("id") or identifier).strip()
            name = str(existing.get("name") or name).strip()
            existing_description = str(existing.get("description") or "").strip()
            description = _merge_character_descriptions(
                existing_description, description
            )

        key = _character_identity_key(identifier, name)
        known_characters[key] = {
            "id": identifier,
            "name": name,
            "description": description,
        }
    while len(known_characters) > max(1, int(max_characters)):
        del known_characters[next(iter(known_characters))]
    return known_characters


def _format_character_continuity(known_characters):
    """Serialize recent named characters compactly for the next Gemini chunk."""
    if not known_characters:
        return ""
    return json.dumps(
        list(known_characters.values()), ensure_ascii=False, separators=(",", ":")
    )


def _format_recent_description_context(descriptions, next_chunk_start,
                                       max_items=6):
    """Serialize a small tail of the prior timeline for cross-chunk prose flow."""
    context = []
    for start, end, description_text in (descriptions or [])[-max_items:]:
        text = " ".join(str(description_text or "").split())
        if not text:
            continue
        context.append({
            "seconds_before_clip": round(max(0.0, float(next_chunk_start) - float(end)), 3),
            "description_text": text[:240],
        })
    return (
        json.dumps(context, ensure_ascii=False, separators=(",", ":"))
        if context else ""
    )


def _items_for_minute(items, minute_start, minute_end, local_origin):
    """Clip slots/anchors to one fallback minute and reset them to zero."""
    selected = []
    for item in items or []:
        midpoint = (float(item["start"]) + float(item["end"])) / 2.0
        if not (minute_start <= midpoint < minute_end):
            continue
        start = max(float(item["start"]), minute_start)
        end = min(float(item["end"]), minute_end)
        if end <= start:
            continue
        clipped = {
            **item,
            "start": start - local_origin,
            "end": end - local_origin,
        }
        if "max_words" in clipped:
            clipped["max_words"] = max(
                1, min(int(clipped["max_words"]), int((end - start) * 2.0))
            )
        selected.append(clipped)
    return selected


def _generate_blocked_chunk_by_minutes(
    *, client, model_name, video_path, prepared_chunk_path, chunk_start, chunk_end, chunk_number,
    total_chunks, clipped_windows, intensive_slots, extended_anchors,
    intensive_mode, extended_mode, user_prompt, character_continuity,
    prior_descriptions, status_update_callback,
):
    """Salvage a blocked three-minute chunk using independent one-minute clips.

    Each minute is attempted once. A minute that remains PROHIBITED_CONTENT is
    deliberately left without descriptions while successful minutes are kept.
    Returned timestamps are already absolute on the full-video timeline.
    """
    enable_glossary = bool(
        config_model.get_setting("enable_character_glossary")
    )
    recovered_descriptions = []
    recovered_glossary = []
    recovered_usage = []
    minute_ranges = []
    minute_start = float(chunk_start)
    chunk_end_value = float(chunk_end)
    while minute_start < chunk_end_value:
        minute_end = min(
            minute_start + _BLOCKED_CHUNK_FALLBACK_DURATION_SEC,
            chunk_end_value,
        )
        minute_duration = minute_end - minute_start
        if minute_duration < _BLOCKED_CHUNK_FALLBACK_MIN_DURATION_SEC:
            app_logger.info(
                "Blocked chunk %d: ignoring %.3fs fallback tail at %.3f-%.3fs.",
                chunk_number, minute_duration, minute_start, minute_end,
            )
            break
        minute_ranges.append((minute_start, minute_end))
        minute_start = minute_end

    minute_count = len(minute_ranges)
    if minute_count == 0:
        app_logger.info(
            "Blocked chunk %d minute fallback has no usable range.",
            chunk_number,
        )
        return recovered_descriptions, recovered_glossary, recovered_usage

    for minute_index, (minute_start, minute_end) in enumerate(minute_ranges):
        minute_duration = minute_end - minute_start
        minute_label = minute_index + 1
        status_update_callback(
            _(
                "Chunk %(chunk)d/%(total)d blocked: trying minute "
                "%(minute)d/%(minutes)d (%(start).0fs - %(end).0fs)…"
            ) % {
                "chunk": chunk_number,
                "total": total_chunks,
                "minute": minute_label,
                "minutes": minute_count,
                "start": minute_start,
                "end": minute_end,
            }
        )

        minute_windows = []
        for start, end in clipped_windows or []:
            local_start = max(float(start), minute_start)
            local_end = min(float(end), minute_end)
            if local_end - local_start >= 0.5:
                minute_windows.append(
                    (local_start - minute_start, local_end - minute_start)
                )
        minute_windows_text = ", ".join(
            f"{start:.3f}-{end:.3f}" for start, end in minute_windows
        )
        minute_slots = _items_for_minute(
            intensive_slots, minute_start, minute_end, minute_start
        )
        minute_anchors = _items_for_minute(
            extended_anchors, minute_start, minute_end, minute_start
        )
        system_instruction, prompt_text = _build_unified_prompts(
            user_prompt,
            model_name,
            minute_windows_text,
            speech_detector.format_intensive_slots_for_prompt(minute_slots),
            intensive_mode=intensive_mode,
            extended_anchors_text=(
                speech_detector.format_extended_anchors_for_prompt(
                    minute_anchors
                )
            ),
            extended_mode=extended_mode,
            character_continuity_text=(
                _format_character_continuity(character_continuity)
                if enable_glossary else ""
            ),
            recent_descriptions_text=_format_recent_description_context(
                [*prior_descriptions, *recovered_descriptions], minute_start
            ),
        )
        prompt_text += (
            "\n*   **Attached fallback clip timeline:** 0.000-"
            f"{minute_duration:.3f} seconds. All windows and slots above are "
            "relative to this one-minute clip. Return local timestamps only."
        )
        config_obj = gemini.build_generation_config(
            system_instruction_text=system_instruction,
            is_json_response=True,
            enable_thinking=True,
        )

        minute_file_obj = None
        try:
            local_minute_start = minute_start - chunk_start
            local_minute_end = minute_end - chunk_start
            minute_video_part, minute_file_obj = _prepare_video_for_gemini(
                client,
                prepared_chunk_path,
                status_update_callback,
                start_offset_sec=local_minute_start,
                end_offset_sec=local_minute_end,
                trusted_prepared_video=True,
            )
            response = gemini.generate_content_with_retry(
                client,
                model=model_name,
                contents=[prompt_text, minute_video_part],
                config=config_obj,
                status_callback=status_update_callback,
                prohibited_content_max_attempts=1,
            )
            gemini.save_raw_ai_output(
                os.path.basename(video_path),
                "blocked_chunk_minute_response",
                response,
                suffix=f"_chunk{chunk_number}_minute{minute_label}",
            )
            usage = gemini.log_token_usage(
                f"Chunk_{chunk_number}_Minute_{minute_label}", response
            )
            if usage:
                recovered_usage.append(usage)
            response_text, success = gemini.process_gemini_response(
                response, status_update_callback
            )
            if not success or not response_text:
                app_logger.warning(
                    "Blocked chunk %d fallback minute %d returned no text; "
                    "leaving it undescribed.",
                    chunk_number, minute_label,
                )
                continue
            raw_descriptions, glossary = _parse_with_optional_json_repair(
                client,
                model_name,
                minute_video_part,
                response_text,
                response,
                status_update_callback=status_update_callback,
            )
            minute_visual_evidence = list(_LAST_PARSED_VISUAL_EVIDENCE_RAW)
            raw_descriptions, language_usage = _correct_description_language(
                client, model_name, raw_descriptions, status_update_callback
            )
            if language_usage:
                recovered_usage.append(language_usage)
            glossary, glossary_language_usage = _correct_glossary_language(
                client, model_name, glossary, status_update_callback
            )
            if glossary_language_usage:
                recovered_usage.append(glossary_language_usage)
            corrected = _post_process_mmss_timestamps(
                raw_descriptions, status_update_callback
            )
            normalized = _normalize_blocked_minute_timestamps(
                corrected,
                minute_start=minute_start,
                minute_end=minute_end,
                chunk_start=chunk_start,
                chunk_number=chunk_number,
                minute_number=minute_label,
            )
            _register_visual_evidence_records(
                minute_visual_evidence, timeline_offset_sec=minute_start
            )
            recovered_descriptions.extend(normalized)
            if enable_glossary:
                recovered_glossary.extend(glossary or [])
                if glossary:
                    _update_character_continuity(character_continuity, glossary)
            app_logger.info(
                "Blocked chunk %d fallback minute %d kept %d descriptions.",
                chunk_number, minute_label, len(normalized),
            )
        except ContentBlockedError as exc:
            if str(getattr(exc, "reason", "")).upper() != "PROHIBITED_CONTENT":
                raise
            app_logger.warning(
                "Blocked chunk %d fallback minute %d is still prohibited; "
                "continuing with original audio and no descriptions for "
                "%.3f-%.3fs.",
                chunk_number, minute_label, minute_start, minute_end,
            )
            status_update_callback(
                _(
                    "Chunk %(chunk)d minute %(minute)d remains blocked; "
                    "keeping original audio without descriptions for this minute."
                ) % {"chunk": chunk_number, "minute": minute_label}
            )
        finally:
            _cleanup_uploaded_file(
                client, minute_file_obj, status_update_callback
            )

    app_logger.info(
        "Blocked chunk %d minute fallback complete: minutes=%d "
        "descriptions=%d.",
        chunk_number, minute_count, len(recovered_descriptions),
    )
    return recovered_descriptions, recovered_glossary, recovered_usage


def generate_descriptions_chunked(video_path, chunk_duration_sec, user_prompt="",
                                  status_update_callback=None, dialogue_free_windows="",
                                  dialogue_intervals=None, prepared_chunks=None,
                                  total_duration_override=None,
                                  initial_character_glossary=None,
                                  resume_completed_chunks=0,
                                  resume_descriptions=None,
                                  resume_character_glossary=None,
                                  checkpoint_callback=None):
    """Generate long-video descriptions in bounded analysis chunks.

    Multi-chunk videos use exact temporary clips for lower Gemini latency. A
    rolling named-character reference preserves identity across independent
    requests without requiring the complete movie in every call.

    Returns: (all_descriptions, character_glossary, all_token_usage_list)
    """
    def _update_status(msg):
        if status_update_callback:
            status_update_callback(msg)
        app_logger.info(f"ChunkedDescriber: {msg}")

    if not os.path.exists(video_path):
        _update_status(_("Error: Video file not found: %s") % video_path)
        raise FileNotFoundError(f"Video file not found: {video_path}")

    _reset_visual_evidence_registry()
    video_file_obj = None
    client = None
    all_descriptions = []
    character_glossary = []
    detected_character_glossary = []
    character_continuity = {}
    all_token_usage = []
    temporary_chunk_paths = []
    per_chunk_uploaded_files = []

    try:
        client = gemini.get_gemini_client()
        model_name_to_use = gemini.validate_model_for_generate_content(
            _get_model_name(), client=client, status_callback=_update_status
        )
        app_logger.info("Chunked generation using Gemini model: %s", model_name_to_use)

        if not prepared_chunks:
            raise ValueError(_(
                "Sonarpad did not provide prepared Gemini chunks."
            ))
        total_duration = float(total_duration_override or 0.0)
        if total_duration <= 0:
            raise ValueError(_("Could not determine video duration or video is empty."))
        normalized_chunks = []
        for index, item in enumerate(prepared_chunks, 1):
            path = os.fspath(item.get("path") or "")
            start = float(item.get("start_sec") or 0.0)
            end = float(item.get("end_sec") or 0.0)
            if not os.path.isfile(path) or end <= start:
                raise ValueError(_("Invalid Sonarpad-prepared video chunk %d.") % index)
            normalized_chunks.append({"path": path, "start": start, "end": end})
        num_chunks = len(normalized_chunks)
        use_per_chunk_uploads = True
        shared_file_obj = None
        resume_completed_chunks = max(0, min(int(resume_completed_chunks or 0), num_chunks))
        for item in resume_descriptions or []:
            if not isinstance(item, dict):
                continue
            try:
                start = float(item.get("start_sec") or 0.0)
                end = float(item.get("end_sec") or start)
            except (TypeError, ValueError):
                continue
            text = str(item.get("text") or "").strip()
            if text and end > start:
                all_descriptions.append((start, end, text))
                evidence = _parse_visual_evidence_seconds(
                    item.get("visual_evidence_time_sec")
                )
                if evidence is not None:
                    _VISUAL_EVIDENCE_RECORDS.append((start, end, text, evidence))
        app_logger.info(
            "Using %d physical chunk(s) prepared by Sonarpad's Rust FFmpeg backend.",
            num_chunks,
        )
        if resume_completed_chunks:
            app_logger.info(
                "Resuming audio description from checkpoint: completed_chunks=%d/%d descriptions=%d.",
                resume_completed_chunks, num_chunks, len(all_descriptions),
            )

        _update_status(_("Processing video..."))

        enable_glossary = config_model.get_setting("enable_character_glossary")
        if enable_glossary and initial_character_glossary:
            _update_character_continuity(
                character_continuity, initial_character_glossary, max_characters=96
            )
        if enable_glossary and resume_character_glossary:
            _update_character_continuity(
                character_continuity, resume_character_glossary, max_characters=96
            )
        if enable_glossary and character_continuity:
            seeded = list(character_continuity.values())
            character_glossary.extend(seeded)
            detected_character_glossary.extend(seeded)
            app_logger.info(
                "Loaded %d established/resumed character(s) for continuity.",
                len(seeded),
            )
        extended_mode = bool(
            config_model.get_setting("enable_extended_audio_description")
        )
        intensive_mode = (
            config_model.get_setting("description_coverage_mode") == "intensive"
            or extended_mode
        )
        min_intensive_silence = float(
            config_model.get_setting("intensive_min_silence_seconds") or 3.0
        )

        for i, prepared_chunk in enumerate(normalized_chunks):
            if i < resume_completed_chunks:
                app_logger.info(
                    "Chunk %d/%d restored from checkpoint; Gemini call skipped.",
                    i + 1, num_chunks,
                )
                continue
            chunk_start = prepared_chunk["start"]
            chunk_end = prepared_chunk["end"]
            chunk_free_windows = dialogue_free_windows
            clipped = None
            if dialogue_intervals is not None and (
                dialogue_free_windows or intensive_mode
            ):
                free = speech_detector.speech_free_intervals(
                    dialogue_intervals, total_duration, min_duration_sec=0.5
                )
                clipped = [
                    (max(start, chunk_start), min(end, chunk_end))
                    for start, end in free
                    if min(end, chunk_end) - max(start, chunk_start) >= 0.5
                ]
                chunk_free_windows = ", ".join(
                    f"{start:.3f}-{end:.3f}" for start, end in clipped
                )
            intensive_slots = (
                speech_detector.intensive_description_slots(
                    dialogue_intervals or [], total_duration,
                    min_intensive_silence, chunk_start, chunk_end,
                    id_suffix=f"C{i + 1:04d}",
                )
                if intensive_mode else []
            )
            intensive_slots_text = speech_detector.format_intensive_slots_for_prompt(
                intensive_slots
            )
            extended_anchors = (
                speech_detector.extended_description_anchors(
                    dialogue_intervals or [], total_duration,
                    min_intensive_silence, chunk_start, chunk_end,
                    id_suffix=f"C{i + 1:04d}",
                )
                if intensive_mode else []
            )
            extended_anchors_text = (
                speech_detector.format_extended_anchors_for_prompt(extended_anchors)
            )
            # An extracted upload has a fresh 0-based media timeline.  Keep the
            # pyannote windows on that same timeline while Gemini is looking at
            # the clip, then add chunk_start exactly once after parsing.  Mixing
            # absolute silence slots with a relative clip made Gemini attach a
            # later scene to an earlier slot (for example a scene near 05:00 was
            # returned at 03:36 in chunk 2).
            if use_per_chunk_uploads:
                chunk_free_windows = ", ".join(
                    f"{start - chunk_start:.3f}-{end - chunk_start:.3f}"
                    for start, end in clipped
                ) if clipped is not None else chunk_free_windows
                prompt_intensive_slots = [
                    {
                        **slot,
                        "start": slot["start"] - chunk_start,
                        "end": slot["end"] - chunk_start,
                    }
                    for slot in intensive_slots
                ]
                intensive_slots_text = (
                    speech_detector.format_intensive_slots_for_prompt(
                        prompt_intensive_slots
                    )
                )
                prompt_extended_anchors = [
                    {
                        **anchor,
                        "start": anchor["start"] - chunk_start,
                        "end": anchor["end"] - chunk_start,
                        "scene_start": anchor["scene_start"] - chunk_start,
                        "scene_end": anchor["scene_end"] - chunk_start,
                    }
                    for anchor in extended_anchors
                ]
                extended_anchors_text = (
                    speech_detector.format_extended_anchors_for_prompt(
                        prompt_extended_anchors
                    )
                )
            if intensive_mode:
                app_logger.info(
                    "Chunk %d intensive plan: mandatory_slots=%d min_silence=%.1fs ids=%s.",
                    i + 1, len(intensive_slots), min_intensive_silence,
                    ",".join(slot["id"] for slot in intensive_slots) or "none",
                )
            if intensive_mode:
                app_logger.info(
                    "Chunk %d intensive short-gap plan: optional_anchors=%d "
                    "media_pauses_allowed=%s ids=%s.",
                    i + 1, len(extended_anchors),
                    extended_mode,
                    ",".join(anchor["id"] for anchor in extended_anchors) or "none",
                )
            system_instruction, user_prompt_text = _build_unified_prompts(
                user_prompt, model_name_to_use, chunk_free_windows,
                intensive_slots_text,
                intensive_mode=intensive_mode,
                extended_anchors_text=extended_anchors_text,
                extended_mode=extended_mode,
                character_continuity_text=_format_character_continuity(
                    character_continuity
                ),
                recent_descriptions_text=_format_recent_description_context(
                    all_descriptions, chunk_start
                ),
            )
            gen_config = gemini.build_generation_config(
                system_instruction_text=system_instruction,
                is_json_response=True,
                enable_thinking=True,
            )

            app_logger.info(
                "Chunk %d/%d audit: requested absolute range %.3f-%.3fs "
                "(duration %.3fs, configured chunk size %.3fs).",
                i + 1, num_chunks, chunk_start, chunk_end,
                chunk_end - chunk_start, float(chunk_duration_sec),
            )

            _update_status(_("Processing chunk %d of %d (%.0fs - %.0fs)...") % (i + 1, num_chunks, chunk_start, chunk_end))

            current_chunk_path = prepared_chunk["path"]
            current_chunk_file_obj = None
            if use_per_chunk_uploads:
                # Sonarpad created this physical clip through its FFmpeg DLL backend.
                # It has a local 0-based timeline; normalization adds chunk_start.
                video_part, current_chunk_file_obj = _prepare_video_for_gemini(
                    client, current_chunk_path, _update_status,
                    trusted_prepared_video=True,
                )
                if current_chunk_file_obj is not None:
                    per_chunk_uploaded_files.append(current_chunk_file_obj)
            elif shared_file_obj is not None:
                video_part = _build_video_part(
                    shared_file_obj,
                    start_offset_sec=chunk_start,
                    end_offset_sec=chunk_end,
                )
            else:
                video_part, maybe_file = _prepare_video_for_gemini(
                    client, video_path, _update_status,
                    start_offset_sec=chunk_start,
                    end_offset_sec=chunk_end,
                )
                if maybe_file is not None:
                    # Promote to shared upload for remaining chunks
                    shared_file_obj = maybe_file
                    video_file_obj = maybe_file
            if use_per_chunk_uploads:
                chunk_prompt_text = user_prompt_text + (
                    "\n*   **Attached clip timeline:** 0.000-"
                    f"{chunk_end - chunk_start:.3f} seconds. All dialogue-free windows "
                    "and mandatory slots above are relative to this attached clip. "
                    "Return timestamps on this local clip timeline only."
                )
            else:
                chunk_prompt_text = user_prompt_text + (
                    "\n*   **Current chunk absolute range:** "
                    f"{chunk_start:.3f}-{chunk_end:.3f} seconds in the full video. "
                    "Return timestamps on the full-video (absolute) timeline and never "
                    "return a timestamp outside this range."
                )
            api_contents = [chunk_prompt_text, video_part]

            _update_status(
                _("Chunk %d/%d: asking Gemini for descriptions…") % (i + 1, num_chunks)
            )
            minute_fallback_used = False
            chunk_visual_evidence = []
            try:
                try:
                    response = gemini.generate_content_with_retry(
                        client, model=model_name_to_use, contents=api_contents,
                        config=gen_config, status_callback=_update_status,
                        prohibited_content_max_attempts=2,
                    )
                except Exception as exc:
                    chunk_size = os.path.getsize(current_chunk_path)
                    if (
                        current_chunk_file_obj is not None
                        and chunk_size <= _INLINE_FALLBACK_MAX_BYTES
                        and _is_permission_denied_error(exc)
                    ):
                        _update_status(
                            _("Gemini denied the uploaded clip; retrying inline (%s MB)...")
                            % f"{chunk_size / (1024 * 1024):.1f}"
                        )
                        app_logger.warning(
                            "Files API generateContent returned permission denied for chunk %d; "
                            "retrying inline (%d bytes).",
                            i + 1, chunk_size,
                        )
                        _cleanup_uploaded_file(
                            client, current_chunk_file_obj, _update_status
                        )
                        if current_chunk_file_obj in per_chunk_uploaded_files:
                            per_chunk_uploaded_files.remove(current_chunk_file_obj)
                        current_chunk_file_obj = None
                        video_part = _build_inline_video_part(current_chunk_path)
                        api_contents = [chunk_prompt_text, video_part]
                        response = gemini.generate_content_with_retry(
                            client, model=model_name_to_use, contents=api_contents,
                            config=gen_config, status_callback=_update_status,
                            prohibited_content_max_attempts=2,
                        )
                    else:
                        raise
                gemini.save_raw_ai_output(
                    os.path.basename(video_path), "unified_raw_response",
                    response, suffix=f"_chunk{i+1}"
                )

                chunk_usage = gemini.log_token_usage(f"Chunk_{i+1}", response)
                if chunk_usage:
                    all_token_usage.append(chunk_usage)

                _update_status(_("Chunk %d/%d: reading Gemini response…") % (i + 1, num_chunks))
                raw_json_text, success = gemini.process_gemini_response(response, _update_status)
                if not success or not raw_json_text:
                    app_logger.warning(f"Chunk {i+1} returned no usable response.")
                    _update_status(_("Chunk %d/%d: no usable AI text.") % (i + 1, num_chunks))
                    if current_chunk_file_obj is not None:
                        _cleanup_uploaded_file(client, current_chunk_file_obj, _update_status)
                        per_chunk_uploaded_files.remove(current_chunk_file_obj)
                    continue

                chunk_descriptions, chunk_glossary = _parse_with_optional_json_repair(
                    client,
                    model_name_to_use,
                    video_part,
                    raw_json_text,
                    response,
                    status_update_callback=_update_status,
                )
                chunk_visual_evidence = list(_LAST_PARSED_VISUAL_EVIDENCE_RAW)

                # Validate this model response before mixing it with descriptions
                # from other chunks. A few English entries can otherwise disappear
                # inside a much larger, predominantly Italian final sample.
                chunk_descriptions, chunk_language_usage = _correct_description_language(
                    client, model_name_to_use, chunk_descriptions, _update_status
                )
                if chunk_language_usage:
                    all_token_usage.append(chunk_language_usage)
                chunk_glossary, glossary_language_usage = _correct_glossary_language(
                    client, model_name_to_use, chunk_glossary, _update_status
                )
                if glossary_language_usage:
                    all_token_usage.append(glossary_language_usage)
            except ContentBlockedError as exc:
                if str(getattr(exc, "reason", "")).upper() != "PROHIBITED_CONTENT":
                    raise
                minute_fallback_used = True
                app_logger.warning(
                    "Chunk %d remained PROHIBITED_CONTENT after two full-chunk "
                    "attempts; falling back to one-minute clips.",
                    i + 1,
                )
                _update_status(
                    _(
                        "Chunk %d remained blocked after two attempts; "
                        "retrying it one minute at a time."
                    ) % (i + 1)
                )
                (
                    normalized_chunk,
                    chunk_glossary,
                    fallback_usage,
                ) = _generate_blocked_chunk_by_minutes(
                    client=client,
                    model_name=model_name_to_use,
                    video_path=video_path,
                    prepared_chunk_path=current_chunk_path,
                    chunk_start=chunk_start,
                    chunk_end=chunk_end,
                    chunk_number=i + 1,
                    total_chunks=num_chunks,
                    clipped_windows=clipped,
                    intensive_slots=intensive_slots,
                    extended_anchors=extended_anchors,
                    intensive_mode=intensive_mode,
                    extended_mode=extended_mode,
                    user_prompt=user_prompt,
                    character_continuity=character_continuity,
                    prior_descriptions=all_descriptions,
                    status_update_callback=_update_status,
                )
                all_token_usage.extend(fallback_usage)

            if enable_glossary and chunk_glossary:
                detected_character_glossary.extend(chunk_glossary)
                character_glossary.extend(chunk_glossary)
                before_count = len(character_continuity)
                _update_character_continuity(
                    character_continuity, chunk_glossary, max_characters=96
                )
                app_logger.info(
                    "Chunk %d character continuity: before=%d after=%d names=%s.",
                    i + 1, before_count, len(character_continuity),
                    ", ".join(
                        item["name"] for item in character_continuity.values()
                    ) or "none",
                )

            if not minute_fallback_used and not chunk_descriptions:
                app_logger.warning(
                    "Chunk %d returned no descriptions; coverage recovery will inspect it.",
                    i + 1,
                )
                _update_status(
                    _("Chunk %d/%d: no descriptions; checking the uncovered chunk again.")
                    % (i + 1, num_chunks)
                )

            # Minute fallback already returns full-video timestamps. Normal
            # chunks still need their local timestamps converted exactly once.
            if not minute_fallback_used:
                corrected_chunk = _post_process_mmss_timestamps(chunk_descriptions, _update_status)
                normalized_chunk = _normalize_chunk_timestamps(
                    corrected_chunk, chunk_start, chunk_end, i + 1,
                    force_mode="relative" if use_per_chunk_uploads else None,
                )
                _register_visual_evidence_records(
                    chunk_visual_evidence,
                    timeline_offset_sec=chunk_start if use_per_chunk_uploads else 0.0,
                )
            max_recovery_passes = 0 if minute_fallback_used else (
                3 if intensive_mode else 1
            )
            for recovery_pass in range(1, max_recovery_passes + 1):
                missing_intensive = speech_detector.uncovered_intensive_slots(
                    normalized_chunk, intensive_slots
                )
                recovery_gaps = (
                    [(slot["start"], slot["end"]) for slot in missing_intensive]
                    if intensive_mode else None
                )
                normalized_chunk, recovery_usage = _recover_large_chunk_gaps(
                    client=client,
                    model_name=model_name_to_use,
                    video_part=video_part,
                    descriptions=normalized_chunk,
                    chunk_start=chunk_start,
                    chunk_end=chunk_end,
                    chunk_number=i + 1,
                    video_basename=os.path.basename(video_path),
                    dialogue_free_windows=chunk_free_windows,
                    status_update_callback=_update_status,
                    required_gaps=recovery_gaps,
                    intensive_mode=intensive_mode,
                    media_uses_local_timeline=use_per_chunk_uploads,
                    character_continuity_text=_format_character_continuity(
                        character_continuity
                    ),
                )
                if recovery_usage:
                    all_token_usage.append(recovery_usage)
                if not intensive_mode:
                    break

                remaining = speech_detector.uncovered_intensive_slots(
                    normalized_chunk, intensive_slots
                )
                app_logger.info(
                    "Chunk %d intensive recovery pass %d/%d: remaining=%d ids=%s.",
                    i + 1, recovery_pass, max_recovery_passes, len(remaining),
                    ",".join(slot["id"] for slot in remaining) or "none",
                )
                if not remaining:
                    break
                if recovery_pass < max_recovery_passes:
                    _update_status(
                        _("Chunk %d: retrying %d intensive silence slots still missing…")
                        % (i + 1, len(remaining))
                    )
            all_descriptions.extend(normalized_chunk)

            if intensive_mode:
                still_missing = speech_detector.uncovered_intensive_slots(
                    normalized_chunk, intensive_slots
                )
                app_logger.info(
                    "Chunk %d intensive coverage audit: slots=%d covered=%d missing=%d ids=%s.",
                    i + 1, len(intensive_slots),
                    len(intensive_slots) - len(still_missing), len(still_missing),
                    ",".join(slot["id"] for slot in still_missing) or "none",
                )
                if still_missing:
                    _update_status(
                        _("Chunk %d: Gemini left %d intensive silence slots undescribed.")
                        % (i + 1, len(still_missing))
                    )

            _update_status(_("Chunk %d: parsed %d descriptions.") % (i + 1, len(normalized_chunk)))

            if checkpoint_callback is not None:
                try:
                    checkpoint_callback(
                        i + 1,
                        num_chunks,
                        list(all_descriptions),
                        list(character_continuity.values()) if enable_glossary else [],
                        str(config_model.get_setting("gemini_model_override") or model_name_to_use),
                    )
                except Exception as exc:
                    app_logger.warning(
                        "Chunk %d checkpoint callback failed; continuing job: %s",
                        i + 1, exc,
                    )

            if current_chunk_file_obj is not None:
                _cleanup_uploaded_file(client, current_chunk_file_obj, _update_status)
                per_chunk_uploaded_files.remove(current_chunk_file_obj)

        # Final deduplication across all chunks
        all_descriptions.sort(key=lambda item: item[0])
        _log_timeline_audit(all_descriptions, total_duration, num_chunks)
        final_descriptions = _remove_consecutive_duplicates(all_descriptions, _update_status)
        if enable_glossary:
            final_descriptions = _suppress_repeated_leading_character_names(
                final_descriptions, detected_character_glossary, _update_status
            )

        if not final_descriptions:
            _update_status(_("No descriptions remained after all post-processing."))

        merged_glossary = list(character_continuity.values()) if enable_glossary else []
        app_logger.info(
            "Chunked generation complete. Descriptions: %d, Glossary: %d",
            len(final_descriptions), len(merged_glossary),
        )
        return final_descriptions, merged_glossary, all_token_usage

    except gemini.GeminiRetryCancelledError as e:
        _update_status(str(e))
        app_logger.info("Chunked generation cancelled by user: %s", e)
        raise
    except Exception as e:
        _update_status(_("Chunked generation failed: %s") % str(e))
        app_logger.error("Chunked generation failed critically: %s", e, exc_info=True)
        raise
    finally:
        _cleanup_uploaded_file(client, video_file_obj, _update_status)
        for uploaded_file in list(per_chunk_uploaded_files):
            _cleanup_uploaded_file(client, uploaded_file, _update_status)
        for chunk_path in list(temporary_chunk_paths):
            try:
                if os.path.exists(chunk_path):
                    os.unlink(chunk_path)
            except OSError as exc:
                app_logger.warning(
                    "Could not remove temporary Gemini chunk %s: %s", chunk_path, exc
                )


def ask_gemini_about_video_segment(video_segment_path, vqa_prompt_text, system_instruction_text_vqa=None):
    if not os.path.exists(video_segment_path):
        raise FileNotFoundError(f"Video segment file not found: {video_segment_path}")

    video_file_obj = None
    vqa_usage = {}
    client = None
    try:
        client = gemini.get_gemini_client()
        model_name_to_use = _get_model_name()

        video_part, video_file_obj = _prepare_video_for_gemini(client, video_segment_path)

        gen_config_obj = gemini.build_generation_config(is_json_response=False, enable_thinking=False)
        response = gemini.generate_content_with_retry(
            client, model=model_name_to_use, contents=[vqa_prompt_text, video_part],
            config=gen_config_obj
        )

        gemini.save_raw_ai_output(os.path.basename(video_segment_path), "vqa_raw_response", response)
        vqa_usage = gemini.log_token_usage("VQA", response)
        answer, success = gemini.process_gemini_response(response, None)

        return (answer, vqa_usage) if success else (_("AI could not provide an answer."), vqa_usage)
    except Exception as e:
        app_logger.error("VQA failed critically: %s", e, exc_info=True)
        raise GeminiAPIError(_("VQA request failed: %s") % str(e))
    finally:
        _cleanup_uploaded_file(client, video_file_obj)


def get_json_response_from_gemini(video_path, prompt_text, status_update_callback=None, suffix=""):
    if not os.path.exists(video_path):
        raise FileNotFoundError(f"Video file not found: {video_path}")
    video_file_obj = None
    client = None
    try:
        client = gemini.get_gemini_client()
        model_name_to_use = _get_model_name()

        video_part, video_file_obj = _prepare_video_for_gemini(
            client, video_path, status_update_callback
        )

        gen_config_obj = gemini.build_generation_config(is_json_response=True, enable_thinking=False)
        response = gemini.generate_content_with_retry(
            client, model=model_name_to_use, contents=[prompt_text, video_part],
            config=gen_config_obj, status_callback=status_update_callback
        )

        gemini.save_raw_ai_output(os.path.basename(video_path), "json_raw_response", response, suffix=suffix)

        if hasattr(response, 'candidates') and response.candidates and hasattr(response.candidates[0], 'finish_reason') and response.candidates[0].finish_reason.name == 'MAX_TOKENS':
            raise TokenLimitError(_("AI process stopped while generating glossary due to token limit. Try simplifying the video or adjusting settings."))

        usage_data = gemini.log_token_usage("JSON_Mode", response)
        json_text, success = gemini.process_gemini_response(response, status_update_callback)
        return (json_text, usage_data) if success else ("", usage_data)
    except Exception as e:
        app_logger.error("get_json_response_from_gemini failed: %s", e, exc_info=True)
        raise GeminiAPIError(_("Failed to get structured data from AI: %s") % str(e))
    finally:
        _cleanup_uploaded_file(client, video_file_obj)

# --- INTERNAL HELPERS ---

def _get_model_name():
    model_override = config_model.get_setting("gemini_model_override")
    selected = model_override if model_override and model_override.strip() else config.GEMINI_MODEL_NAME
    return gemini.normalize_model_id(selected)


def _merge_token_usage(primary, additional):
    if not additional:
        return primary
    merged = dict(primary or {})
    for key, value in additional.items():
        if isinstance(value, (int, float)):
            merged[key] = (merged.get(key) or 0) + value
    return merged


def _parse_language_correction_response(response_text, expected_count):
    """Read indexed corrected strings without allowing timestamps to change."""
    try:
        payload = json.loads(_strip_json_fences(response_text))
    except (TypeError, json.JSONDecodeError):
        return None
    items = payload.get("descriptions") if isinstance(payload, dict) else None
    if not isinstance(items, list) or len(items) != expected_count:
        return None
    corrected = {}
    for item in items:
        if not isinstance(item, dict):
            return None
        index = item.get("index")
        text = item.get("text")
        if not isinstance(index, int) or not isinstance(text, str) or not text.strip():
            return None
        if index in corrected or not 0 <= index < expected_count:
            return None
        corrected[index] = text.strip()
    if set(corrected) != set(range(expected_count)):
        return None
    return [corrected[index] for index in range(expected_count)]


def _correct_description_language(
    client, model_name, descriptions, status_update_callback=None
):
    """Detect every description and correct wrong-language entries once.

    Each string is detected independently so one English sentence cannot hide
    among mostly Italian entries. Detection calls run concurrently; a single
    guided Gemini retry receives only the mismatched strings and cannot alter
    timings, ordering, or the number of descriptions.
    """
    if not descriptions:
        return descriptions, None

    target_code, target_name, _examples = _target_language_details()
    texts = [str(item[2] or "").strip() for item in descriptions]

    def _detect_one(text):
        try:
            return language_detector.detect_language(text, target_code), None
        except Exception as exc:
            return None, exc

    workers = min(4, max(1, len(texts)))
    with ThreadPoolExecutor(max_workers=workers) as executor:
        detection_results = list(executor.map(_detect_one, texts))

    mismatches = []
    skipped = 0
    low_confidence = 0
    errors = 0
    for index, (detection, error) in enumerate(detection_results):
        if error is not None:
            errors += 1
            app_logger.warning(
                "Description language detection failed for entry %d; keeping it: %s",
                index, error,
            )
            continue
        if detection is None:
            skipped += 1
            continue
        if language_detector.languages_match(detection.language, target_code):
            continue
        if (
            detection.confidence is not None
            and detection.confidence < _LANGUAGE_DETECTION_MIN_CONFIDENCE
        ):
            low_confidence += 1
            continue
        mismatches.append((index, detection))

    app_logger.info(
        "Per-description language audit: entries=%d expected=%s mismatches=%d "
        "too_short=%d low_confidence=%d errors=%d.",
        len(descriptions), target_code, len(mismatches), skipped,
        low_confidence, errors,
    )
    if not mismatches:
        return descriptions, None

    if status_update_callback:
        status_update_callback(
            _("AI used the wrong language; asking it to correct the descriptions…")
        )
    app_logger.warning(
        "Description language mismatches at indexes %s; expected=%s detected=%s. "
        "Requesting one guided correction pass for those entries only.",
        ",".join(str(index) for index, _detection in mismatches), target_code,
        ",".join(detection.language for _index, detection in mismatches),
    )

    indexed = [
        {"index": correction_index, "text": texts[original_index]}
        for correction_index, (original_index, _detection) in enumerate(mismatches)
    ]
    system_instruction = (
        "You correct the output language of audio descriptions. Return ONLY valid JSON with "
        'one top-level key, "descriptions". Its value must be an array containing exactly the '
        'same indexes as the input. Each item must be {"index": integer, "text": string}. '
        f"Rewrite every text entirely in {target_name}. Preserve meaning, names, brevity and "
        "audio-description style. Do not add, remove, merge, reorder or renumber entries."
    )
    user_prompt = (
        "The following individual descriptions were detected in the wrong language. "
        f"The required output language is {target_name}. Correct every supplied description:\n"
        + json.dumps({"descriptions": indexed}, ensure_ascii=False)
    )
    try:
        gen_config = gemini.build_generation_config(
            system_instruction_text=system_instruction,
            is_json_response=True,
            enable_thinking=False,
        )
        response = gemini.generate_content_with_retry(
            client,
            model=model_name,
            contents=[user_prompt],
            config=gen_config,
            status_callback=status_update_callback,
        )
        gemini.save_raw_ai_output(
            "language_correction", "description_language_response", response
        )
        usage = gemini.log_token_usage("Description_Language_Correction", response)
        response_text, success = gemini.process_gemini_response(
            response, status_update_callback
        )
        corrected_texts = (
            _parse_language_correction_response(response_text, len(mismatches))
            if success else None
        )
        if corrected_texts is None:
            raise ValueError("Language correction returned invalid indexed JSON")
        corrected_descriptions = list(descriptions)
        for correction_index, (original_index, _detection) in enumerate(mismatches):
            item = descriptions[original_index]
            corrected_descriptions[original_index] = (
                item[0], item[1], corrected_texts[correction_index]
            )
        app_logger.info(
            "Description language correction completed for %d of %d descriptions.",
            len(mismatches), len(descriptions),
        )
        return corrected_descriptions, usage
    except Exception as exc:
        app_logger.warning(
            "Description language correction failed; keeping original descriptions: %s",
            exc,
            exc_info=True,
        )
        if status_update_callback:
            status_update_callback(
                _("The description language correction failed; keeping the original result.")
            )
        return descriptions, None


def _parse_glossary_language_correction_response(response_text, expected_count):
    """Read indexed corrected glossary descriptions without changing IDs or names."""
    try:
        payload = json.loads(_strip_json_fences(response_text))
    except (TypeError, json.JSONDecodeError):
        return None
    items = payload.get("entries") if isinstance(payload, dict) else None
    if not isinstance(items, list) or len(items) != expected_count:
        return None
    corrected = {}
    for item in items:
        if not isinstance(item, dict):
            return None
        index = item.get("index")
        description = item.get("description")
        if (
            not isinstance(index, int)
            or not isinstance(description, str)
            or not description.strip()
            or index in corrected
            or not 0 <= index < expected_count
        ):
            return None
        corrected[index] = description.strip()
    if set(corrected) != set(range(expected_count)):
        return None
    return [corrected[index] for index in range(expected_count)]


def _correct_glossary_language(
    client, model_name, glossary, status_update_callback=None
):
    """Correct only wrong-language physical descriptions in the character glossary."""
    if not glossary or not config_model.get_setting("enable_character_glossary"):
        return glossary or [], None

    target_code, target_name, _examples = _target_language_details()
    candidates = []
    for original_index, item in enumerate(glossary):
        if not isinstance(item, dict):
            continue
        description = str(item.get("description") or "").strip()
        if description:
            candidates.append((original_index, description))
    if not candidates:
        return glossary, None

    def _detect_one(candidate):
        original_index, text = candidate
        try:
            detection = language_detector.detect_language(text, target_code)
            return original_index, text, detection, None
        except Exception as exc:
            return original_index, text, None, exc

    workers = min(4, max(1, len(candidates)))
    with ThreadPoolExecutor(max_workers=workers) as executor:
        detection_results = list(executor.map(_detect_one, candidates))

    mismatches = []
    for original_index, text, detection, error in detection_results:
        if error is not None:
            app_logger.warning(
                "Glossary language detection failed for entry %d; keeping it: %s",
                original_index, error,
            )
            continue
        if detection is None or language_detector.languages_match(
            detection.language, target_code
        ):
            continue
        if (
            detection.confidence is not None
            and detection.confidence < _LANGUAGE_DETECTION_MIN_CONFIDENCE
        ):
            continue
        mismatches.append((original_index, text, detection))

    if not mismatches:
        return glossary, None

    if status_update_callback:
        status_update_callback(
            _("AI used the wrong language in the character glossary; correcting it…")
        )
    indexed = [
        {"index": correction_index, "description": text}
        for correction_index, (_original_index, text, _detection)
        in enumerate(mismatches)
    ]
    system_instruction = (
        "You correct only the language of physical descriptions in a character glossary. "
        "Return ONLY valid JSON with one top-level key, \"entries\". Its value must "
        "contain exactly the same indexes as the input. Each item must be "
        '{"index": integer, "description": string}. '
        f"Rewrite every description entirely in {target_name}. Preserve physical meaning, "
        "brevity, IDs and character names. Do not add, remove, merge, reorder or rename entries."
    )
    user_prompt = (
        f"The required output language is {target_name}. Correct every supplied physical "
        "description while leaving character IDs and names untouched:\n"
        + json.dumps({"entries": indexed}, ensure_ascii=False)
    )
    try:
        gen_config = gemini.build_generation_config(
            system_instruction_text=system_instruction,
            is_json_response=True,
            enable_thinking=False,
        )
        response = gemini.generate_content_with_retry(
            client,
            model=model_name,
            contents=[user_prompt],
            config=gen_config,
            status_callback=status_update_callback,
        )
        gemini.save_raw_ai_output(
            "language_correction", "glossary_language_response", response
        )
        usage = gemini.log_token_usage("Glossary_Language_Correction", response)
        response_text, success = gemini.process_gemini_response(
            response, status_update_callback
        )
        corrected_texts = (
            _parse_glossary_language_correction_response(
                response_text, len(mismatches)
            )
            if success else None
        )
        if corrected_texts is None:
            raise ValueError("Glossary language correction returned invalid indexed JSON")
        corrected_glossary = [dict(item) if isinstance(item, dict) else item for item in glossary]
        for correction_index, (original_index, _text, _detection) in enumerate(mismatches):
            corrected_glossary[original_index]["description"] = corrected_texts[correction_index]
        return corrected_glossary, usage
    except Exception as exc:
        app_logger.warning(
            "Glossary language correction failed; keeping original glossary: %s",
            exc,
            exc_info=True,
        )
        return glossary, None


def _effective_chunk_duration(configured_duration_sec):
    """Cap all chunked analysis slices to prevent accumulated timestamp drift."""
    duration = max(1.0, float(configured_duration_sec))
    return min(duration, _MAX_CHUNK_DURATION_SEC)


def _normalize_blocked_minute_timestamps(
    descriptions, *, minute_start, minute_end, chunk_start, chunk_number,
    minute_number, tolerance_sec=2.0,
):
    """Normalize Gemini timestamps returned by a PROHIBITED_CONTENT minute fallback.

    Gemini may timestamp a videoMetadata sub-range in any of three coordinate
    systems despite being asked for clip-local times: the one-minute clip
    itself, the prepared parent chunk, or the full video. Resolve that origin
    before converting to the full-video timeline, so an offset is never added
    twice.
    """
    if not descriptions:
        app_logger.info(
            "Blocked chunk %d fallback minute %d timestamp audit: no descriptions.",
            chunk_number, minute_number,
        )
        return []

    minute_start = float(minute_start)
    minute_end = float(minute_end)
    chunk_start = float(chunk_start)
    minute_duration = max(0.0, minute_end - minute_start)
    chunk_local_start = minute_start - chunk_start
    chunk_local_end = minute_end - chunk_start

    def _fits_range(item, range_start, range_end):
        start, end, _ = item
        return (
            start >= range_start - tolerance_sec
            and end <= range_end + tolerance_sec
        )

    scores = {
        "minute_local": sum(
            1 for item in descriptions
            if _fits_range(item, 0.0, minute_duration)
        ),
        "chunk_local": sum(
            1 for item in descriptions
            if _fits_range(item, chunk_local_start, chunk_local_end)
        ),
        "absolute": sum(
            1 for item in descriptions
            if _fits_range(item, minute_start, minute_end)
        ),
    }

    # Prefer the most specific non-zero interpretation. Ties are harmless for
    # the first fallback minute because minute-local and chunk-local have the
    # same origin there. For later minutes, their numeric ranges are distinct.
    best_score = max(scores.values())
    if best_score <= 0:
        mode = "minute_local"
        app_logger.warning(
            "Blocked chunk %d fallback minute %d timestamp origin is unclear "
            "(minute_local=%d chunk_local=%d absolute=%d); trying minute-local "
            "coordinates and retaining the normal range guard.",
            chunk_number, minute_number, scores["minute_local"],
            scores["chunk_local"], scores["absolute"],
        )
    else:
        candidates = [name for name, score in scores.items() if score == best_score]
        if "absolute" in candidates:
            mode = "absolute"
        elif "chunk_local" in candidates:
            mode = "chunk_local"
        else:
            mode = "minute_local"

    app_logger.info(
        "Blocked chunk %d fallback minute %d timestamp origin: "
        "minute_local=%d chunk_local=%d absolute=%d mode=%s.",
        chunk_number, minute_number, scores["minute_local"],
        scores["chunk_local"], scores["absolute"], mode,
    )

    normalized = []
    rejected = 0
    for start, end, text in descriptions:
        if mode == "minute_local":
            start += minute_start
            end += minute_start
        elif mode == "chunk_local":
            start += chunk_start
            end += chunk_start

        if (
            start < minute_start - tolerance_sec
            or end > minute_end + tolerance_sec
            or end <= start
        ):
            rejected += 1
            app_logger.warning(
                "Blocked chunk %d fallback minute %d rejected out-of-range "
                "timestamp %.3f-%.3fs (expected %.3f-%.3fs, origin=%s, text=%r).",
                chunk_number, minute_number, start, end, minute_start, minute_end,
                mode, (text or "")[:100],
            )
            continue

        start = max(minute_start, start)
        end = min(minute_end, end)
        if end > start:
            normalized.append((start, end, text))

    app_logger.info(
        "Blocked chunk %d fallback minute %d timestamp audit result: "
        "kept=%d rejected=%d.",
        chunk_number, minute_number, len(normalized), rejected,
    )
    return normalized


def _normalize_chunk_timestamps(descriptions, chunk_start, chunk_end, chunk_number,
                                tolerance_sec=2.0, force_mode=None):
    """Resolve absolute-vs-relative Gemini timestamps using the whole chunk.

    The previous implementation inspected only the first description, which
    could misclassify a relative timeline when the first event occurred late in
    a chunk.  Out-of-range results are rejected instead of leaking a badly
    shifted description into the final timeline.
    """
    if not descriptions:
        app_logger.info("Chunk %d timestamp audit: no descriptions.", chunk_number)
        return []

    chunk_duration = max(0.0, chunk_end - chunk_start)

    def _fits_relative(item):
        start, end, _ = item
        return start >= -tolerance_sec and end <= chunk_duration + tolerance_sec

    def _fits_absolute(item):
        start, end, _ = item
        return start >= chunk_start - tolerance_sec and end <= chunk_end + tolerance_sec

    relative_score = sum(1 for item in descriptions if _fits_relative(item))
    absolute_score = sum(1 for item in descriptions if _fits_absolute(item))
    raw_starts = [item[0] for item in descriptions]
    raw_ends = [item[1] for item in descriptions]

    if force_mode in {"relative", "absolute"}:
        mode = force_mode
    elif chunk_start <= tolerance_sec:
        mode = "relative"
    elif absolute_score > relative_score:
        mode = "absolute"
    elif relative_score > absolute_score:
        mode = "relative"
    else:
        # videoMetadata normally yields absolute timestamps. Prefer that on a
        # genuinely ambiguous boundary, but make the decision conspicuous.
        mode = "absolute"
        app_logger.warning(
            "Chunk %d timestamp audit is ambiguous (absolute=%d, relative=%d); "
            "preferring absolute timestamps.",
            chunk_number, absolute_score, relative_score,
        )

    app_logger.info(
        "Chunk %d timestamp audit: raw_count=%d raw_range=%.3f-%.3fs "
        "chunk_range=%.3f-%.3fs absolute_score=%d relative_score=%d mode=%s.",
        chunk_number, len(descriptions), min(raw_starts), max(raw_ends),
        chunk_start, chunk_end, absolute_score, relative_score, mode,
    )

    normalized = []
    rejected = 0
    for start, end, text in descriptions:
        if mode == "relative":
            start += chunk_start
            end += chunk_start
        if (
            start < chunk_start - tolerance_sec
            or end > chunk_end + tolerance_sec
            or end <= start
        ):
            rejected += 1
            app_logger.warning(
                "Chunk %d rejected out-of-range timestamp %.3f-%.3fs "
                "(expected %.3f-%.3fs, mode=%s, text=%r).",
                chunk_number, start, end, chunk_start, chunk_end, mode,
                (text or "")[:100],
            )
            continue
        # Clamp only the small boundary drift covered by the tolerance.
        start = max(chunk_start, start)
        end = min(chunk_end, end)
        if end > start:
            normalized.append((start, end, text))

    if normalized:
        app_logger.info(
            "Chunk %d timestamp audit result: kept=%d rejected=%d final_range=%.3f-%.3fs.",
            chunk_number, len(normalized), rejected,
            normalized[0][0], normalized[-1][1],
        )
    else:
        app_logger.warning(
            "Chunk %d timestamp audit result: kept=0 rejected=%d.",
            chunk_number, rejected,
        )
    return normalized


def _log_timeline_audit(descriptions, total_duration, num_chunks):
    """Record enough information to diagnose future long-video timing bugs."""
    out_of_bounds = 0
    overlaps = 0
    previous_end = -1.0
    for start, end, _description_text in descriptions:
        if start < 0 or end > total_duration + 0.01 or end <= start:
            out_of_bounds += 1
        if start < previous_end:
            overlaps += 1
        previous_end = max(previous_end, end)
    first = descriptions[0][0] if descriptions else None
    last = descriptions[-1][1] if descriptions else None
    app_logger.info(
        "Final chunk timeline audit: chunks=%d descriptions=%d duration=%.3fs "
        "first=%s last=%s overlaps=%d out_of_bounds=%d.",
        num_chunks, len(descriptions), total_duration,
        f"{first:.3f}" if first is not None else "N/A",
        f"{last:.3f}" if last is not None else "N/A",
        overlaps, out_of_bounds,
    )


def _find_large_chunk_gaps(descriptions, chunk_start, chunk_end,
                           minimum_gap_sec=_CHUNK_COVERAGE_GAP_SEC):
    """Return uncovered timeline ranges large enough to indicate an abandoned chunk."""
    gaps = []
    cursor = chunk_start
    for start, end, _text in sorted(descriptions or [], key=lambda item: item[0]):
        start = max(chunk_start, float(start))
        end = min(chunk_end, float(end))
        if start - cursor >= minimum_gap_sec:
            gaps.append((cursor, start))
        cursor = max(cursor, end)
    if chunk_end - cursor >= minimum_gap_sec:
        gaps.append((cursor, chunk_end))
    return gaps


def _description_belongs_to_gaps(description, gaps, tolerance_sec=2.0):
    start, end, _text = description
    midpoint = (start + end) / 2.0
    return any(
        gap_start - tolerance_sec <= midpoint <= gap_end + tolerance_sec
        for gap_start, gap_end in gaps
    )


def _recover_large_chunk_gaps(
    client,
    model_name,
    video_part,
    descriptions,
    chunk_start,
    chunk_end,
    chunk_number,
    video_basename,
    dialogue_free_windows="",
    status_update_callback=None,
    required_gaps=None,
    intensive_mode=False,
    media_uses_local_timeline=False,
    character_continuity_text="",
):
    """Request descriptions specifically for an apparently abandoned chunk region."""
    gaps = (
        list(required_gaps)
        if required_gaps is not None
        else _find_large_chunk_gaps(descriptions, chunk_start, chunk_end)
    )
    if not gaps:
        if intensive_mode:
            app_logger.info(
                "Chunk %d intensive coverage audit: first pass covered every mandatory slot.",
                chunk_number,
            )
        else:
            app_logger.info(
                "Chunk %d coverage audit: no gaps >= %.1fs.",
                chunk_number, _CHUNK_COVERAGE_GAP_SEC,
            )
        return descriptions, None

    media_offset = chunk_start if media_uses_local_timeline else 0.0
    media_chunk_start = chunk_start - media_offset
    media_chunk_end = chunk_end - media_offset
    media_gaps = [
        (start - media_offset, end - media_offset) for start, end in gaps
    ]
    gap_text = ", ".join(
        f"{start:.3f}-{end:.3f}" for start, end in media_gaps
    )
    existing = [
        {
            "start_time_seconds": round(start - media_offset, 3),
            "end_time_seconds": round(end - media_offset, 3),
            "description_text": text,
        }
        for start, end, text in descriptions
    ]
    enable_glossary = bool(
        config_model.get_setting("enable_character_glossary")
    )
    recovery_subject_rule = (
        "When consecutive descriptions keep the same character as their clear subject, use "
        "the character's full name only in the first one; then use a natural pronoun or omit "
        "the subject when the target language allows it. Repeat the name after a scene or "
        "subject change, a long interval, or whenever omission could be ambiguous. "
        if enable_glossary else
        "Do not identify or name characters and do not infer identity across clips. Use concise "
        "generic visual references, with pronouns or omitted subjects only when they remain clear. "
    )
    system_instruction = (
        "You recover missing coverage in long-form audio description. Inspect only the explicitly "
        "listed missing ranges in the attached video. Describe the important visible actions and "
        "scene changes that the prior pass omitted. Before writing each entry, re-inspect the frames "
        "inside that exact missing range. Every subject, object, and action in the description must "
        "be visibly present during the returned start/end interval. Never move or reuse an action "
        "seen earlier or later in the clip merely because it is more interesting. If the range is "
        "static, describe the current visible character, pose, setting, or resulting state. "
        "Never describe the same ongoing action more than "
        "once, even with synonyms, a character alias, or different visual details. Compare every new "
        "entry both with the existing descriptions and with every other entry you are about to return. "
        "For example, after describing someone placing a crown, do not describe that person placing "
        "or setting the crown again. Use only a genuinely new visible development or the resulting "
        "state. " + recovery_subject_rule + _AUDIO_CONTEXT_ONLY_RULE +
        "Return only valid JSON with keys character_glossary (an empty array) "
        "and audio_descriptions. Each audio description must contain start_time_mmss, end_time_mmss, "
        "visual_evidence_time_seconds and description_text, using the attached video's timeline. "
        "The visual evidence value is a JSON number for the exact second where the fact is directly visible "
        "and must fall inside that entry's start/end interval. Use a dot before milliseconds (MM:SS.ms), never a third colon."
    )
    if intensive_mode:
        system_instruction += (
            " INTENSIVE MODE: return exactly one concise visual description for EACH listed "
            "missing range, even when the image is static. Use at most two words per available "
            "second and fit fully within that range. Do not omit a range. When an action was already "
            "described in an earlier range, describe a different useful visual fact or its new result; "
            "never merely rephrase the same action to fill the next range."
        )
    prompt_parts = [
        f"Attached video timeline: {media_chunk_start:.3f}-{media_chunk_end:.3f} seconds.",
        f"Missing ranges requiring another visual pass: {gap_text}.",
        "Existing descriptions (do not repeat): " + json.dumps(existing, ensure_ascii=False),
    ]
    if dialogue_free_windows:
        prompt_parts.append(
            "Authoritative dialogue-free windows: " + dialogue_free_windows
        )
        prompt_parts.append(
            "Every returned description must fit fully inside one of those dialogue-free windows."
        )
    if character_continuity_text:
        prompt_parts.extend([
            "Established named characters from earlier clips: "
            + character_continuity_text,
            "Reuse an established name only when the visible person's identity and stable "
            "physical appearance match; otherwise use a generic label.",
        ])
    prompt_parts.append(
        'Return {"character_glossary":[],"audio_descriptions":[...]} and nothing else.'
    )

    app_logger.warning(
        "Chunk %d coverage audit found %d large gap(s): %s. Requesting recovery pass.",
        chunk_number, len(gaps), gap_text,
    )
    if status_update_callback:
        status_update_callback(
            _("Chunk %d: recovering descriptions from a large uncovered section…")
            % chunk_number
        )
    try:
        config_obj = gemini.build_generation_config(
            system_instruction_text=system_instruction,
            is_json_response=True,
            enable_thinking=True,
        )
        response = gemini.generate_content_with_retry(
            client,
            model=model_name,
            contents=["\n".join(prompt_parts), video_part],
            config=config_obj,
            status_callback=status_update_callback,
        )
        gemini.save_raw_ai_output(
            video_basename,
            "chunk_gap_recovery_response",
            response,
            suffix=f"_chunk{chunk_number}",
        )
        usage = gemini.log_token_usage(f"Chunk_{chunk_number}_Gap_Recovery", response)
        response_text, success = gemini.process_gemini_response(
            response, status_update_callback
        )
        if not success or not response_text:
            raise ValueError("Gemini gap recovery returned no usable text")
        recovered_raw, _glossary, parse_ok = _parse_unified_response(
            response_text, status_update_callback
        )
        recovery_visual_evidence = list(_LAST_PARSED_VISUAL_EVIDENCE_RAW)
        if not parse_ok:
            raise ValueError("Gemini gap recovery returned invalid JSON")
        recovered = _post_process_mmss_timestamps(
            recovered_raw,
            status_update_callback,
            local_timeline_window=(media_chunk_start, media_chunk_end)
            if media_uses_local_timeline else None,
        )
        recovered = _normalize_chunk_timestamps(
            recovered, media_chunk_start, media_chunk_end, chunk_number,
            force_mode="relative" if media_uses_local_timeline else None,
        )
        if media_offset:
            recovered = [
                (start + media_offset, end + media_offset, text)
                for start, end, text in recovered
            ]
        _register_visual_evidence_records(
            _retime_visual_evidence_records(
                recovery_visual_evidence,
                recovered,
                timeline_offset_sec=media_offset,
            ),
            timeline_offset_sec=media_offset,
        )
        # Gap recovery is a separate Gemini response and can occasionally
        # switch language even when the main chunk was correct. Check it before
        # combining it with the much larger Italian result set.
        recovered, language_usage = _correct_description_language(
            client, model_name, recovered, status_update_callback
        )
        usage = _merge_token_usage(usage, language_usage)
        recovered = [
            item for item in recovered if _description_belongs_to_gaps(item, gaps)
        ]
        combined = sorted(descriptions + recovered, key=lambda item: item[0])
        app_logger.info(
            "Chunk %d coverage recovery result: original=%d recovered=%d combined=%d gaps=%s.",
            chunk_number, len(descriptions), len(recovered), len(combined), gap_text,
        )
        return combined, usage
    except Exception as exc:
        app_logger.warning(
            "Chunk %d coverage recovery failed; keeping existing descriptions: %s",
            chunk_number, exc, exc_info=True,
        )
        if status_update_callback:
            status_update_callback(
                _("Chunk %d: missing-section recovery failed; keeping existing descriptions.")
                % chunk_number
            )
        return descriptions, None


def _mmss_to_total_seconds(mmss_string):
    """Convert Gemini timestamps without guessing across incompatible timelines.

    The requested format is MM:SS[.ms], but Gemini occasionally emits the
    millisecond separator as a colon (for example ``01:13:473``).  Accept that
    specific, mechanically reversible typo while keeping the downstream chunk
    audit responsible for deciding whether the resulting timestamp belongs to
    the current video slice.

    Standard HH:MM:SS[.ms] is also accepted for completeness.  A three-part
    timestamp whose final component is exactly three decimal digits is treated
    as MM:SS:ms, because that is the malformed variant observed from Gemini;
    otherwise three parts are interpreted as HH:MM:SS[.ms].
    """
    if not isinstance(mmss_string, str):
        app_logger.warning("Invalid MM:SS string format for conversion: '%s'" % mmss_string)
        raise ValueError(_("Invalid MM:SS string format: %s") % mmss_string)

    value = mmss_string.strip()
    colon_count = value.count(':')
    if colon_count == 0:
        try:
            total_seconds = float(value.replace(',', '.'))
            if total_seconds < 0:
                raise ValueError(_("Calculated total seconds is negative."))
            return total_seconds
        except (ValueError, TypeError):
            app_logger.warning("Invalid MM:SS string format for conversion: '%s'" % mmss_string)
            raise ValueError(_("Invalid MM:SS string format: %s") % mmss_string)

    try:
        if colon_count == 1:
            minutes_str, seconds_str = value.split(':', 1)
            minutes = int(minutes_str)
            seconds = float(seconds_str.replace(',', '.'))
            total_seconds = float(minutes * 60 + seconds)
        elif colon_count == 2:
            first_str, second_str, third_str = value.split(':')
            if re.fullmatch(r"\d{3}", third_str):
                # Gemini typo: MM:SS:ms -> MM:SS.ms.  This preserves the
                # numeric timestamp exactly; no temporal snapping occurs here.
                minutes = int(first_str)
                seconds = int(second_str)
                milliseconds = int(third_str)
                if not 0 <= seconds < 60:
                    raise ValueError(_("Seconds component is outside 0-59."))
                total_seconds = float(minutes * 60 + seconds + milliseconds / 1000.0)
                app_logger.info(
                    "Normalized Gemini MM:SS:ms timestamp '%s' to %.3fs.",
                    mmss_string, total_seconds,
                )
            else:
                # Unambiguous standard HH:MM:SS[.ms].
                hours = int(first_str)
                minutes = int(second_str)
                seconds = float(third_str.replace(',', '.'))
                if not 0 <= minutes < 60 or not 0 <= seconds < 60:
                    raise ValueError(_("HH:MM:SS component is outside its valid range."))
                total_seconds = float(hours * 3600 + minutes * 60 + seconds)
        elif colon_count == 3:
            # Also tolerate HH:MM:SS:ms when Gemini applies the same typo to
            # an hour-bearing timestamp.  Require exactly three ms digits so
            # malformed extra fields are not silently guessed.
            hours_str, minutes_str, seconds_str, milliseconds_str = value.split(':')
            if not re.fullmatch(r"\d{3}", milliseconds_str):
                raise ValueError(_("Millisecond component must contain exactly three digits."))
            hours = int(hours_str)
            minutes = int(minutes_str)
            seconds = int(seconds_str)
            milliseconds = int(milliseconds_str)
            if not 0 <= minutes < 60 or not 0 <= seconds < 60:
                raise ValueError(_("HH:MM:SS component is outside its valid range."))
            total_seconds = float(
                hours * 3600 + minutes * 60 + seconds + milliseconds / 1000.0
            )
            app_logger.info(
                "Normalized Gemini HH:MM:SS:ms timestamp '%s' to %.3fs.",
                mmss_string, total_seconds,
            )
        else:
            raise ValueError(_("Unexpected number of timestamp separators."))

        if total_seconds < 0:
            raise ValueError(_("Calculated total seconds is negative."))
        return total_seconds
    except (ValueError, TypeError) as e:
        app_logger.error("Error parsing MM:SS components in '%s': %s" % (mmss_string, e))
        raise ValueError(_("Invalid MM:SS components in '%s': %s") % (mmss_string, e))

def _mmss_to_total_seconds_in_local_window(value, window_start, window_end):
    """Parse a timestamp, repairing one contextually unambiguous Gemini typo.

    ``MM:SS:ff`` is ambiguous with valid ``HH:MM:SS`` and therefore must not
    be accepted by the global parser. Gap recovery over an extracted chunk,
    however, has an authoritative local timeline. If the normal hour-based
    interpretation is outside that timeline while the fractional
    interpretation is inside it, the repair is mechanically safe.
    """
    parsed = _mmss_to_total_seconds(value)
    start = float(window_start)
    end = float(window_end)
    tolerance_sec = 2.0
    if start - tolerance_sec <= parsed <= end + tolerance_sec:
        return parsed

    match = re.fullmatch(r"\s*(\d+):([0-5]\d):(\d{1,2})\s*", str(value))
    if match is None:
        return parsed

    minutes = int(match.group(1))
    seconds = int(match.group(2))
    fraction_text = match.group(3)
    repaired = float(
        minutes * 60
        + seconds
        + int(fraction_text) / (10 ** len(fraction_text))
    )
    if not start - tolerance_sec <= repaired <= end + tolerance_sec:
        return parsed

    app_logger.warning(
        "Normalized contextually unambiguous Gemini MM:SS:fraction timestamp "
        "'%s' from %.3fs to %.3fs for local recovery window %.3f-%.3fs.",
        value, parsed, repaired, start, end,
    )
    return repaired


def _post_process_mmss_timestamps(
    descriptions_list_raw_times,
    status_update_callback=None,
    local_timeline_window=None,
):
    if not descriptions_list_raw_times:
        return []
    def _log_status_local(message):
        if status_update_callback:
            status_update_callback(message)
    parsed_descriptions = []
    for original_index, (start_mmss_str, end_mmss_str, text) in enumerate(
        descriptions_list_raw_times
    ):
        try:
            if local_timeline_window is None:
                current_start_sec = _mmss_to_total_seconds(start_mmss_str)
                current_end_sec = _mmss_to_total_seconds(end_mmss_str)
            else:
                window_start, window_end = local_timeline_window
                current_start_sec = _mmss_to_total_seconds_in_local_window(
                    start_mmss_str, window_start, window_end
                )
                current_end_sec = _mmss_to_total_seconds_in_local_window(
                    end_mmss_str, window_start, window_end
                )
        except ValueError:
            _log_status_local(_("Skipping description due to invalid MM:SS format."))
            continue
        parsed_descriptions.append(
            (current_start_sec, current_end_sec, original_index, text)
        )

    original_order = [item[2] for item in parsed_descriptions]
    parsed_descriptions.sort(key=lambda item: (item[0], item[1], item[2]))
    reordered_count = sum(
        before != after
        for before, after in zip(
            original_order, (item[2] for item in parsed_descriptions)
        )
    )
    if reordered_count:
        app_logger.warning(
            "Timestamp post-processing restored chronological order for "
            "%d of %d descriptions before overlap handling.",
            reordered_count, len(parsed_descriptions),
        )

    corrected_descriptions = []
    last_corrected_end_time_sec = 0.0
    num_adjustments_made = 0
    preserved_overlaps = 0
    for current_start_sec, current_end_sec, _original_index, text in parsed_descriptions:
        made_adjustment_this_iteration = False
        adjusted_start_sec = current_start_sec
        adjusted_end_sec = current_end_sec
        if corrected_descriptions and adjusted_start_sec < last_corrected_end_time_sec:
            # Preserve Gemini's visual timestamp.  Overlap is a scheduling
            # conflict, not evidence that the later visual event happened after
            # the previous narration.  The silence schedulers resolve the
            # collision later without rewriting the visual origin.
            preserved_overlaps += 1
        if adjusted_end_sec <= adjusted_start_sec:
            original_duration_from_ai = current_end_sec - current_start_sec
            duration_to_add = max(0.1, original_duration_from_ai if original_duration_from_ai > 0 else 0.1)
            adjusted_end_sec = adjusted_start_sec + duration_to_add
            _log_status_local(_("Ensuring minimum description duration..."))
            made_adjustment_this_iteration = True
        if made_adjustment_this_iteration:
            num_adjustments_made += 1
        corrected_descriptions.append((adjusted_start_sec, adjusted_end_sec, text))
        last_corrected_end_time_sec = max(last_corrected_end_time_sec, adjusted_end_sec)
    if preserved_overlaps:
        app_logger.warning(
            "Timestamp post-processing preserved %d overlapping Gemini timestamp(s) "
            "for downstream silence scheduling instead of shifting their visual timing.",
            preserved_overlaps,
        )
    if status_update_callback:
        _log_status_local(_("Timestamp correction complete. %(count)d descriptions processed, %(adjusted_count)d had timestamps adjusted.") % {'count': len(corrected_descriptions), 'adjusted_count': num_adjustments_made})
    return corrected_descriptions

def _remove_consecutive_duplicates(descriptions_list, status_update_callback=None):
    if not descriptions_list:
        return []
    cleaned_list = [descriptions_list[0]]
    duplicates_removed = 0
    for i in range(1, len(descriptions_list)):
        if descriptions_list[i][2].strip() != cleaned_list[-1][2].strip():
            cleaned_list.append(descriptions_list[i])
        else:
            duplicates_removed += 1
    if duplicates_removed > 0 and status_update_callback:
        status_update_callback(_("Removed %(count)d repetitive descriptions.") % {'count': duplicates_removed})
    return cleaned_list


def _character_names_from_glossary(character_glossary):
    """Return unique usable character names, longest first."""
    names = {}
    for item in character_glossary or []:
        if not isinstance(item, dict):
            continue
        name = " ".join(str(item.get("name") or "").split()).strip(" ,.;:")
        if len(name) < 2 or not any(char.isalpha() for char in name):
            continue
        names.setdefault(name.casefold(), name)
    return sorted(names.values(), key=len, reverse=True)


def _suppress_repeated_leading_character_names(
    descriptions_list,
    character_glossary,
    status_update_callback=None,
    max_gap_sec=_REPEATED_SUBJECT_NAME_MAX_GAP_SEC,
):
    """Remove only redundant leading character names from a continuous run.

    The first explicit subject is preserved. Later adjacent descriptions may
    omit that same leading name, while object mentions ("looks at Janet"),
    coordinated subjects ("Janet and Mark"), different subjects, and names
    after a long gap remain untouched.
    """
    names = _character_names_from_glossary(character_glossary)
    if not descriptions_list or not names:
        return list(descriptions_list or [])

    patterns = [
        (
            name,
            re.compile(rf"^({re.escape(name)})(?=\s|,)", re.IGNORECASE),
        )
        for name in names
    ]
    cleaned = []
    active_name = None
    previous_end = None
    suppressed = 0

    for start, end, original_text in descriptions_list:
        text = str(original_text or "").strip()
        leading_name = None
        match = None
        for name, pattern in patterns:
            candidate = pattern.match(text)
            if candidate:
                leading_name = name
                match = candidate
                break

        close_to_previous = (
            previous_end is not None
            and float(start) - float(previous_end) <= float(max_gap_sec)
        )
        replacement = text
        if leading_name is not None and match is not None:
            remainder = text[match.end():].lstrip()
            remainder_without_comma = remainder.lstrip(", ")
            lowered_remainder = remainder_without_comma.casefold()
            coordinated_subject = lowered_remainder.startswith(
                ("e ", "ed ", "and ", "& ", "con ", "with ")
            )
            if (
                active_name == leading_name.casefold()
                and close_to_previous
                and remainder_without_comma
                and not coordinated_subject
            ):
                replacement = (
                    remainder_without_comma[0].upper()
                    + remainder_without_comma[1:]
                )
                suppressed += 1
            # A coordinated subject is not a continuation of the same single
            # subject, so require the next description to name it explicitly.
            active_name = None if coordinated_subject else leading_name.casefold()
        else:
            # An intervening description with another or implicit subject ends
            # the explicit-name run; the next occurrence restores clarity.
            active_name = None

        cleaned.append((start, end, replacement))
        previous_end = end

    if suppressed:
        app_logger.info(
            "Repeated leading character-name filter suppressed %d occurrence(s).",
            suppressed,
        )
        if status_update_callback:
            status_update_callback(
                _("Removed %(count)d repeated character names.")
                % {"count": suppressed}
            )
    return cleaned

def _extract_descriptions_and_glossary_from_dict(data, status_update_callback):
    """Extracts descriptions and glossary from a parsed JSON dict."""
    if bool(config_model.get_setting("recognize_screen_text")):
        # Diagnostic metadata only: never creates narration or changes its timing.
        screen_text = data.get("on_screen_text")
        if isinstance(screen_text, list):
            app_logger.info("Gemini on-screen text audit (response timeline): %s",
                            json.dumps(screen_text, ensure_ascii=False))
        else:
            app_logger.info("Gemini on-screen text audit: missing or invalid metadata")
    descriptions_raw = data.get("audio_descriptions", [])
    descriptions = []
    if isinstance(descriptions_raw, list):
        for item in descriptions_raw:
            if isinstance(item, dict):
                _capture_visual_evidence_item(item)
                start = item.get("start_time_mmss")
                end = item.get("end_time_mmss")
                text = item.get("description_text")
                if start is not None and end is not None and text is not None:
                    descriptions.append((str(start), str(end), str(text).strip()))
    else:
        if status_update_callback:
            status_update_callback(_("Warning: 'audio_descriptions' key was not a list."))

    glossary = data.get("character_glossary", [])
    if not isinstance(glossary, list):
        if status_update_callback:
            status_update_callback(_("Warning: 'character_glossary' key was not a list."))
        glossary = []

    return descriptions, glossary


def _salvage_partial_unified_json(processed_str, status_update_callback):
    """Salvage complete description/glossary objects from truncated MAX_TOKENS JSON.

    When the model hits the token limit mid-string, json.loads fails. We still
    recover every complete {...} object via JSONDecoder.raw_decode.
    """
    decoder = json.JSONDecoder()
    glossary = []
    descriptions = []
    pos = 0
    while pos < len(processed_str):
        start = processed_str.find("{", pos)
        if start == -1:
            break
        try:
            obj, end_rel = decoder.raw_decode(processed_str[start:])
            pos = start + end_rel
        except json.JSONDecodeError:
            pos = start + 1
            continue
        if not isinstance(obj, dict):
            continue
        if "audio_descriptions" in obj or "character_glossary" in obj:
            partial_desc, partial_gloss = _extract_descriptions_and_glossary_from_dict(
                obj, status_update_callback
            )
            descriptions.extend(partial_desc)
            glossary.extend(partial_gloss)
            continue
        # Standalone timed description object
        start_t = obj.get("start_time_mmss")
        end_t = obj.get("end_time_mmss")
        text = obj.get("description_text")
        if start_t is not None and end_t is not None and text is not None:
            _capture_visual_evidence_item(obj)
            descriptions.append((str(start_t), str(end_t), str(text).strip()))
        elif "id" in obj and "description" in obj and "description_text" not in obj:
            glossary.append(obj)

    if descriptions or glossary:
        msg = _(
            "Recovered %(d)d descriptions and %(g)d glossary entries from truncated/malformed JSON."
        ) % {"d": len(descriptions), "g": len(glossary)}
        if status_update_callback:
            status_update_callback(msg)
        app_logger.warning(msg)
    return descriptions, glossary


def _strip_json_fences(text):
    processed = (text or "").strip()
    if processed.startswith("```json"):
        processed = processed[7:].strip()
    elif processed.startswith("```"):
        processed = processed[3:].strip()
    if processed.endswith("```"):
        processed = processed[:-3].strip()
    return processed


def _is_valid_unified_json(text):
    """True if text is parseable JSON object with expected keys (or at least an object)."""
    try:
        data = json.loads(_strip_json_fences(text))
        return isinstance(data, dict)
    except (json.JSONDecodeError, TypeError):
        return False


def _parse_unified_response(json_string, status_update_callback):
    """Parses the combined JSON response for descriptions and glossary.

    Returns (descriptions, glossary, parse_ok) where parse_ok is True only if
    the full document was valid JSON (not merely salvaged fragments).
    """
    _LAST_PARSED_VISUAL_EVIDENCE_RAW.clear()
    if not json_string:
        return [], [], False

    processed_str = _strip_json_fences(json_string)

    try:
        data = json.loads(processed_str)
        descs, gloss = _extract_descriptions_and_glossary_from_dict(data, status_update_callback)
        return descs, gloss, True

    except json.JSONDecodeError as e:
        if status_update_callback:
            status_update_callback(_("Error: Could not decode the AI's JSON response: %s") % str(e))
        app_logger.error(f"Failed to parse unified JSON response: {e}", exc_info=True)
        # Attempt to find a JSON object within the string if the initial parse fails
        try:
            start_idx = processed_str.find('{')
            end_idx = processed_str.rfind('}') + 1
            if 0 <= start_idx < end_idx:
                corrected_json_string = processed_str[start_idx:end_idx]
                data = json.loads(corrected_json_string)
                if status_update_callback:
                    status_update_callback(_("Successfully parsed a fallback JSON object."))
                descs, gloss = _extract_descriptions_and_glossary_from_dict(
                    data, status_update_callback
                )
                return descs, gloss, True
        except json.JSONDecodeError:
            if status_update_callback:
                status_update_callback(_("Fallback JSON parsing also failed."))

        # MAX_TOKENS / truncated string: salvage every complete object
        salvaged_desc, salvaged_gloss = _salvage_partial_unified_json(
            processed_str, status_update_callback
        )
        if salvaged_desc or salvaged_gloss:
            return salvaged_desc, salvaged_gloss, False

    return [], [], False


def _request_json_repair_from_gemini(
    client,
    model_name,
    video_part,
    broken_text,
    parse_error_hint,
    status_update_callback=None,
):
    """Ask Gemini to re-emit valid unified JSON from a broken/truncated fragment.

    Uses the video part for context and disables thinking to maximize room for
    a complete JSON answer.
    """
    def _status(msg):
        if status_update_callback:
            status_update_callback(msg)
        app_logger.info("JSONRepair: %s", msg)

    fragment = broken_text or ""
    if len(fragment) > _JSON_REPAIR_FRAGMENT_MAX_CHARS:
        fragment = fragment[:_JSON_REPAIR_FRAGMENT_MAX_CHARS] + "\n…[truncated for repair prompt]…"

    enable_glossary = bool(
        config_model.get_setting("enable_character_glossary")
    )
    glossary_repair_rule = (
        '1. "character_glossary": array of objects { "id", "description", "name" } '
        '(name may be null).\n'
        if enable_glossary else
        '1. "character_glossary": an empty array. Do not identify or name characters.\n'
    )
    screen_text_repair_rule = (
        '3. "on_screen_text": array of objects { "text", "visual_evidence_time_seconds", '
        '"narratively_relevant" }. Preserve complete entries from the fragment, or use [] '
        'if unavailable. This is metadata only; do not turn it into extra narration.\n'
        if bool(config_model.get_setting("recognize_screen_text")) else ""
    )
    system_instruction = (
        "You are a JSON repair assistant for an audio-description app.\n"
        "Output ONLY one valid JSON object (no markdown fences, no commentary) with exactly these keys:\n"
        + glossary_repair_rule
        + '2. "audio_descriptions": array of objects { "start_time_mmss", "end_time_mmss", '
        '"visual_evidence_time_seconds", "description_text" } using MM:SS or MM:SS.ms times. '
        'Preserve the exact numeric visual_evidence_time_seconds value for every recovered description.\n'
        + screen_text_repair_rule +
        "Rules:\n"
        "- The JSON MUST parse with a standard JSON parser (closed braces/brackets, escaped quotes).\n"
        "- If the previous output was truncated, keep every complete description you can recover "
        "and close the document cleanly.\n"
        "- You may use the attached video to fill gaps or continue from the last good timestamp.\n"
        "- Prefer fewer valid entries over inventing broken structure.\n"
        "- " + _AUDIO_CONTEXT_ONLY_RULE + "\n" +
        "- Do not wrap the answer in ``` fences."
    )
    user_prompt = (
        "The previous model output was invalid or truncated and could not be parsed.\n"
        f"Parser note: {parse_error_hint or 'malformed or incomplete JSON'}\n\n"
        "Broken output to repair:\n"
        "----- BEGIN BROKEN OUTPUT -----\n"
        f"{fragment}\n"
        "----- END BROKEN OUTPUT -----\n\n"
        "Re-emit ONE complete, valid JSON object following the schema. "
        "Use the video if you need to complete missing later timestamps."
    )

    _status(_("Invalid JSON from AI — asking Gemini to reformat it correctly…"))
    # No thinking: repair must spend tokens on the JSON body, not internal monologue.
    gen_config = gemini.build_generation_config(
        system_instruction_text=system_instruction,
        is_json_response=True,
        enable_thinking=False,
    )
    api_contents = [user_prompt, video_part]
    response = gemini.generate_content_with_retry(
        client,
        model=model_name,
        contents=api_contents,
        config=gen_config,
        status_callback=status_update_callback,
    )
    gemini.save_raw_ai_output("json_repair", "unified_raw_response", response, suffix="_repair")
    gemini.log_token_usage("JSON_Repair", response)
    repaired_text, ok = gemini.process_gemini_response(response, status_update_callback)
    if not ok or not repaired_text:
        _status(_("JSON repair request returned no usable text."))
        return [], [], False

    descs, gloss, parse_ok = _parse_unified_response(repaired_text, status_update_callback)
    if parse_ok or descs:
        _status(
            _("JSON repair recovered %(d)d descriptions and %(g)d glossary entries.")
            % {"d": len(descs), "g": len(gloss)}
        )
    else:
        _status(_("JSON repair still could not produce usable descriptions."))
    return descs, gloss, parse_ok


def _parse_with_optional_json_repair(
    client,
    model_name,
    video_part,
    raw_json_text,
    response,
    status_update_callback=None,
):
    """Parse unified JSON; if invalid/truncated, salvage then re-ask Gemini to reformat."""
    def _status(msg):
        if status_update_callback:
            status_update_callback(msg)
        app_logger.info("Describer: %s", msg)

    finish = gemini.get_response_finish_reason(response)
    if finish:
        _status(_("Parsing AI output (finish reason: %s)…") % finish)

    descs, gloss, parse_ok = _parse_unified_response(raw_json_text, status_update_callback)

    needs_repair = (not parse_ok) or (finish == "MAX_TOKENS" and not parse_ok)
    # Also repair when we got zero descriptions despite non-empty text
    if not descs and raw_json_text and raw_json_text.strip():
        needs_repair = True

    if not needs_repair:
        return descs, gloss

    _status(
        _("JSON incomplete or invalid (finish=%(fr)s). Salvaged %(d)d descriptions so far; "
          "requesting a well-formed rewrite from Gemini…")
        % {"fr": finish or "?", "d": len(descs)}
    )

    best_descs, best_gloss = descs, gloss
    for attempt in range(1, _JSON_REPAIR_MAX_ATTEMPTS + 1):
        _status(_("JSON repair attempt %d of %d…") % (attempt, _JSON_REPAIR_MAX_ATTEMPTS))
        try:
            r_descs, r_gloss, r_ok = _request_json_repair_from_gemini(
                client,
                model_name,
                video_part,
                raw_json_text,
                parse_error_hint=f"finish_reason={finish or 'n/a'}; parse_ok={parse_ok}",
                status_update_callback=status_update_callback,
            )
        except Exception as e:
            app_logger.error("JSON repair attempt failed: %s", e, exc_info=True)
            _status(_("JSON repair attempt failed: %s") % e)
            break

        # Keep the better result
        if len(r_descs) > len(best_descs) or (r_ok and r_descs):
            best_descs, best_gloss = r_descs, r_gloss or best_gloss
        if r_ok and r_descs:
            break
        # Feed the repair output back if still broken
        if r_descs or r_gloss:
            # rebuild a fragment from salvaged for next attempt is overkill; stop after max
            pass

    if best_descs:
        _status(
            _("Using %(d)d descriptions after parse/repair (glossary: %(g)d).")
            % {"d": len(best_descs), "g": len(best_gloss)}
        )
    else:
        _status(_("Could not recover descriptions even after JSON repair."))

    return best_descs, best_gloss

def parse_gemini_json_response_mmss(json_string):
    if not json_string:
        return []
    processed_str = json_string.strip()
    if processed_str.startswith("```json"):
        processed_str = processed_str[7:].strip()
    if processed_str.endswith("```"):
        processed_str = processed_str[:-3].strip()
    start_index = processed_str.find('[')
    if start_index == -1:
        app_logger.error("Could not find start of JSON array '[' in the response.")
        return []
    processed_str = processed_str[start_index:]
    decoder = json.JSONDecoder()
    descriptions_raw_times = []
    pos = 0
    while pos < len(processed_str):
        obj_start = processed_str.find('{', pos)
        if obj_start == -1:
            break
        try:
            obj, end_pos = decoder.raw_decode(processed_str[obj_start:])
            if isinstance(obj, dict):
                start_mmss = obj.get("start_time_mmss")
                end_mmss = obj.get("end_time_mmss")
                desc_text = obj.get("description_text")
                if start_mmss is not None and end_mmss is not None and desc_text is not None:
                    descriptions_raw_times.append((str(start_mmss), str(end_mmss), str(desc_text).strip()))
            pos = obj_start + end_pos
        except json.JSONDecodeError:
            break
    return descriptions_raw_times


def _build_unified_prompts(user_prompt, model_name_to_use, dialogue_free_windows="",
                           intensive_slots_text="", intensive_mode=False,
                           character_continuity_text="",
                           recent_descriptions_text="",
                           extended_anchors_text="", extended_mode=False):
    """Builds the system and user prompts for the unified generation API call."""
    _target_language_code, target_language_name, language_examples = (
        _target_language_details()
    )
    description_example, glossary_example = language_examples

    enable_glossary = bool(
        config_model.get_setting("enable_character_glossary")
    )
    if enable_glossary:
        character_name_directive = (
            "3.  **USE NAMES AND ESTABLISHED IDENTITIES ACCURATELY:** A saved series "
            "catalog is authoritative. Reuse an established catalog name and ID when the "
            "person is confidently recognized, even if that name is not spoken again in this "
            "clip. For a genuinely new character, use a proper name only after it is clearly "
            "revealed in dialogue. Do not invent names or duplicate an established identity."
        )
        subject_repetition_directive = (
            "6.  **AVOID REPEATING THE SUBJECT'S NAME:** When immediately consecutive "
            "descriptions clearly keep the same character as their subject, write the full "
            "name only in the first description. Then use a natural pronoun or omit the subject "
            "when the target language allows it. Repeat the name after a scene or subject change, "
            "a long gap, or whenever omission could be unclear."
        )
        system_mission = (
            "You are an expert Audio Describer. Analyze the provided video and generate "
            "a character glossary plus timed audio descriptions in one JSON object."
        )
        glossary_schema = f"""1.  **"character_glossary":** An array of objects, where each object represents a distinct character. Each character object must contain:
    *   `"id"`: The permanent identity key. If the character matches an established saved-catalog entry, return EXACTLY that existing ID, character-for-character. Never shorten it, regenerate it from the visible/spoken name, or create an alias ID. Only a genuinely new character may receive a new unique descriptive ID (e.g., "man_in_red_shirt").
    *   `"description"`: For a genuinely new character, provide a definitive physical description in {target_language_name}. For an ESTABLISHED catalog character, do NOT rewrite, summarize, correct, or paraphrase the saved biography. Return only genuinely NEW visual appearance information observed in this clip (for example a new outfit, hat, bandage, glasses, injury, or other distinguishing visible detail). If there is no genuinely new visual fact, repeat the established description exactly rather than inventing a variation. Never add or alter relationships, names, ages, roles, backstory, or other non-visual facts for an established character.
    *   `"name"`: For an established character, keep the established catalog name. For a genuinely new character, use a proper name only if it is spoken clearly in the video; otherwise use a concise visual label."""
        example_glossary_json = json.dumps(
            [{"id": "man_in_suit", "description": glossary_example, "name": "David"}],
            ensure_ascii=False,
            separators=(",", ":"),
        )
    else:
        character_name_directive = (
            "3.  **DO NOT IDENTIFY CHARACTERS BY NAME:** Do not build a character glossary, "
            "do not infer identity across clips, and use concise generic visual references "
            "such as the man, the woman, the child, or the driver."
        )
        subject_repetition_directive = (
            "6.  **AVOID REPETITIVE SUBJECT LABELS:** When consecutive descriptions clearly "
            "keep the same person as their subject, use a pronoun or omit the subject when this "
            "remains clear. Do not introduce a name or infer identity across clips."
        )
        system_mission = (
            "You are an expert Audio Describer. Analyze the provided video and generate "
            "timed audio descriptions only. Character recognition and glossary creation are disabled."
        )
        glossary_schema = (
            '1.  **"character_glossary":** Always return an empty array. Do not identify, '
            'catalogue, or name characters.'
        )
        example_glossary_json = "[]"

    # Get verbosity setting and convert to meaningful instruction
    verbosity_setting = config_model.get_setting('gemini_description_verbosity')
    verbosity_instructions = {
        config.VERBOSITY_SHORT: "Keep descriptions extremely brief (1-3 words maximum). Only describe the most critical visual elements that are essential for understanding the scene.",
        config.VERBOSITY_STANDARD: "Provide balanced descriptions (3-6 words). Focus on important visual information without overwhelming detail. This is the recommended setting.",
        config.VERBOSITY_DETAILED: "Provide rich, detailed descriptions (6-12 words). Include important visual context, emotions, scene details, and atmospheric elements that enhance understanding."
    }
    verbosity_instruction = verbosity_instructions.get(verbosity_setting, verbosity_instructions[config.VERBOSITY_STANDARD])
    app_logger.info(f"Using verbosity setting: {verbosity_setting} -> {verbosity_instruction[:50]}...")

    # Core directives remain largely the same, but the output format instruction is new.
    intensive_mode = bool(intensive_mode or intensive_slots_text)
    selection_directive = (
        "2.  **FILL EVERY USABLE SILENCE:** This is intensive mode. Produce at least one "
        "description for every numbered mandatory slot supplied by the user. Temporal grounding "
        "has absolute priority over choosing an interesting action. Do not omit a slot, even if "
        "the view is static. If no action can be confirmed inside that exact slot, describe only "
        "what is visibly present there: a logo, title card, setting, framing, stationary character "
        "or object, or the current resulting state. A static or mundane description that is correct "
        "for the slot is always preferable to an interesting action seen even a few seconds before "
        "or after it. A mandatory slot is a container, not a license to shift an action in time: "
        "choose the exact sub-range where the described action, pose, or state is actually visible. "
        "If an action has already ended earlier in the same slot, do not describe it later in that "
        "slot; describe what is visible at the chosen timestamp instead. You may add more descriptions "
        "inside the same slot when distinct, useful visual changes occur and there is enough time. "
        "Keep entries chronological, non-overlapping, and keep their combined words within the slot's "
        "word budget."
        if intensive_mode else
        "2.  **BE SELECTIVE AND CONCISE (2 WORDS/SECOND RULE):** Describe only NEW and "
        "PLOT-CRITICAL visual information. A 3-second description can have a maximum of 6 words."
    )
    grounding_directive = (
        "7.  **GROUND EVERY DESCRIPTION IN THE CORRECT VISUAL MOMENT:** For normal and mandatory "
        "descriptions, re-inspect only the frames inside the chosen start/end interval and describe "
        "only what is actually visible there. For a mandatory slot, the returned start/end must "
        "coincide with the moment the described visual fact is visible; never place an already-finished "
        "action later in the same slot merely because there is more narration room there. For an "
        "OPTIONAL INTENSIVE SHORT-GAP anchor, the "
        "timestamp marks the PAUSE point, so use the anchor's explicitly listed IMMEDIATE_SCENE "
        "window instead: describe only the scene that begins directly after that pause. Never use "
        "an extended pause as storage for an action from farther ahead in the clip. Earlier or later "
        "scenes are context only, not evidence for the current description."
        if intensive_mode and extended_anchors_text else
        "7.  **GROUND EVERY DESCRIPTION IN ITS EXACT TIME RANGE:** Before writing each entry, "
        "re-inspect the frames inside its chosen start/end interval. Every described character, "
        "object, pose, and action must actually be visible during that same interval. When the entry "
        "belongs to a mandatory slot, the returned start/end must coincide with the moment the visual "
        "fact is visible; the slot boundaries do not make earlier and later moments interchangeable. "
        "Never borrow, move, or repeat an action seen earlier or later in the clip just to fill an available "
        "silence. Earlier descriptions are context, not evidence for the current image. If nothing "
        "changes, describe the current visible character, pose, setting, or resulting state instead "
        "of recalling a previous action."
    )
    core_directives = f"""
**CORE DIRECTIVES (Apply to `audio_descriptions`):**
1.  **DO NOT OVERLAP DIALOGUE:** The most critical rule. Never describe over spoken dialogue. Omit the visual information if there is no sufficiently long dialogue-free window.
{selection_directive}
{character_name_directive}
4.  **USE AUDIO ONLY AS PRIVATE CONTEXT — NEVER AS DESCRIPTION CONTENT:** {_AUDIO_CONTEXT_ONLY_RULE}
5.  **NEVER REPEAT AN ACTION:** Before returning the timeline, compare every description with all
    earlier descriptions in this response. Do not narrate the same continuing action twice by using
    synonyms, character aliases, or extra details. For example, after describing someone placing a
    crown, do not later say that the person sets the crown on the ruler's head. Describe only a truly
    new visual development or the resulting state.
{subject_repetition_directive}
{grounding_directive}
8.  **REPORT THE EXACT VISUAL-EVIDENCE INSTANT:** For every description, set
    `visual_evidence_time_seconds` to the precise video second at which the described visual fact
    is directly visible. Re-inspect that exact instant before returning it. This is not a guessed
    narration time and not the start of the silence: it is the evidence frame for the sentence.
    It MUST use the same local/absolute timeline requested for the description and MUST fall inside
    that description's returned start/end interval. If the event is already over at the candidate
    time, do not reuse it; choose a fact that is actually visible there instead.
"""

    output_keys = 'two top-level keys: "character_glossary" and "audio_descriptions"'
    screen_text_schema = ""
    screen_text_example = ""
    if bool(config_model.get_setting("recognize_screen_text")):
        output_keys = 'three top-level keys: "character_glossary", "audio_descriptions", and "on_screen_text"'
        screen_text_schema = """
3.  **"on_screen_text":** A compact array recording distinct legible text in the current video
    chunk, including text visible during dialogue. Each object must contain "text" (the actual
    visible words in their original language), "visual_evidence_time_seconds" (the exact visible
    instant, using the same local/absolute timeline requested for descriptions), and
    "narratively_relevant" (a JSON boolean). Exclude decorative text, persistent watermarks,
    routine credits, and subtitles that only repeat speech. Never guess unreadable words.
    Return [] if no qualifying text is legible. This array is evidence metadata, not narration.
    For each relevant entry, include its readable content in audio_descriptions when it is
    visible inside an authorized dialogue-free window and fits that window's word budget.
    Otherwise keep it only in on_screen_text. Never move it to a later silence, overlap dialogue,
    or change any existing timing, visual-grounding, or mandatory-slot rules to accommodate it.
"""
        screen_text_example = ',\n  "on_screen_text": []'
        core_directives += """9.  **READ NARRATIVELY IMPORTANT ON-SCREEN TEXT WHEN IT IS VISUALLY PRESENT:** Treat visible text
    as visual information when it adds story or scene information that the soundtrack does not
    already provide. Prioritize time jumps (for example "Three years later"), dates, locations,
    title cards, letters or messages, signs, labels, and a logo or brand only when it is relevant
    to understanding the scene. Render the meaning naturally in the target language when useful.
    When describing important, legible text, include what it says rather than merely saying
    that a title, sign, or message is visible. Never guess words that you cannot read.
    Do NOT read decorative text, persistent channel logos/watermarks, routine credits, or subtitles/
    closed captions that merely repeat audible dialogue. This rule NEVER overrides dialogue
    protection: the resulting description must still fit completely inside an authorized
    dialogue-free window, and if no such window is available, omit the text rather than speaking
    over dialogue. The text must be visible at the reported `visual_evidence_time_seconds`.
"""

    # The main system instruction, now asking for a unified JSON object.
    system_instruction = f"""
{system_mission}

**OUTPUT FORMAT (Strict JSON):**
Your entire output MUST be a single JSON object with {output_keys}.

{glossary_schema}

2.  **"audio_descriptions":** An array of objects, where each object represents a timed description. Each description object must contain:
    *   `"start_time_mmss"`: The start time of the description in "MM:SS" or "MM:SS.ms" format. Use a dot before milliseconds; never write `MM:SS:ms`.
    *   `"end_time_mmss"`: The end time of the description in "MM:SS" or "MM:SS.ms" format. Use a dot before milliseconds; never write `MM:SS:ms`.
    *   `"visual_evidence_time_seconds"`: A JSON number giving the exact second, on the same timeline as the timestamps above, where the described visual fact is directly visible. It must fall between this object's start and end times.
    *   `"description_text"`: The concise description text, written entirely in {target_language_name} and following all core directives.

{screen_text_schema}{core_directives}

**EXAMPLE OUTPUT:**
{{
  "character_glossary": {example_glossary_json},
  "audio_descriptions": [
    {{"start_time_mmss": "00:10.500", "end_time_mmss": "00:12.000", "visual_evidence_time_seconds": 11.2, "description_text": {json.dumps(description_example, ensure_ascii=False)}}}
  ]{screen_text_example}
}}
"""

    user_prompt_parts = [
        (
            "Analyze the provided video and generate a unified JSON object containing "
            + ("the character glossary and " if enable_glossary else "")
            + "the timed audio descriptions. "
            + ("Follow all instructions." if enable_glossary else
               "Set character_glossary to an empty array and do not use character names.")
        ),
        "\n**Current Task Specifications:**",
        (
            "*   **Target language for every natural-language output field "
            "(`description_text` and `character_glossary[].description`):** "
            f"{target_language_name}. Names and JSON keys must remain unchanged."
        ),
        f"*   **Verbosity Level:** {verbosity_instruction}",
    ]
    if dialogue_free_windows:
        user_prompt_parts.extend([
            "*   **Authoritative dialogue-free windows (seconds):** " + dialogue_free_windows,
            "*   Every audio description MUST fit completely inside one of these windows. "
            "These windows were measured from the soundtrack with pyannote; never place a "
            "description outside them or across a window boundary.",
        ])
    if intensive_mode:
        if intensive_slots_text:
            user_prompt_parts.extend([
                "*   **INTENSIVE MODE — mandatory numbered slots:** " + intensive_slots_text,
                "*   Return at least one `audio_descriptions` entry for every mandatory slot. You "
                "may add further entries inside a slot when separate important visual changes occur "
                "and the available time can accommodate them. Keep every entry in chronological "
                "order, do not overlap entries, place each start/end completely inside one listed "
                "slot, and keep the combined word count of all entries in a slot within that slot's "
                "maximum. Do not invent timestamps outside the listed slots. A mandatory slot does "
                "not permit repeating or paraphrasing an action already described in an earlier slot: "
                "use a different useful visual fact or the newly reached state instead. For EACH slot, "
                "inspect only the frames inside that slot before choosing the text. Every character, "
                "object, and action named in the entry must be visible inside that exact slot; never "
                "pull an action from a preceding or following scene. The slot boundaries are only "
                "the allowed container: choose start/end around the exact frames that show the fact "
                "you describe. Also report `visual_evidence_time_seconds` for the exact evidence frame "
                "inside that returned interval; do not merely copy the silence start unless that frame "
                "really shows the described fact. Never delay an action to a later part of the same slot after that action "
                "has ended, and never anticipate an action that has not started yet. If those frames are "
                "static, describe their current visible state or setting, including a logo or title card "
                "when that is what is actually visible. Temporal correctness is more important than visual "
                "interest: a plain but correct description is always preferable to an action seen "
                "outside the slot.",
            ])
            if "LONG_SILENCE_PART" in intensive_slots_text:
                user_prompt_parts.append(
                    "*   **LONG-SILENCE PARTITIONS:** Any slot marked `LONG_SILENCE_PART n/N` is "
                    "an artificial balanced subdivision of one longer dialogue-free window, NOT a "
                    "scene boundary or narrative beat. Treat each marked part as an independent visual "
                    "checkpoint. Re-inspect only frames inside that exact part and describe what is "
                    "visibly true there now. Never carry an action, pose, reaction, movement, or setting "
                    "forward from the preceding part merely for narrative continuity; if it ended before "
                    "this part starts, it is invalid here. Likewise, never borrow an action from a later "
                    "part. Temporal correctness has absolute priority over preserving a story sequence."
                )
        else:
            user_prompt_parts.append(
                "*   **INTENSIVE MODE:** This chunk has no dialogue-free interval long enough. "
                "Use only an optional intensive short-gap anchor listed below, if one exists; "
                "otherwise return an empty `audio_descriptions` array and do not invent timestamps."
            )
    if intensive_mode and extended_anchors_text:
        short_gap_instruction = (
            "The player may pause the original media only if the synthesized narration still "
            "cannot fit naturally. "
            if extended_mode else
            "The original media will NOT be paused: after synthesis, any narration that cannot "
            "fit naturally between dialogue will be discarded. "
        )
        user_prompt_parts.extend([
            "*   **OPTIONAL INTENSIVE SHORT-GAP ANCHORS (1+ second speech-free gaps):** "
            + extended_anchors_text,
            "*   Each item has a `PAUSE` range and a bounded `IMMEDIATE_SCENE` range. The "
            "description timestamp MUST stay entirely inside that item's PAUSE range, but the "
            "`description_text` MUST describe ONLY visual information visible in that same item's "
            "IMMEDIATE_SCENE range — the scene beginning directly after the pause. This mapping is "
            "strict: NEVER use E0001 to describe E0002, a later shot, a later scene, or anything "
            "outside E0001's IMMEDIATE_SCENE window, even if it is more important or visually "
            "interesting. Before returning each extended-anchor entry, re-inspect that exact "
            "IMMEDIATE_SCENE window and discard the entry if its action, object, character, or "
            "setting is not visibly present there.",
            "*   These anchors are OPTIONAL, not mandatory. Use one only for important, "
            "plot-relevant visual information from its immediate following scene that cannot be "
            "placed in a normal mandatory slot. Keep it as concise as possible. Use at most one "
            "description per anchor. Do not fill minor pauses, breaths, or every available anchor. "
            + short_gap_instruction,
        ])
    elif intensive_mode:
        user_prompt_parts.append(
            "*   **INTENSIVE SHORT GAPS:** No optional short speech-free anchor is available "
            "in this clip; do not invent one."
        )
    if character_continuity_text:
        user_prompt_parts.extend([
            "*   **ESTABLISHED CHARACTER CONTINUITY FROM EARLIER CLIPS OR A SAVED SERIES CATALOG:** "
            + character_continuity_text,
            "*   This catalog is AUTHORITATIVE for character identity. These names and IDs were "
            "established earlier in the same video or in a prior episode and may be reused without "
            "being spoken again. Match a person by stable physical appearance and identity, not "
            "merely by clothing. When a match is confident, copy EXACTLY the established `id` into "
            "`character_glossary`; do not derive a shorter ID from the name used in narration. Keep "
            "the established catalog name in the glossary. A first name, surname, title, nickname, "
            "abbreviation, or different outfit does NOT create a new character. Never transfer an "
            "identity to a different person; when uncertain, use a generic label in narration rather "
            "than inventing a duplicate catalog entry. For an established character's glossary "
            "description, never restate or rewrite stable biography/relationships. Supply only a "
            "genuinely new visible appearance fact from this clip; otherwise copy the established "
            "description exactly. Do not 'correct' the catalog from visual guesswork.",
        ])
    if recent_descriptions_text:
        user_prompt_parts.extend([
            "*   **RECENT DESCRIPTIONS IMMEDIATELY BEFORE THIS CLIP:** "
            + recent_descriptions_text,
            "*   These entries are context only and must not be returned again. If this clip "
            "directly continues the same scene with the same clear subject, follow the rule against "
            "repeating that subject's name. After a scene or subject change, restore an explicit "
            "name whenever needed for clarity.",
        ])
    if user_prompt and user_prompt.strip():
        user_prompt_parts.append(f"*   **User's Specific Focus:** {user_prompt.strip()}")
        app_logger.info(f"Custom prompt applied: {user_prompt.strip()[:100]}...")
    else:
        app_logger.info("No custom prompt provided by user.")

    return system_instruction, "\n".join(user_prompt_parts)
