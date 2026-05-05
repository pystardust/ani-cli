# Fixture manifest

Every fixture under `tests/fixtures/` is a recorded response or sample input
used by tests across every layer (Bash, Rust, TypeScript). Each subdirectory
has its own `MANIFEST.json` describing each file's source URL, capture
timestamp, and SHA-256. Fixtures over 1 MB live in git-LFS.

## Subdirectories (populated as tests are added)

| Path | Contents |
|---|---|
| `allanime/` | GraphQL responses for `search_anime`, `episodes_list`, `get_episode_url`. `tobeparsed` blobs for `decode_tobeparsed`. Embed-page HTML samples per provider. |
| `kitsu/` | JSON:API responses for `/anime?filter[text]=`, `/anime/:id`, `/anime?filter[status]=`, `/anime/:id/relationships/genres`. |
| `anilist/` | GraphQL responses for `Page.media(sort: TRENDING_DESC)`, `Page.media(season:, seasonYear:)`. |
| `m3u8/` | Master playlist (multi-bitrate), media playlist with absolute URIs, media playlist with relative URIs, edge cases (`EXT-X-I-FRAME-STREAM-INF`, `EXT-X-BYTERANGE`, encrypted with `EXT-X-KEY`). |
| `history/` | `ani-hsts` samples: empty, single-entry, multi-entry, duplicate-id, malformed-line. |

## Refresh flow

```
make fixtures-refresh        # re-records against live APIs, writes diff report
git diff tests/fixtures/     # human-reviewed in PR
```

The refresh target writes a per-subdirectory `MANIFEST.json` update with new
SHA-256s. Reviewers should look for *unexpected* diffs (e.g. a Kitsu response
that gained a new field — investigate before accepting), and for changes in
fixtures that property tests depend on.

## Capture conventions

- Responses are captured with the same User-Agent the production code uses.
- For allanime, the same `Referer:` and `Origin:` headers ani-cli sends.
- Personally-identifying fields (none expected for these APIs, but worth
  checking) are scrubbed before commit.
- Binary fixtures (the `tobeparsed` blobs) are committed as base64 text in
  `.b64` files so a reviewer can `git diff` them.
