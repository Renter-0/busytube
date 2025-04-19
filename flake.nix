{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
  }:
    flake-utils.lib.eachSystem flake-utils.lib.allSystems (system: let
      pkgs = nixpkgs.legacyPackages.${system};
    in {
      devShells.default = pkgs.callPackage ./shell.nix {inherit pkgs;};
      packages.default = pkgs.callPackage ./default.nix {
        inherit pkgs;
        src = self;
      };
    });
}
