{ 
  pkgs ? import <nixpkgs> {}, 
  withMpv ? true,
  withVlc ? false,
  withIina ? false,
  chromecastSupport ? false,
  syncSupport ? false
}:

# To start the dev shell use the comment nix-shell
# use --arg withVlc true to use VLC
# use --arg withIina true to use Iina
# use --arg chromecastSupport true to use chromecastSupport
# use --arg syncSupport true to use syncSupport

assert withMpv || withVlc || withIina;

with pkgs;
mkShell {
  name = "ani-cli dev shell";
  buildInputs = [ (ani-cli.override ({ withMpv = withMpv; withVlc = withVlc; withIina = withIina; chromecastSupport = chromecastSupport; syncSupport = syncSupport; })).runtimeDependencies ];
}
