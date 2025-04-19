{pkgs ? import <nixpkgs>}:
pkgs.mkShell {
  nativeBuildInputs = [pkgs.pkg-config];
  buildInputs = [
    pkgs.openssl
    pkgs.rustc
    pkgs.cargo
    pkgs.rust-analyzer
    pkgs.rustfmt
    pkgs.clippy
    pkgs.gcc
  ];
}
