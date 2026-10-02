#!/usr/bin/env python3
"""Compare Sonarpad Windows and mobile pyannote parity JSON exports."""

from __future__ import annotations

import argparse
import json
from pathlib import Path


def _load(path):
    data = json.loads(Path(path).read_text(encoding="utf-8"))
    if data.get("schema") != "sonarpad_pyannote_parity_v1":
        raise SystemExit(f"Unsupported parity JSON: {path}")
    return data


def _decode_rle(rle):
    values = []
    for value, length in rle or []:
        values.extend([int(value)] * int(length))
    return values


def _interval_seconds(intervals):
    return sum(max(0.0, float(end) - float(start)) for start, end in intervals or [])


def _intersection_seconds(left, right):
    i = j = 0
    total = 0.0
    left = sorted((float(a), float(b)) for a, b in left or [])
    right = sorted((float(a), float(b)) for a, b in right or [])
    while i < len(left) and j < len(right):
        a0, a1 = left[i]
        b0, b1 = right[j]
        total += max(0.0, min(a1, b1) - max(a0, b0))
        if a1 <= b1:
            i += 1
        else:
            j += 1
    return total


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("windows_json")
    parser.add_argument("mobile_json")
    args = parser.parse_args()

    windows = _load(args.windows_json)
    mobile = _load(args.mobile_json)

    print("Sonarpad pyannote parity comparison")
    print("=" * 38)
    print(f"Windows ORT: {windows['runtime'].get('onnxruntime_version')}")
    print(f"Mobile ORT:  {mobile['runtime'].get('onnxruntime_version')}")
    print(f"Windows model: {windows['model'].get('sha256')}")
    print(f"Mobile model:  {mobile['model'].get('sha256')}")
    print(f"Same model: {windows['model'].get('sha256') == mobile['model'].get('sha256')}")

    wa = windows["audio"]
    ma = mobile["audio"]
    same_audio = (
        int(wa.get("sample_rate", -1)) == int(ma.get("sample_rate", -2))
        and int(wa.get("sample_count", -1)) == int(ma.get("sample_count", -2))
    )
    print(f"Same canonical audio: {same_audio}")
    print(f"Windows samples: {wa.get('sample_count')}")
    print(f"Mobile samples:  {ma.get('sample_count')}")

    w_analysis = windows["analysis"]
    m_analysis = mobile["analysis"]
    same_hash = w_analysis.get("frame_counts_sha256") == m_analysis.get("frame_counts_sha256")
    print(f"Exact aggregated frame match: {same_hash}")
    print(f"Windows frame hash: {w_analysis.get('frame_counts_sha256')}")
    print(f"Mobile frame hash:  {m_analysis.get('frame_counts_sha256')}")

    w_frames = _decode_rle(w_analysis.get("frame_counts_rle"))
    m_frames = _decode_rle(m_analysis.get("frame_counts_rle"))
    max_len = max(len(w_frames), len(m_frames))
    differing = 0
    first_difference = None
    max_count_delta = 0
    for index in range(max_len):
        w = w_frames[index] if index < len(w_frames) else None
        m = m_frames[index] if index < len(m_frames) else None
        if w != m:
            differing += 1
            if first_difference is None:
                first_difference = index
            if w is not None and m is not None:
                max_count_delta = max(max_count_delta, abs(w - m))
    print(f"Differing aggregated frames: {differing} / {max_len}")
    if first_difference is not None:
        frame_step = float(windows["parameters"]["frame_step_seconds"])
        print(
            f"First differing frame: {first_difference} "
            f"(~{first_difference * frame_step:.6f} s)"
        )
    print(f"Maximum speaker-count delta: {max_count_delta}")

    w_intervals = w_analysis.get("protected_intervals", [])
    m_intervals = m_analysis.get("protected_intervals", [])
    w_seconds = _interval_seconds(w_intervals)
    m_seconds = _interval_seconds(m_intervals)
    intersection = _intersection_seconds(w_intervals, m_intervals)
    union = w_seconds + m_seconds - intersection
    iou = intersection / union if union > 0 else 1.0
    print(f"Windows protected intervals: {len(w_intervals)}")
    print(f"Mobile protected intervals:  {len(m_intervals)}")
    print(f"Windows protected seconds: {w_seconds:.6f}")
    print(f"Mobile protected seconds:  {m_seconds:.6f}")
    print(f"Protected-seconds delta: {m_seconds - w_seconds:+.6f}")
    print(f"Protected-interval time IoU: {iou:.9f}")

    if same_audio and same_hash:
        print("RESULT: EXACT PARITY at aggregated-frame level.")
        return 0
    if not same_audio:
        print("RESULT: INVALID COMPARISON: the canonical audio is not identical.")
        return 2
    print("RESULT: MODEL/RUNTIME DIFFERENCE DETECTED; inspect the metrics above.")
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
