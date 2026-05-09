# Proposal: Luma-generated bespoke assets for ani-gui

**Status**: future, post-v1. Not in any current milestone. Captured during the M3.1 design-pass review when Luma's API came up as a possible enhancement track.

## Why this matters

The M3.1 design pass landed a coherent visual voice — late-night repertory cinema, warm ink, editorial serif/mono pairing, manga-page hairlines, oversized tabular numerals. That voice is currently expressed entirely through type, color, motion, and licensed Kitsu poster art. There is no proprietary visual identity yet: no mascot, no illustrated empty states, no hand-drawn dividers, no animated transitions specific to ani-gui.

A premium-experience product usually has a bespoke layer of **hosted illustration** — assets the app ships with that are unmistakably *its*, not sourced from third parties. Apple TV+ has its slate-grey letter-pressed branding. The Criterion Channel has its sleeve typography and the "Closet Picks" interludes. MUBI has its serif identity and the daily-curation ritual. None of those use generative AI. We don't have the budget to commission a comparable bespoke layer in the same way.

[Luma Labs](https://lumalabs.ai/api) ships an image and video generation API (Photon, UNI-1.1, Ray2/Ray3). Used carefully, with a strong character bible and aggressive caching, it could give ani-gui its own illustrative tier without the "AI-styled abstract blob" cliche AGENTS.md §7 names as a hard avoid.

This proposal sketches what such a tier could look like, what it shouldn't look like, and how to introduce it without compromising the design discipline we just locked in.

## What this is and isn't

**This proposal is about original ani-gui IP** — a recurring mascot, illustrated empty / loading / error states, hand-feel dividers, character-driven onboarding moments. The assets ship with the build, not generated per-user-session.

**This is explicitly not about** generating fake "official" banners or hero art for licensed anime. That direction was raised and rejected during the M3.1 critic review — passing-off and trademark exposure on a piracy-adjacent app, plus the AGENTS.md §7 anti-pattern of letting AI imagery substitute for real catalog art. Kitsu's poster artwork stays primary. Cover-image fallbacks stay as the M3.1 blurred-poster + film-grain solution.

## The opportunity (where bespoke assets actually fit)

The current design has several surfaces that are typographically resolved but visually under-furnished. None of them carry licensed third-party imagery, so adding original illustration there has zero rights friction and meaningful visual upside.

| Surface | Today | What original assets buy us |
|---|---|---|
| Empty search results ("nothing matched 'xxx'") | Two-line typeset state | A small framed illustration — mascot leafing through a card catalog, pulling a blank — turns the empty into a moment, not a dead-end. |
| Continue Watching, when empty | "No entries yet" copy | Mascot or symbolic cabinet — gives onboarding a face. |
| First-launch onboarding | None yet | A 3-4 panel visual welcome that establishes the app's voice before any data exists. |
| Network-down / Kitsu timeout | "Couldn't reach Kitsu" alert | A poster-style illustration that frames the failure as part of the world (closed-for-the-day sign, projector dark) rather than a system error. |
| Locale chooser / settings empty states | Plain copy | Small editorial vignettes per locale — a quiet way to signal "we built this for you, specifically". |
| Loading transitions between pages | Skeleton cards | Optional: a brief mascot glyph that fades while the data loads. Sparingly. |
| About / credits | Doesn't exist yet | A long-form illustrated slate (mascot + the app's manifest + the upstream `ani-cli` credit) that doubles as the FOSS-license + GPL-3.0 attribution surface. |
| Error / 404 | None | Same illustrative pattern as network-down. |
| Player end-card | Stock | A "the projectionist is taking a break" frame for between-episode autoplay pauses. Optional, post-MVP. |

Across those eight surfaces, what we're really asking for is a small set (15-25) of static illustrations and a handful (≤ 5) of short looping vignettes, all in one consistent visual world.

## The original-IP angle

The user's instinct — *"we can make original anime characters to illustrate and embellish the UI"* — is the design unlock. The moment the illustrated tier is *original characters living inside ani-gui's world*, the asset class stops competing with licensed catalog art and starts being **brand identity**. Three layers are worth thinking about distinctly:

1. **Mascot** — a single recurring character (a librarian-projectionist of late-night cinema; matches the M3.1 aesthetic). Appears across empty states, onboarding, error frames. Functions like the way Linux distributions used to have mascots, or Discord's Wumpus, but illustrated as a quiet presence rather than a sticker. The mascot is *not* a chibi girl. It's an illustrated host figure with editorial restraint.
2. **Setting** — a recurring place: a small repertory cinema with a card catalog, a projector booth, a card-stamping desk. Empty states and error frames take place in this setting. This gives the illustrated tier internal coherence.
3. **Editorial vignettes** — single-frame illustrations of catalog motifs (a stack of VHS sleeves, a hand stamping a return card, a half-developed film reel). Used as section dividers or hero accents on rarely-visited pages (settings, about). These are the "manga-page hairlines" of the illustrated tier — they punctuate, they don't decorate.

**Why this works against the AGENTS.md §7 anti-patterns:**
- The mascot is a *character with a job*, not an "AI girl" face. Editorial illustration register, not anime-fanart-default.
- The setting is a *place with continuity*, not procedurally-generated abstract space.
- The vignettes are *single objects with weight*, not "aurora" / "nebula" decoration.

All three layers are genuinely *original IP*. ani-gui owns them. They can ship under GPL-3.0 alongside the code.

## Style guard rails (to prevent AI-cliche output)

Luma's defaults out of the box are exactly what AGENTS.md §7 names as anti-patterns: over-rendered, generic moe, glossy-eyed, abstract-blob backgrounds. Using the API without a tight character bible would produce Pinterest-tier output — the worst possible outcome for a project where UI is top priority.

Required before any generation runs:

1. **Character bible**, ~5-10 pages: face design (specific eye shape, specific nose, specific skin-tone range), wardrobe (specific palette restricted to the M3.1 warm-ink + bone-white system, plus exactly one accent at a time from our 8-accent palette), pose vocabulary (10-15 reference poses), hand language, a hard ban on the AI tells: galaxy eyes, gradient hair, holographic accents, chrome reflections, perfectly symmetric faces.
2. **Setting bible**: the repertory cinema's interior in 6-8 reference angles. Lighting is tungsten-warm, never neon. Surfaces are wood, paper, brass, fabric — not glass or chrome.
3. **Style reference**: think *Yokohama Kaidashi Kikō* (Ashinano) for line economy, mid-period Studio Ghibli backgrounds (specifically Whisper of the Heart and The Cat Returns) for interior warmth, *Kids on the Slope* for character framing. **Not** Makoto Shinkai (over-rendered skies are precisely the AI tell to avoid). **Not** Demon Slayer (post-2018 hyper-saturated digital is the other AI tell).
4. **Output review process**: every Luma generation goes through a critic agent that checks against the character bible. The critic agent for M3.1 is the right pattern; same harness, different rubric.
5. **Hand-edit pass**: every shipped asset has a human (or another agent acting as illustrator) clean up Luma artifacts before bake — extra fingers, melting jewelry, gradient artifacts on solid surfaces. No raw Luma output ships.

## Production pipeline (build-time, not runtime)

Generation is **never per-user-session**. Two practical reasons:

- Cost. UNI-1.1 is $0.04-0.10 per image; Ray2 video is unpriced in the docs but presumably higher. Per-user generation scales linearly with users.
- Determinism. The illustrated tier needs to be the *same* on every install. A user can't be asked to wait 8 seconds while their copy of an empty-state mascot generates.

Concrete pipeline:

1. Asset directory `gui/frontend/static/illustrations/`, gitignored at the source level but populated by a `pnpm run assets:bake` script that hits Luma, runs through the critic agent, and saves the surviving .webp / .mp4 outputs into the directory.
2. The bake script is run by maintainers, not CI. The output is committed to git (binary blobs under git-LFS) so that fresh checkouts don't regenerate.
3. Each asset has a manifest entry (`assets/MANIFEST.json`) recording: prompt, character bible version, generation timestamp, sha256, the critic verdict and any human edits applied. Same hygiene as our existing `tests/fixtures/` — the asset is a fixture, not a runtime artifact.
4. Components reference assets by their static path. No protocol handler, no remote URL. They ship as part of the bundled SPA.
5. To regenerate (e.g. when the character bible iterates), bump the bible version, re-run the bake, review the diff, commit.

## Cost ceiling

A back-of-envelope for the full illustrated tier:

- 25 static illustrations × ~$0.10 (Photon, large) = $2.50 for one full bake
- 5 looping vignettes × ~$1.00 (Ray2, conservative estimate) = $5.00
- ~10 character-bible reference generations (only run once, tuning) = $1.00
- **Total per full asset bake: ~$10.** Per maintainer, per character-bible iteration.

If we re-bake every quarter, that's $40/year. At v1 scale this is well below "hire an illustrator for a one-off" and doesn't scale with users. If usage data later shows real traction, swap to a commissioned human illustrator working from the same character bible.

## Open questions for the v1+ revisit

These are not blocking the proposal; they should be answered before any code commits.

1. **Does the illustrated tier feel right with the M3.1 aesthetic, or does it feel grafted?** Get a 3-4 frame mock from Luma against the character bible draft and compare side-by-side with the current /search and /anime/[id] screens. If the warm-ink-cinema voice tolerates illustration, proceed. If it doesn't, drop the proposal.
2. **Mascot or no mascot?** A single recurring character is a stronger commitment than a stable of vignettes. Mascots can age badly (Microsoft's Clippy) but also ship a lot of brand equity (Octocat). The user's read here is decisive.
3. **GPL-3.0 implications for assets.** GPL only binds the code; assets need explicit license declaration. Probably CC-BY-SA-4.0 to match the spirit, with attribution to ani-gui (and to Luma if their TOS requires it — to verify).
4. **Asset format.** WebP for stills is right. For looping vignettes, AV1 is smallest but webkit2gtk's media support is uneven; H.264 in MP4 is the safe path. Worth a one-day spike before locking.
5. **Bake on what cadence?** Once-and-done after launch, or quarterly to refresh the character?
6. **Translation of asset captions.** Empty-state illustrations may include readable text (catalog cards, signs). That text needs to be either symbolic / illegible *or* fully localized — text in only English would break the i18n discipline.

## Sketch of v1 of this feature (if approved post-v1)

1. **M-illustration-1** — character bible + setting bible. Two markdown documents in `docs/illustration/`, plus 6-8 reference images bundled under git-LFS. No Luma yet — drafted by hand or commissioned from a human illustrator if the budget allows. The bibles must be settled before generation begins.
2. **M-illustration-2** — bake pipeline. `pnpm run assets:bake` script, MANIFEST.json discipline, critic-agent review harness adapted from the M3.1 design-critic pattern.
3. **M-illustration-3** — bake the first 25 stills (empty states + onboarding + error frames). Hand-edit pass. Commit under git-LFS.
4. **M-illustration-4** — wire into existing routes. Empty states get illustrations. Onboarding ships if `ani-hsts` is empty on first launch. Error states upgrade. Settings page gets vignette dividers.
5. **M-illustration-5** — the 5 looping vignettes. Player end-card, between-episode pause, one onboarding hero. Optional; only if M-illustration-4 ships well and the cost model holds.

Steps 1 and 2 are the load-bearing ones. Once those exist, steps 3-5 are mostly content production with a stable pipeline.

## What this proposal isn't asking for now

- Approval to start work — this is a v1+ proposal.
- A budget commitment — costs are bounded but worth confirming.
- A character design — that's a v1+ task.
- An immediate revisit — file it. Revisit when v1 ships, when usage data exists, and when the UI surfaces named in the table above feel under-furnished in real use.
