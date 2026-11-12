"""Structured logging and a dependency-free Prometheus registry."""
from __future__ import annotations

import json
import logging
import sys
import time
from collections import defaultdict
from threading import Lock

_lock = Lock()
_counters: dict[tuple, float] = defaultdict(float)
_hist: dict[tuple, list[float]] = defaultdict(list)
_HIST_MAX = 512


def _key(name: str, labels: dict | None) -> tuple:
    return (name, tuple(sorted((labels or {}).items())))


def counter(name: str, labels: dict | None = None, value: float = 1.0) -> None:
    with _lock:
        _counters[_key(name, labels)] += value


def observe(name: str, value: float, labels: dict | None = None) -> None:
    with _lock:
        bucket = _hist[_key(name, labels)]
        bucket.append(value)
        if len(bucket) > _HIST_MAX:
            del bucket[: len(bucket) - _HIST_MAX]


def render() -> str:
    out = []
    with _lock:
        for (name, labels), val in _counters.items():
            lbl = ("{" + ",".join(f'{k}="{v}"' for k, v in labels) + "}") if labels else ""
            out.append(f"# TYPE {name} counter\n{name}{lbl} {val:g}")
        for (name, labels), vals in _hist.items():
            lbl = ("{" + ",".join(f'{k}="{v}"' for k, v in labels) + "}") if labels else ""
            total = sum(vals)
            out.append(f"# TYPE {name} summary\n{name}_count{lbl} {len(vals)}")
            out.append(f"{name}_sum{lbl} {total:.3f}")
            if "resolve_latency_seconds" in name:
                ordered = sorted(vals)
                for q in (0.5, 0.95):
                    idx = min(len(ordered) - 1, int(q * len(ordered)))
                    out.append(f'{name}{{quantile="{q}"}} {ordered[idx]:.3f}')
    return "\n".join(out) + "\n"


class JsonFormatter(logging.Formatter):
    def format(self, record: logging.LogRecord) -> str:
        payload = {"ts": round(time.time(), 3), "level": record.levelname.lower(),
                   "event": record.getMessage()}
        extra = getattr(record, "fields", None)
        if extra:
            payload.update(extra)
        return json.dumps(payload, separators=(",", ":"))


def configure(level: str = "INFO") -> None:
    handler = logging.StreamHandler(sys.stdout)
    handler.setFormatter(JsonFormatter())
    root = logging.getLogger("ani_api")
    root.handlers = [handler]
    root.setLevel(level.upper())
    root.propagate = False


def log(event: str, **fields) -> None:
    logging.getLogger("ani_api").info(event, extra={"fields": fields})
