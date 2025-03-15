{
  inputs = {
    nixpkgs.url = "nixpkgs";
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
        pkgs.clippy
        pkgs.gcc
      ];
    };
  };
}
