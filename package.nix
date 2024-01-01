{ fetchFromGitHub
, stdenv
, makeWrapper
, stdenvNoCC
, lib
, gnugrep
, gnused
, curl
, catt
, syncplay
, ffmpeg
, fzf
, writeShellScriptBin
, aria2
, withMpv ? true
, mpv
, withVlc ? false
, vlc
, withIina ? stdenv.isDarwin
, iina
,
}:
assert withMpv || withVlc || withIina;
stdenvNoCC.mkDerivation rec {
  pname = "ani-cli";
  version = "4.7";

  iina-fix = writeShellScriptBin "iina" "exec -a $0 ${iina}/Applications/IINA.app/Contents/MacOS/iina-cli $@";

  src = ./.;
  nativeBuildInputs = [ makeWrapper ];
  runtimeDependencies =
    let
      player =
        [ ]
        ++ lib.optional withMpv mpv
        ++ lib.optional withVlc vlc
        ++ lib.optional withIina "${iina-fix}";
    in
    [ gnugrep gnused curl fzf ffmpeg aria2 catt syncplay ]
    ++ player;

  installPhase = ''
    runHook preInstall

    install -Dm755 ani-cli $out/bin/ani-cli
    wrapProgram $out/bin/ani-cli \
      --prefix PATH : ${lib.makeBinPath runtimeDependencies}

    runHook postInstall
  '';
}
