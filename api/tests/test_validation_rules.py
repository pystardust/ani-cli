"""Unit coverage for the reliability rules that do not need the network."""
import pytest

from ani_api.errors import AniError
from ani_api.services.security import assert_allowed_host, sign_payload, verify_token


def test_token_roundtrip_and_tamper():
    token = sign_payload({"u": "https://hls.dramahot.top/x.m3u8", "r": "https://zokoanime.video/"})
    assert verify_token(token)["u"] == "https://hls.dramahot.top/x.m3u8"
    tampered = token[:-3] + ("aaa" if not token.endswith("aaa") else "bbb")
    with pytest.raises(AniError):
        verify_token(tampered)


def test_proxy_host_allowlist_blocks_ssrf():
    assert_allowed_host("https://hls1.drama1.cfd/a/b", {"drama1.cfd"})
    with pytest.raises(AniError):
        assert_allowed_host("https://evil.example.com/a", {"drama1.cfd"})
