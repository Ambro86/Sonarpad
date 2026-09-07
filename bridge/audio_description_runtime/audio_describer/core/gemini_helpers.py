# audio_describer/core/gemini_helpers.py
import sys
import importlib.util
import json
import os
import re
import datetime
import threading
import time
from typing import Any, Union

from ..i18n_setup import _
from .. import config
from ..utils.logger import app_logger
from ..models import config_model

# --- Retry Configuration ---
# Long video analysis is expensive and can hit temporary Gemini capacity or
# network failures late in the job. Transient failures retry indefinitely at a
# fixed interval so an extended outage does not abandon the whole generation.
RETRY_DELAY_SEC = 5
# A Gemini request that never returns would otherwise prevent the retry loop
# from ever seeing an exception.  google-genai expects this value in
# milliseconds, so each hung request is abandoned after eight minutes and the
# existing unbounded transient-error retry can try the same work again.
GEMINI_REQUEST_TIMEOUT_MS = 8 * 60 * 1000

_MODEL_ID_ALIASES = {
    # Gemini 3.1 Pro is currently exposed only through its preview endpoint.
    "gemini-3.1-pro": "gemini-3.1-pro-preview",
}
_VALIDATED_GENERATE_MODELS = set()
_QUOTA_DECISION_HANDLER = None
_OVERLOAD_DECISION_HANDLER = None
OVERLOAD_PROMPT_AFTER_CONSECUTIVE_ERRORS = 3

# Windows Winsock codes that are usually transient (timeout / reset / refused).
_WIN_TRANSIENT_SOCKET_ERRORS = frozenset({
    10053,  # WSAECONNABORTED — software caused connection abort
    10054,  # WSAECONNRESET — connection reset by peer
    10060,  # WSAETIMEDOUT — connection timed out
    10061,  # WSAECONNREFUSED — connection refused (brief outage / restart)
})

# errno values that commonly mean transient network failure (POSIX + Windows).
_POSIX_TRANSIENT_ERRNOS = frozenset({
    104,  # ECONNRESET
    110,  # ETIMEDOUT (Linux)
    111,  # ECONNREFUSED
    10053, 10054, 10060, 10061,  # also appear as errno on some Windows builds
})

# Substrings matched case-insensitively against str(exception) / type name.
_RETRYABLE_ERROR_KEYWORDS = (
    # Rate limit / capacity
    "429", "resource exhausted", "rate limit", "quota",
    "503", "service unavailable", "overloaded", "unavailable",
    "504", "deadline exceeded", "deadline_exceeded", "deadline expired",
    # Network / timeouts (incl. Italian Windows messages)
    "10060", "10054", "10053", "10061",
    "timed out", "timeout", "time out",
    "connection reset", "connection aborted", "connection refused",
    "connection error", "connect error", "connecttimeout", "readtimeout",
    "write timeout", "pool timeout",
    "broken pipe", "network is unreachable", "temporary failure",
    "temporarily unavailable", "remote end closed", "server disconnected",
    "failed to establish", "failed to connect",
    "impossibile stabilire la connessione",
    "risposta non corretta della parte connessa",
    "mancata risposta",
    "name resolution", "getaddrinfo", "nodename nor servname",
)

# --- Enhanced Debugging for PyInstaller and google-genai ---
app_logger.debug(f"Python sys.path for SDK import: {sys.path}")
google_spec = importlib.util.find_spec("google")
if google_spec:
    app_logger.debug(f"Found 'google' namespace package. Search locations: {google_spec.submodule_search_locations}")
else:
    app_logger.warning("Could not find spec for the 'google' namespace package.")

try:
    genai_spec = importlib.util.find_spec("google.genai")
except (ImportError, ModuleNotFoundError, AttributeError, ValueError):
    # find_spec("google.genai") raises ModuleNotFoundError when the parent
    # namespace package "google" is not installed.  The Gemini SDK is an
    # optional/lazy dependency here, so absence must be reported as an
    # unavailable SDK rather than aborting module import (notably in clean CI).
    genai_spec = None

if genai_spec:
    app_logger.debug(f"Found 'google.genai' spec before import attempt. Origin: {genai_spec.origin}")
else:
    app_logger.warning("Could not find spec for 'google.genai' before import attempt. Install the 'google-genai' package.")
# --- End Enhanced Debugging ---


# --- Lazy-loading Gemini SDK ---
genai = None
types = None
HarmCategory = None
HarmBlockThreshold = None
google_api_exceptions = None
GEMINI_SDK_AVAILABLE = False
_sdk_import_lock = threading.Lock()

def _lazy_import_gemini_sdk():
    """
    Imports the Gemini SDK modules when first needed.
    This defers potential startup crashes from native libraries until they are actually used.
    """
    global genai, types, HarmCategory, HarmBlockThreshold, google_api_exceptions, GEMINI_SDK_AVAILABLE

    if GEMINI_SDK_AVAILABLE:
        return

    with _sdk_import_lock:
        if GEMINI_SDK_AVAILABLE:
            return

        app_logger.info("Attempting to lazy-load Gemini SDK...")
        try:
            # google-genai package: https://pypi.org/project/google-genai/
            from google import genai as genai_module
            from google.genai import types as types_module
            HarmCategory_module = types_module.HarmCategory
            HarmBlockThreshold_module = types_module.HarmBlockThreshold
            try:
                import google.api_core.exceptions as google_api_exceptions_module
            except ImportError:
                google_api_exceptions_module = None
                app_logger.warning("google.api_core.exceptions not available; rate-limit retries will use message matching only.")

            genai = genai_module
            types = types_module
            HarmCategory = HarmCategory_module
            HarmBlockThreshold = HarmBlockThreshold_module
            google_api_exceptions = google_api_exceptions_module

            GEMINI_SDK_AVAILABLE = True
            app_logger.info(f"Successfully lazy-imported Gemini SDK. Location: {genai.__file__}")

        except ImportError as e:
            GEMINI_SDK_AVAILABLE = False
            app_logger.error("Failed to lazy-import Gemini SDK: %s", e, exc_info=True)
            raise GeminiAPIError(
                "Google's Gemini SDK could not be loaded. Install the 'google-genai' package "
                "(pip install google-genai). This might also be a compatibility issue or a corrupted installation."
            ) from e
        except Exception as e:
            GEMINI_SDK_AVAILABLE = False
            app_logger.critical("A critical, non-import error occurred during Gemini SDK lazy-loading: %s", e, exc_info=True)
            raise GeminiAPIError("A critical error occurred while loading Google's Gemini SDK, which may be due to a system incompatibility.") from e


class GeminiAPIError(Exception):
    """Custom exception for Gemini API errors."""
    pass

class ContentBlockedError(GeminiAPIError):
    """Exception raised when content is blocked by safety filters or other reasons."""
    def __init__(self, message, reason=""):
        super().__init__(message)
        self.reason = reason

class TokenLimitError(GeminiAPIError):
    """Exception raised when the AI process stops because it hit a token limit."""
    def __init__(self, message, reason=""):
        super().__init__(message)
        self.reason = reason

class GeminiRetryCancelledError(GeminiAPIError):
    """Raised when the user explicitly stops a request waiting on quota."""


class GeminiMalformedResponseError(GeminiAPIError):
    """Raised when Gemini returns a transient MALFORMED_RESPONSE with no content."""


def set_quota_decision_handler(handler):
    """Set the process-wide quota callback used by the desktop worker.

    The callback receives ``(current_model, exception)`` and returns a new
    model id, ``None`` to keep waiting, or ``False`` to cancel processing.
    """
    global _QUOTA_DECISION_HANDLER
    _QUOTA_DECISION_HANDLER = handler


def set_overload_decision_handler(handler):
    """Set the callback for repeated Gemini 503 high-demand errors.

    The callback receives ``(current_model, exception)`` and returns True to
    keep retrying indefinitely, or False to stop the current job.
    """
    global _OVERLOAD_DECISION_HANDLER
    _OVERLOAD_DECISION_HANDLER = handler

# --- Global Client Instance ---
_GEMINI_CLIENT = None


def _create_gemini_client(api_key):
    """Create a Gemini client with a finite per-request HTTP timeout."""
    return genai.Client(
        api_key=api_key,
        http_options=types.HttpOptions(timeout=GEMINI_REQUEST_TIMEOUT_MS),
    )

def reset_gemini_client():
    """Resets the global Gemini client instance, forcing re-initialization on next use."""
    global _GEMINI_CLIENT
    if _GEMINI_CLIENT is not None:
        app_logger.info("Resetting Gemini API client due to settings change.")
        _GEMINI_CLIENT = None

def using_sonarpad_service():
    return bool(str(config_model.get_setting("sonarpad_ai_service_url") or "").strip())


def get_gemini_client():
    """Gets or initializes the Gemini transport.

    Personal-key mode uses Google's SDK directly. Sonarpad AI mode uses a
    constrained transport where media uploads go PC -> Google and only
    authorization/generation metadata goes through sonarpad.com.
    """
    global _GEMINI_CLIENT
    _lazy_import_gemini_sdk()

    if _GEMINI_CLIENT:
        return _GEMINI_CLIENT

    service_url = str(config_model.get_setting("sonarpad_ai_service_url") or "").strip()
    if service_url:
        access_code = str(config_model.get_setting("sonarpad_ai_access_code") or "").strip()
        device_id = str(config_model.get_setting("sonarpad_ai_device_id") or "").strip()
        try:
            from audio_describer.core.sonarpad_service import SonarpadServiceClient
            app_logger.info("Initializing Sonarpad AI service transport...")
            _GEMINI_CLIENT = SonarpadServiceClient(
                service_url, access_code, device_id, "Sonarpad Windows"
            )
            settings = config_model.load_settings()
            settings["gemini_model_override"] = _GEMINI_CLIENT.model
            config_model.save_settings(settings)
            app_logger.info(
                "Sonarpad AI service active: model=%s balance_eur=%.4f",
                _GEMINI_CLIENT.model, _GEMINI_CLIENT.balance_eur,
            )
            return _GEMINI_CLIENT
        except Exception as e:
            _GEMINI_CLIENT = None
            app_logger.error("Failed to initialize Sonarpad AI service: %s", e, exc_info=True)
            raise GeminiAPIError(_("Failed to initialize Sonarpad AI service: %s") % e) from e

    api_key = config_model.get_setting("user_gemini_api_key") or config.GEMINI_API_KEY
    if not api_key or api_key == "YOUR_GEMINI_API_KEY_HERE":
        raise GeminiAPIError(_("Gemini API Key is not configured in settings."))
    try:
        app_logger.info("Initializing genai.Client...")
        _GEMINI_CLIENT = _create_gemini_client(api_key)
        return _GEMINI_CLIENT
    except Exception as e:
        _GEMINI_CLIENT = None
        app_logger.error("Failed to initialize Gemini Client: %s", e, exc_info=True)
        raise GeminiAPIError(_("Failed to initialize Gemini Client: %s") % e)


def normalize_model_id(model_id):
    """Normalize user-entered model names and migrate known obsolete aliases."""
    normalized = (model_id or "").strip()
    if normalized.startswith("models/"):
        normalized = normalized[len("models/"):]
    replacement = _MODEL_ID_ALIASES.get(normalized, normalized)
    if replacement != normalized:
        app_logger.warning(
            "Migrating unavailable Gemini model id '%s' to '%s'.",
            normalized, replacement,
        )
    return replacement


def validate_model_for_generate_content(model_id, client=None, status_callback=None):
    """Fail early when the selected model is absent or cannot generate content."""
    model_id = normalize_model_id(model_id)
    if not model_id:
        raise GeminiAPIError(_("No Gemini model is configured."))
    if model_id in _VALIDATED_GENERATE_MODELS:
        return model_id
    if status_callback:
        status_callback(_("Checking whether Gemini model '%s' is available…") % model_id)
    client = client or get_gemini_client()
    try:
        model_info = run_with_retry(
            lambda: client.models.get(model=model_id),
            status_callback=status_callback,
            operation_label=_("Gemini model verification"),
        )
    except Exception as exc:
        message = str(exc)
        code = getattr(exc, "code", None)
        if code == 404 or "404" in message or "NOT_FOUND" in message:
            raise GeminiAPIError(
                _("Gemini model '%s' is not available for this API key or does not exist. "
                  "Open Settings -> AI and choose a currently available model.") % model_id
            ) from exc
        raise GeminiAPIError(
            _("Could not verify Gemini model '%(model)s': %(error)s")
            % {"model": model_id, "error": message}
        ) from exc

    supported = getattr(model_info, "supported_actions", None) or []
    if "generateContent" not in supported:
        raise GeminiAPIError(
            _("Gemini model '%s' does not support video description (generateContent).")
            % model_id
        )
    _VALIDATED_GENERATE_MODELS.add(model_id)
    app_logger.info(
        "Validated Gemini model '%s'; supported_actions=%s.", model_id, supported
    )
    return model_id


def list_generate_content_models(client=None, api_key=None):
    """Ask ModelService.ListModels for current general-purpose Gemini models.

    The API also reports specialized TTS, image-generation, robotics and
    computer-use endpoints as supporting ``generateContent``. Those endpoints
    cannot perform Omni Describer's normal video-analysis request, so they are
    excluded by capability-family suffix rather than by maintaining model ids.
    """
    _lazy_import_gemini_sdk()
    if client is None:
        client = _create_gemini_client(api_key) if api_key else get_gemini_client()

    excluded_families = (
        "tts", "image", "robotics", "computer-use", "customtools",
    )
    available = []
    for model_info in client.models.list():
        model_id = normalize_model_id(getattr(model_info, "name", ""))
        actions = getattr(model_info, "supported_actions", None) or []
        lowered = model_id.lower()
        if (
            model_id.startswith("gemini-")
            and "generateContent" in actions
            and not any(family in lowered for family in excluded_families)
        ):
            available.append(model_id)

    # Preserve the API's ordering while removing aliases/duplicates.
    unique = list(dict.fromkeys(available))
    app_logger.info(
        "ModelService.ListModels returned %d general Gemini generateContent models: %s",
        len(unique), ", ".join(unique),
    )
    return unique

def build_safety_settings():
    """Constructs the safety_settings list for the Gemini API call.

    Uses proper google.genai SafetySetting objects. When the user opts to
    disable filters, prefer HarmBlockThreshold.OFF (full off) and fall back
    to BLOCK_NONE. Note: Google still enforces non-overridable policy blocks
    (often reported as block_reason OTHER) for prohibited content.
    """
    _lazy_import_gemini_sdk()
    disable_safety = config_model.get_setting("gemini_disable_safety_block_none")

    if not disable_safety:
        return None

    app_logger.warning("Disabling all configurable Gemini safety filters as per user setting.")
    # OFF is stronger than BLOCK_NONE when available on this SDK version.
    threshold = getattr(HarmBlockThreshold, "OFF", None) or HarmBlockThreshold.BLOCK_NONE
    # Only categories accepted by generativelanguage v1beta HarmCategory.
    # Never send IMAGE_* — some SDKs expose those enums but the API returns
    # 400 INVALID_ARGUMENT for them.
    allowed_names = (
        "HARM_CATEGORY_HARASSMENT",
        "HARM_CATEGORY_HATE_SPEECH",
        "HARM_CATEGORY_SEXUALLY_EXPLICIT",
        "HARM_CATEGORY_DANGEROUS_CONTENT",
        "HARM_CATEGORY_CIVIC_INTEGRITY",
    )
    settings = []
    for name in allowed_names:
        cat = getattr(HarmCategory, name, None)
        if cat is None:
            continue
        cat_name = getattr(cat, "name", str(cat))
        if "IMAGE" in cat_name:
            continue
        try:
            settings.append(types.SafetySetting(category=cat, threshold=threshold))
        except Exception as e:
            app_logger.warning("Could not build SafetySetting for %s: %s", cat, e)
    app_logger.debug(
        "Safety settings categories: %s",
        [getattr(s.category, "name", s.category) for s in settings],
    )
    return settings or None

def build_generation_config(system_instruction_text=None, is_json_response=False, enable_thinking=False):
    """Builds the generation config for the API call."""
    _lazy_import_gemini_sdk()

    config_params = {}

    if (temp_str := config_model.get_setting("gemini_temperature")) is not None:
        try:
            config_params["temperature"] = float(temp_str)
        except ValueError:
            app_logger.warning(f"Invalid temperature value '{temp_str}' in settings, using default 0.2.")
            config_params["temperature"] = 0.2
    else:
        config_params["temperature"] = 0.2

    if enable_thinking:
        model_name_to_use = config_model.get_setting("gemini_model_override") or config.GEMINI_MODEL_NAME
        if "1.5" in model_name_to_use or "2.5" in model_name_to_use or "2.0" in model_name_to_use:
            # Cap thinking budget: -1 (unlimited) can stall for a very long time on video.
            app_logger.info(f"Model '{model_name_to_use}' supports thinking. Enabling thinking_config (budget=8192).")
            thinking_config = types.ThinkingConfig(
                include_thoughts=True,
                thinking_budget=8192,
            )
            config_params["thinking_config"] = thinking_config
        else:
            app_logger.info(f"Model '{model_name_to_use}' may not support thinking. Disabling thinking_config.")

    if is_json_response:
        config_params["response_mime_type"] = "application/json"

    safety_settings = build_safety_settings()
    if safety_settings:
        config_params["safety_settings"] = safety_settings

    if system_instruction_text and isinstance(system_instruction_text, str) and system_instruction_text.strip():
        config_params["system_instruction"] = types.Content(parts=[types.Part.from_text(text=system_instruction_text)])

    app_logger.info(f"Built GenerationConfig. JSON: {is_json_response}, Thinking: {enable_thinking}. Params: {config_params}")
    return types.GenerateContentConfig(**config_params)

def log_token_usage(context, response):
    """Logs token usage from a response."""
    usage = getattr(response, 'usage_metadata', None)
    usage_dict = {}
    if usage:
        prompt_tokens = getattr(usage, 'prompt_token_count', 0)
        candidates_tokens = getattr(usage, 'candidates_token_count', None)
        total_tokens = getattr(usage, 'total_token_count', 0)
        thoughts_tokens = getattr(usage, 'thoughts_token_count', 0)

        usage_dict = {
            'prompt_tokens': prompt_tokens,
            'candidates_tokens': candidates_tokens,
            'total_tokens': total_tokens,
            'thoughts_tokens': thoughts_tokens
        }
        app_logger.info(f"Tokens ({context}): Prmpt={prompt_tokens}, Thnk={thoughts_tokens}, Ans={candidates_tokens}, Tot={total_tokens}")
    else:
        app_logger.warning(f"Token usage metadata not found in response for {context}.")
    return usage_dict

def get_response_finish_reason(response):
    """Return finish_reason name (e.g. STOP, MAX_TOKENS) or empty string."""
    try:
        if not response or not getattr(response, "candidates", None):
            return ""
        fr = getattr(response.candidates[0], "finish_reason", None)
        if fr is None:
            return ""
        return getattr(fr, "name", str(fr)) or ""
    except Exception:
        return ""


class _GenerationHeartbeat:
    """Background status updates while a long Gemini call is in flight.

    generate_content blocks for minutes on video; without this the UI only shows
    the last "Requesting..." line.
    """

    def __init__(self, status_callback, context_label=""):
        self._status_callback = status_callback
        self._context = context_label or _("AI")
        self._stop = threading.Event()
        self._thread = None
        self._started = time.time()
        self._phase_idx = 0
        self._phases = (
            _("Waiting for Gemini — analyzing video…"),
            _("Waiting for Gemini — watching scenes…"),
            _("Waiting for Gemini — drafting descriptions…"),
            _("Waiting for Gemini — still working (long videos take several minutes)…"),
            _("Waiting for Gemini — writing JSON output…"),
        )

    def __enter__(self):
        if not self._status_callback:
            return self
        self._started = time.time()
        self._thread = threading.Thread(target=self._run, name="gemini-heartbeat", daemon=True)
        self._thread.start()
        return self

    def __exit__(self, exc_type, exc, tb):
        self._stop.set()
        if self._thread and self._thread.is_alive():
            self._thread.join(timeout=2.0)
        return False

    def _run(self):
        # First tick soon so the user sees progress immediately
        intervals = (8, 15, 20, 25, 30)
        i = 0
        while not self._stop.wait(intervals[min(i, len(intervals) - 1)]):
            elapsed = int(time.time() - self._started)
            mins, secs = divmod(elapsed, 60)
            phase = self._phases[min(self._phase_idx, len(self._phases) - 1)]
            self._phase_idx += 1
            msg = _("%(ctx)s: %(phase)s (%(m)d:%(s)02d elapsed)") % {
                "ctx": self._context,
                "phase": phase,
                "m": mins,
                "s": secs,
            }
            try:
                self._status_callback(msg)
            except Exception:
                pass
            i += 1


def _single_exception_is_retryable(exc: BaseException) -> bool:
    """True if this one exception object looks like a transient API/network failure."""
    # A PROHIBITED_CONTENT prompt block is sometimes a transient false positive
    # on an otherwise valid film chunk. Retrying prevents one HTTP-200 blocked
    # response from discarding all earlier chunks.
    if (
        isinstance(exc, ContentBlockedError)
        and str(getattr(exc, "reason", "")).upper() == "PROHIBITED_CONTENT"
    ):
        return True

    # Every HTTP server-side failure is transient from the client's point of
    # view.  Prefer structured status attributes used by google-genai, httpx,
    # requests and google-api-core so new 5xx variants do not need to be added
    # to the message keyword list one by one.
    status_candidates = [
        getattr(exc, "code", None),
        getattr(exc, "status_code", None),
        getattr(getattr(exc, "response", None), "status_code", None),
    ]
    for candidate in status_candidates:
        try:
            status_code = int(candidate)
        except (TypeError, ValueError):
            continue
        if 500 <= status_code <= 599:
            return True

    # Built-in network exceptions
    if isinstance(exc, (ConnectionError, TimeoutError, BrokenPipeError)):
        return True

    # OSError / socket errors (WinError 10060 etc. often surface as OSError)
    if isinstance(exc, OSError):
        winerr = getattr(exc, "winerror", None)
        if winerr in _WIN_TRANSIENT_SOCKET_ERRORS:
            return True
        errno_val = getattr(exc, "errno", None)
        if errno_val in _POSIX_TRANSIENT_ERRNOS or errno_val in _WIN_TRANSIENT_SOCKET_ERRORS:
            return True

    # google.api_core typed exceptions (when available)
    if google_api_exceptions is not None:
        retryable_types = []
        for name in (
            "ResourceExhausted",
            "ServiceUnavailable",
            "TooManyRequests",
            "DeadlineExceeded",
            "Aborted",
            "InternalServerError",
            "Unknown",
        ):
            t = getattr(google_api_exceptions, name, None)
            if t is not None:
                retryable_types.append(t)
        if retryable_types and isinstance(exc, tuple(retryable_types)):
            return True

    # httpx / httpcore / urllib3 style names (package may or may not be imported)
    type_name = type(exc).__name__.lower()
    if any(
        token in type_name
        for token in (
            "timeout",
            "connecterror",
            "connecttimeout",
            "readtimeout",
            "writetimeout",
            "pooltimeout",
            "networkerror",
            "protocolerror",
            "remoteprotocol",
        )
    ):
        return True

    error_str = str(exc).lower()
    # Some wrappers discard structured attributes but retain the HTTP status
    # in their message (for example "502 Bad Gateway").
    if re.search(r"(?<!\d)5\d{2}(?!\d)", error_str):
        return True
    return any(keyword in error_str for keyword in _RETRYABLE_ERROR_KEYWORDS)


def is_prepaid_credits_depleted_error(exc: BaseException) -> bool:
    """Return True only for Gemini's permanent prepaid-billing depletion error.

    This is deliberately narrower than generic HTTP 429 / RESOURCE_EXHAUSTED
    handling. Other quota/rate-limit responses remain retryable or eligible for
    the existing model-switch decision flow.
    """
    seen = set()
    current: BaseException | None = exc
    while current is not None and id(current) not in seen:
        seen.add(id(current))
        if "prepayment credits are depleted" in str(current).casefold():
            return True
        current = current.__cause__ or current.__context__
    return False


def _single_exception_is_permanent_invalid_argument(exc: BaseException) -> bool:
    """True for permanent HTTP 400 request-validation failures.

    A structured 400 from Sonarpad AI/Gemini cannot recover by retrying the
    identical payload, even when Google's message says "Invalid value" rather
    than the literal INVALID_ARGUMENT status string.
    """
    status_candidates = [
        getattr(exc, "code", None),
        getattr(exc, "status_code", None),
        getattr(getattr(exc, "response", None), "status_code", None),
    ]
    for candidate in status_candidates:
        try:
            if int(candidate) == 400:
                return True
        except (TypeError, ValueError):
            continue

    message = str(exc).casefold()
    return (
        re.search(r"(?<!\d)400(?!\d)", message) is not None
        and (
            "invalid_argument" in message
            or "invalid argument" in message
            or "invalid value" in message
            or "unknown name" in message
            or "cannot find field" in message
        )
    )


def is_retryable_transient_error(exc: BaseException) -> bool:
    """True if *exc* (or any cause/context in its chain) is a transient network/API error.

    Covers rate limits (429), every HTTP server error (5xx), and connection
    failures such as Windows WinError 10060. A concrete Gemini 400
    INVALID_ARGUMENT always wins over incidental transient-looking exception
    context because retrying the same malformed request cannot recover.
    """
    chain = []
    seen = set()
    current: BaseException | None = exc
    while current is not None and id(current) not in seen:
        seen.add(id(current))
        chain.append(current)
        nxt = current.__cause__
        if nxt is None:
            nxt = current.__context__
        current = nxt

    if any(_single_exception_is_permanent_invalid_argument(item) for item in chain):
        return False
    return any(_single_exception_is_retryable(item) for item in chain)


def is_high_demand_unavailable_error(exc: BaseException) -> bool:
    """Return True only for Gemini HTTP 503 high-demand/capacity responses."""
    seen = set()
    current: BaseException | None = exc
    while current is not None and id(current) not in seen:
        seen.add(id(current))
        message = str(current).casefold()
        code = getattr(current, "code", None)
        status_code = getattr(current, "status_code", None)
        response_code = getattr(getattr(current, "response", None), "status_code", None)
        structured_503 = any(
            candidate == 503 or str(candidate) == "503"
            for candidate in (code, status_code, response_code)
            if candidate is not None
        )
        textual_503 = re.search(r"(?<!\d)503(?!\d)", message) is not None
        high_demand = (
            "high demand" in message
            or "spikes in demand" in message
            or "currently experiencing high demand" in message
        )
        unavailable = "unavailable" in message or "service unavailable" in message
        if (structured_503 or textual_503) and high_demand and unavailable:
            return True
        current = current.__cause__ or current.__context__
    return False


def is_quota_exhausted_error(exc: BaseException) -> bool:
    """Return True for Gemini 429/resource-exhausted quota responses."""
    seen = set()
    current: BaseException | None = exc
    while current is not None and id(current) not in seen:
        seen.add(id(current))
        message = str(current).lower()
        code = getattr(current, "code", None)
        if (
            code == 429
            or "429" in message
            or "resource_exhausted" in message
            or "resource exhausted" in message
        ) and ("quota" in message or "rate limit" in message):
            return True
        current = current.__cause__ or current.__context__
    return False


def run_with_retry(operation, *, status_callback=None, operation_label=None):
    """Run *operation()* indefinitely on transient network/API errors.

    *operation* is a zero-arg callable. Non-retryable errors are re-raised
    immediately. Transient failures wait RETRY_DELAY_SEC before every retry.
    """
    label = operation_label or _("API request")
    attempt = 0

    while True:
        attempt += 1
        try:
            if attempt > 1 and status_callback:
                status_callback(
                    _("Retrying %(what)s (attempt %(attempt)d)…")
                    % {"what": label, "attempt": attempt}
                )
            return operation()
        except Exception as e:
            if is_prepaid_credits_depleted_error(e):
                app_logger.error(
                    "Permanent Gemini billing error on %s attempt %d: %s",
                    label, attempt, e,
                )
                raise
            if not is_retryable_transient_error(e):
                raise

            retry_msg = _(
                "Connection or temporary API error (attempt %(attempt)d). "
                "Retrying in %(delay)d seconds..."
            ) % {"attempt": attempt, "delay": RETRY_DELAY_SEC}
            app_logger.warning(
                "Transient error on %s attempt %d: %s. Retrying in %ds...",
                label, attempt, e, RETRY_DELAY_SEC,
            )
            if status_callback:
                status_callback(retry_msg)
            time.sleep(RETRY_DELAY_SEC)


def _call_generate_content(client, model, contents, config, status_callback=None):
    """One generate_content call with a background heartbeat for UI progress.

    We intentionally use blocking generate_content (not stream): for video the
    stream either stays quiet until the end or is hard to reassemble, and a
    failed stream must never trigger a second full video analysis.
    """
    if status_callback:
        status_callback(_("Contacting Gemini API (model: %s)…") % model)

    with _GenerationHeartbeat(status_callback, _("Gemini")):
        response = client.models.generate_content(
            model=model, contents=contents, config=config
        )

    # Gemini can return HTTP 200 with no candidate when its non-configurable
    # input filter produces a false-positive PROHIBITED_CONTENT decision. If
    # this escaped to the parser, the whole multi-chunk job would be abandoned.
    # Raise it inside the retry boundary so the same chunk is retained.
    prompt_feedback = getattr(response, "prompt_feedback", None)
    block_reason_obj = getattr(prompt_feedback, "block_reason", None)
    block_reason = (
        getattr(block_reason_obj, "name", None) or str(block_reason_obj or "")
    )
    if (
        block_reason in ("PROHIBITED_CONTENT", "BlockReason.PROHIBITED_CONTENT")
        and not getattr(response, "candidates", None)
    ):
        extra = getattr(prompt_feedback, "block_reason_message", None) or ""
        message = _("AI request was blocked due to: %s") % block_reason
        if extra:
            message = f"{message} ({extra})"
        app_logger.warning(
            "Gemini returned retryable prompt block %s with no candidates.",
            block_reason,
        )
        raise ContentBlockedError(message, reason="PROHIBITED_CONTENT")

    # Gemini can occasionally return HTTP 200 but finish with
    # MALFORMED_RESPONSE and no usable content. This is a server-side/transient
    # response failure rather than a safety decision or malformed client
    # request. Raise it inside the retry boundary so the exact same chunk can be
    # retried without losing checkpoint progress.
    candidates = getattr(response, "candidates", None) or []
    if candidates:
        candidate = candidates[0]
        content = getattr(candidate, "content", None)
        parts = getattr(content, "parts", None) if content is not None else None
        finish_reason = get_response_finish_reason(response).strip().upper()
        if (
            not parts
            and (
                finish_reason == "MALFORMED_RESPONSE"
                or finish_reason.endswith(".MALFORMED_RESPONSE")
            )
        ):
            message = _(
                "Gemini returned a temporary malformed response with no content."
            )
            app_logger.warning(
                "Gemini returned MALFORMED_RESPONSE with no content; "
                "the same request is eligible for a bounded retry."
            )
            raise GeminiMalformedResponseError(message)

    if status_callback:
        fr = get_response_finish_reason(response)
        if fr:
            status_callback(_("Gemini response received (finish: %s). Parsing…") % fr)
        else:
            status_callback(_("Gemini response received. Parsing…"))
    return response


def generate_content_with_retry(client, model, contents, config, status_callback=None,
                                prohibited_content_max_attempts=1,
                                malformed_response_max_attempts=3):
    """Call Gemini, retrying transient errors every five seconds.

    When the desktop UI installs a quota decision handler, the first quota
    failure for each model lets the user switch model, keep waiting, or stop.
    A switched model is also used by later requests/chunks because the UI
    persists it in the current settings.
    """
    _lazy_import_gemini_sdk()
    configured_model = config_model.get_setting("gemini_model_override")
    current_model = normalize_model_id(configured_model or model)
    quota_prompted_models = set()
    overload_failures = 0
    overload_wait_forever = False
    malformed_response_failures = 0
    attempt = 0

    while True:
        attempt += 1
        try:
            if attempt > 1 and status_callback:
                status_callback(
                    _("Retrying %(what)s (attempt %(attempt)d)…")
                    % {"what": _("Gemini request"), "attempt": attempt}
                )
            return _call_generate_content(
                client, current_model, contents, config, status_callback
            )
        except Exception as exc:
            if isinstance(exc, GeminiMalformedResponseError):
                malformed_response_failures += 1
                max_malformed_attempts = max(
                    1, int(malformed_response_max_attempts or 1)
                )
                if malformed_response_failures >= max_malformed_attempts:
                    app_logger.warning(
                        "Gemini MALFORMED_RESPONSE persisted for %d attempt(s); "
                        "stopping this request and preserving existing checkpoint progress.",
                        malformed_response_failures,
                    )
                    raise
                retry_msg = _(
                    "Gemini returned an invalid temporary response "
                    "(attempt %(attempt)d/%(max_attempts)d). "
                    "Retrying in %(delay)d seconds..."
                ) % {
                    "attempt": malformed_response_failures,
                    "max_attempts": max_malformed_attempts,
                    "delay": RETRY_DELAY_SEC,
                }
                app_logger.warning(
                    "Retrying the same Gemini request after MALFORMED_RESPONSE "
                    "(%d/%d) in %ds...",
                    malformed_response_failures, max_malformed_attempts, RETRY_DELAY_SEC,
                )
                if status_callback:
                    status_callback(retry_msg)
                # Keep this bounded server-response retry independent from the
                # existing transient/prohibited-content attempt counters.
                attempt -= 1
                time.sleep(RETRY_DELAY_SEC)
                continue

            if is_prepaid_credits_depleted_error(exc):
                app_logger.error(
                    "Permanent Gemini prepaid-billing error on model %s: %s",
                    current_model, exc,
                )
                raise
            if not is_retryable_transient_error(exc):
                raise

            if (
                isinstance(exc, ContentBlockedError)
                and str(getattr(exc, "reason", "")).upper() == "PROHIBITED_CONTENT"
                and prohibited_content_max_attempts is not None
                and attempt >= max(1, int(prohibited_content_max_attempts))
            ):
                app_logger.warning(
                    "PROHIBITED_CONTENT persisted for %d attempt(s); handing "
                    "control back to the caller for media fallback.",
                    attempt,
                )
                raise

            if is_high_demand_unavailable_error(exc):
                overload_failures += 1
                if (
                    not overload_wait_forever
                    and _OVERLOAD_DECISION_HANDLER is not None
                    and overload_failures >= OVERLOAD_PROMPT_AFTER_CONSECUTIVE_ERRORS
                ):
                    app_logger.warning(
                        "Gemini high-demand 503 persisted for %d consecutive attempt(s) "
                        "on model %s; asking the user whether to keep waiting.",
                        overload_failures, current_model,
                    )
                    if not _OVERLOAD_DECISION_HANDLER(current_model, exc):
                        raise GeminiRetryCancelledError(
                            _("Processing stopped by the user while Gemini is temporarily overloaded.")
                        ) from exc
                    overload_wait_forever = True
                    app_logger.warning(
                        "User chose to keep waiting for overloaded Gemini model %s; "
                        "retrying indefinitely every %ds until success or cancellation.",
                        current_model, RETRY_DELAY_SEC,
                    )
            else:
                overload_failures = 0

            if (
                is_quota_exhausted_error(exc)
                and _QUOTA_DECISION_HANDLER is not None
                and current_model not in quota_prompted_models
            ):
                quota_prompted_models.add(current_model)
                decision = _QUOTA_DECISION_HANDLER(current_model, exc)
                if decision is False:
                    raise GeminiRetryCancelledError(
                        _("Processing stopped by the user while waiting for Gemini quota.")
                    ) from exc
                if decision:
                    new_model = validate_model_for_generate_content(
                        decision, client=client, status_callback=status_callback
                    )
                    normalized_new_model = normalize_model_id(new_model)
                    if (
                        normalized_new_model.casefold() == current_model.casefold()
                        or normalized_new_model in quota_prompted_models
                    ):
                        app_logger.error(
                            "Rejected quota switch to exhausted Gemini model: %s -> %s.",
                            current_model, normalized_new_model,
                        )
                        raise GeminiRetryCancelledError(
                            _(
                                "The selected Gemini model has already exhausted its quota. "
                                "Choose a different model."
                            )
                        ) from exc
                    app_logger.warning(
                        "Continuing current Gemini request after quota failure: %s -> %s.",
                        current_model, normalized_new_model,
                    )
                    current_model = normalized_new_model
                    updated_settings = config_model.load_settings()
                    updated_settings["gemini_model_override"] = normalized_new_model
                    config_model.save_settings(updated_settings)
                    attempt = 0
                    continue

            retry_msg = _(
                "Connection or temporary API error (attempt %(attempt)d). "
                "Retrying in %(delay)d seconds..."
            ) % {"attempt": attempt, "delay": RETRY_DELAY_SEC}
            app_logger.warning(
                "Transient error on Gemini request attempt %d (model %s): %s. "
                "Retrying in %ds...",
                attempt, current_model, exc, RETRY_DELAY_SEC,
            )
            if status_callback:
                status_callback(retry_msg)
            time.sleep(RETRY_DELAY_SEC)


def process_gemini_response(response, status_callback):
    """Processes a Gemini response, handling thoughts and errors."""
    final_text_parts = []
    thoughts_found = False

    if hasattr(response, 'prompt_feedback') and response.prompt_feedback and response.prompt_feedback.block_reason:
        reason_obj = response.prompt_feedback.block_reason
        reason = getattr(reason_obj, "name", None) or str(reason_obj)
        extra = getattr(response.prompt_feedback, "block_reason_message", None) or ""
        app_logger.warning(
            "Gemini prompt_feedback block: reason=%s message=%s ratings=%s usage=%s",
            reason, extra,
            getattr(response.prompt_feedback, "safety_ratings", None),
            getattr(response, "usage_metadata", None),
        )
        # If candidates still exist, prefer them over a hard fail on soft blocks.
        has_candidates = bool(getattr(response, "candidates", None))
        if has_candidates:
            app_logger.warning(
                "Block reason %s reported but candidates are present; continuing with candidates.",
                reason,
            )
        else:
            if reason in ("OTHER", "BlockReason.OTHER", "BLOCK_REASON_UNSPECIFIED"):
                block_msg = _(
                    "Gemini refused this video (block reason: %s). "
                    "This is often a non-overridable content-policy block (e.g. adult / prohibited "
                    "material), not a wrong model name. Try a different video, or a paid API key "
                    "with fewer free-tier restrictions."
                ) % reason
            else:
                block_msg = _("AI request was blocked due to: %s") % reason
            if extra:
                block_msg = f"{block_msg} ({extra})"
            if status_callback:
                status_callback(block_msg)
            raise ContentBlockedError(block_msg, reason=reason)

    if not hasattr(response, 'candidates') or not response.candidates:
        if status_callback:
            status_callback(_("AI returned no answer or an invalid response structure."))
        app_logger.warning("Response has no candidates list or it's empty.")
        return "", False

    candidate = response.candidates[0]

    if not hasattr(candidate, 'content') or not candidate.content or not candidate.content.parts:
        finish_reason_obj = getattr(candidate, 'finish_reason', None)
        finish_reason_name = getattr(finish_reason_obj, 'name', 'UNKNOWN') if finish_reason_obj else 'NOT_SPECIFIED'

        if finish_reason_name == 'MAX_TOKENS':
            block_msg = _("AI process stopped because it reached its processing limit (MAX_TOKENS).")
            if status_callback:
                status_callback(block_msg)
            app_logger.warning("Response candidate finished with reason 'MAX_TOKENS' and had no content parts.")
            raise TokenLimitError(block_msg, reason=finish_reason_name)

        if str(finish_reason_name).upper().endswith('MALFORMED_RESPONSE'):
            message = _("Gemini returned a temporary malformed response with no content.")
            if status_callback:
                status_callback(message)
            app_logger.warning(
                "Response candidate finished with MALFORMED_RESPONSE and had no content parts."
            )
            raise GeminiMalformedResponseError(message)

        if finish_reason_name != 'STOP':
            block_msg = _("AI content generation was stopped. Reason: %s") % finish_reason_name
            if status_callback:
                status_callback(block_msg)
            app_logger.warning(f"Response candidate finished with reason '{finish_reason_name}' and had no content parts.")
            raise ContentBlockedError(block_msg, reason=finish_reason_name)
        else:
            if status_callback:
                status_callback(_("AI returned an empty response."))
            app_logger.warning("Response candidate finished with STOP but had no content parts.")
            return "", False

    for part in candidate.content.parts:
        text_content = getattr(part, 'text', "") or ""
        if getattr(part, "thought", None):
            thoughts_found = True
            if status_callback:
                status_callback(_("AI is thinking..."))
            app_logger.info(f"🧠 AI Thought: {text_content.strip()}")
        else:
            final_text_parts.append(text_content)

    if thoughts_found:
        app_logger.info("AI completed thinking process.")

    final_text_result = "".join(final_text_parts).strip()

    if hasattr(candidate, 'finish_reason') and candidate.finish_reason.name == 'SAFETY':
        reason = candidate.finish_reason.name
        block_msg = _("AI content generation was stopped due to: %s") % reason
        if status_callback:
            status_callback(block_msg)
        app_logger.warning(block_msg)
        raise ContentBlockedError(block_msg, reason=reason)

    if not final_text_result and not thoughts_found:
         app_logger.warning("Processed response, but final text and thoughts are empty.")
         return "", False

    return final_text_result, True

def _serialize_gemini_response_to_dict(obj):
    """
    Recursively converts a Gemini SDK object into a dictionary suitable for JSON serialization.
    """
    if isinstance(obj, (str, int, float, bool)) or obj is None:
        return obj

    if isinstance(obj, (list, tuple)):
        return [_serialize_gemini_response_to_dict(item) for item in obj]

    if hasattr(obj, 'pb') and hasattr(obj.pb, 'DESCRIPTOR'):
        try:
            from google.protobuf.json_format import MessageToDict
            return MessageToDict(obj.pb, preserving_proto_field_name=True, use_integers_for_enums=True)
        except ImportError:
            app_logger.warning("google.protobuf.json_format.MessageToDict not found. Falling back to manual serialization for Protobuf objects.")
            d = {}
            for field in obj.pb.DESCRIPTOR.fields:
                field_name = field.name
                if hasattr(obj.pb, field_name):
                    value = getattr(obj.pb, field_name)
                    d[field_name] = _serialize_gemini_response_to_dict(value)
            return d

    if hasattr(obj, '__dict__'):
        d = {}
        for k, v in obj.__dict__.items():
            if k.startswith('_'):
                continue
            d[k] = _serialize_gemini_response_to_dict(v)
        return d

    if hasattr(obj, 'name') and isinstance(obj.name, str):
        return obj.name

    return str(obj)

def save_raw_ai_output(video_filename: str, output_type: str, content: Union[str, Any], suffix: str = ""):
    """
    Saves the raw AI response (either string or object) to a file in a debug folder if running in non-frozen mode.
    """
    if getattr(sys, 'frozen', False):
        return

    try:
        _lazy_import_gemini_sdk()

        sanitized_video_name = re.sub(r'[\\/:*?"<>|]', '_', os.path.splitext(video_filename)[0])[:50]
        timestamp = datetime.datetime.now().strftime("%Y%m%d_%H%M%S")

        file_extension = ".txt"
        data_to_write_str = ""

        if isinstance(content, str):
            data_to_write_str = content
            file_extension = ".txt"
        elif GEMINI_SDK_AVAILABLE and isinstance(content, genai.types.GenerateContentResponse):
            data_dict = _serialize_gemini_response_to_dict(content)
            data_to_write_str = json.dumps(data_dict, indent=4, ensure_ascii=False)
            file_extension = ".json"
        else:
            try:
                data_to_write_str = json.dumps(content, indent=4, ensure_ascii=False)
                file_extension = ".json"
            except TypeError:
                app_logger.error(f"Cannot serialize object of type {type(content)} to JSON directly. Saving as plain text (repr).", exc_info=True)
                data_to_write_str = repr(content)
                file_extension = ".txt"

        output_filename = f"{sanitized_video_name}_{output_type}{suffix}_{timestamp}{file_extension}"
        output_dir = os.path.abspath(os.path.join(config.get_app_root(), "..", "debug_ai_outputs"))
        os.makedirs(output_dir, exist_ok=True)

        file_path = os.path.join(output_dir, output_filename)

        with open(file_path, 'w', encoding='utf-8') as f:
            f.write(data_to_write_str)

        app_logger.info(f"Raw AI output saved to: {file_path}")
    except Exception as e:
        app_logger.error(f"Failed to save raw AI output to file: {e}", exc_info=True)
