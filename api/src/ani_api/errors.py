"""Error taxonomy. Every code maps to a failure mode observed in testing."""
from fastapi import HTTPException


class AniError(HTTPException):
    def __init__(self, status: int, code: str, title: str, *, stage: str = "", provider: str = "",
                 retryable: bool = False, retry_after: int | None = None, detail: str = ""):
        super().__init__(status_code=status, detail={
            "code": code, "title": title, "stage": stage, "provider": provider,
            "retryable": retryable, "retry_after": retry_after, "detail": detail,
        })


def no_results(q: str) -> AniError:
    return AniError(404, "NO_RESULTS", "No results", stage="search", detail=q)


def upstream_blocked(stage: str, detail: str = "") -> AniError:
    # Cloudflare challenge or WAF page. Deliberately distinct from NO_RESULTS: the
    # shell script conflates these, which is how a blocked upstream looks like "no anime".
    return AniError(503, "UPSTREAM_BLOCKED", "Upstream blocked the request",
                    stage=stage, provider="hianime", retryable=True, retry_after=60, detail=detail)


def unsupported_server(name: str, reason: str) -> AniError:
    return AniError(422, "SERVER_LISTED_UNSUPPORTED", f"Server {name} is listed but unsupported",
                    stage="servers", provider=name, detail=reason)


def embed_decode_failed(detail: str = "") -> AniError:
    return AniError(502, "EMBED_DECODE_FAILED", "Could not decode provider embed payload",
                    stage="embed", provider="zokoanime", retryable=True, detail=detail)


def no_stream(reason: str) -> AniError:
    return AniError(502, "NO_STREAM", "No usable stream", stage="resolve", provider="zokoanime",
                    retryable=True, detail=reason)


def object_blocked_segment(*, quality: str, http_code: int, cached: bool) -> AniError:
    return AniError(409, "PROVIDER_OBJECT_BLOCKED",
                    "Provider blocks this video object",
                    stage="segment", provider="zokoanime", retryable=False,
                    detail=f"quality {quality}: first segment HTTP {http_code}"
                           + (" (edge-cached 403; retrying the same URL will keep failing)" if cached else ""))
