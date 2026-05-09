# Title resolution and the cross-API bridge

`ani-gui` reads from four catalogues that don't share an id space:

- **Kitsu** (REST/JSON:API) — discovery surface (search, trending fallback, top rated, recently released, detail pages, episode metadata).
- **AniList** (GraphQL) — recency-weighted "Trending Now" row and banner backfill when Kitsu's banner is null.
- **allmanga** (allanime's GraphQL backend) — the catalogue `ani-cli` actually scrapes streams from.
- **aniskip** (REST) — community OP / ED skip-time intervals, keyed by MyAnimeList id.

Every interaction other than discovery has to find the same show in two or more of these. None of them carry the others' ids, and any given anime may appear in some but not others (aniskip in particular is sparse). This document describes how the backend bridges them and what the cache stores in the process.

## The four bridges

```
                       ┌──────────────────────────────────────────┐
                       │                                          │
                       │             Discovery surface            │
                       │                                          │
                       │   ┌──────────────┐    ┌──────────────┐   │
                       │   │   AniList    │    │    Kitsu     │   │
                       │   │  (trending)  │    │ (everything  │   │
                       │   │              │    │   else)      │   │
                       │   └──────┬───────┘    └──────┬───────┘   │
                       │          │                   │           │
                       │     mal id only         kitsu id (canonical
                       │          │              for the renderer)
                       └──────────┼───────────────────┼───────────┘
                                  │                   │
                  ┌───────────────┴───────────────────┘
                  │       Kitsu mappings endpoint
                  │       (kitsu id ↔ mal id)
                  ▼
       ┌──────────────────┐                ┌──────────────────────┐
       │     aniskip      │                │       allmanga       │
       │   (mal_id, ep)   │                │  (canonical title +  │
       │  → skip times    │                │   alt titles → list  │
       │                  │                │   of candidates →    │
       │                  │                │   pick by ep_count)  │
       └──────────────────┘                └──────────────────────┘
```

Four distinct lookups, each with its own gotchas:

1. **Kitsu → allmanga title match** — `ani-cli`'s search runs against allmanga's catalogue. Kitsu canonical titles (often the licensed English form) and allmanga's index don't always agree, so the bridge tries the canonical first and falls back to romanized Japanese, native script, and known synonyms before giving up.
2. **allmanga candidate disambiguation** — multiple allmanga shows can match the same query string ("Naruto Shippuden" returns the main series and several side-story spin-offs in unstable order). Kitsu's authoritative `episode_count` picks the candidate whose count is closest, so the play path lands on the main show even when allmanga happens to rank the side story first.
3. **Kitsu ↔ MAL** — neither Kitsu's id nor allmanga's slug matches MAL's. Kitsu publishes a mappings endpoint that exposes the third-party ids it knows about; the backend fetches `kitsu/anime/:id?include=mappings` and walks the included documents for the MyAnimeList row.
4. **MAL → aniskip / AniList** — once the MAL id is in hand, aniskip and AniList's `Media(idMal:)` query are direct lookups.

## Title resolution: Kitsu → allmanga

When the user clicks an episode, the backend builds a list of search terms from the Kitsu metadata in priority order and feeds them to allmanga in turn:

1. Canonical title.
2. `titles.en_jp` (romanized Japanese — what allmanga indexes most consistently).
3. `titles.ja_jp` (native script).
4. `titles.en` / `titles.en_us` (English alternates).
5. allmanga's own `english_name`, `native_name`, and `alt_names` from its `Show` document (used for show-page enrichment, not initial search).

Empty / whitespace-only titles are skipped, and exact-string duplicates are deduped, so the backend never makes a redundant allmanga query.

## Disambiguation by episode count

`pick_by_ep_count(candidates, expected, mode)` returns the candidate whose available episode count in the requested mode is closest to Kitsu's authoritative `episode_count`. Ties are broken by allmanga's own ordering. The same picker is used by:

- **The play path** — so clicking "play" lands on the right show even when allmanga's first hit is the side story.
- **The availability probe** — so the home / detail page's "is this on allmanga?" gate matches what the play path would do, with no risk of "available" cards turning out to be name-collision side stories.
- **The download path** — so a download started from the player picks the same show the player was streaming.

When Kitsu's episode count is unknown (rare, but happens for upcoming shows), the picker falls back to allmanga's first hit. The frontend treats this as a soft signal and still renders the card; the lazy click path will surface a real error if the bridge picked wrong.

## Half-episodes and the integer cap

Allmanga's `availableEpisodes.<mode>` is a COUNT, not the maximum integer episode number. Long-running shows like One Piece accumulate non-integer episode tags ("1061.5" recap specials) over the years, and the count includes them — so a show with episodes 1..1160 plus one "1061.5" reports 1161, and proposing 1161 as the next-episode cap would 404.

The backend reads allmanga's per-mode `availableEpisodesDetail` tag list directly and picks the largest integer entry, ignoring half-episode tags. The half-episode tags themselves are surfaced separately as `extra_episodes` so the episode strip can still render them (spliced in at their numeric position) without polluting the integer cap that the resume CTA, download range, and pagination logic depend on.

## Kitsu → MAL via the mappings endpoint

The Kitsu API exposes its known third-party ids on the `mappings` relationship of an anime resource. The backend queries `GET /anime/:id?include=mappings` and walks the `included` documents for one whose `attributes.externalSite` is `"myanimelist/anime"`; its `attributes.externalId` is the MAL id.

The mappings response is cached in `meta_cache` indefinitely — Kitsu's mapping table doesn't move once a show has shipped. A miss on `MAL` is itself cached (as `None`), so shows that aren't on MAL don't get re-probed on every page visit.

## What this enables

- **Trending Now** uses AniList's `TRENDING_DESC` sort, then bridges each MAL id back to a Kitsu id so the rest of the renderer can treat the row uniformly with Kitsu-sourced rows.
- **Banner backfill** — when the detail page sees a null `coverImage` from Kitsu, it bridges to MAL and asks AniList for `bannerImage`. Roughly half of any week's currently-airing top 20 shows hit this fallback path.
- **aniskip** lookups need the MAL id to query, and the same Kitsu→MAL mapping is reused.
- **Availability** can answer "is this on allmanga in the requested mode?" without spawning `ani-cli`, by running the same disambiguator pre-flight and caching the verdict per `(kitsu_id, mode)`.

## Failure modes the bridge tolerates

- **Kitsu has no MAL mapping** — aniskip lookup returns an empty list (the player just doesn't show the skip button); banner backfill falls through to the blurred-poster placeholder.
- **allmanga has no candidate matching any title** — the play path returns `NoResults`; the frontend renders an "isn't on the streaming source" overlay instead of a cryptic backend error.
- **The picker can't disambiguate** — first-hit fallback. This is the worst case for correctness, but it's still a correct entry on allmanga; the user sees a sub-show rather than no show. They can pick the right one manually from search.
- **A cached play-resolution URL stops working** — the silent retry path evicts the cached row and re-resolves once before surfacing an error to the user.
