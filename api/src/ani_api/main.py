"""App factory: request ids, problem+json errors, no implicit public access."""
from __future__ import annotations

import uuid

from fastapi import FastAPI, Request
from fastapi.responses import JSONResponse

from .api.routes import router
from . import obs
from .config import settings
from .errors import AniError


def create_app() -> FastAPI:
    obs.configure(settings.log_level)
    app = FastAPI(title="ani-api", version="0.1.0",
                  description="Independent streaming API over replaceable source adapters.")

    @app.middleware("http")
    async def attach_request_id(request: Request, call_next):
        request.state.request_id = request.headers.get("x-request-id") or f"req_{uuid.uuid4().hex[:16]}"
        response = await call_next(request)
        response.headers["x-request-id"] = request.state.request_id
        return response

    @app.exception_handler(AniError)
    async def problem_json(request: Request, exc: AniError):  # RFC 9457-ish
        body = dict(exc.detail) if isinstance(exc.detail, dict) else {"detail": str(exc.detail)}
        body.update({"status": exc.status_code, "instance": str(request.url.path),
                     "request_id": getattr(request.state, "request_id", "")})
        headers = {}
        if body.get("retry_after"):
            headers["Retry-After"] = str(body["retry_after"])
        return JSONResponse(status_code=exc.status_code, content=body,
                            media_type="application/problem+json", headers=headers)

    app.include_router(router)
    return app


app = create_app()
