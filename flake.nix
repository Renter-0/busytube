{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
  };

  outputs = {
    self,
    nixpkgs,
  }: let
    system = "x86_64-linux";
    pkgs = import nixpkgs {inherit system;};
  in {
    devShells.${system}.default = pkgs.mkShell {
      buildInputs = [
        pkgs.openssl
        pkgs.pkg-config
        pkgs.rustc
        pkgs.cargo
        pkgs.rust-analyzer
        pkgs.rustfmt
        pkgs.clippy
        pkgs.gcc
      ];
    };
  };
}
