# Proposal: Cast and multi-viewer support

**Status**: future, post-v1. Not in any current milestone.

## Why this matters

Anime is often watched together — with friends in the same room, or with friends on a call. The current `ani-gui` design plays inside a single desktop window for one viewer. Two adjacent capabilities can extend that:

- **Cast** — push playback from the desktop app to a TV via a Cast-capable receiver (Chromecast, AirPlay-compatible TV, smart TVs running a generic media receiver).
- **Multi-viewer / shared session** — let two or more `ani-gui` instances on different machines stay in sync (play / pause / seek), like Syncplay does for `mpv` today.

These are different problems with related primitives. Cast is one-to-many output from one user's session. Multi-viewer is one-to-many control across separate users' sessions.

## Cast

### What's needed

1. **Receiver discovery** — mDNS-SD on the LAN to enumerate Cast/AirPlay/DLNA receivers.
2. **Stream URL** — receivers can't reach `127.0.0.1` on the casting laptop. The local stream proxy needs an opt-in mode that binds to the LAN interface (`0.0.0.0`) for the duration of a cast session, with token-gated access to prevent open relays.
3. **Receiver protocol** — Cast (Google) or AirPlay (Apple) or DLNA. Cast has the broadest device coverage; the protocol is documented but the official SDK is closed. Open-source implementations exist (e.g. `castnow`, `go-chromecast`). On the Rust side: a crate that talks Cast directly, OR shell out to `catt` (already a dependency in some `ani-cli` installs).
4. **UI** — a Cast button in the player chrome; receiver picker overlay; "Now casting on <device>" banner; control parity (play/pause/seek/quality).

### Risks

- LAN exposure of the proxy is a security boundary change. Token TTL and audience binding must be tight.
- Receiver compatibility is uneven; some Smart TVs require DLNA, not Cast. Picking one protocol means leaving devices behind.
- Subtitles. Cast receivers often consume only WebVTT inline; our `.vtt` proxy will need to honor receiver expectations.
- DRM. Anime stream sources are not DRM-protected today, so this is less of a concern than for Netflix-likes — but it is theoretically a future risk.

### Sketch of v1 of this feature

- LAN-binding mode for the proxy, behind a Settings toggle ("Allow casting") that defaults to off.
- mDNS discovery of Cast receivers only (skip AirPlay/DLNA for v1).
- Cast button on the player chrome that opens a receiver picker.
- Single device at a time; no multi-room.

## Multi-viewer / shared session

The user described anime watching as inherently multi-person. There are two levels of this:

### Level 1 — Syncplay parity

Today `ani-cli` supports launching `mpv` under [Syncplay](https://syncplay.pl), which keeps two `mpv` instances synced via a relay server. The `ani-gui` "Open in external player" escape hatch already preserves this, but it's clunky — the user falls out of the GUI.

Native equivalent: `ani-gui` joins a Syncplay-like relay session for the embedded `<video>` element. Each participant runs their own `ani-gui`, opens the same anime/episode, and clicks "Watch with friends". State (play, pause, seek, ready) is broadcast through a relay; each client's player obeys.

### Level 2 — Single-stream, multi-viewer

A more ambitious version: one user resolves the stream URL via `ani-cli`, and others view that user's already-resolved stream over the relay. This solves "we want to be sure we're watching exactly the same source." Trade-off: the host's bandwidth bottlenecks the room; quality switching is host-only.

### Risks

- Relay infrastructure cost. Syncplay runs free public relays; we'd inherit those for v1, but a busier project would need its own.
- Privacy. Joining a session leaks the anime title and episode to the relay operator. Make this an explicit Settings opt-in with a clear consent flow.
- Latency tolerance. Anime is more forgiving than competitive games; a 250–500ms drift is fine. Worth setting expectations in the UI.

### Sketch of v1 of this feature

- Syncplay-protocol client implemented in the Rust backend; the embedded player exposes play/pause/seek hooks the backend forwards to the relay.
- "Watch with friends" button in the detail page; it generates a join code, share via clipboard.
- No voice chat, no video chat. Discord exists.

## Why this is post-v1

Both features double the surface area:

- Cast adds receiver discovery, network exposure, an additional protocol implementation, and platform-specific testing across many TVs.
- Multi-viewer adds a network protocol, a relay dependency, latency handling, and a new failure surface.

Both require dedicated UI work that interacts with the player chrome — and player chrome is itself top-priority UI work that lands in M3+. Stacking these on the v1 milestone would harm both.

We capture them here so they're remembered when the v1 ship is done and the team has appetite for new surface area. They are explicitly the kind of feature that makes `ani-gui` more than "a Netflix-like for one viewer" — they are the feature set that aligns the product with how anime is actually watched.
