; Custom install/uninstall hooks injected into electron-builder's
; generated NSIS script via `nsis.include` in package.json.
;
; Today's only job: fetch ffmpeg.exe at install time so downloads
; work out of the box. fzf and aria2c are bundled (small enough);
; ffmpeg's Windows static build is ~80 MB compressed, too heavy
; to ship in every installer copy when most users are online during
; install anyway.
;
; The download lands in $INSTDIR\resources\bin\ alongside the
; bundled fzf.exe and aria2c.exe — exactly where the Rust backend's
; AppState::bundled_bin already looks for POSIX-side deps. No
; runtime code change needed.
;
; Failure modes are non-fatal: if the download fails (offline, host
; down, mismatch) the install completes anyway. The user retries by
; running the installer again, or installs ffmpeg manually. The
; runtime then surfaces ani-cli's stderr through the dock's generic
; error path; we'll wire a friendlier in-app retry once the install-
; time path has been exercised in the wild.

; Pinned ffmpeg-essentials build from gyan.dev's official Windows
; build repo — static single-binary GPL build (compatible with
; ani-cli's licence). Bump VERSION when refreshing. We skip SHA-256
; verification here because HTTPS-to-GitHub already protects
; integrity end-to-end and PowerShell-quoted Get-FileHash inside
; nsExec is fragile to escape rules; the bundled deps (fzf, aria2c)
; still SHA-verify at build time via scripts/fetch-windows-deps.mjs.
!define FFMPEG_VERSION "7.1.1"
!define FFMPEG_URL "https://github.com/GyanD/codexffmpeg/releases/download/${FFMPEG_VERSION}/ffmpeg-${FFMPEG_VERSION}-essentials_build.zip"

!macro customInstall
    ; --- 0. Skip if ffmpeg is already reachable -------------------
    ; Users who installed ffmpeg via winget/scoop/standalone get
    ; their copy on PATH. `where` exits 0 when at least one match
    ; is found, non-zero otherwise. We don't bundle a redundant copy
    ; in that case — keeps the install fast and avoids a stale
    ; pinned version overshadowing a user-managed up-to-date one.
    DetailPrint "Checking for an existing ffmpeg on PATH..."
    nsExec::ExecToStack 'cmd /c where ffmpeg.exe'
    Pop $0
    Pop $1
    StrCmp $0 "0" ffmpeg_already_present 0
    Goto ffmpeg_fetch

    ffmpeg_already_present:
        DetailPrint "ffmpeg already installed — skipping bundled fetch."
        Goto ffmpeg_done

    ffmpeg_fetch:
        DetailPrint "Fetching ffmpeg ${FFMPEG_VERSION} (~80 MB; required for downloads)..."

    ; $PLUGINSDIR is a temp dir NSIS creates and cleans on install
    ; finish, so the ~80 MB zip + extracted tree don't linger after
    ; we've copied the one binary we need.
    InitPluginsDir

    ; --- 1. Download with progress UI -----------------------------
    ; inetc ships with the NSIS that electron-builder uses; /POPUP ""
    ; opens a per-download progress dialog instead of using the main
    ; installer log, which gives the user a clear sense the install
    ; is still alive during the multi-second download. Failures here
    ; do NOT abort the install — ani-gui itself works without ffmpeg
    ; (playback still functions; only downloads need it). User can
    ; retry by reinstalling with a working connection.
    inetc::get /CAPTION "Downloading ffmpeg ${FFMPEG_VERSION}" /POPUP "" \
        "${FFMPEG_URL}" "$PLUGINSDIR\ffmpeg.zip" /END
    Pop $0
    StrCmp $0 "OK" ffmpeg_extract ffmpeg_failed

    ; --- 2. Extract the single binary we need ---------------------
    ; The zip nests ffmpeg.exe under
    ;   ffmpeg-<VERSION>-essentials_build/bin/ffmpeg.exe
    ; PowerShell Expand-Archive lays the whole tree out into the
    ; plugin temp dir; we then copy just the binary into the install
    ; resources dir and let $PLUGINSDIR cleanup wipe the rest.
    ffmpeg_extract:
        DetailPrint "Extracting ffmpeg.exe..."
        nsExec::ExecToStack 'powershell -NoProfile -ExecutionPolicy Bypass -Command "Expand-Archive -Path $\"$PLUGINSDIR\ffmpeg.zip$\" -DestinationPath $\"$PLUGINSDIR\ffmpeg-extract$\" -Force"'
        Pop $0
        Pop $1
        StrCmp $0 "0" ffmpeg_install ffmpeg_extract_fail

    ffmpeg_extract_fail:
        DetailPrint "ffmpeg extraction failed (exit=$0). Continuing without it."
        Goto ffmpeg_done

    ffmpeg_install:
        ; resources/bin already contains fzf.exe and aria2c.exe via
        ; electron-builder's extraResources. CreateDirectory is a
        ; no-op when the dir is present, and the bash subprocess on
        ; the runtime side reads PATH literally so adding ffmpeg here
        ; just works — no code change required.
        CreateDirectory "$INSTDIR\resources\bin"
        CopyFiles /SILENT \
            "$PLUGINSDIR\ffmpeg-extract\ffmpeg-${FFMPEG_VERSION}-essentials_build\bin\ffmpeg.exe" \
            "$INSTDIR\resources\bin\ffmpeg.exe"
        DetailPrint "ffmpeg.exe installed at $INSTDIR\resources\bin\ffmpeg.exe"
        Goto ffmpeg_done

    ffmpeg_failed:
        DetailPrint "ffmpeg download failed: $0. ani-gui will install anyway — playback still works; only downloads need ffmpeg. To retry, run the installer again with an internet connection, or drop ffmpeg.exe into $INSTDIR\resources\bin\ manually."

    ffmpeg_done:
!macroend

!macro customUnInstall
    ; Remove the ffmpeg.exe we dropped at install. The rest of
    ; resources/bin is electron-builder's own (fzf, aria2c) and gets
    ; cleaned up by the standard uninstall sequence; ours is the only
    ; file we have to take care of explicitly because customInstall
    ; brought it in outside the manifest.
    Delete "$INSTDIR\resources\bin\ffmpeg.exe"

    ; Optional: purge the per-user data dirs the running app writes to
    ; (cache.sqlite with play resolutions + image bytes, config.toml,
    ; ani-cli history, log dir). NSIS would never touch these on its
    ; own because they live outside $INSTDIR — they're created at
    ; runtime via the `directories` Rust crate (ProjectDirs::from(
    ; "net", "thirdmovement", "ani-gui")), which on Windows resolves
    ; to %LOCALAPPDATA%\thirdmovement\ani-gui\ for cache/data and
    ; %APPDATA%\thirdmovement\ani-gui\ for config.
    ;
    ; Default is No: a "reinstall to fix something" cycle should
    ; preserve the user's settings + cached resolutions (instant
    ; playback for previously-watched episodes is the main UX win).
    ; Explicit Yes wipes both trees so a deliberate "I'm done with
    ; this app" uninstall leaves nothing behind. MB_ICONQUESTION +
    ; MB_DEFBUTTON2 makes No the highlighted choice if the user just
    ; mashes Enter.
    MessageBox MB_YESNO|MB_ICONQUESTION|MB_DEFBUTTON2 \
        "Also delete your ani-gui settings, cache, and watch history?$\r$\n$\r$\nChoose No to keep them — handy if you plan to reinstall." \
        IDYES purge_user_data IDNO purge_done

    purge_user_data:
        DetailPrint "Removing user data under $LOCALAPPDATA\thirdmovement\ani-gui..."
        RMDir /r "$LOCALAPPDATA\thirdmovement\ani-gui"
        DetailPrint "Removing user data under $APPDATA\thirdmovement\ani-gui..."
        RMDir /r "$APPDATA\thirdmovement\ani-gui"
        Goto purge_done

    purge_done:
!macroend
