"""Response models. Fields we cannot verify are typed Optional and default to None/False."""
from __future__ import annotations

from typing import Literal, Optional
from pydantic import BaseModel, Field

AudioType = Literal["sub", "dub"]


class AnimeRef(BaseModel):
    id: str
    title: str


class EpisodeRef(BaseModel):
    id: str
    number: str


class AnimeDetail(BaseModel):
    id: str
    title: Optional[str] = None
    episode_count: int
    detail_source: str = "episodes_api"


class ServerRef(BaseModel):
    name: str
    audio: AudioType
    supported: bool
    reason: Optional[str] = None


class SubtitleTrack(BaseModel):
    format: Literal["vtt"] = "vtt"
    url: str
    proxied_url: Optional[str] = None
    default: bool = False
    language_inferred: Optional[str] = None
    language_verified: bool = False


class Variant(BaseModel):
    quality: str                       # normalised label, e.g. "1080p"
    height: Optional[int] = None
    width: Optional[int] = None
    bandwidth_bps: Optional[int] = None
    playlist_url: str
    first_segment_url: Optional[str] = None
    playable: Optional[bool] = None    # None = not probed
    probe_http_code: Optional[int] = None
    probe_bytes: Optional[int] = None
    edge_cached: Optional[bool] = None
    blocked_reason: Optional[str] = None


class Validation(BaseModel):
    """Proof that we actually delivered video, not just that a URL existed."""
    first_segment_checked: bool
    segment_http_code: Optional[int] = None
    segment_bytes: Optional[int] = None
    segment_container: Optional[str] = None
    edge_cache_status: Optional[str] = None
    cached_403_detected: bool = False
    checked_at: Optional[str] = None


class AudioInfo(BaseModel):
    type: AudioType
    declared: AudioType
    detected_language: Optional[str] = None
    verified: bool = False
    note: Optional[str] = None


class UrlLifetime(BaseModel):
    extracted_at: Optional[str] = None
    expires_at: Optional[str] = None      # only ever set from a measured observation
    expiry_known: bool = False
    observed_valid_for_minutes: Optional[int] = None
    note: Optional[str] = None


class DownloadInfo(BaseModel):
    available: bool = False
    reason: Optional[str] = None


class StreamResponse(BaseModel):
    request_id: str
    provider: str
    metadata_source: str
    anime_id: str
    episode_id: str
    episode_number: Optional[str] = None
    audio: AudioInfo
    quality_requested: str
    quality_selected: Optional[str] = None
    stream_type: Literal["hls"] = "hls"
    master_url: Optional[str] = None
    proxied_master_url: Optional[str] = None
    variants: list[Variant] = Field(default_factory=list)
    alternatives: list[Variant] = Field(default_factory=list)
    subtitles: list[SubtitleTrack] = Field(default_factory=list)
    audio_tracks: list[dict] = Field(default_factory=list)
    headers_required: dict = Field(default_factory=dict)
    validation: Validation
    url_lifetime: UrlLifetime
    download: DownloadInfo = Field(default_factory=DownloadInfo)
    playback_hint: Optional[str] = None
    cache: dict = Field(default_factory=dict)
    warnings: list[str] = Field(default_factory=list)
