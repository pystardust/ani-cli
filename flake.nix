{
  description = "A cli to watch anime";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    { nixpkgs
    , self
    , flake-utils
    , ...
    }:
    flake-utils.lib.eachDefaultSystem
      (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in
      rec {
        packages = rec {
          ani-cli = pkgs.callPackage ./package.nix { };
          ani-cli-vlc = pkgs.callPackage ./package.nix {
            withVlc = true;
          };

          default = ani-cli;
        };

        devShells = {
          default = pkgs.mkShell {
            name = "Ani-cli shell";
            packages = with pkgs; [
              shfmt
              shellcheck
            ];
          };
        };

        apps = rec {
          ani-cli = flake-utils.lib.mkApp {
            drv = self.packages.${system}.ani-cli;
          };
          default = ani-cli;
        };
      });
}
