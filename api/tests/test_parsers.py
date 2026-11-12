"""Parser tests against real captured provider responses (no synthetic data)."""
import re
from pathlib import Path

import pytest

FIX = Path(__file__).parent / "fixtures"


def test_search_parses_real_page():
    from ani_api.adapters.hianime import _FILM
    body = (FIX / "search_solo_full.html").read_text(errors="replace").replace("\n", " ")
    hits = _FILM.findall(body)
    assert hits, "expected film-name matches on a real search page"
    seen, unique = set(), []
    for slug, title in hits:
        if slug not in seen:
            seen.add(slug)
            unique.append((slug, title))
    assert unique[0][0] == "solo-leveling-235"
    assert unique[0][1] == "Solo Leveling"
    assert len(unique) == 16


def test_episodes_parse_real_payload():
    raw = (FIX / "episodes_real_235.json").read_text(errors="replace")
    flat = raw.replace("\\", "").replace("ep-item", "\nep-item")
    eps = re.findall(r'data-number="([^"]*)".*?data-id="([0-9]+)".*?/watch/[^?]*\?ep=', flat, re.S)
    assert eps, "expected episode entries"
    assert eps[0] == ("1", "4341")


def test_playlist_variants_real_master():
    from ani_api.adapters.playlist import parse_master
    body = (FIX / "master_solo_s1e1_sub.m3u8").read_text()
    master = "https://hls.dramahot.top/p/x/y/z/98ri2ajbgpcva6/master.m3u8"
    variants = parse_master(body, master)
    assert [v.height for v in variants] == [1080, 360], "best-first ordering"
    assert variants[0].quality == "1080p"
    assert variants[0].playlist_url.endswith("/1080/index.m3u8")


def test_first_segment_real_media_playlist():
    from ani_api.adapters.playlist import parse_first_segment
    body = (FIX / "media_solo_s1e1_sub_1080.m3u8").read_text()
    seg = parse_first_segment(body, "https://hls.dramahot.top/p/x/y/z/98ri2ajbgpcva6/1080/index.m3u8")
    assert seg and seg.endswith(".ts.jpg"), seg


def test_deobfuscate_real_embed_blob():
    """The XOR key and pipeline must reproduce the provider's JSON exactly."""
    import json
    from ani_api.adapters.zokoanime import deobfuscate
    blob = (FIX / "embed_blob_zoko_solo_s1e1_sub.txt").read_text().strip()
    payload = json.loads(deobfuscate(blob).decode("utf-8", "replace").rstrip("\x00"))
    assert payload["src"].endswith(".m3u8")
    assert payload["subtitles"], "expected at least one subtitle track"
    assert payload["subtitles"][0]["src"].endswith(".vtt")
