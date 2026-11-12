"""Settings from environment. No secrets in code, no default credentials."""
import os
from dataclasses import dataclass, field


def _int(name: str, default: int) -> int:
    try:
        return int(os.environ.get(name, default))
    except ValueError:
        return default


@dataclass
class Settings:
    # auth: an empty key set means the API refuses non-loopback traffic entirely
    api_keys: set[str] = field(default_factory=lambda: {
        k.strip() for k in os.environ.get("ANI_API_KEYS", "").split(",") if k.strip()
    })
    allow_loopback_without_key: bool = os.environ.get("ANI_API_ALLOW_LOOPBACK", "1") == "1"
    token_secret: str = os.environ.get("ANI_API_TOKEN_SECRET", "")
    token_ttl_seconds: int = _int("ANI_API_TOKEN_TTL", 900)

    # upstream
    metadata_base: str = os.environ.get("ANI_API_METADATA_BASE", "https://hianime.at")
    user_agent: str = os.environ.get(
        "ANI_API_UA",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 "
        "(KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
    )
    upstream_timeout: float = float(os.environ.get("ANI_API_TIMEOUT", "12"))

    # cache
    redis_url: str = os.environ.get("ANI_API_REDIS_URL", "")
    ttl_search: int = _int("ANI_API_TTL_SEARCH", 600)
    ttl_episodes: int = _int("ANI_API_TTL_EPISODES", 3600)
    ttl_servers: int = _int("ANI_API_TTL_SERVERS", 900)
    ttl_stream: int = _int("ANI_API_TTL_STREAM", 300)

    # validation
    probe_budget: int = _int("ANI_API_PROBE_BUDGET", 3)      # variants probed per request
    # Cached-403 denylist. Observed recovery: the same dub variant returned 403 at
    # 21:25 and served 206 again at 21:44, so the base window is short and escalates
    # for objects that keep failing (30m -> 1h -> 2h -> 4h).
    deny_ttl: int = _int("ANI_API_DENY_TTL", 1800)
    deny_ttl_max: int = _int("ANI_API_DENY_TTL_MAX", 14400)
    reextract_min_age: int = _int("ANI_API_REEXTRACT_MIN_AGE", 600)
    segment_probe_bytes: int = 1024

    # limits
    rate_limit_per_minute: int = _int("ANI_API_RATE_LIMIT", 60)
    log_level: str = os.environ.get("ANI_API_LOG_LEVEL", "INFO")
    prober_out: str = os.environ.get("ANI_API_PROBER_OUT", "observations.jsonl")


settings = Settings()
