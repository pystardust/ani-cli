"""Background URL-lifetime prober.

Answers the open questions (2h / 6h / 24h validity) with measurements instead of
guesses. Each run appends one JSONL row per target; the API aggregates the curve.

    python -m ani_api.workers.lifetime_prober --targets scripts/targets.json \
        --out observations.jsonl [--loop 900]

It probes three things per target, in order: master playlist (needs Referer),
first variant playlist, and one real media segment (ranged GET).
"""
from __future__ import annotations

import argparse
import asyncio
import json
import time
from datetime import datetime, timezone
from pathlib import Path

from ..adapters.playlist import parse_first_segment
from ..config import settings
from ..http_client import LoopClient


def _now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


async def check_target(http: LoopClient, target: dict) -> dict:
    master, referer = target["master"], target.get("referer", "")
    row = {"checked_at": _now(), "label": target["label"],
           "extracted_at": target.get("extracted_at"), "master_code": None,
           "variant_code": None, "segment_code": None, "segment_bytes": 0,
           "edge_cache_status": None, "variant_url": None}
    try:
        r = await http.client.get(master, headers={"Referer": referer})
        row["master_code"] = r.status_code
        if r.status_code >= 400:
            return row
        variants = [ln.strip() for ln in r.text.splitlines() if ln.strip() and not ln.startswith("#")]
        if not variants:
            return row
        variant = variants[-1]
        if not variant.startswith("http"):
            variant = master.rsplit("/", 1)[0] + "/" + variant
        row["variant_url"] = variant
        v = await http.client.get(variant, headers={"Referer": referer})
        row["variant_code"] = v.status_code
        if v.status_code >= 400:
            return row
        segment = parse_first_segment(v.text, variant)
        if not segment:
            return row
        s = await http.client.get(segment, headers={"Range": f"bytes=0-{settings.segment_probe_bytes - 1}"})
        row["segment_code"] = s.status_code
        row["segment_bytes"] = len(s.content)
        row["edge_cache_status"] = s.headers.get("cf-cache-status")
    except Exception as exc:  # a prober must never die mid-sweep
        row["error"] = type(exc).__name__
    if row["extracted_at"]:
        try:
            started = datetime.fromisoformat(row["extracted_at"].replace("Z", "+00:00"))
            row["age_minutes"] = int((time.time() - started.timestamp()) / 60)
        except ValueError:
            pass
    return row


def append(path: Path, rows: list[dict]) -> None:
    with path.open("a", encoding="utf-8") as fh:
        for row in rows:
            fh.write(json.dumps(row, separators=(",", ":")) + "\n")


def read_rows(path: Path) -> list[dict]:
    if not path.exists():
        return []
    rows = []
    with path.open(encoding="utf-8") as fh:
        for line in fh:
            line = line.strip()
            if line:
                try:
                    rows.append(json.loads(line))
                except json.JSONDecodeError:
                    continue
    return rows


def curve(rows: list[dict]) -> dict:
    """Group observations into the checkpoints we promised to answer."""
    checkpoints = {"2h": 120, "6h": 360, "24h": 1440}
    summary = {}
    for name, minutes in checkpoints.items():
        near = [r for r in rows if isinstance(r.get("age_minutes"), int)
                and minutes - 20 <= r["age_minutes"] <= minutes + 60]
        if not near:
            summary[name] = {"status": "pending", "samples": 0,
                             "note": "no observation in this window yet"}
            continue
        ok = [r for r in near if r.get("segment_code") in (200, 206) and r.get("segment_bytes")]
        summary[name] = {"status": "measured", "samples": len(near),
                         "playable": len(ok),
                         "success_rate": round(len(ok) / len(near), 3),
                         "max_age_minutes": max(r["age_minutes"] for r in near)}
    observed = [r["age_minutes"] for r in rows if isinstance(r.get("age_minutes"), int)]
    summary["_sample_count"] = len(rows)
    summary["_oldest_age_minutes"] = max(observed) if observed else None
    return summary


async def sweep(targets: list[dict], out: Path) -> list[dict]:
    http = LoopClient()
    rows = []
    try:
        for target in targets:
            row = await check_target(http, target)
            rows.append(row)
            print(json.dumps(row, separators=(",", ":")))
    finally:
        await http.aclose()
    append(out, rows)
    return rows


async def _main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--targets", default="scripts/targets.json")
    ap.add_argument("--out", default="observations.jsonl")
    ap.add_argument("--loop", type=int, default=0, help="seconds between sweeps; 0 = run once")
    args = ap.parse_args()
    targets = json.loads(Path(args.targets).read_text())
    out = Path(args.out)
    while True:
        await sweep(targets, out)
        if not args.loop:
            break
        await asyncio.sleep(args.loop)


if __name__ == "__main__":
    asyncio.run(_main())
