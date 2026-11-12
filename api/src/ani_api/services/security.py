"""Auth, signed proxy tokens, and rate limiting.

Policy: with no API keys configured the service refuses non-loopback callers.
That makes 'unauthenticated public API' impossible by construction rather than by
promise, and adding keys later is a config change.
"""
from __future__ import annotations

import base64
import hashlib
import hmac
import json
import time
from urllib.parse import urlsplit

from fastapi import Request

from ..cache import cache
from ..config import settings
from ..errors import AniError


def sign_payload(payload: dict, ttl: int | None = None) -> str:
    body = dict(payload, exp=int(time.time()) + (ttl or settings.token_ttl_seconds))
    raw = json.dumps(body, separators=(",", ":")).encode()
    mac = hmac.new(settings.token_secret.encode(), raw, hashlib.sha256).digest()
    return base64.urlsafe_b64encode(raw).decode().rstrip("=") + "." + base64.urlsafe_b64encode(mac).decode().rstrip("=")


def verify_token(token: str) -> dict:
    try:
        b64, sig = token.split(".", 1)
        raw = base64.urlsafe_b64decode(b64 + "=" * (-len(b64) % 4))
        mac = base64.urlsafe_b64decode(sig + "=" * (-len(sig) % 4))
    except Exception as exc:
        raise AniError(400, "BAD_TOKEN", "Malformed token", detail=str(exc)) from exc
    expected = hmac.new(settings.token_secret.encode(), raw, hashlib.sha256).digest()
    if not hmac.compare_digest(mac, expected):
        raise AniError(403, "BAD_SIGNATURE", "Token signature invalid")
    payload = json.loads(raw)
    if payload.get("exp", 0) < time.time():
        raise AniError(410, "TOKEN_EXPIRED", "Token expired", retryable=True, retry_after=5)
    return payload


PRIVATE_PREFIXES = ("10.", "192.168.", "172.16.", "172.17.", "172.18.", "172.19.",
                    "172.2", "172.30.", "172.31.", "127.", "::1", "testclient")


def client_ip(request: Request) -> str:
    """Effective client IP.

    X-Forwarded-For is only honoured when the direct peer is itself local/private
    (i.e. our own reverse proxy). Otherwise the header is attacker-controlled and
    must never be allowed to impersonate a loopback caller.
    """
    direct = request.client.host if request.client else ""
    if direct.startswith(PRIVATE_PREFIXES):
        fwd = request.headers.get("x-forwarded-for", "").split(",")[0].strip()
        if fwd:
            return fwd
    return direct


def _client_id(request: Request) -> str:
    key = request.headers.get("x-api-key")
    if key:
        return "k:" + hashlib.sha256(key.encode()).hexdigest()[:16]
    return "i:" + hashlib.sha256(client_ip(request).encode()).hexdigest()[:16]


LOOPBACK = {"127.0.0.1", "::1", "localhost", "testclient"}


async def require_access(request: Request) -> None:
    supplied = request.headers.get("x-api-key", "")
    if settings.api_keys:
        if supplied not in settings.api_keys:
            raise AniError(401, "UNAUTHORIZED", "Missing or invalid API key")
    else:
        host = client_ip(request)
        if not (settings.allow_loopback_without_key and host in LOOPBACK):
            # no keys configured and not a local caller: refuse rather than serve openly
            raise AniError(503, "AUTH_NOT_CONFIGURED",
                           "No API keys configured; this service does not serve public traffic")


async def enforce_rate_limit(request: Request) -> None:
    cid = _client_id(request)
    minute = int(time.time() // 60)
    count = await cache.incr(f"rl:{cid}:{minute}", 120)
    if count > settings.rate_limit_per_minute:
        raise AniError(429, "RATE_LIMITED", "Too many requests", retryable=True, retry_after=60)


def assert_allowed_host(url: str, allow: set[str]) -> None:
    """SSRF guard for the proxy endpoints: only hosts the adapters produced."""
    host = (urlsplit(url).hostname or "").lower()
    if not any(host == a or host.endswith("." + a) for a in allow):
        raise AniError(400, "HOST_NOT_ALLOWED", "Refusing to proxy an unlisted host", detail=host)
