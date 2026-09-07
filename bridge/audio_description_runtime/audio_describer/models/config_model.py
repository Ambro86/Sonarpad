"""In-memory settings adapter matching the Omni core's config_model API."""
from __future__ import annotations

from copy import deepcopy

from audio_describer import config

DEFAULT_SETTINGS = {
    "user_gemini_api_key": "",
    "gemini_description_verbosity": config.DEFAULT_VERBOSITY,
    "gemini_model_override": config.GEMINI_MODEL_NAME,
    "gemini_disable_safety_block_none": True,
    "gemini_temperature": 0.3,
    "application_language": "it",
    "logging_level": "INFO",
    "send_silenced_video_to_ai": False,
    "enable_dialogue_protection": True,
    "description_coverage_mode": "intensive",
    "intensive_min_silence_seconds": 3.0,
    "enable_extended_audio_description": True,
    "huggingface_access_token": "",
    "frame_rate_for_ai": 0,
    "enable_video_chunking": True,
    "video_chunk_duration_seconds": 180,
    "enable_character_glossary": True,
    "recognize_screen_text": False,
    "verify_chunk_timing_with_gemini": True,
}

app_settings = deepcopy(DEFAULT_SETTINGS)


def configure(values: dict) -> None:
    global app_settings
    app_settings = deepcopy(DEFAULT_SETTINGS)
    app_settings.update(values or {})


def load_settings() -> dict:
    return deepcopy(app_settings)


def get_setting(key_name: str):
    return app_settings.get(key_name, DEFAULT_SETTINGS.get(key_name))


def save_settings(settings_dict: dict) -> bool:
    configure(settings_dict)
    return True


def get_load_warning():
    return None
