"""Checkpoint scheduler: due-time maths, dedupe, persistence, retry, sub/dub split."""
from datetime import datetime, timedelta, timezone

import pytest

from ani_api.workers import checkpoints as cp

NOW = datetime(2026, 11, 12, 20, 47, 10, tzinfo=timezone.utc)


def _state(tmp_path, audio="sub"):
    state = cp.load_state(tmp_path / "s.json")
    cp.add_target(state, {"id": f"solo_1_{audio}", "anime": "Solo Leveling", "episode": "1",
                          "audio": audio, "referer": "https://zokoanime.video/",
                          "master": "https://hls.dramahot.top/p/x/master.m3u8",
                          "extracted_at": cp._iso(NOW)})
    return state


def test_nothing_due_at_extraction_time(tmp_path):
    assert cp.due_checkpoints(_state(tmp_path), NOW) == []


def test_two_hour_checkpoint_fires_exactly_then(tmp_path):
    state = _state(tmp_path)
    just_before = NOW + timedelta(minutes=119)
    assert cp.due_checkpoints(state, just_before) == []
    due = cp.due_checkpoints(state, NOW + timedelta(minutes=120))
    assert [d["checkpoint"] for d in due] == ["2h"]
    assert due[0]["elapsed_minutes"] == 120


def test_all_checkpoints_fire_in_order(tmp_path):
    state = _state(tmp_path)
    assert [d["checkpoint"] for d in cp.due_checkpoints(state, NOW + timedelta(hours=2))] == ["2h"]
    assert [d["checkpoint"] for d in cp.due_checkpoints(state, NOW + timedelta(hours=7))] == ["2h", "6h"]
    at24 = [d["checkpoint"] for d in cp.due_checkpoints(state, NOW + timedelta(hours=24))]
    assert at24 == ["2h", "6h", "24h"]


def test_sub_and_dub_are_independent_targets(tmp_path):
    state = cp.load_state(tmp_path / "s.json")
    for audio in ("sub", "dub"):
        cp.add_target(state, {"id": f"solo_1_{audio}", "audio": audio, "anime": "Solo Leveling",
                              "episode": "1", "master": f"https://hls.dramahot.top/{audio}/master.m3u8",
                              "referer": "https://zokoanime.video/",
                              "extracted_at": cp._iso(NOW)})
    due = cp.due_checkpoints(state, NOW + timedelta(hours=2, minutes=1))
    assert len(due) == 2
    assert {d["target_id"] for d in due} == {"solo_1_sub", "solo_1_dub"}


def test_registered_target_is_never_replaced(tmp_path):
    """An existing target keeps its original URL even if re-registered later."""
    state = _state(tmp_path)
    cp.add_target(state, {"id": "solo_1_sub", "audio": "sub", "master": "https://other/new.m3u8",
                          "extracted_at": cp._iso(NOW + timedelta(hours=5))})
    assert state["targets"]["solo_1_sub"]["master"].endswith("/master.m3u8")
    assert state["targets"]["solo_1_sub"]["extracted_at"] == cp._iso(NOW)


async def test_run_once_records_once_and_persists(tmp_path, monkeypatch):
    state_path, csv_path = tmp_path / "s.json", tmp_path / "o.csv"
    state = cp.load_state(state_path)
    cp.add_target(state, {"id": "t1", "audio": "sub", "anime": "Solo Leveling", "episode": "1",
                          "master": "https://hls.dramahot.top/p/x/master.m3u8",
                          "extracted_at": cp._iso(NOW)})
    cp.save_state(state_path, state)

    calls = {"n": 0}

    async def fake_probe(http, target, retry=True):
        calls["n"] += 1
        return {"master_code": 200, "variant_code": 200, "segment_code": 206,
                "segment_bytes": 1024, "ranged_bytes": 1024, "age": "66",
                "cf_cache_status": "HIT", "cache_control": "public, max-age=1209600",
                "playlist_ok": True, "segment_ok": True, "attempts": 1, "error": None}

    monkeypatch.setattr(cp, "probe_target", fake_probe)
    first = await cp.run_once(state_path, csv_path, NOW + timedelta(hours=2))
    assert len(first) == 1 and first[0]["outcome"] == "playable"
    assert first[0]["checkpoint"] == "2h" and first[0]["playback_validated"] is True

    # re-running must not duplicate the checkpoint nor touch the network again
    second = await cp.run_once(state_path, csv_path, NOW + timedelta(hours=2, minutes=5))
    assert second == []
    assert calls["n"] == 1

    # a restart reloads state from disk and still refuses to duplicate
    reloaded = cp.load_state(state_path)
    assert reloaded["targets"]["t1"]["checkpoints"]["2h"]["status"] == "done"
    assert calls["n"] == 1
    assert csv_path.read_text().count("\n") == 2   # header + one row


async def test_persistence_survives_corrupt_file(tmp_path):
    state_path = tmp_path / "s.json"
    state_path.write_text('{"targets": {"t1": {"mast')     # simulate a crash mid-write
    state = cp.load_state(state_path)
    assert state == {"targets": {}}
    assert state_path.with_suffix(".corrupt").exists(), "corrupt state is preserved, not deleted"


async def test_retry_on_transient_then_success(tmp_path, monkeypatch):
    calls = {"n": 0}

    class Resp:
        def __init__(self, code):
            self.status_code, self.headers, self.content, self.text = code, {}, b"", ""

    class StubHttp:
        def __init__(self):
            self.client = self
        async def get(self, *a, **k):
            calls["n"] += 1
            if calls["n"] == 1:
                raise RuntimeError("connection reset")
            return Resp(200)

        async def aclose(self):
            return None

    monkeypatch.setattr(cp, "RETRY_DELAYS", (0, 0, 0))
    result = await cp.probe_target(StubHttp(), {"master": "https://x/m.m3u8", "referer": ""})
    assert result["attempts"] >= 2
    assert result["master_code"] == 200


def test_classify_names_cached_403():
    outcome, ok = cp.classify({"master_code": 200, "playlist_ok": True, "variant_code": 200,
                               "segment_code": 403, "ranged_bytes": 548,
                               "cf_cache_status": "HIT", "age": "66"})
    assert outcome == "cached_403" and ok is False
    outcome, ok = cp.classify({"master_code": 200, "playlist_ok": True, "variant_code": 200,
                               "segment_code": 403, "ranged_bytes": 548,
                               "cf_cache_status": None, "age": None})
    assert outcome == "blocked_403" and ok is False
