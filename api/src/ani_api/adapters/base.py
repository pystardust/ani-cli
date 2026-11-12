"""Adapter contracts.

Metadata adapters answer "what exists"; stream adapters answer "where are the bytes".
A provider dying is a swap of one class, not a rewrite of the service.
"""
from __future__ import annotations

from dataclasses import dataclass, field
from typing import Protocol, runtime_checkable

from ..models import AnimeRef, AudioType, EpisodeRef, ServerRef, SubtitleTrack, Variant


@dataclass
class StreamSet:
    provider: str
    master_url: str
    referer: str
    variants: list[Variant] = field(default_factory=list)
    subtitles: list[SubtitleTrack] = field(default_factory=list)
    audio_track_url: str | None = None
    mal_id: str | None = None


@runtime_checkable
class MetadataAdapter(Protocol):
    name: str

    async def search(self, query: str) -> list[AnimeRef]: ...
    async def episodes(self, anime_id: str) -> list[EpisodeRef]: ...
    async def servers(self, episode_id: str, audio: AudioType) -> list[ServerRef]: ...


@runtime_checkable
class StreamAdapter(Protocol):
    name: str
    supports: set[str]

    async def resolve(self, embed_url: str, referer: str) -> StreamSet: ...
