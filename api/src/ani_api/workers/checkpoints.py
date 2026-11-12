"""Exact URL-lifetime checkpoints (2h / 6h / 24h) with durable state.

Guarantees:
  * Only the ORIGINAL extracted URLs are probed. Nothing here re-extracts or
    substitutes a fresh URL, so a 2h result really describes the URL from t+0.
  * Each (target, checkpoint) pair is recorded once. Re-running the scheduler
    after a restart or crash cannot duplicate a checkpoint.
  * Sub and dub are separate targets, so one working audio variant never implies
    the other works.
  * Transient network errors retry with backoff; definitive HTTP answers are
    recorded immediately.
  * State lives in a JSON file and every result is appended to CSV, so history
    survives restarts.

    python -m ani_api.workers.checkpoints --state checkpoints.json \
        --csv observations.csv [--loop 300]
"""
from __future__ import annotations

import argparse
import asyncio
import csv
import json
import time
from datetime import datetime, timezone
from pathlib import Path

from ..adapters.playlist import parse_first_segment
from ..config import settings
from ..http_client import LoopClient

CHECKPOINTS = (("2h", 120), ("6h", 360), ("24h", 1440))
RETRY_DELAYS = (2, 5, 10)          # transient only: exceptions and 5xx
CSV_FIELDS = [
    "target_id", "provider", "anime", "episode", "audio", "checkpoint",
    "extracted_at", "checked_at", "elapsed_minutes", "master_code", "variant_code",
    "segment_code", "segment_bytes", "ranged_bytes", "age", "cf_cache_status",
    "cache_control", "playback_validated", "outcome", "attempts", "error",
]


def _iso(dt: datetime) -> str:
    return dt.astimezone(timezone.utc).isoformat(timespec="seconds")


def _parse(ts: str) -> datetime:
    return datetime.fromisoformat(ts.replace("Z", "+00:00"))


def load_state(path: Path) -> dict:
    if path.exists():
        try:
            return json.loads(path.read_text())
        except json.JSONDecodeError:
            # never lose history because of a half-written file
            broken = path.with_suffix(".corrupt")
            path.replace(broken)
    return {"targets": {}}


def save_state(path: Path, state: dict) -> None:
    tmp = path.with_suffix(".tmp")
    tmp.write_text(json.dumps(state, indent=1))
    tmp.replace(path)          # atomic: a crash cannot truncate the state


def add_target(state: dict, target: dict) -> None:
    """Register a captured extraction. Re-registering the same id is a no-op."""
    tid = target["id"]
    if tid in state["targets"]:
        return
    state["targets"][tid] = {
        "provider": target.get("provider", "zokoanime"),
        "anime": target.get("anime", ""),
        "episode": target.get("episode", ""),
        "audio": target.get("audio", "sub"),
        "master": target["master"],
        "referer": target.get("referer", ""),
        "extracted_at": target["extracted_at"],
        "checkpoints": {},
    }


def due_checkpoints(state: dict, now: datetime) -> list[dict]:
    due = []
    for tid, target in state["targets"].items():
        extracted = _parse(target["extracted_at"])
        for name, minutes in CHECKPOINTS:
            if name in target["checkpoints"]:
                continue
            fire_at = extracted.timestamp() + minutes * 60
            if now.timestamp() >= fire_at:
                due.append({"target_id": tid, "checkpoint": name, "minutes": minutes,
                            "due_at": _iso(datetime.fromtimestamp(fire_at, timezone.utc)),
                            "elapsed_minutes": int((now.timestamp() - extracted.timestamp()) / 60)})
    return due


def pending_report(state: dict, now: datetime) -> list[dict]:
    out = []
    for tid, target in state["targets"].items():
        extracted = _parse(target["extracted_at"])
        for name, minutes in CHECKPOINTS:
            entry = target["checkpoints"].get(name)
            if entry:
                out.append({"target": tid, "checkpoint": name, "status": entry["status"]})
            else:
                fire_at = extracted.timestamp() + minutes * 60
                out.append({"target": tid, "checkpoint": name, "status": "pending",
                            "due_at": _iso(datetime.fromtimestamp(fire_at, timezone.utc))})
    return out


async def probe_target(http: LoopClient, target: dict, retry: bool = True) -> dict:
    """Probe the ORIGINAL URLs only: master -> first variant -> first segment."""
    master, referer = target["master"], target.get("referer", "")
    result = {"master_code": None, "variant_code": None, "segment_code": None,
              "segment_bytes": 0, "ranged_bytes": 0, "age": None, "cf_cache_status": None,
              "cache_control": None, "playlist_ok": False, "segment_ok": False,
              "attempts": 0, "error": None}
    delays = list(RETRY_DELAYS) if retry else []
    while True:
        result["attempts"] += 1
        try:
            r = await http.client.get(master, headers={"Referer": referer})
            result["master_code"] = r.status_code
            result["cache_control"] = r.headers.get("cache-control")
            if r.status_code >= 500 and delays:
                await asyncio.sleep(delays.pop(0))
                continue
            if r.status_code >= 400:
                return result
            result["playlist_ok"] = True
            variants = [ln.strip() for ln in r.text.splitlines()
                        if ln.strip() and not ln.startswith("#")]
            if not variants:
                result["error"] = "no variants in master"
                return result
            variant = variants[-1]
            if not variant.startswith("http"):
                variant = master.rsplit("/", 1)[0] + "/" + variant
            v = await http.client.get(variant, headers={"Referer": referer})
            result["variant_code"] = v.status_code
            if v.status_code >= 500 and delays:
                await asyncio.sleep(delays.pop(0))
                continue
            if v.status_code >= 400:
                return result
            segment = parse_first_segment(v.text, variant)
            if not segment:
                result["error"] = "no segment in media playlist"
                return result
            ranged = await http.client.get(
                segment, headers={"Range": f"bytes=0-{settings.segment_probe_bytes - 1}"})
            result["segment_code"] = ranged.status_code
            result["ranged_bytes"] = len(ranged.content)
            result["age"] = ranged.headers.get("age")
            result["cf_cache_status"] = ranged.headers.get("cf-cache-status")
            result["cache_control"] = result["cache_control"] or ranged.headers.get("cache-control")
            if ranged.status_code in (200, 206):
                result["segment_bytes"] = len(ranged.content)
                if ranged.status_code == 200:
                    result["segment_bytes"] = len(ranged.content)
                result["segment_ok"] = bool(result["ranged_bytes"])
                return result
            if ranged.status_code >= 500 and delays:
                await asyncio.sleep(delays.pop(0))
                continue
            return result
        except Exception as exc:                      # transient network failure
            result["error"] = type(exc).__name__
            if delays:
                await asyncio.sleep(delays.pop(0))
                continue
            return result


def classify(result: dict) -> tuple[str, bool]:
    """Map a probe to (outcome, playback_validated). Cached 403 is named explicitly."""
    if result["master_code"] is None:
        return "network_error", False
    if result["master_code"] >= 400:
        return ("expired_or_denied" if result["master_code"] == 403 else "playlist_error"), False
    if not result["playlist_ok"]:
        return "playlist_error", False
    if result["variant_code"] and result["variant_code"] >= 400:
        return "variant_error", False
    code = result["segment_code"]
    if code in (200, 206) and result["ranged_bytes"]:
        return "playable", True
    if code == 403:
        cached = (result["cf_cache_status"] or "").upper() in {"HIT", "REVALIDATED"} \
            or (result["age"] or "").isdigit() and int(result["age"] or 0) > 0
        return ("cached_403" if cached else "blocked_403"), False
    if code is None:
        return "no_segment_found", False
    return f"segment_http_{code}", False


def append_csv(path: Path, row: dict) -> None:
    new = not path.exists()
    with path.open("a", newline="", encoding="utf-8") as fh:
        writer = csv.DictWriter(fh, fieldnames=CSV_FIELDS)
        if new:
            writer.writeheader()
        writer.writerow({k: row.get(k) for k in CSV_FIELDS})


async def run_once(state_path: Path, csv_path: Path, now: datetime | None = None) -> list[dict]:
    now = now or datetime.now(timezone.utc)
    state = load_state(state_path)
    due = due_checkpoints(state, now)
    created = []
    if not due:
        return created
    http = LoopClient()
    try:
        for item in due:
            target = state["targets"][item["target_id"]]
            result = await probe_target(http, target)
            outcome, validated = classify(result)
            record = {
                "target_id": item["target_id"], "checkpoint": item["checkpoint"],
                "provider": target["provider"], "anime": target["anime"],
                "episode": target["episode"], "audio": target["audio"],
                "extracted_at": target["extracted_at"], "checked_at": _iso(now),
                "elapsed_minutes": item["elapsed_minutes"],
                "master_code": result["master_code"], "variant_code": result["variant_code"],
                "segment_code": result["segment_code"], "segment_bytes": result["segment_bytes"],
                "ranged_bytes": result["ranged_bytes"], "age": result["age"],
                "cf_cache_status": result["cf_cache_status"],
                "cache_control": result["cache_control"],
                "playback_validated": validated, "outcome": outcome,
                "attempts": result["attempts"], "error": result["error"],
            }
            target["checkpoints"][item["checkpoint"]] = {
                "status": "done", "recorded_at": record["checked_at"], "observation": record}
            save_state(state_path, state)      # persist before moving on
            append_csv(csv_path, record)
            created.append(record)
    finally:
        await http.aclose()
    return created


async def _main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--state", default="checkpoints.json")
    ap.add_argument("--csv", default="observations.csv")
    ap.add_argument("--loop", type=int, default=0)
    args = ap.parse_args()
    state_path, csv_path = Path(args.state), Path(args.csv)
    while True:
        created = await run_once(state_path, csv_path)
        for rec in created:
            print(json.dumps(rec, separators=(",", ":")))
        if not created:
            print(json.dumps({"event": "no_checkpoint_due",
                              "pending": [p for p in pending_report(load_state(state_path),
                                                                    datetime.now(timezone.utc))
                                          if p["status"] == "pending"]}))
        if not args.loop:
            break
        await asyncio.sleep(args.loop)


if __name__ == "__main__":
    asyncio.run(_main())
