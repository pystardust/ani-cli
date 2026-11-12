"""API-key, rate-limit, SSRF and access-control tests.

These run without touching a provider: they target protected endpoints that do not
call upstream (/metrics, /v1/admin/*), so a network outage cannot mask a security bug.
"""
import pytest
from fastapi.testclient import TestClient

from ani_api.config import settings
from ani_api.main import app
from ani_api.services import security
from ani_api.services.security import require_access


@pytest.fixture()
def client():
    with TestClient(app) as c:
        yield c


@pytest.fixture(autouse=True)
def restore_keys():
    original = set(settings.api_keys)
    yield
    settings.api_keys = original


def test_no_keys_configured_refuses_remote(client):
    settings.api_keys = set()
    r = client.get("/metrics", headers={"x-forwarded-for": "203.0.113.9"})
    assert r.status_code == 503
    assert r.json()["code"] == "AUTH_NOT_CONFIGURED"


def test_no_keys_configured_allows_loopback(client):
    settings.api_keys = set()
    assert client.get("/metrics").status_code == 200


def test_valid_key_accepted(client):
    settings.api_keys = {"key-alpha"}
    assert client.get("/metrics", headers={"x-api-key": "key-alpha"}).status_code == 200


def test_invalid_key_rejected(client):
    settings.api_keys = {"key-alpha"}
    r = client.get("/metrics", headers={"x-api-key": "wrong"})
    assert r.status_code == 401
    assert r.json()["code"] == "UNAUTHORIZED"


def test_missing_key_rejected_when_configured(client):
    settings.api_keys = {"key-alpha"}
    assert client.get("/metrics").status_code == 401


def test_key_rotation(client):
    settings.api_keys = {"key-old"}
    assert client.get("/metrics", headers={"x-api-key": "key-old"}).status_code == 200
    settings.api_keys = {"key-new"}
    assert client.get("/metrics", headers={"x-api-key": "key-old"}).status_code == 401
    assert client.get("/metrics", headers={"x-api-key": "key-new"}).status_code == 200


def test_forwarded_header_cannot_impersonate_loopback(client):
    """A spoofed XFF from a trusted-proxy peer must still be evaluated on its value."""
    settings.api_keys = set()
    r = client.get("/metrics", headers={"x-forwarded-for": "198.51.100.7, 127.0.0.1"})
    assert r.status_code == 503, r.text


def test_every_v1_route_requires_access():
    """Structural guarantee: no protected surface can be added without auth."""
    unprotected = []
    for route in app.routes:
        path = getattr(route, "path", "")
        if not (path.startswith("/v1") or path == "/metrics" or path == "/health/providers"):
            continue
        calls = [d.call for d in getattr(route, "dependant", None).dependencies] if getattr(route, "dependant", None) else []
        if require_access not in calls:
            unprotected.append(path)
    assert not unprotected, f"routes missing auth: {unprotected}"


def test_rate_limit_returns_429(client, monkeypatch):
    settings.api_keys = set()
    monkeypatch.setattr(settings, "rate_limit_per_minute", 3)
    codes = [client.get("/v1/search", params={"q": "one piece"}).status_code for _ in range(6)]
    assert 429 in codes, codes


def test_ssrf_guard_rejects_foreign_host():
    from ani_api.errors import AniError
    from ani_api.services.security import assert_allowed_host
    with pytest.raises(AniError):
        assert_allowed_host("http://169.254.169.254/latest/meta-data/", {"drama1.cfd"})
    with pytest.raises(AniError):
        assert_allowed_host("file:///etc/passwd", {"drama1.cfd"})
    assert_allowed_host("https://hls1.drama1.cfd/a/b.m3u8", {"drama1.cfd"}) is None


def test_token_expiry_and_signature():
    from ani_api.errors import AniError
    from ani_api.services.security import sign_payload, verify_token
    token = sign_payload({"u": "https://hls1.drama1.cfd/x.m3u8"}, ttl=-1)
    with pytest.raises(AniError) as expired:
        verify_token(token)
    assert expired.value.detail["code"] == "TOKEN_EXPIRED"
    with pytest.raises(AniError):
        verify_token("not-a-token")


def test_query_validation_rejects_bad_input(client):
    settings.api_keys = set()
    assert client.get("/v1/search", params={"q": "a"}).status_code == 422          # min_length=2
    assert client.get("/v1/streams/abc", params={"audio": "sub"}).status_code == 422  # non-numeric id
    assert client.get("/v1/streams/4341", params={"audio": "both"}).status_code == 422  # bad enum
