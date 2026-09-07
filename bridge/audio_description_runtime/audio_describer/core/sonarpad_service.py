"""Sonarpad AI service transport.

The PHP backend never receives media bytes. It only authenticates the user,
creates a Gemini resumable-upload session, authorizes generateContent, and
accounts usage. Video bytes are uploaded directly from the user's PC to the
temporary Google upload URL returned by the backend.
"""
from __future__ import annotations

import json
import mimetypes
import os
import re
import socket
import urllib.error
import urllib.request
import uuid
from dataclasses import dataclass
from types import SimpleNamespace


class SonarpadServiceError(Exception):
    def __init__(self, message: str, status_code: int | None = None):
        super().__init__(message)
        self.status_code = status_code
        self.code = status_code


class _EnumValue:
    def __init__(self, name):
        self.name = str(name or "")

    def __str__(self):
        return self.name


def _snake_to_camel(name: str) -> str:
    if "_" not in name:
        return name
    first, *rest = name.split("_")
    return first + "".join(part[:1].upper() + part[1:] for part in rest)


def _jsonable(value):
    if value is None or isinstance(value, (str, int, float, bool)):
        return value
    if isinstance(value, (list, tuple)):
        return [_jsonable(item) for item in value]
    if isinstance(value, dict):
        return {_snake_to_camel(str(key)): _jsonable(item) for key, item in value.items() if item is not None}
    model_dump = getattr(value, "model_dump", None)
    if callable(model_dump):
        try:
            dumped = model_dump(mode="json", by_alias=True, exclude_none=True)
        except TypeError:
            dumped = model_dump(by_alias=True, exclude_none=True)
        return _jsonable(dumped)
    enum_value = getattr(value, "value", None)
    if enum_value is not None and isinstance(enum_value, (str, int, float, bool)):
        return enum_value
    if hasattr(value, "__dict__"):
        return _jsonable({key: item for key, item in vars(value).items() if not key.startswith("_")})
    return str(value)


def _normalize_contents_for_rest(contents):
    """Convert google-genai convenience contents into REST Content objects.

    The Python SDK accepts convenience forms such as:
        ["prompt text", types.Part(file_data=...)]
    while the REST generateContent endpoint requires:
        [{"role": "user", "parts": [{"text": ...}, {"fileData": ...}]}]
    """
    raw = _jsonable(contents)

    def is_content(item):
        return isinstance(item, dict) and isinstance(item.get("parts"), list)

    def as_part(item):
        if isinstance(item, str):
            return {"text": item}
        if isinstance(item, dict):
            if is_content(item):
                return None
            return item
        if item is None:
            return None
        return {"text": str(item)}

    if isinstance(raw, dict):
        if is_content(raw):
            return [raw]
        part = as_part(raw)
        return [{"role": "user", "parts": [part]}] if part is not None else []

    if not isinstance(raw, list):
        part = as_part(raw)
        return [{"role": "user", "parts": [part]}] if part is not None else []

    if raw and all(is_content(item) for item in raw):
        return raw

    parts = []
    for item in raw:
        if is_content(item):
            parts.extend(item.get("parts") or [])
            continue
        part = as_part(item)
        if part is not None:
            parts.append(part)
    return [{"role": "user", "parts": parts}]


def _normalize_system_instruction_for_rest(value):
    raw = _jsonable(value)
    if raw is None:
        return None
    if isinstance(raw, dict) and isinstance(raw.get("parts"), list):
        return raw
    if isinstance(raw, str):
        return {"parts": [{"text": raw}]}
    if isinstance(raw, list):
        parts = []
        for item in raw:
            if isinstance(item, str):
                parts.append({"text": item})
            elif isinstance(item, dict):
                if isinstance(item.get("parts"), list):
                    parts.extend(item["parts"])
                else:
                    parts.append(item)
        return {"parts": parts}
    if isinstance(raw, dict):
        return {"parts": [raw]}
    return {"parts": [{"text": str(raw)}]}


def _objectify(value, key_name: str = ""):
    if isinstance(value, list):
        return [_objectify(item) for item in value]
    if not isinstance(value, dict):
        if key_name in {"finish_reason", "block_reason", "state"} and value is not None:
            return _EnumValue(value)
        return value
    converted = {}
    for key, item in value.items():
        snake = re.sub(r"(?<!^)(?=[A-Z])", "_", str(key)).lower()
        converted[snake] = _objectify(item, snake)
    return SimpleNamespace(**converted)


def _http_error_message(status: int, raw: str) -> str:
    message = raw.strip()
    try:
        value = json.loads(raw)
        if isinstance(value, dict):
            err = value.get("error")
            if isinstance(err, dict):
                message = str(err.get("message") or err.get("status") or err)
            elif err:
                message = str(err)
    except Exception:
        pass
    if status == 401:
        return f"HTTP 401 Sonarpad AI authentication failed: {message}"
    if status == 402:
        return f"HTTP 402 Sonarpad AI credit exhausted: {message}"
    if status == 403:
        return f"HTTP 403 Sonarpad AI access denied: {message}"
    if status == 429:
        return f"HTTP 429 Sonarpad AI rate limit or quota exceeded: {message}"
    return f"HTTP {status} Sonarpad AI request failed: {message}"


@dataclass
class _UploadRecord:
    upload_id: str
    file_name: str
    mime_type: str


class SonarpadServiceClient:
    def __init__(self, base_url: str, access_code: str, device_id: str, device_name: str = "Sonarpad Windows"):
        self.base_url = str(base_url or "").rstrip("/")
        self.access_code = str(access_code or "").strip()
        self.device_id = str(device_id or "").strip()
        self.device_name = str(device_name or "Sonarpad Windows")[:100]
        if not self.base_url.startswith("https://"):
            raise SonarpadServiceError("Sonarpad AI backend must use HTTPS.")
        if not self.access_code.startswith("sp_"):
            raise SonarpadServiceError("Invalid Sonarpad AI access code.")
        if not self.device_id:
            raise SonarpadServiceError("Missing Sonarpad AI device identifier.")
        self.session_token = ""
        self.account = {}
        self._uploads: dict[str, _UploadRecord] = {}
        self._activate()
        self._load_account()
        self.files = _FilesApi(self)
        self.models = _ModelsApi(self)

    @property
    def model(self) -> str:
        return str(self.account.get("model") or "gemini-3.8-flash")

    @property
    def balance_eur(self) -> float:
        try:
            return float(self.account.get("balance_eur") or 0.0)
        except (TypeError, ValueError):
            return 0.0

    def _request_json(self, method: str, path_or_url: str, payload=None, *, auth=True, headers=None, timeout=650):
        if path_or_url.startswith("https://"):
            url = path_or_url
        else:
            url = self.base_url + "/" + path_or_url.lstrip("/")
        body = None
        request_headers = {"User-Agent": "Sonarpad-AI/0.9.5", "Accept": "application/json"}
        if payload is not None:
            body = json.dumps(payload, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
            request_headers["Content-Type"] = "application/json; charset=utf-8"
        if auth and self.session_token:
            request_headers["Authorization"] = "Bearer " + self.session_token
        if headers:
            request_headers.update(headers)
        request = urllib.request.Request(url, data=body, headers=request_headers, method=method)
        try:
            with urllib.request.urlopen(request, timeout=timeout) as response:
                raw = response.read().decode("utf-8", errors="replace")
                if not raw.strip():
                    return {}
                return json.loads(raw)
        except urllib.error.HTTPError as exc:
            raw = exc.read().decode("utf-8", errors="replace")
            raise SonarpadServiceError(_http_error_message(exc.code, raw), exc.code) from exc
        except (urllib.error.URLError, TimeoutError, socket.timeout, OSError) as exc:
            raise SonarpadServiceError(f"Sonarpad AI network error: {exc}", 503) from exc
        except json.JSONDecodeError as exc:
            raise SonarpadServiceError("Sonarpad AI returned invalid JSON.", 502) from exc

    def _activate(self):
        value = self._request_json(
            "POST",
            "/v1/activate",
            {"code": self.access_code, "device_id": self.device_id, "device_name": self.device_name},
            auth=False,
            timeout=60,
        )
        token = str(value.get("session_token") or "").strip()
        if not token.startswith("sst_"):
            raise SonarpadServiceError("Sonarpad AI did not return a valid session token.", 502)
        self.session_token = token

    def _load_account(self):
        value = self._request_json("GET", "/v1/account", None, timeout=60)
        if not isinstance(value, dict):
            raise SonarpadServiceError("Invalid Sonarpad AI account response.", 502)
        self.account = value

    def start_upload(self, path: str, mime_type: str):
        size = os.path.getsize(path)
        value = self._request_json(
            "POST",
            "/v1/upload/start",
            {"mime_type": mime_type, "bytes": size, "display_name": os.path.basename(path)},
            timeout=60,
        )
        upload_id = str(value.get("upload_id") or "")
        upload_url = str(value.get("upload_url") or "")
        if not upload_id or not upload_url.startswith("https://"):
            raise SonarpadServiceError("Sonarpad AI did not return a valid Google upload session.", 502)
        return upload_id, upload_url

    def direct_google_upload(self, path: str, mime_type: str, upload_url: str):
        # Important: the media body goes straight to Google's temporary upload URL.
        # No video bytes are posted to sonarpad.com.
        with open(path, "rb") as handle:
            data = handle.read()
        headers = {
            "Content-Type": mime_type,
            "Content-Length": str(len(data)),
            "X-Goog-Upload-Offset": "0",
            "X-Goog-Upload-Command": "upload, finalize",
            "User-Agent": "Sonarpad-AI/0.9.5",
        }
        request = urllib.request.Request(upload_url, data=data, headers=headers, method="POST")
        try:
            with urllib.request.urlopen(request, timeout=650) as response:
                raw = response.read().decode("utf-8", errors="replace")
        except urllib.error.HTTPError as exc:
            raw = exc.read().decode("utf-8", errors="replace")
            raise SonarpadServiceError(_http_error_message(exc.code, raw), exc.code) from exc
        except (urllib.error.URLError, TimeoutError, socket.timeout, OSError) as exc:
            raise SonarpadServiceError(f"Direct Google upload network error: {exc}", 503) from exc
        try:
            value = json.loads(raw)
        except json.JSONDecodeError as exc:
            raise SonarpadServiceError("Google upload returned invalid JSON.", 502) from exc
        file_info = value.get("file") if isinstance(value, dict) else None
        if not isinstance(file_info, dict):
            file_info = value if isinstance(value, dict) else {}
        file_name = str(file_info.get("name") or "")
        if not file_name.startswith("files/"):
            raise SonarpadServiceError("Google upload did not return a file name.", 502)
        return file_name

    def complete_upload(self, upload_id: str, file_name: str, mime_type: str):
        value = self._request_json(
            "POST",
            "/v1/upload/complete",
            {"upload_id": upload_id, "file_name": file_name},
            timeout=60,
        )
        file_uri = str(value.get("file_uri") or "")
        if not file_uri:
            raise SonarpadServiceError("Sonarpad AI did not return a verified Google file URI.", 502)
        self._uploads[file_name] = _UploadRecord(upload_id, file_name, mime_type)
        # Match the subset of google.genai File attributes used by the existing
        # audio-description core without exposing the provider key.
        return _objectify({
            "name": file_name,
            "uri": file_uri,
            "mimeType": mime_type,
            "state": value.get("state"),
        })

    def generate(self, contents, config):
        config_dict = _jsonable(config) or {}
        if not isinstance(config_dict, dict):
            config_dict = {}
        body = {"contents": _normalize_contents_for_rest(contents)}
        special = {
            "systemInstruction": "systemInstruction",
            "safetySettings": "safetySettings",
            "tools": "tools",
            "toolConfig": "toolConfig",
            "cachedContent": "cachedContent",
        }
        generation = {}
        for key, item in config_dict.items():
            camel = _snake_to_camel(str(key))
            if camel in special:
                body[special[camel]] = (_normalize_system_instruction_for_rest(item) if camel == "systemInstruction" else item)
            elif camel in {"httpOptions", "serviceTier", "automaticFunctionCalling", "shouldReturnHttpResponse"}:
                continue
            else:
                generation[camel] = item
        if generation:
            body["generationConfig"] = generation
        idem = "ad_" + uuid.uuid4().hex
        value = self._request_json(
            "POST",
            "/v1/generate",
            body,
            headers={"X-Idempotency-Key": idem},
            timeout=650,
        )
        return _objectify(value)


class _FilesApi:
    def __init__(self, owner: SonarpadServiceClient):
        self.owner = owner

    def upload(self, file, config=None):
        path = os.fspath(file)
        cfg = _jsonable(config) if config is not None else {}
        mime_type = ""
        if isinstance(cfg, dict):
            mime_type = str(cfg.get("mimeType") or cfg.get("mime_type") or "")
        if not mime_type:
            mime_type = mimetypes.guess_type(path)[0] or "application/octet-stream"
        upload_id, upload_url = self.owner.start_upload(path, mime_type)
        file_name = self.owner.direct_google_upload(path, mime_type, upload_url)
        return self.owner.complete_upload(upload_id, file_name, mime_type)

    def get(self, name):
        record = self.owner._uploads.get(str(name))
        if record is None:
            raise SonarpadServiceError("Unknown Sonarpad AI upload reference.", 400)
        return self.owner.complete_upload(record.upload_id, record.file_name, record.mime_type)

    def delete(self, name):
        record = self.owner._uploads.get(str(name))
        if record is None:
            return None
        self.owner._request_json(
            "POST",
            "/v1/upload/delete",
            {"upload_id": record.upload_id},
            timeout=60,
        )
        self.owner._uploads.pop(str(name), None)
        return None


class _ModelsApi:
    def __init__(self, owner: SonarpadServiceClient):
        self.owner = owner

    def get(self, model):
        return SimpleNamespace(name=self.owner.model, supported_actions=["generateContent"])

    def list(self):
        return [self.get(self.owner.model)]

    def generate_content(self, model, contents, config=None):
        return self.owner.generate(contents, config)
