"""Dialogue protection using the pyannote segmentation model in ONNX format.

Only voice activity is needed here: speaker embeddings, clustering, PyTorch,
and the rest of the diarization pipeline would add weight without changing the
dialogue-free windows consumed by Omni Describer.
"""

import math
import os
import re
import sys
import wave

from ..i18n_setup import _
from ..utils.logger import app_logger


BUNDLED_MODEL_REVISION = "3533c8cf8e369892e6b79ff1bf80f7b0286a54ee"
BUNDLED_MODEL_RELATIVE_PATH = os.path.join(
    "assets", "pyannote-segmentation", "model.onnx"
)
SEGMENTATION_SAMPLE_RATE = 16000
SEGMENTATION_WINDOW_SEC = 10.0
SEGMENTATION_STEP_SEC = 1.0
SEGMENTATION_FRAME_DURATION_SEC = 0.0619375
SEGMENTATION_FRAME_STEP_SEC = 0.016875
SEGMENTATION_BATCH_SIZE = 32
POWERSET_MAPPING = (
    (0.0, 0.0, 0.0),
    (1.0, 0.0, 0.0),
    (0.0, 1.0, 0.0),
    (0.0, 0.0, 1.0),
    (1.0, 1.0, 0.0),
    (1.0, 0.0, 1.0),
    (0.0, 1.0, 1.0),
)
DEFAULT_PADDING_SEC = 0.25
# Allow descriptions to move into a nearby dialogue-free gap while still
# bounding the distance from the visual event they describe.
DEFAULT_MAX_SHIFT_SEC = 5.0
DEFAULT_MAX_INTENSIVE_SLOT_SEC = 15.0
DEFAULT_EXTENDED_ANCHOR_SEC = 1.0
DEFAULT_EXTENDED_SCENE_LOOKAHEAD_SEC = 4.0
DEFAULT_EXTENDED_SCENE_MIN_CONTEXT_SEC = 0.5
_ONNX_SESSION = None
_ONNX_SESSION_PATH = None


class DialogueDetectionError(RuntimeError):
    """Raised when requested dialogue detection cannot be completed."""


def merge_intervals(intervals, padding_sec=0.0, duration_sec=None):
    """Normalize, pad and merge ``(start, end)`` intervals."""
    normalized = []
    for start, end in intervals or []:
        try:
            start = max(0.0, float(start) - padding_sec)
            end = float(end) + padding_sec
        except (TypeError, ValueError):
            continue
        if duration_sec is not None:
            end = min(float(duration_sec), end)
        if end > start:
            normalized.append((start, end))

    normalized.sort()
    merged = []
    for start, end in normalized:
        if merged and start <= merged[-1][1]:
            merged[-1] = (merged[-1][0], max(merged[-1][1], end))
        else:
            merged.append((start, end))
    return merged


def speech_free_intervals(protected_intervals, duration_sec, min_duration_sec=0.0):
    """Return the complement of protected speech intervals within the media."""
    duration_sec = max(0.0, float(duration_sec))
    protected = merge_intervals(protected_intervals, duration_sec=duration_sec)
    free = []
    cursor = 0.0
    for start, end in protected:
        if start - cursor >= min_duration_sec:
            free.append((cursor, start))
        cursor = max(cursor, end)
    if duration_sec - cursor >= min_duration_sec:
        free.append((cursor, duration_sec))
    return free


def estimate_spoken_duration(text, words_per_second=2.0):
    """Conservative duration estimate used before the TTS audio exists."""
    words = re.findall(r"\w+", text or "", flags=re.UNICODE)
    return max(0.5, len(words) / max(0.1, words_per_second))


def _choose_slot(free_intervals, desired_start, required_duration,
                 earliest_start=0.0, max_shift_sec=DEFAULT_MAX_SHIFT_SEC):
    candidates = []
    for gap_start, gap_end in free_intervals:
        lower = max(gap_start, earliest_start)
        upper = gap_end - required_duration
        if upper < lower:
            continue
        start = min(max(desired_start, lower), upper)
        distance = abs(start - desired_start)
        if distance <= max_shift_sec:
            candidates.append((distance, start))
    return min(candidates)[1] if candidates else None


def align_descriptions(descriptions, protected_intervals, duration_sec,
                       max_shift_sec=DEFAULT_MAX_SHIFT_SEC):
    """Move/drop descriptions so their estimated speech never hits dialogue.

    Returns ``(aligned, dropped_count)``.  Ordering is preserved and a
    description is dropped instead of moving it far away from its visual event.
    """
    protected = merge_intervals(protected_intervals, duration_sec=duration_sec)
    free = speech_free_intervals(protected, duration_sec)
    aligned = []
    dropped = 0
    moved = 0
    maximum_shift = 0.0
    earliest = 0.0
    for start, _end, text in sorted(descriptions or [], key=lambda item: item[0]):
        desired_start = max(0.0, float(start))
        # Gemini often uses the whole available silence as start/end bounds.
        # That interval is not the TTS duration: reserve only the time needed
        # to speak the generated text. The exact synthesized duration is
        # checked again later by schedule_audio_segments.
        required = estimate_spoken_duration(text)
        slot = _choose_slot(
            free, desired_start, required, earliest_start=earliest,
            max_shift_sec=max_shift_sec,
        )
        if slot is None:
            dropped += 1
            app_logger.warning(
                "Dropping description at %.3fs: no dialogue-free slot of %.3fs nearby.",
                desired_start, required,
            )
            continue
        shift = abs(slot - desired_start)
        if shift > 0.001:
            moved += 1
            maximum_shift = max(maximum_shift, shift)
            app_logger.info(
                "Dialogue alignment moved description %.3fs -> %.3fs "
                "(shift %.3fs, required %.3fs, text=%r).",
                desired_start, slot, shift, required, (text or "")[:100],
            )
        aligned.append((slot, slot + required, text))
        earliest = slot + required + 0.001
    overlap_violations = sum(
        1
        for start, end, _description_text in aligned
        if any(start < speech_end and end > speech_start
               for speech_start, speech_end in protected)
    )
    app_logger.info(
        "Dialogue alignment audit: input=%d kept=%d moved=%d dropped=%d "
        "max_shift=%.3fs protected_intervals=%d overlap_violations=%d.",
        len(descriptions or []), len(aligned), moved, dropped, maximum_shift,
        len(protected), overlap_violations,
    )
    return aligned, dropped


def _choose_pause_anchor(free_intervals, desired_start, earliest_start=0.0,
                         min_anchor_sec=DEFAULT_EXTENDED_ANCHOR_SEC,
                         max_shift_sec=DEFAULT_MAX_SHIFT_SEC):
    """Choose a short speech-free point where playback may safely be paused."""
    candidates = []
    required = max(0.1, float(min_anchor_sec))
    for gap_start, gap_end in free_intervals:
        lower = max(gap_start, earliest_start)
        upper = gap_end - required
        if upper < lower:
            continue
        start = min(max(desired_start, lower), upper)
        distance = abs(start - desired_start)
        if distance <= max_shift_sec:
            candidates.append((distance, start))
    return min(candidates)[1] if candidates else None


def _subtract_reserved_intervals(free_intervals, reserved_intervals):
    """Remove already scheduled narration spans from speech-free intervals."""
    reserved = merge_intervals(reserved_intervals or [])
    available = []
    for free_start, free_end in free_intervals or []:
        cursor = free_start
        for block_start, block_end in reserved:
            if block_end <= cursor or block_start >= free_end:
                continue
            if block_start > cursor:
                available.append((cursor, min(block_start, free_end)))
            cursor = max(cursor, block_end)
            if cursor >= free_end:
                break
        if cursor < free_end:
            available.append((cursor, free_end))
    return [(start, end) for start, end in available if end > start]


def _slot_for_description(description, required_slots):
    start, end, _text = description
    midpoint = (float(start) + float(end)) / 2.0
    for slot in required_slots or []:
        if float(slot["start"]) <= midpoint <= float(slot["end"]):
            return slot
    return None


def _align_descriptions_prioritizing_slots(
    descriptions, protected_intervals, duration_sec, required_slots,
    max_shift_sec=DEFAULT_MAX_SHIFT_SEC, allow_extended_pauses=False,
    min_anchor_sec=DEFAULT_EXTENDED_ANCHOR_SEC, rejected_descriptions=None,
):
    """Place mandatory-slot descriptions before optional narration.

    Intensive generation can return useful optional descriptions in addition to
    the mandatory silence slots. A chronological greedy scheduler lets an
    optional cue consume room that a later mandatory cue needs. This scheduler
    reserves one fitting cue per mandatory slot first, then fills the remaining
    speech-free space with optional cues.
    """
    protected = merge_intervals(protected_intervals, duration_sec=duration_sec)
    free = speech_free_intervals(protected, duration_sec)
    source = list(descriptions or [])
    used = set()
    reserved = []
    aligned = []
    dropped = 0
    extended = 0
    mandatory_scheduled = 0
    mandatory_missing = 0
    mandatory_clamped = 0

    for slot in required_slots or []:
        slot_start = float(slot["start"])
        slot_end = float(slot["end"])
        slot_center = (slot_start + slot_end) / 2.0
        candidates = []
        for index, item in enumerate(source):
            if index in used or _slot_for_description(item, [slot]) is None:
                continue
            required = estimate_spoken_duration(item[2])
            midpoint = (float(item[0]) + float(item[1])) / 2.0
            candidates.append((
                required > (slot_end - slot_start) + 1e-6,
                abs(midpoint - slot_center),
                required,
                index,
                item,
            ))
        candidates.sort(key=lambda candidate: candidate[:4])

        placed = False
        for _too_long, _distance, required, index, item in candidates:
            available = _subtract_reserved_intervals(free, reserved)
            slot_free = [
                (max(start, slot_start), min(end, slot_end))
                for start, end in available
                if min(end, slot_end) > max(start, slot_start)
            ]
            desired_start = max(slot_start, float(item[0]))
            # A mandatory cue has already been grounded to this exact silence
            # slot by the intensive-coverage pass, but the exact timestamp still
            # represents the visual moment Gemini chose.  Do not treat the whole
            # slot as temporally interchangeable: keep the provisional placement
            # within the normal shift limit.  Because the pre-TTS duration is only
            # an estimate (2 words/sec), clamp the provisional reservation near
            # that visual moment rather than moving the cue far across the slot.
            grounded_max_available = 0.0
            shift_limit = max(0.0, float(max_shift_sec))
            for free_start, free_end in slot_free:
                earliest_grounded_start = max(
                    free_start, desired_start - shift_limit
                )
                latest_grounded_start = min(
                    free_end, desired_start + shift_limit
                )
                if latest_grounded_start + 1e-6 < earliest_grounded_start:
                    continue
                grounded_max_available = max(
                    grounded_max_available, free_end - earliest_grounded_start
                )
            if grounded_max_available <= 0.0:
                continue
            planning_duration = min(required, grounded_max_available)
            chosen = _choose_slot(
                slot_free, desired_start, planning_duration, earliest_start=0.0,
                max_shift_sec=shift_limit,
            )
            if chosen is None:
                continue
            if planning_duration + 1e-6 < required:
                mandatory_clamped += 1
                app_logger.info(
                    "Mandatory description for slot %s needs %.3fs by estimate; "
                    "keeping it in the %.3fs available slot for exact TTS scheduling.",
                    slot.get("id") or "?", required, planning_duration,
                )
            aligned.append((chosen, chosen + planning_duration, item[2]))
            reserved.append((chosen, chosen + planning_duration))
            used.add(index)
            mandatory_scheduled += 1
            placed = True
            break

        if not placed:
            mandatory_missing += 1

    for index, item in sorted(
        enumerate(source), key=lambda pair: (float(pair[1][0]), pair[0])
    ):
        if index in used:
            continue
        desired_start = max(0.0, float(item[0]))
        required = estimate_spoken_duration(item[2])
        available = _subtract_reserved_intervals(free, reserved)
        chosen = _choose_slot(
            available, desired_start, required, earliest_start=0.0,
            max_shift_sec=max_shift_sec,
        )
        if chosen is not None:
            aligned.append((chosen, chosen + required, item[2]))
            reserved.append((chosen, chosen + required))
            used.add(index)
            continue

        if allow_extended_pauses:
            anchor = _choose_pause_anchor(
                available, desired_start, earliest_start=0.0,
                min_anchor_sec=min_anchor_sec, max_shift_sec=max_shift_sec,
            )
            if anchor is not None:
                aligned.append((anchor, anchor + min_anchor_sec, item[2]))
                reserved.append((anchor, anchor + min_anchor_sec))
                used.add(index)
                extended += 1
                continue

        dropped += 1
        if rejected_descriptions is not None:
            rejected_descriptions.append(item)
        app_logger.warning(
            "Dropping optional description at %.3fs after mandatory-slot reservation: "
            "no dialogue-free slot of %.3fs nearby.",
            desired_start, required,
        )

    aligned.sort(key=lambda item: (item[0], item[1]))
    app_logger.info(
        "Priority dialogue alignment audit: input=%d kept=%d mandatory=%d/%d "
        "mandatory_missing=%d mandatory_clamped=%d optional_dropped=%d "
        "extended=%d protected_intervals=%d.",
        len(source), len(aligned), mandatory_scheduled, len(required_slots or []),
        mandatory_missing, mandatory_clamped, dropped, extended, len(protected),
    )
    return aligned, dropped, extended


def align_descriptions_prioritizing_slots(
    descriptions, protected_intervals, duration_sec, required_slots,
    max_shift_sec=DEFAULT_MAX_SHIFT_SEC, rejected_descriptions=None,
):
    aligned, dropped, _extended = _align_descriptions_prioritizing_slots(
        descriptions, protected_intervals, duration_sec, required_slots,
        max_shift_sec=max_shift_sec, allow_extended_pauses=False,
        rejected_descriptions=rejected_descriptions,
    )
    return aligned, dropped


def align_descriptions_with_extended_pauses_prioritizing_slots(
    descriptions, protected_intervals, duration_sec, required_slots,
    max_shift_sec=DEFAULT_MAX_SHIFT_SEC,
    min_anchor_sec=DEFAULT_EXTENDED_ANCHOR_SEC, rejected_descriptions=None,
):
    return _align_descriptions_prioritizing_slots(
        descriptions, protected_intervals, duration_sec, required_slots,
        max_shift_sec=max_shift_sec, allow_extended_pauses=True,
        min_anchor_sec=min_anchor_sec, rejected_descriptions=rejected_descriptions,
    )


def align_descriptions_with_extended_pauses(
    descriptions, protected_intervals, duration_sec,
    max_shift_sec=DEFAULT_MAX_SHIFT_SEC,
    min_anchor_sec=DEFAULT_EXTENDED_ANCHOR_SEC,
):
    """Align normally, retaining otherwise-unplaceable cues at short gaps.

    The returned short timestamp is a candidate location. Exact TTS scheduling
    later either fits it naturally, drops it, or—only in extended mode—uses it
    as the point where playback pauses.
    """
    protected = merge_intervals(protected_intervals, duration_sec=duration_sec)
    free = speech_free_intervals(protected, duration_sec)
    aligned = []
    dropped = 0
    extended = 0
    earliest = 0.0
    for start, _end, text in sorted(descriptions or [], key=lambda item: item[0]):
        desired_start = max(0.0, float(start))
        required = estimate_spoken_duration(text)
        slot = _choose_slot(
            free, desired_start, required, earliest_start=earliest,
            max_shift_sec=max_shift_sec,
        )
        if slot is not None:
            aligned.append((slot, slot + required, text))
            earliest = slot + required + 0.001
            continue

        anchor = _choose_pause_anchor(
            free, desired_start, earliest_start=earliest,
            min_anchor_sec=min_anchor_sec, max_shift_sec=max_shift_sec,
        )
        if anchor is None:
            dropped += 1
            app_logger.warning(
                "Dropping extended description at %.3fs: no %.1fs speech-free "
                "pause anchor nearby.", desired_start, min_anchor_sec,
            )
            continue
        anchor_end = anchor + float(min_anchor_sec)
        aligned.append((anchor, anchor_end, text))
        earliest = anchor_end + 0.001
        extended += 1
        app_logger.info(
            "Short-gap description candidate anchored at %.3fs "
            "(estimated narration %.3fs, text=%r).",
            anchor, required, (text or "")[:100],
        )
    app_logger.info(
        "Short-gap alignment audit: input=%d kept=%d candidates=%d dropped=%d.",
        len(descriptions or []), len(aligned), extended, dropped,
    )
    return aligned, dropped, extended


def schedule_audio_segments(description_details, protected_intervals, duration_sec,
                            max_shift_sec=DEFAULT_MAX_SHIFT_SEC):
    """Schedule already-synthesized TTS segments using their exact durations."""
    free = speech_free_intervals(protected_intervals, duration_sec)
    scheduled = []
    dropped = 0
    earliest = 0.0
    for detail in description_details or []:
        desired_start = float(detail["start_sec"])
        required = max(0.001, detail["segment"].duration_seconds)
        slot = _choose_slot(
            free, desired_start, required, earliest_start=earliest,
            max_shift_sec=max_shift_sec,
        )
        if slot is None:
            dropped += 1
            app_logger.warning(
                "Skipping synthesized description at %.3fs: no dialogue-free slot of %.3fs nearby.",
                desired_start, required,
            )
            continue
        updated = dict(detail)
        updated["start_sec"] = slot
        scheduled.append(updated)
        earliest = slot + required + 0.001
    app_logger.info(
        "TTS dialogue scheduling audit: input=%d scheduled=%d dropped=%d "
        "protected_intervals=%d.",
        len(description_details or []), len(scheduled), dropped,
        len(protected_intervals or []),
    )
    return scheduled, dropped


def schedule_audio_segments_with_extended_pauses(
    description_details, protected_intervals, duration_sec,
    max_shift_sec=DEFAULT_MAX_SHIFT_SEC,
    min_anchor_sec=DEFAULT_EXTENDED_ANCHOR_SEC,
):
    """Split synthesized cues into normal overlays and playback pauses."""
    free = speech_free_intervals(protected_intervals, duration_sec)
    scheduled = []
    pauses = []
    dropped = 0
    earliest = 0.0
    for detail in sorted(
        description_details or [], key=lambda item: float(item["start_sec"])
    ):
        desired_start = float(detail["start_sec"])
        required = max(0.001, detail["segment"].duration_seconds)
        slot = _choose_slot(
            free, desired_start, required, earliest_start=earliest,
            max_shift_sec=max_shift_sec,
        )
        updated = dict(detail)
        if slot is not None:
            updated["start_sec"] = slot
            scheduled.append(updated)
            earliest = slot + required + 0.001
            continue
        anchor = _choose_pause_anchor(
            free, desired_start, earliest_start=earliest,
            min_anchor_sec=min_anchor_sec, max_shift_sec=max_shift_sec,
        )
        if anchor is None:
            dropped += 1
            app_logger.warning(
                "Skipping extended synthesized description at %.3fs: no %.1fs "
                "speech-free pause anchor nearby.", desired_start, min_anchor_sec,
            )
            continue
        updated["start_sec"] = anchor
        updated["extended_pause"] = True
        pauses.append(updated)
        earliest = anchor + float(min_anchor_sec) + 0.001
    app_logger.info(
        "Extended TTS scheduling audit: input=%d normal=%d pauses=%d dropped=%d.",
        len(description_details or []), len(scheduled), len(pauses), dropped,
    )
    return scheduled, pauses, dropped


def format_intervals_for_prompt(protected_intervals, duration_sec, max_chars=12000):
    """Compact dialogue-free windows for the Gemini prompt."""
    free = speech_free_intervals(protected_intervals, duration_sec, min_duration_sec=0.5)
    lines = [f"{start:.3f}-{end:.3f}" for start, end in free]
    result = ", ".join(lines)
    if len(result) > max_chars:
        result = result[:max_chars].rsplit(",", 1)[0] + ", ..."
    return result


def intensive_description_slots(protected_intervals, duration_sec,
                                min_duration_sec=3.0, range_start=0.0,
                                range_end=None, id_suffix="",
                                max_slot_duration_sec=DEFAULT_MAX_INTENSIVE_SLOT_SEC):
    """Return numbered usable speech-free slots, optionally clipped to a chunk."""
    range_start = max(0.0, float(range_start))
    range_end = float(duration_sec if range_end is None else range_end)
    max_slot_duration_sec = max(
        float(min_duration_sec), float(max_slot_duration_sec)
    )
    slots = []
    for index, (free_start, free_end) in enumerate(
        speech_free_intervals(protected_intervals, duration_sec), start=1
    ):
        start = max(free_start, range_start)
        end = min(free_end, range_end)
        duration = end - start
        if duration >= float(min_duration_sec):
            # A single description at the start used to mark an arbitrarily
            # long silence as fully covered. Split long windows into balanced
            # sub-slots so intensive mode keeps describing visible changes.
            part_count = max(1, math.ceil(duration / max_slot_duration_sec))
            part_duration = duration / part_count
            for part_index in range(part_count):
                part_start = start + part_duration * part_index
                part_end = end if part_index == part_count - 1 else (
                    start + part_duration * (part_index + 1)
                )
                part_marker = f"P{part_index + 1:03d}" if part_count > 1 else ""
                slots.append({
                    # Chunked callers provide a suffix so a silence crossing a
                    # boundary keeps both usable portions without duplicate IDs.
                    "id": f"S{index:04d}{part_marker}{id_suffix}",
                    "start": part_start,
                    "end": part_end,
                    "max_words": max(1, int((part_end - part_start) * 2.0)),
                    # These are synthetic subdivisions of one long silence,
                    # not scene boundaries.  Keep that fact available to the
                    # first-pass Gemini prompt without triggering another call.
                    "partition_index": part_index + 1,
                    "partition_count": part_count,
                })
    return slots


def extended_description_anchors(
    protected_intervals, duration_sec, normal_min_duration_sec=3.0,
    range_start=0.0, range_end=None, id_suffix="",
    min_anchor_sec=DEFAULT_EXTENDED_ANCHOR_SEC,
    scene_lookahead_sec=DEFAULT_EXTENDED_SCENE_LOOKAHEAD_SEC,
):
    """Return optional short gaps paired with their immediate following scene.

    The short gap is where playback may pause.  The narration must describe only
    the visual scene immediately after that pause, never a convenient scene much
    later in the clip.  A bounded post-anchor scene window makes that temporal
    contract explicit to Gemini.
    """
    range_start = max(0.0, float(range_start))
    range_end = float(duration_sec if range_end is None else range_end)
    minimum = max(0.1, float(min_anchor_sec))
    normal_minimum = max(minimum, float(normal_min_duration_sec))
    lookahead = max(DEFAULT_EXTENDED_SCENE_MIN_CONTEXT_SEC, float(scene_lookahead_sec))
    anchors = []
    for index, (free_start, free_end) in enumerate(
        speech_free_intervals(protected_intervals, duration_sec), start=1
    ):
        start = max(free_start, range_start)
        end = min(free_end, range_end)
        duration = end - start
        scene_start = end
        scene_end = min(range_end, scene_start + lookahead)
        if (
            minimum <= duration < normal_minimum
            and scene_end - scene_start >= DEFAULT_EXTENDED_SCENE_MIN_CONTEXT_SEC
        ):
            anchors.append({
                "id": f"E{index:04d}{id_suffix}",
                "start": start,
                "end": end,
                "scene_start": scene_start,
                "scene_end": scene_end,
            })
    return anchors


def format_extended_anchors_for_prompt(anchors):
    formatted = []
    for anchor in anchors or []:
        scene_start = float(anchor.get("scene_start", anchor["end"]))
        scene_end = float(
            anchor.get(
                "scene_end",
                scene_start + DEFAULT_EXTENDED_SCENE_LOOKAHEAD_SEC,
            )
        )
        formatted.append(
            f"{anchor['id']}=PAUSE {anchor['start']:.3f}-{anchor['end']:.3f} "
            f"-> IMMEDIATE_SCENE {scene_start:.3f}-{scene_end:.3f}"
        )
    return ", ".join(formatted)


def format_intensive_slots_for_prompt(slots):
    """Format mandatory slots with a word budget Gemini can follow."""
    formatted = []
    for slot in slots or []:
        partition_count = int(slot.get("partition_count", 1))
        partition_note = ""
        if partition_count > 1:
            partition_note = (
                f"; LONG_SILENCE_PART {int(slot.get('partition_index', 1))}/"
                f"{partition_count}; inspect this part independently"
            )
        formatted.append(
            f"{slot['id']}={slot['start']:.3f}-{slot['end']:.3f} "
            f"(max {slot['max_words']} words{partition_note})"
        )
    return ", ".join(formatted)


def uncovered_intensive_slots(descriptions, slots):
    """Return mandatory slots that received no description midpoint."""
    missing = []
    for slot in slots or []:
        if not any(
            slot["start"] <= (float(start) + float(end)) / 2.0 <= slot["end"]
            for start, end, _text in descriptions or []
        ):
            missing.append(slot)
    return missing


def _load_pcm16_waveform(wav_path):
    """Load the mono PCM16 WAV extracted by our canonical FFmpeg runtime."""
    import numpy as np

    with wave.open(wav_path, "rb") as wav_file:
        if wav_file.getnchannels() != 1 or wav_file.getsampwidth() != 2:
            raise DialogueDetectionError(_("Unexpected audio format during dialogue detection."))
        sample_rate = wav_file.getframerate()
        samples = np.frombuffer(
            wav_file.readframes(wav_file.getnframes()), dtype="<i2"
        ).astype(np.float32)
    return samples / 32768.0, sample_rate


def get_bundled_model_path():
    """Return the bundled ONNX segmentation model when available."""
    if getattr(sys, "frozen", False):
        base_dir = getattr(sys, "_MEIPASS", os.path.dirname(sys.executable))
    else:
        base_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
    model_path = os.path.join(base_dir, BUNDLED_MODEL_RELATIVE_PATH)
    return model_path if os.path.isfile(model_path) else None


def _get_onnx_session(status):
    """Load and cache the CPU ONNX session used for voice segmentation."""
    global _ONNX_SESSION, _ONNX_SESSION_PATH
    bundled_path = get_bundled_model_path()
    try:
        import onnxruntime as ort
    except ImportError as exc:
        raise DialogueDetectionError(_(
            "ONNX Runtime is not installed. Install the project requirements and try again."
        )) from exc
    if not bundled_path:
        raise DialogueDetectionError(_(
            "The bundled pyannote segmentation model is missing. Reinstall the Sonarpad audio-description module."
        ))
    if _ONNX_SESSION is not None and _ONNX_SESSION_PATH == bundled_path:
        status(_("Using the cached pyannote segmentation model..."))
        return _ONNX_SESSION

    status(_("Loading the bundled pyannote segmentation model..."))
    options = ort.SessionOptions()
    options.intra_op_num_threads = max(1, min(4, os.cpu_count() or 1))
    session = ort.InferenceSession(
        bundled_path,
        sess_options=options,
        providers=["CPUExecutionProvider"],
    )
    _ONNX_SESSION = session
    _ONNX_SESSION_PATH = bundled_path
    app_logger.info(
        "Loaded bundled pyannote ONNX segmentation model: path=%s revision=%s threads=%d.",
        bundled_path, BUNDLED_MODEL_REVISION, options.intra_op_num_threads,
    )
    return session


def _segmentation_chunk_starts(num_samples, sample_rate):
    """Return starts matching pyannote Inference.slide, including its padded tail."""
    window_samples = round(SEGMENTATION_WINDOW_SEC * sample_rate)
    step_samples = round(SEGMENTATION_STEP_SEC * sample_rate)
    if num_samples >= window_samples:
        complete_count = 1 + (num_samples - window_samples) // step_samples
    else:
        complete_count = 0
    has_last_chunk = num_samples < window_samples or (
        (num_samples - window_samples) % step_samples > 0
    )
    starts = [index * step_samples for index in range(complete_count)]
    if has_last_chunk:
        starts.append(complete_count * step_samples)
    return starts


def _run_onnx_segmentation(session, samples, sample_rate,
                           confidence_outputs=None):
    """Return per-window instantaneous speaker counts from ONNX powerset scores.

    ``confidence_outputs`` is diagnostic-only.  When supplied, it receives the
    per-window probability mass assigned to any non-empty powerset class.  The
    production speaker-count path remains exactly the same: argmax -> powerset
    mapping -> active-speaker count.
    """
    import numpy as np

    if sample_rate != SEGMENTATION_SAMPLE_RATE:
        raise DialogueDetectionError(_(
            "Unexpected sample rate during dialogue detection."
        ))
    window_samples = round(SEGMENTATION_WINDOW_SEC * sample_rate)
    starts = _segmentation_chunk_starts(len(samples), sample_rate)
    if not starts:
        return np.empty((0, 589), dtype=np.float32)
    mapping = np.asarray(POWERSET_MAPPING, dtype=np.float32)
    input_name = session.get_inputs()[0].name
    outputs = []
    for batch_start in range(0, len(starts), SEGMENTATION_BATCH_SIZE):
        batch_starts = starts[batch_start:batch_start + SEGMENTATION_BATCH_SIZE]
        batch = np.zeros(
            (len(batch_starts), 1, window_samples), dtype=np.float32
        )
        for index, start in enumerate(batch_starts):
            available = min(window_samples, len(samples) - start)
            if available > 0:
                batch[index, 0, :available] = samples[start:start + available]
        scores = session.run(None, {input_name: batch})[0]
        if scores.ndim != 3 or scores.shape[-1] != len(POWERSET_MAPPING):
            raise RuntimeError(f"Unexpected segmentation output shape: {scores.shape!r}")

        # Diagnostic probe only: convert the raw powerset scores to a stable
        # softmax distribution and retain the probability mass of all classes
        # containing at least one speaker.  This value is never used to decide
        # whether a frame is protected, so it cannot alter the AD pipeline.
        if confidence_outputs is not None:
            shifted = scores - np.max(scores, axis=-1, keepdims=True)
            exp_scores = np.exp(shifted)
            probabilities = exp_scores / np.maximum(
                np.sum(exp_scores, axis=-1, keepdims=True), 1e-12
            )
            confidence_outputs.append(
                (1.0 - probabilities[..., 0]).astype(np.float32, copy=False)
            )

        multilabel = mapping[np.argmax(scores, axis=-1)]
        outputs.append(np.sum(multilabel, axis=-1, dtype=np.float32))
    return np.concatenate(outputs, axis=0)


def _closest_segmentation_frame(timestamp):
    import numpy as np

    return int(np.rint(
        (timestamp - 0.5 * SEGMENTATION_FRAME_DURATION_SEC)
        / SEGMENTATION_FRAME_STEP_SEC
    ))


def _aggregate_segmentation_counts(chunk_counts):
    """Overlap-average window counts exactly like pyannote speaker_count."""
    import numpy as np

    if len(chunk_counts) == 0:
        return np.empty(0, dtype=np.uint8)
    chunk_count, frames_per_chunk = chunk_counts.shape
    end_time = (
        SEGMENTATION_WINDOW_SEC
        + (chunk_count - 1) * SEGMENTATION_STEP_SEC
        + 0.5 * SEGMENTATION_FRAME_DURATION_SEC
    )
    frame_count = _closest_segmentation_frame(end_time) + 1
    summed = np.zeros(frame_count, dtype=np.float32)
    contributors = np.zeros(frame_count, dtype=np.float32)
    for chunk_index, values in enumerate(chunk_counts):
        start_frame = _closest_segmentation_frame(
            chunk_index * SEGMENTATION_STEP_SEC
            + 0.5 * SEGMENTATION_FRAME_DURATION_SEC
        )
        end_frame = start_frame + frames_per_chunk
        summed[start_frame:end_frame] += values
        contributors[start_frame:end_frame] += 1.0
    return np.rint(
        summed / np.maximum(contributors, 1e-12)
    ).astype(np.uint8)


def _aggregate_segmentation_confidence(chunk_confidence):
    """Overlap-average diagnostic confidence without affecting speaker counts."""
    import numpy as np

    if len(chunk_confidence) == 0:
        return np.empty(0, dtype=np.float32)
    chunk_count, frames_per_chunk = chunk_confidence.shape
    end_time = (
        SEGMENTATION_WINDOW_SEC
        + (chunk_count - 1) * SEGMENTATION_STEP_SEC
        + 0.5 * SEGMENTATION_FRAME_DURATION_SEC
    )
    frame_count = _closest_segmentation_frame(end_time) + 1
    summed = np.zeros(frame_count, dtype=np.float32)
    contributors = np.zeros(frame_count, dtype=np.float32)
    for chunk_index, values in enumerate(chunk_confidence):
        start_frame = _closest_segmentation_frame(
            chunk_index * SEGMENTATION_STEP_SEC
            + 0.5 * SEGMENTATION_FRAME_DURATION_SEC
        )
        end_frame = start_frame + frames_per_chunk
        summed[start_frame:end_frame] += values
        contributors[start_frame:end_frame] += 1.0
    return summed / np.maximum(contributors, 1e-12)


def _log_pyannote_confidence_probe(frame_counts, voice_confidence):
    """Log diagnostic confidence to study speech vs non-verbal vocalizations.

    This deliberately does not feed back into dialogue protection.  The goal is
    only to learn whether the existing segmentation model naturally assigns
    different confidence to speech, cries, groans, laughter, and similar sounds.
    """
    import numpy as np

    frame_counts = np.asarray(frame_counts)
    voice_confidence = np.asarray(voice_confidence, dtype=np.float32)
    usable = min(len(frame_counts), len(voice_confidence))
    if usable <= 0:
        app_logger.info("Pyannote confidence probe: no frames available.")
        return

    frame_counts = frame_counts[:usable]
    voice_confidence = np.clip(voice_confidence[:usable], 0.0, 1.0)
    active = frame_counts > 0
    active_values = voice_confidence[active]
    all_quantiles = np.quantile(
        voice_confidence, [0.10, 0.25, 0.50, 0.75, 0.90]
    )
    app_logger.info(
        "Pyannote confidence probe summary: frames=%d active_frames=%d "
        "all[min=%.4f p10=%.4f p25=%.4f p50=%.4f p75=%.4f p90=%.4f max=%.4f].",
        usable, int(np.sum(active)), float(np.min(voice_confidence)),
        *[float(value) for value in all_quantiles],
        float(np.max(voice_confidence)),
    )

    if len(active_values):
        active_quantiles = np.quantile(
            active_values, [0.10, 0.25, 0.50, 0.75, 0.90]
        )
        bins = (
            int(np.sum(active_values < 0.50)),
            int(np.sum((active_values >= 0.50) & (active_values < 0.70))),
            int(np.sum((active_values >= 0.70) & (active_values < 0.85))),
            int(np.sum((active_values >= 0.85) & (active_values < 0.95))),
            int(np.sum(active_values >= 0.95)),
        )
        app_logger.info(
            "Pyannote confidence probe active: min=%.4f p10=%.4f p25=%.4f "
            "p50=%.4f p75=%.4f p90=%.4f max=%.4f "
            "bins[<.50=%d .50-.70=%d .70-.85=%d .85-.95=%d >=.95=%d].",
            float(np.min(active_values)),
            *[float(value) for value in active_quantiles],
            float(np.max(active_values)), *bins,
        )

    # Log each raw voice interval with confidence statistics.  This is compact
    # enough for normal logs and lets a tester compare known timestamps of
    # dialogue, crying, laughter, groans, etc. without logging every frame.
    start = None
    for index, is_active in enumerate(list(active) + [False]):
        if is_active and start is None:
            start = index
        elif not is_active and start is not None:
            stop = index
            values = voice_confidence[start:stop]
            if len(values):
                quantiles = np.quantile(values, [0.10, 0.50, 0.90])
                start_sec = start * SEGMENTATION_FRAME_STEP_SEC
                end_sec = (
                    (stop - 1) * SEGMENTATION_FRAME_STEP_SEC
                    + SEGMENTATION_FRAME_DURATION_SEC
                )
                app_logger.info(
                    "Pyannote confidence probe interval: %.3f-%.3fs duration=%.3fs "
                    "voice_prob[min=%.4f p10=%.4f p50=%.4f p90=%.4f max=%.4f] "
                    "speaker_count[max=%d mean=%.3f].",
                    start_sec, end_sec, end_sec - start_sec,
                    float(np.min(values)), float(quantiles[0]),
                    float(quantiles[1]), float(quantiles[2]),
                    float(np.max(values)), int(np.max(frame_counts[start:stop])),
                    float(np.mean(frame_counts[start:stop])),
                )
            start = None


def _counts_to_intervals(frame_counts):
    """Convert aggregated frame counts into unpadded voice intervals."""
    intervals = []
    start = None
    for index, active in enumerate(list(frame_counts > 0) + [False]):
        if active and start is None:
            start = index * SEGMENTATION_FRAME_STEP_SEC
        elif not active and start is not None:
            intervals.append((
                start,
                (index - 1) * SEGMENTATION_FRAME_STEP_SEC
                + SEGMENTATION_FRAME_DURATION_SEC,
            ))
            start = None
    return intervals


def candidate_nonverbal_voice_intervals(frame_counts, voice_confidence, duration_sec):
    """Identify LOW-CONFIDENCE voice intervals for optional Gemini review only.

    These are *not* silence and are never made safe without video/audio
    verification by Gemini.  In particular, understandable speech can have
    the same confidence as a yawn, as established by our Daima field test.
    """
    import numpy as np

    counts = np.asarray(frame_counts)
    probs = np.asarray(voice_confidence, dtype=np.float32)
    usable = min(len(counts), len(probs))
    result = []
    start = None
    for index in range(usable + 1):
        active = index < usable and counts[index] > 0
        if active and start is None:
            start = index
        elif not active and start is not None:
            stop = index
            begin = start * SEGMENTATION_FRAME_STEP_SEC
            end = min(float(duration_sec),
                      (stop - 1) * SEGMENTATION_FRAME_STEP_SEC
                      + SEGMENTATION_FRAME_DURATION_SEC)
            values = probs[start:stop]
            if len(values) and end - begin >= 0.25:
                p50, p90 = np.quantile(values, [0.50, 0.90])
                if p50 < 0.75 and p90 < 0.80:
                    result.append({
                        "id": "NV-%010d" % round(begin * 1000),
                        "start": round(begin, 3),
                        "end": round(end, 3),
                        "p50": round(float(p50), 4),
                        "p90": round(float(p90), 4),
                    })
            start = None
    return result


def detect_dialogue_intervals_from_wav(wav_path, duration_sec, status_callback=None,
                                       padding_sec=DEFAULT_PADDING_SEC,
                                       nonverbal_candidates_out=None):
    """Run Pyannote ONNX on a mono 16 kHz PCM WAV prepared by Sonarpad.

    Media decoding belongs to the Rust host and its already bundled FFmpeg DLLs.
    This worker never invokes ffmpeg.exe or ffprobe.exe.
    """
    if not get_bundled_model_path():
        raise DialogueDetectionError(_(
            "The bundled pyannote segmentation model is missing. Reinstall the Sonarpad audio-description module."
        ))
    if not os.path.isfile(wav_path):
        raise DialogueDetectionError(_("The Sonarpad-prepared dialogue WAV is missing."))

    def status(message):
        if status_callback:
            status_callback(message)
        app_logger.info("DialogueDetector: %s", message)

    try:
        session = _get_onnx_session(status)
        status(_("Detecting spoken dialogue with pyannote segmentation..."))
        samples, sample_rate = _load_pcm16_waveform(wav_path)
        if not duration_sec:
            duration_sec = len(samples) / sample_rate
        confidence_chunks = []
        chunk_counts = _run_onnx_segmentation(
            session, samples, sample_rate, confidence_outputs=confidence_chunks
        )
        frame_counts = _aggregate_segmentation_counts(chunk_counts)
        if confidence_chunks:
            import numpy as np
            voice_confidence = _aggregate_segmentation_confidence(
                np.concatenate(confidence_chunks, axis=0)
            )
            _log_pyannote_confidence_probe(frame_counts, voice_confidence)
            if nonverbal_candidates_out is not None:
                proposals = candidate_nonverbal_voice_intervals(
                    frame_counts, voice_confidence, duration_sec
                )
                nonverbal_candidates_out.extend(proposals)
                app_logger.info(
                    "Pyannote optional nonverbal candidates: count=%d "
                    "thresholds=p50<0.75,p90<0.80,min_duration=0.25s; "
                    "all remain PROTECTED until individually verified by Gemini.",
                    len(proposals),
                )
        raw_intervals = _counts_to_intervals(frame_counts)
    except Exception as exc:
        app_logger.error(
            "Pyannote ONNX segmentation failed: %s", exc, exc_info=True
        )
        if isinstance(exc, DialogueDetectionError):
            raise
        raise DialogueDetectionError(_(
            "Could not load or run the bundled pyannote segmentation model. "
            "Reinstall the Sonarpad audio-description module and check the log for details."
        )) from exc

    intervals = merge_intervals(
        raw_intervals,
        padding_sec=padding_sec,
        duration_sec=duration_sec,
    )
    protected_seconds = sum(end - start for start, end in intervals)
    free_windows = speech_free_intervals(intervals, duration_sec)
    app_logger.info(
        "Dialogue detection audit: media_duration=%.3fs protected_intervals=%d "
        "protected_seconds=%.3fs protected_percent=%.2f free_windows=%d "
        "free_seconds=%.3fs padding=%.3fs.",
        duration_sec, len(intervals), protected_seconds,
        (protected_seconds / duration_sec * 100.0) if duration_sec else 0.0,
        len(free_windows), sum(end - start for start, end in free_windows),
        padding_sec,
    )
    app_logger.debug("Protected dialogue intervals: %r", intervals)
    status(_("Dialogue detection complete: %d protected speech intervals.") % len(intervals))
    return intervals


def detect_dialogue_intervals(*_args, **_kwargs):
    raise DialogueDetectionError(_(
        "Media decoding must be performed by Sonarpad before Pyannote analysis."
    ))


def protected_intervals_after_verified_nonverbal(protected_intervals,
                                                 nonverbal_candidates, approved_ids,
                                                 duration_sec, edge_margin_sec=0.05):
    """Open only originally protected pieces individually confirmed by Gemini.

    Return a NEW interval list; normal mode never calls this function.
    Leave a small boundary margin to preserve adjacency with true dialogue.
    """
    accepted = set(approved_ids or [])
    approved = [
        (max(0., float(row["start"]) + edge_margin_sec),
         min(float(duration_sec), float(row["end"]) - edge_margin_sec))
        for row in nonverbal_candidates or [] if row["id"] in accepted
    ]
    approved = [(start, end) for start, end in approved if end > start]
    if not approved:
        return list(protected_intervals)
    remaining = list(protected_intervals)
    for gap_start, gap_end in approved:
        new_remaining = []
        for start, end in remaining:
            if gap_start >= end or gap_end <= start:
                new_remaining.append((start, end))
            else:
                if start < gap_start:
                    new_remaining.append((start, gap_start))
                if gap_end < end:
                    new_remaining.append((gap_end, end))
        remaining = new_remaining
    app_logger.info(
        "Optional nonverbal authorization: approved=%d available=%.3fs; "
        "untouched speech remains protected.", len(approved),
        sum(end-start for start,end in approved),
    )
    return remaining
