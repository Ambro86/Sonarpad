#!/usr/bin/env python3
from __future__ import annotations

import argparse
import base64
import json
import os
import re
import shutil
import subprocess
import sys
import types
from collections import deque
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

import requests


def _pick_addon_root() -> Path:
    candidates = [
        Path(os.path.expandvars(r"%APPDATA%\Kodi\addons\plugin.video.thegroove360")),
        Path(os.path.expandvars(r"%USERPROFILE%\Downloads\plugin.video.thegroove360")),
    ]
    for candidate in candidates:
        if (candidate / "resources" / "modules" / "thegroove" / "Token.py").is_file():
            return candidate
    raise FileNotFoundError("plugin.video.thegroove360 non trovato")


def _load_server_url(addon_root: Path) -> str:
    token_json = addon_root / "resources" / "modules" / "thegroove" / "token.json"
    try:
        payload = json.loads(token_json.read_text(encoding="utf-8"))
        url = str(payload.get("url") or "").strip()
        if url:
            return url if url.endswith("/") else url + "/"
    except (OSError, json.JSONDecodeError):
        pass
    return "https://thegroove360.org/"


ADDON_ROOT = _pick_addon_root()
if str(ADDON_ROOT) not in sys.path:
    sys.path.insert(0, str(ADDON_ROOT))

SERVER_URL = _load_server_url(ADDON_ROOT)
ITEM_BLOCK_RE = re.compile(
    r"(<item>(?:.*?)</item>|<dir>(?:.*?)</dir>|<plugin>(?:.*?)</plugin>)",
    re.MULTILINE | re.DOTALL,
)
TAG_RE = re.compile(r"\[/?[A-Z]+(?: [^\]]+)?\]")
INTERACTIVE_SECTIONS = [
    ("Ricerca", ""),
    ("Serie TV", "/thegroove/scripters/Torrent/path=serieTG360.php"),
    ("Film", "/thegroove/scripters/Torrent/path=filmTG360.php"),
    ("TV Show", "/thegroove/scripters/Torrent/path=showTG360.php"),
    ("Documentari", "/thegroove/scripters/Torrent/path=Docutg360.php"),
    ("Anime", "/thegroove/scripters/Torrent/path=animeTG360.php"),
    ("Sport", "/thegroove/php_script_loader?scripter=risroom&path=sport.xml"),
    ("TV / IPTV", "/thegroove/tvhome"),
]


@dataclass
class Item:
    label: str
    url: str
    thumb: str
    raw: str
    is_folder: bool
    sublinks: list[tuple[str, str]]


@dataclass
class EpisodeChoice:
    label: str
    url: str
    source_page: str


@dataclass
class CatalogEntry:
    label: str
    clean_label: str
    url: str
    thumb: str
    page: str
    depth: int
    is_folder: bool


class TheGrooveClient:
    def __init__(self, addon_root: Path) -> None:
        self.addon_root = addon_root
        self.session = requests.Session()
        self.session.headers.update({"User-Agent": "Mozilla/5.0"})
        self.token_class = self._load_token_class()

    def _load_token_class(self):
        from Crypto.Cipher import AES as crypto_aes
        addon_root = self.addon_root

        xbmcaddon = types.ModuleType("xbmcaddon")
        xbmcvfs = types.ModuleType("xbmcvfs")
        xbmcgui = types.ModuleType("xbmcgui")
        xbmc = types.ModuleType("xbmc")
        cryptodome = types.ModuleType("Cryptodome")
        cryptodome_cipher = types.ModuleType("Cryptodome.Cipher")

        class Addon:
            def __init__(self, id=None):
                self.id = id

            def getAddonInfo(self, key):
                if key == "path":
                    return str(addon_root)
                if key == "id":
                    return "plugin.video.thegroove360"
                return ""

        xbmcaddon.Addon = Addon
        xbmcvfs.translatePath = lambda p: p
        xbmc.getInfoLabel = lambda _: "plugin.video.thegroove360"
        cryptodome_cipher.AES = crypto_aes
        cryptodome.Cipher = cryptodome_cipher

        sys.modules["xbmcaddon"] = xbmcaddon
        sys.modules["xbmcvfs"] = xbmcvfs
        sys.modules["xbmcgui"] = xbmcgui
        sys.modules["xbmc"] = xbmc
        sys.modules["Cryptodome"] = cryptodome
        sys.modules["Cryptodome.Cipher"] = cryptodome_cipher

        namespace: dict[str, object] = {}
        token_py = self.addon_root / "resources" / "modules" / "thegroove" / "Token.py"
        exec(token_py.read_text(encoding="utf-8", errors="ignore"), namespace)
        return namespace["Token"]

    @staticmethod
    def _compose_page(page: str) -> str:
        page = page.strip()
        if ("/thegroove/scripters/" in page or page.startswith("scripters/")) and "path=" in page:
            if page.startswith("/thegroove/"):
                page = page[len("/thegroove/") :]
            page = page.replace("/thegroove/scripters/", "", 1).replace("scripters/", "", 1)
            scripter, subpage = page.split("/", 1)
            page = f"php_script_loader?scripter={scripter}&{subpage}"
        elif page.startswith("/thegroove/"):
            page = page[len("/thegroove/") :]
        if "?" not in page:
            return page
        route, params = page.split("?", 1)
        if not params:
            return route
        encoded = base64.urlsafe_b64encode(params.encode("utf-8")).decode("ascii")
        return f"{route}&page_params={encoded}"

    def fetch_page(self, page: str) -> str:
        token_obj = self.token_class()
        token_obj.set_token()
        route = self._compose_page(page)
        url = f"{SERVER_URL}loader.php?page={route}&token={token_obj.token}"
        response = self.session.get(url, timeout=20)
        response.raise_for_status()
        token_obj.set_result(response)
        if not token_obj.result:
            raise RuntimeError("header token non valido")
        return token_obj.result


def _tag_value(xml: str, tag_names: Iterable[str]) -> str:
    for tag in tag_names:
        match = re.search(fr"<{tag}(?:\s+[^>]*)?>(.*?)</{tag}>", xml, re.DOTALL)
        if match:
            return match.group(1).strip()
    return ""


def _extract_sublinks(xml: str) -> list[tuple[str, str]]:
    matches = re.findall(r"<sublink(.*?)>(.*?)</sublink>", xml, re.DOTALL)
    out: list[tuple[str, str]] = []
    for attrs, value in matches:
        name_match = re.search(r'name="([^"]+)"', attrs)
        out.append(((name_match.group(1) if name_match else "").strip(), value.strip()))
    return out


def parse_items(xml: str) -> list[Item]:
    items: list[Item] = []
    for block in ITEM_BLOCK_RE.findall(xml):
        label = _tag_value(block, ("title", "name"))
        url = _tag_value(block, ("link",))
        thumb = _tag_value(block, ("thumbnail",))
        sublinks = _extract_sublinks(block)
        is_folder = False
        if url.startswith("/thegroove/") or url.startswith(SERVER_URL):
            is_folder = True
        elif url.startswith("$doregex["):
            is_folder = False
        elif not url and not sublinks:
            is_folder = False
        items.append(
            Item(
                label=label,
                url=url,
                thumb=thumb,
                raw=block,
                is_folder=is_folder,
                sublinks=sublinks,
            )
        )
    return items


def crawl_for_query(
    client: TheGrooveClient,
    start_page: str,
    query: str,
    max_depth: int,
    max_pages: int,
) -> tuple[list[tuple[int, str, Item]], dict[str, list[Item]]]:
    needle = query.casefold()
    queue: deque[tuple[int, str]] = deque([(0, start_page)])
    visited: set[str] = set()
    pages: dict[str, list[Item]] = {}
    matches: list[tuple[int, str, Item]] = []

    while queue and len(visited) < max_pages:
        depth, page = queue.popleft()
        normalized = page.strip()
        if normalized in visited:
            continue
        visited.add(normalized)

        try:
            xml = client.fetch_page(page)
        except Exception as exc:
            print(f"[skip] {page} -> {exc}", file=sys.stderr)
            continue

        items = parse_items(xml)
        pages[normalized] = items

        for item in items:
            hay = f"{item.label} {item.url}".casefold()
            if needle in hay:
                matches.append((depth, normalized, item))
            if depth < max_depth and item.is_folder and item.url:
                queue.append((depth + 1, item.url))

    return matches, pages


def crawl_catalog(
    client: TheGrooveClient,
    start_page: str,
    max_depth: int,
    max_pages: int,
) -> list[CatalogEntry]:
    queue: deque[tuple[int, str]] = deque([(0, start_page)])
    visited: set[str] = set()
    out: list[CatalogEntry] = []

    while queue and len(visited) < max_pages:
        depth, page = queue.popleft()
        normalized = page.strip()
        if normalized in visited:
            continue
        visited.add(normalized)

        try:
            xml = client.fetch_page(page)
        except Exception as exc:
            print(f"[skip] {page} -> {exc}", file=sys.stderr)
            continue

        for item in parse_items(xml):
            clean_label = strip_formatting(item.label)
            if not is_noise(item):
                out.append(
                    CatalogEntry(
                        label=item.label,
                        clean_label=clean_label,
                        url=item.url,
                        thumb=item.thumb,
                        page=normalized,
                        depth=depth,
                        is_folder=item.is_folder,
                    )
                )
            if depth < max_depth and item.is_folder and item.url:
                queue.append((depth + 1, item.url))

    return out


def resolve_known_host(url: str) -> str:
    if "sibnet.ru" not in url:
        return url

    media_match = re.search(r"(?:shell\.php\?videoid=|video)([0-9A-Za-z]+)", url)
    if not media_match:
        return url

    headers = {
        "Referer": "https://video.sibnet.ru/",
        "User-Agent": "Mozilla/5.0",
    }
    shell_url = f"https://video.sibnet.ru/shell.php?videoid={media_match.group(1)}"
    response = requests.get(shell_url, headers=headers, timeout=20)
    response.raise_for_status()
    match = re.search(r'src:\s*"([^"]+)', response.text)
    if not match:
        return url
    return "https://video.sibnet.ru" + match.group(1)


def resolve_final_redirect_url(url: str) -> str:
    if "sibnet.ru" not in url:
        return url
    headers = {
        "Referer": "https://video.sibnet.ru/",
        "User-Agent": "Mozilla/5.0",
    }
    response = requests.get(url, headers=headers, timeout=20, stream=True, allow_redirects=True)
    try:
        return response.url or url
    finally:
        response.close()


def strip_formatting(text: str) -> str:
    text = TAG_RE.sub("", text or "")
    text = text.replace("\xa0", " ")
    text = re.sub(r"\s+", " ", text)
    return text.strip(" -|")


def is_noise(item: Item) -> bool:
    url = (item.url or "").strip()
    lowered = url.casefold()
    if lowered in {"ignore", "ignora"}:
        return True
    if not strip_formatting(item.label):
        return True
    if lowered.startswith("$doregex[") or lowered.startswith("plugin://"):
        return True
    return False


def matches_section(section: str, item: Item) -> bool:
    blob = f"{item.label} {item.url}".casefold()
    if section == "Serie TV":
        return "serietv_" in blob
    if section == "Film":
        return "serietv_" not in blob and "anime" not in blob
    if section == "Anime":
        return "anime" in blob
    if section == "Documentari":
        return "docu" in blob
    if section == "TV Show":
        return "show" in blob and "serietv_" not in blob
    if section == "Sport":
        return "sport" in blob
    if section == "TV / IPTV":
        return "tvhome" in blob or "iptv" in blob
    return True


def search_backend(client: TheGrooveClient, query: str) -> list[tuple[int, str, Item]]:
    query = query.strip()
    if not query:
        return []
    page = f"/thegroove/php_search?par={query}"
    xml = client.fetch_page(page)
    return [
        (0, page, item)
        for item in parse_items(xml)
        if not is_noise(item)
    ]


def collect_playable(
    client: TheGrooveClient,
    item: Item,
    depth: int = 0,
    max_depth: int = 2,
) -> list[EpisodeChoice]:
    label = strip_formatting(item.label) or "senza titolo"
    http_subs = [(name, url) for name, url in item.sublinks if url.startswith("http")]
    if http_subs:
        return [
            EpisodeChoice(
                label=strip_formatting(name) or label,
                url=url,
                source_page=item.url if item.url.startswith("/thegroove/") else "",
            )
            for name, url in http_subs
        ]
    if item.url.startswith("http") and not item.is_folder:
        return [EpisodeChoice(label=label, url=item.url, source_page="")]
    if depth >= max_depth or not item.is_folder or not item.url.startswith("/thegroove/"):
        return []
    try:
        children = parse_items(client.fetch_page(item.url))
    except Exception as exc:
        print(f"[skip] {item.url} -> {exc}", file=sys.stderr)
        return []
    found: list[EpisodeChoice] = []
    for child in children:
        if is_noise(child):
            continue
        found.extend(collect_playable(client, child, depth + 1, max_depth))
    return found


def safe_filename(text: str) -> str:
    cleaned = strip_formatting(text)
    cleaned = re.sub(r'[<>:"/\\|?*]', "_", cleaned)
    cleaned = re.sub(r"\s+", " ", cleaned).strip(" .")
    return cleaned or "download"


def choose_from_list(prompt: str, count: int) -> int | None:
    while True:
        raw = input(prompt).strip().lower()
        if raw in {"", "q", "quit", "exit"}:
            return None
        if raw.isdigit():
            idx = int(raw)
            if 1 <= idx <= count:
                return idx - 1
        print("Scelta non valida. Inserisci un numero o premi Invio per uscire.")


def choose_download_mode() -> str | None:
    while True:
        raw = input(
            "\nModalita download: [1] solo questo episodio, [2] da questo episodio in poi: "
        ).strip().lower()
        if raw in {"", "q", "quit", "exit"}:
            return None
        if raw == "1":
            return "single"
        if raw == "2":
            return "from_here"
        print("Scelta non valida. Inserisci 1, 2 o premi Invio per uscire.")


def choose_section() -> tuple[str, str] | None:
    print("Sezione:")
    for idx, (label, _) in enumerate(INTERACTIVE_SECTIONS, start=1):
        print(f"{idx}. {label}")
    selected = choose_from_list("\nNumero sezione: ", len(INTERACTIVE_SECTIONS))
    if selected is None:
        return None
    return INTERACTIVE_SECTIONS[selected]


def find_ytdlp() -> str | None:
    candidates = [
        shutil.which("yt-dlp.exe"),
        shutil.which("yt-dlp"),
        os.path.expandvars(r"%APPDATA%\Sonarpad\yt-dlp.exe"),
        os.path.expandvars(r"%APPDATA%\Sonarpad\bin\yt-dlp.exe"),
    ]
    for candidate in candidates:
        if candidate and Path(candidate).exists():
            return candidate
    return None


def find_aria2c() -> str | None:
    candidates = [
        shutil.which("aria2c.exe"),
        shutil.which("aria2c"),
        os.path.expandvars(r"%APPDATA%\Sonarpad\aria2c.exe"),
        os.path.expandvars(r"%APPDATA%\Sonarpad\bin\aria2c.exe"),
    ]
    for candidate in candidates:
        if candidate and Path(candidate).exists():
            return candidate
    return None


def default_download_dir() -> Path:
    return Path(os.path.expandvars(r"%APPDATA%\Sonarpad\downloads"))


def default_catalog_dir() -> Path:
    return Path(os.path.expandvars(r"%APPDATA%\Sonarpad"))


def collect_episode_choices(
    client: TheGrooveClient,
    matches: list[tuple[int, str, Item]],
) -> list[EpisodeChoice]:
    out: list[EpisodeChoice] = []
    for _, _, item in matches:
        out.extend(collect_playable(client, item))
    return out


def collect_section_entries(
    client: TheGrooveClient,
    start_page: str,
    max_depth: int,
    max_pages: int,
) -> list[tuple[int, str, Item]]:
    queue: deque[tuple[int, str]] = deque([(0, start_page)])
    visited: set[str] = set()
    out: list[tuple[int, str, Item]] = []

    while queue and len(visited) < max_pages:
        depth, page = queue.popleft()
        normalized = page.strip()
        if normalized in visited:
            continue
        visited.add(normalized)

        try:
            xml = client.fetch_page(page)
        except Exception as exc:
            print(f"[skip] {page} -> {exc}", file=sys.stderr)
            continue

        for item in parse_items(xml):
            if not is_noise(item):
                out.append((depth, normalized, item))
            if depth < max_depth and item.is_folder and item.url:
                queue.append((depth + 1, item.url))

    return out


def download_episode(choice: EpisodeChoice, resolve_sibnet: bool) -> int:
    ytdlp = find_ytdlp()
    if not ytdlp:
        print("yt-dlp.exe non trovato.")
        return 1
    aria2c = find_aria2c()

    download_dir = default_download_dir()
    download_dir.mkdir(parents=True, exist_ok=True)

    source_url = choice.url
    if "sibnet.ru" in source_url or resolve_sibnet:
        try:
            source_url = resolve_known_host(choice.url)
        except Exception:
            source_url = choice.url

    final_url = source_url
    if "sibnet.ru" in source_url:
        try:
            final_url = resolve_final_redirect_url(source_url)
        except Exception:
            final_url = source_url

    output_tpl = str(download_dir / f"{safe_filename(choice.label)}.%(ext)s")
    cmd = [ytdlp, "--no-part", "--force-overwrites", "-o", output_tpl]
    if aria2c:
        cmd.extend(
            [
                "--downloader",
                "aria2c",
                "--downloader-args",
                "aria2c:-x 16 -s 16 -k 1M --file-allocation=none --summary-interval=1",
            ]
        )

    if "sibnet.ru" in source_url or "sibnet.ru" in final_url:
        cmd.extend(
            [
                "--add-header",
                "Referer: https://video.sibnet.ru/",
                "--add-header",
                "User-Agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/137.0.0.0 Safari/537.36",
            ]
        )

    cmd.append(final_url)
    print(f"\nDownload: {choice.label}")
    print(f"URL: {source_url}")
    if final_url != source_url:
        print(f"Final URL: {final_url}")
    if aria2c:
        print(f"Downloader: aria2c ({aria2c})")
    else:
        print("Downloader: yt-dlp")
    print(f"Output: {output_tpl}")
    return subprocess.run(cmd, check=False).returncode


def download_episode_range(
    episodes: list[EpisodeChoice], start_index: int, resolve_sibnet: bool
) -> int:
    result = 0
    for offset, episode in enumerate(episodes[start_index:], start=start_index + 1):
        print(f"\n[{offset}/{len(episodes)}]")
        current = download_episode(episode, resolve_sibnet=resolve_sibnet)
        if current != 0:
            result = current
            print("Download interrotto per errore.")
            break
    return result


def run_interactive(client: TheGrooveClient, args: argparse.Namespace) -> int:
    section = choose_section()
    if section is None:
        return 0
    section_label, start_page = section

    query = input("Parola di ricerca: ").strip()
    print(f"\nRicerca in: {section_label}")
    if query:
        matches = search_backend(client, query)
        if section_label != "Ricerca":
            filtered = [item for item in matches if matches_section(section_label, item[2])]
            if filtered:
                matches = filtered
    elif not start_page:
        print("Scrivi una parola di ricerca.")
        return 1
    else:
        matches = collect_section_entries(
            client,
            start_page=start_page,
            max_depth=args.depth,
            max_pages=args.max_pages,
        )
    if not matches:
        print("Nessun risultato.")
        return 1

    print("\nRisultati:")
    for idx, (_, page, item) in enumerate(matches, start=1):
        print(f"{idx}. {strip_formatting(item.label)}")
        if item.thumb:
            print(f"   thumb: {item.thumb}")
        print(f"   page:  {page}")

    selected_series = choose_from_list("\nNumero serie da aprire: ", len(matches))
    if selected_series is None:
        return 0

    target_match = matches[selected_series]
    episodes = collect_episode_choices(client, [target_match])
    if not episodes:
        print("Nessun episodio scaricabile trovato per questa voce.")
        return 1

    print("\nEpisodi:")
    for idx, episode in enumerate(episodes, start=1):
        print(f"{idx}. {episode.label}")

    selected_episode = choose_from_list("\nNumero episodio da scaricare: ", len(episodes))
    if selected_episode is None:
        return 0

    mode = choose_download_mode()
    if mode is None:
        return 0
    if mode == "single":
        return download_episode(episodes[selected_episode], resolve_sibnet=args.resolve_sibnet)
    return download_episode_range(
        episodes,
        start_index=selected_episode,
        resolve_sibnet=args.resolve_sibnet,
    )


def dump_catalog(client: TheGrooveClient, args: argparse.Namespace) -> int:
    entries = crawl_catalog(
        client,
        start_page=args.start_page,
        max_depth=args.depth,
        max_pages=args.max_pages,
    )
    if not entries:
        print("Catalogo vuoto.")
        return 1

    out_dir = default_catalog_dir()
    out_dir.mkdir(parents=True, exist_ok=True)
    json_path = out_dir / "thegroove_catalog.json"
    txt_path = out_dir / "thegroove_catalog.txt"

    payload = [
        {
            "label": entry.clean_label,
            "raw_label": entry.label,
            "url": entry.url,
            "thumb": entry.thumb,
            "page": entry.page,
            "depth": entry.depth,
            "is_folder": entry.is_folder,
        }
        for entry in entries
    ]
    json_path.write_text(
        json.dumps(payload, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )

    lines = []
    for idx, entry in enumerate(entries, start=1):
        lines.append(f"{idx}. {entry.clean_label}")
        lines.append(f"   page:  {entry.page}")
        lines.append(f"   url:   {entry.url}")
        if entry.thumb:
            lines.append(f"   thumb: {entry.thumb}")
        lines.append(f"   depth: {entry.depth} | folder: {str(entry.is_folder).lower()}")
        lines.append("")
    txt_path.write_text("\n".join(lines), encoding="utf-8")

    print(f"Catalogo salvato in:\n- {json_path}\n- {txt_path}")
    print(f"Elementi: {len(entries)}")
    return 0


def print_episode_page(items: list[Item], resolve_sibnet: bool) -> None:
    for item in items:
        if not item.label and not item.url and not item.sublinks:
            continue
        print(f"- {item.label or '(senza titolo)'}")
        if item.url:
            raw = item.url
            final = raw
            print(f"  raw:   {raw}")
            if resolve_sibnet:
                try:
                    final = resolve_known_host(raw)
                except Exception:
                    final = raw
            if resolve_sibnet and final != raw:
                print(f"  final: {final}")
        for sub_name, sub_url in item.sublinks:
            print(f"  sublink: {sub_name or '(senza nome)'} -> {sub_url}")


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Cerca una serie nel backend TheGroove e stampa gli URL trovati."
    )
    parser.add_argument("--query", default="famiglia addams")
    parser.add_argument(
        "--start-page",
        default="",
        help="Se valorizzata, scorre quella pagina invece della ricerca php_search.",
    )
    parser.add_argument("--depth", type=int, default=2)
    parser.add_argument("--max-pages", type=int, default=120)
    parser.add_argument("--show-children", action="store_true")
    parser.add_argument("--resolve-sibnet", action="store_true")
    parser.add_argument("--interactive", action="store_true")
    parser.add_argument("--dump-all", action="store_true")
    args = parser.parse_args()

    client = TheGrooveClient(ADDON_ROOT)
    if args.dump_all:
        if not args.start_page:
            args.start_page = "/thegroove/scripters/Torrent/path=serieTG360.php"
        return dump_catalog(client, args)
    if args.interactive or len(sys.argv) == 1:
        return run_interactive(client, args)

    if args.start_page:
        matches, _pages = crawl_for_query(
            client,
            start_page=args.start_page,
            query=args.query,
            max_depth=args.depth,
            max_pages=args.max_pages,
        )
    else:
        matches = search_backend(client, args.query)

    if not matches:
        print("Nessun match trovato.")
        return 1

    print("Match trovati:")
    for idx, (depth, page, item) in enumerate(matches, start=1):
        print(f"{idx}. depth={depth} page={page}")
        print(f"   label: {item.label}")
        if item.url:
            print(f"   url:   {item.url}")
        if item.thumb:
            print(f"   thumb: {item.thumb}")

    if not args.show_children:
        return 0

    episodes = collect_episode_choices(client, [matches[0]])
    if not episodes:
        print("Il primo match non ha episodi o link apribili.")
        return 0

    print("\nContenuto:")
    for episode in episodes:
        print(f"- {episode.label}")
        raw = episode.url
        final = raw
        print(f"  raw:   {raw}")
        if args.resolve_sibnet:
            try:
                final = resolve_known_host(raw)
            except Exception:
                final = raw
        if args.resolve_sibnet and final != raw:
            print(f"  final: {final}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
