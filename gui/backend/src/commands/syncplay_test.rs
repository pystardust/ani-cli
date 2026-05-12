use super::*;

fn args(stream: &str, binary: &str) -> SyncplayLaunchArgs {
    SyncplayLaunchArgs {
        stream_url: stream.into(),
        binary: binary.into(),
        referer: None,
        subtitle_url: None,
        player_kind: ExternalPlayerKind::Mpv,
        player_binary: String::new(),
    }
}

#[test]
fn argv_emits_player_path_before_stream_when_set() {
    // Syncplay's `--player-path=` is a Syncplay option, not a
    // player option, so it lives BEFORE the file/positional and
    // BEFORE any `--` separator. Setting it overrides Syncplay's
    // own .ini config — that's the whole point: guarantee the
    // wrapped binary matches the player_kind we picked flags for.
    let mut a = args("https://example.com/master.m3u8", "syncplay");
    a.player_binary = "/usr/bin/vlc".into();
    a.player_kind = ExternalPlayerKind::Vlc;
    let v = build_argv(&a);
    assert_eq!(
        v,
        vec![
            "--player-path=/usr/bin/vlc".to_string(),
            "https://example.com/master.m3u8".to_string(),
        ]
    );
}

#[test]
fn argv_emits_player_path_alongside_post_separator_flags() {
    // The Syncplay option and the post-`--` player options can
    // both be present in the same argv. Order: `--player-path=`
    // first (Syncplay option), then the URL (positional), then
    // `--` separator, then the player-kind-specific referrer.
    let mut a = args("https://example.com/master.m3u8", "syncplay");
    a.player_binary = "/usr/bin/vlc".into();
    a.player_kind = ExternalPlayerKind::Vlc;
    a.referer = Some("https://allmanga.to".into());
    let v = build_argv(&a);
    assert_eq!(
        v,
        vec![
            "--player-path=/usr/bin/vlc".to_string(),
            "https://example.com/master.m3u8".to_string(),
            "--".to_string(),
            "--http-referrer=https://allmanga.to".to_string(),
        ]
    );
}

#[test]
fn argv_omits_player_path_when_player_binary_empty() {
    // Empty player_binary = back-compat path: don't emit the
    // flag, let Syncplay's own config pick. Old IPC payloads
    // that pre-date this field decode as empty-string by serde
    // default, so this also covers the schema-rollforward case.
    let mut a = args("https://example.com/v.mp4", "syncplay");
    a.player_binary = String::new();
    let v = build_argv(&a);
    assert_eq!(v, vec!["https://example.com/v.mp4".to_string()]);
}

#[test]
fn argv_emits_player_path_for_bare_command_names() {
    // Syncplay's player classes resolve bare names by walking
    // `os.environ['PATH']` via `getExpandedPath` (see e.g.
    // syncplay/players/mpv.py — `for path in
    // os.environ['PATH'].split(':'): ... os.access(...)`), so a
    // value like "mpv" or "vlc" still pins the wrapped binary
    // and keeps the .ini-mismatch guarantee. Skipping the flag
    // for bare commands — which an earlier attempt did — broke
    // the mismatch scenario (ani-gui VLC, Syncplay .ini mpv).
    for bare in ["mpv", "vlc", "iina"] {
        let mut a = args("https://example.com/v.mp4", "syncplay");
        a.player_binary = bare.into();
        let v = build_argv(&a);
        assert_eq!(
            v,
            vec![
                format!("--player-path={bare}"),
                "https://example.com/v.mp4".to_string(),
            ],
            "bare command {bare:?} must still pin via --player-path",
        );
    }
}

#[test]
fn argv_emits_player_path_for_relative_path_with_separator() {
    // A relative path is still a path — Syncplay resolves it
    // against its CWD (its player classes call `os.access` after
    // the PATH-walk fallback).
    let mut a = args("https://example.com/v.mp4", "syncplay");
    a.player_binary = "./vendor/mpv".into();
    let v = build_argv(&a);
    assert_eq!(
        v,
        vec![
            "--player-path=./vendor/mpv".to_string(),
            "https://example.com/v.mp4".to_string(),
        ]
    );
}

#[test]
fn argv_emits_player_path_for_windows_style_path() {
    // Windows paths use `\` as the separator. The default
    // installer location for Syncplay-targeted mpv on Windows
    // lives under `C:\Program Files\...`, so backslashes are
    // the live case we have to cover.
    let mut a = args("https://example.com/v.mp4", "syncplay");
    a.player_binary = r"C:\Program Files\mpv\mpv.exe".into();
    let v = build_argv(&a);
    assert_eq!(
        v,
        vec![
            r"--player-path=C:\Program Files\mpv\mpv.exe".to_string(),
            "https://example.com/v.mp4".to_string(),
        ]
    );
}

#[test]
fn launch_args_decode_without_player_binary_for_back_compat() {
    // Old payloads (pre-player-path) don't include the field.
    // They must decode and default to empty-string so build_argv
    // skips the `--player-path=` emission.
    let json = r#"{
        "stream_url": "https://example.com/v.mp4",
        "binary": "syncplay"
    }"#;
    let a: SyncplayLaunchArgs =
        serde_json::from_str(json).expect("decodes with default player_binary");
    assert!(a.player_binary.is_empty());
}

#[test]
fn argv_with_no_referer_is_just_the_url() {
    // Bare URL is the no-referer baseline. Most catalogues work
    // this way; only referer-required CDNs (fast4speed.rsvp)
    // exercise the forwarding path.
    let v = build_argv(&args("https://example.com/master.m3u8", "syncplay"));
    assert_eq!(v, vec!["https://example.com/master.m3u8".to_string()]);
}

#[test]
fn argv_forwards_referer_after_separator() {
    // Syncplay's CLI grammar is `syncplay [options] [file] --
    // [player options]`. The `--` separator hands the rest to
    // the wrapped player (mpv by default), so the mpv-style
    // `--referrer=` flag is what reaches the upstream CDN.
    // Without this, fast4speed.rsvp 403s under Syncplay's mpv
    // even though play_external can play the same URL.
    let mut a = args("https://example.com/master.m3u8", "syncplay");
    a.referer = Some("https://allmanga.to".into());
    let v = build_argv(&a);
    assert_eq!(
        v,
        vec![
            "https://example.com/master.m3u8".to_string(),
            "--".to_string(),
            "--referrer=https://allmanga.to".to_string(),
        ]
    );
}

#[test]
fn argv_drops_empty_referer() {
    // An empty-string referer is no better than no referer at
    // all — emitting `--referrer=` with nothing after it would
    // make mpv complain. Drop the whole `--` block in that case.
    let mut a = args("https://example.com/v.mp4", "syncplay");
    a.referer = Some(String::new());
    let v = build_argv(&a);
    assert_eq!(v, vec!["https://example.com/v.mp4".to_string()]);
}

#[test]
fn launch_args_decode_without_referer_for_back_compat() {
    // Old client payloads (pre-referer-forwarding) don't include
    // the `referer` field. They must still decode and default to
    // None.
    let json = r#"{
        "stream_url": "https://example.com/v.mp4",
        "binary": "syncplay"
    }"#;
    let a: SyncplayLaunchArgs = serde_json::from_str(json).expect("decodes with default referer");
    assert!(a.referer.is_none());
    assert!(a.subtitle_url.is_none());
}

#[test]
fn argv_forwards_subtitle_after_separator() {
    // Soft-subtitle streams: ani-cli's parser surfaces a sidecar
    // `.vtt` URL alongside the stream. play_external forwards it
    // as `--sub-file=`; Syncplay's wrapped mpv needs the same
    // flag past the `--` separator, or the user sees the video
    // play but loses subtitles.
    let mut a = args("https://example.com/master.m3u8", "syncplay");
    a.subtitle_url = Some("https://example.com/subs.vtt".into());
    let v = build_argv(&a);
    assert_eq!(
        v,
        vec![
            "https://example.com/master.m3u8".to_string(),
            "--".to_string(),
            "--sub-file=https://example.com/subs.vtt".to_string(),
        ]
    );
}

#[test]
fn argv_forwards_referer_and_subtitle_together() {
    // Both flags share one `--` separator. Order matches mpv's
    // typical argv shape: title-y flags first (none here), then
    // sub-file, then referrer, then the URL — but here the URL
    // is the file argument BEFORE `--`, so post-separator order
    // is what counts: referrer then sub-file (stable across CDN
    // shapes that supply both).
    let mut a = args("https://example.com/master.m3u8", "syncplay");
    a.referer = Some("https://allmanga.to".into());
    a.subtitle_url = Some("https://example.com/subs.vtt".into());
    let v = build_argv(&a);
    assert_eq!(
        v,
        vec![
            "https://example.com/master.m3u8".to_string(),
            "--".to_string(),
            "--referrer=https://allmanga.to".to_string(),
            "--sub-file=https://example.com/subs.vtt".to_string(),
        ]
    );
}

#[test]
fn argv_drops_empty_subtitle() {
    // Defensive: an empty `subtitle_url` falls through the same
    // way an empty `referer` does — emitting `--sub-file=` with
    // nothing after the equals just makes mpv complain.
    let mut a = args("https://example.com/v.mp4", "syncplay");
    a.subtitle_url = Some(String::new());
    let v = build_argv(&a);
    assert_eq!(v, vec!["https://example.com/v.mp4".to_string()]);
}

#[test]
fn argv_for_vlc_uses_http_referrer() {
    // VLC's flag for the Referer header is `--http-referrer=`,
    // not mpv's `--referrer=`. Codex flagged on PR #12 that a
    // Syncplay→VLC user gets either an "unknown option" error
    // or a silent fall-through to no-Referer with the mpv flag,
    // breaking fast4speed.rsvp streams that play fine under
    // Syncplay→mpv.
    let mut a = args("https://example.com/v.mp4", "syncplay");
    a.player_kind = ExternalPlayerKind::Vlc;
    a.referer = Some("https://allmanga.to".into());
    a.subtitle_url = Some("https://example.com/subs.vtt".into());
    let v = build_argv(&a);
    assert_eq!(
        v,
        vec![
            "https://example.com/v.mp4".to_string(),
            "--".to_string(),
            "--http-referrer=https://allmanga.to".to_string(),
            "--sub-file=https://example.com/subs.vtt".to_string(),
        ]
    );
}

#[test]
fn argv_for_iina_uses_mpv_prefixed_referrer() {
    // IINA wraps mpv on macOS and forwards flags through
    // `--mpv-` prefixes. Mirror what external_player.rs's
    // Iina branch emits so a Syncplay→IINA setup carries the
    // same headers a direct IINA launch would.
    let mut a = args("https://example.com/v.mp4", "syncplay");
    a.player_kind = ExternalPlayerKind::Iina;
    a.referer = Some("https://allmanga.to".into());
    a.subtitle_url = Some("https://example.com/subs.vtt".into());
    let v = build_argv(&a);
    assert_eq!(
        v,
        vec![
            "https://example.com/v.mp4".to_string(),
            "--".to_string(),
            "--mpv-referrer=https://allmanga.to".to_string(),
            "--sub-file=https://example.com/subs.vtt".to_string(),
        ]
    );
}

#[test]
fn argv_for_custom_emits_no_player_specific_flags() {
    // Custom is the escape hatch — we don't know what flag
    // shape the user's player accepts, so passing anything
    // would risk an "unknown option" error. The user is
    // expected to configure referer / sub-file in their own
    // player's config (~/.config/mpv/mpv.conf etc.).
    let mut a = args("https://example.com/v.mp4", "syncplay");
    a.player_kind = ExternalPlayerKind::Custom;
    a.referer = Some("https://allmanga.to".into());
    a.subtitle_url = Some("https://example.com/subs.vtt".into());
    let v = build_argv(&a);
    assert_eq!(v, vec!["https://example.com/v.mp4".to_string()]);
}

#[test]
fn launch_args_decode_without_player_kind_defaults_to_mpv() {
    // Old payloads (pre-player-kind threading) don't include
    // `player_kind`. They must still decode and default to Mpv
    // — that's the upstream Syncplay default and matches the
    // pre-merge syncplay path's behaviour.
    let json = r#"{
        "stream_url": "https://example.com/v.mp4",
        "binary": "syncplay"
    }"#;
    let a: SyncplayLaunchArgs =
        serde_json::from_str(json).expect("decodes with default player_kind");
    assert_eq!(a.player_kind, ExternalPlayerKind::Mpv);
}

#[test]
fn open_syncplay_with_blank_binary_returns_spawn_failed() {
    // Blank binary is a misconfigured-settings case; treat it the
    // same as "binary not found" so the frontend can surface the
    // same syncplay.pl install pointer.
    let r = open_syncplay(&args("https://example.com/v.mp4", ""));
    match r {
        Err(AniError::SyncplaySpawnFailed { binary }) => assert!(binary.is_empty()),
        other => panic!("expected SyncplaySpawnFailed, got {other:?}"),
    }
}

#[test]
fn open_syncplay_with_unknown_binary_carries_binary_name() {
    // The whole point of the typed variant: the frontend can name
    // which binary failed in the error dialog. Pin that the
    // configured value flows through verbatim.
    let r = open_syncplay(&args(
        "https://example.com/v.mp4",
        "__definitely_not_a_real_syncplay__",
    ));
    match r {
        Err(AniError::SyncplaySpawnFailed { binary }) => {
            assert_eq!(binary, "__definitely_not_a_real_syncplay__");
        }
        other => panic!("expected SyncplaySpawnFailed, got {other:?}"),
    }
}
