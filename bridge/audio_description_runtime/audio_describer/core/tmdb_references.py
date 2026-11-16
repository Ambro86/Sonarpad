"""Optional TMDB cast context. Pictures are evidence, never verified identities."""
import hashlib
import json
import os
from pathlib import Path
import time
import urllib.parse
import urllib.request

from ..utils.logger import app_logger

MAX_IMAGE_BYTES = 1024 * 1024

ACTIVE_CAST_RULE = 'ACTIVE CAST MATCHING: The supplied TMDB listing is authoritative for which character each listed actor portrays in this selected production/season/episode. Before writing each description, actively compare clearly visible faces with the supplied actor portraits. If you confidently recognize a listed actor in the video, use the associated character name from their FIRST visible appearance in this clip, even if dialogue has not yet spoken the name. Do not unnecessarily replace a recognized character with a generic age/gender label. Do not wait for later dialogue to confirm an already confident visual match. Account for hairstyle, costume and age differences. The cast listing alone does not prove that a person in a frame is that actor: if the face is unclear or the match is uncertain, retain a neutral description. Never force a match merely because someone is in the cast. All timing and dialogue-protection rules remain unchanged. CAST COMPLETENESS: The TMDB cast may be incomplete. A person visible in the video may be absent from the supplied catalog and photographs. Never assume that every visible person must match a listed character, and never assign the closest-looking available character merely because the correct person is missing. If the available references do not support a confident match, use a name only when the video clearly and unambiguously identifies that person; otherwise use a neutral identifying visual description. A missing cast entry is not evidence that this person is someone else in the list. This does not prevent using a listed character name when the visual match is confident.'

LOCALIZATION_RULE = "CHARACTER NAME LOCALIZATION: TMDB character fields may contain untranslated original or English names. For TMDB-derived or provisional identities, use the established conventional character name in the requested audio-description language in BOTH character_glossary.name and description_text. Prefer the name clearly used in the video's target-language dialogue when available. Do not translate actor names or change character IDs, TMDB metadata, or explicitly user-confirmed names. Do not translate proper names literally or invent localized names: if no established localized form is known, retain the original. This rule concerns name spelling only, and never establishes who is visible in the video. Keep one consistent localized name per identity throughout the output. This is an explicit exception to instructions to keep names unchanged, only for TMDB-derived or provisional identities."


def valid_image_url(url):
    parsed = urllib.parse.urlsplit(str(url or ""))
    return (parsed.scheme == "https" and parsed.netloc == "image.tmdb.org"
            and parsed.path.startswith("/t/p/w185/")
            and parsed.path.endswith(".jpg") and ".." not in parsed.path
            and not parsed.query and not parsed.fragment)


def download_image(url, cache_dir=None):
    if not valid_image_url(url):
        raise ValueError("Invalid TMDB portrait URL")
    directory = Path(cache_dir or Path(os.environ.get("LOCALAPPDATA") or Path.home())
                     / "Sonarpad" / "tmdb_portraits")
    directory.mkdir(parents=True, exist_ok=True)
    target = directory / (hashlib.sha256(url.encode()).hexdigest() + ".jpg")
    if target.exists() and time.time() - target.stat().st_mtime < 30 * 86400:
        data = target.read_bytes()
        if 0 < len(data) <= MAX_IMAGE_BYTES and data.startswith(b"\xff\xd8\xff"):
            return target
    request = urllib.request.Request(url, headers={"User-Agent": "Sonarpad/TMDB"})
    with urllib.request.urlopen(request, timeout=12) as response:
        if not valid_image_url(response.geturl()):
            raise ValueError("Unexpected portrait redirect")
        data = response.read(MAX_IMAGE_BYTES + 1)
    if not data.startswith(b"\xff\xd8\xff") or len(data) > MAX_IMAGE_BYTES:
        raise ValueError("Invalid or oversized portrait")
    target.write_bytes(data)
    # Bounded local cache; nothing is embedded in the character JSON.
    for old in sorted(directory.glob("*.jpg"), key=lambda item: item.stat().st_mtime)[:-128]:
        old.unlink(missing_ok=True)
    return target


def prepare(client, glossary):
    candidates = [item for item in glossary or [] if isinstance(item.get("tmdb"), dict)][:96]
    if not candidates:
        return [], []
    from google.genai import types
    established_ids = [item.get("id") for item in glossary
                       if not item.get("tmdb") and not item.get("provisional")]
    context = (
        "\nTMDB OPTIONAL CHARACTER REFERENCES: The following is a verified cast listing, "
        "NOT verified identification of anyone in the video. Actor photographs may differ "
        "in age, hairstyle, makeup or costume. Use them only as supporting visual evidence. "
        "A cast listing or weak resemblance alone NEVER establishes identity. Only name a "
        "person when the video and clear visual/dialogue evidence support that match; "
        "otherwise use a concise neutral visual description. Names spoken and clearly linked "
        "in the video and user catalog names take precedence over untranslated cast names. "
        "Pictures of voice actors are omitted for animation; describe animated characters "
        "normally using the video. References are not frames from the video: never describe "
        "these portraits as scenes. All identities inferred in previous chunks remain "
        "PROVISIONAL and may be corrected; repetition is not independent evidence. "
        "Where other instructions call continuity AUTHORITATIVE, that applies ONLY to "
        "these user catalog IDs: " + json.dumps(established_ids, ensure_ascii=False) + ".\n"
        "CAST CANDIDATES:\n" + json.dumps(candidates, ensure_ascii=False, separators=(",", ":"))
    )
    parts = [context + "\n" + LOCALIZATION_RULE + "\n" + ACTIVE_CAST_RULE]
    uploaded = []
    seen = set()
    for item in candidates:
        reference = item["tmdb"]
        url = reference.get("profile_image_url")
        if reference.get("animation") or not valid_image_url(url) or url in seen:
            continue
        seen.add(url)
        try:
            path = download_image(url)
            file = client.files.upload(file=str(path), config={"mime_type": "image/jpeg"})
            uploaded.append(file)
            parts.extend([
                "Reference portrait of actor " + str(reference.get("actor_name", ""))
                + "; candidate role: " + str(item.get("name", ""))
                + ". Not a frame from the video; not a confirmed identity.",
                types.Part.from_uri(file_uri=file.uri, mime_type="image/jpeg"),
            ])
        except Exception as exc:
            app_logger.warning("Optional TMDB portrait unavailable (%s); continuing.", type(exc).__name__)
    app_logger.info("TMDB references: %d candidates, %d portraits attached.", len(candidates), (len(parts)-1)//2)
    return parts, uploaded


def with_references(client, contents):
    """Enrich existing video calls, including retry/minute fallback, never text-only calls."""
    references = getattr(client, "_sonarpad_character_references", None)
    if not isinstance(references, list) or not references or not isinstance(contents, list):
        return contents
    for part in contents:
        data = getattr(part, "file_data", None) or getattr(part, "inline_data", None)
        mime = getattr(data, "mime_type", "") if data is not None else ""
        if isinstance(mime, str) and mime.startswith("video/"):
            return contents + references
    return contents
