{
  pkgs ? import <nixpkgs> {},
  src ? ./.,
}: let
  manifest = (pkgs.lib.importTOML "${src}/Cargo.toml").package;
  source = src;
in
  pkgs.rustPlatform.buildRustPackage rec {
    pname = manifest.name;
    version = manifest.version;
    cargoLock.lockFile = "${source}/Cargo.lock";
    src = pkgs.lib.cleanSource "${source}";
    nativeBuildInputs = [pkgs.pkg-config];
    buildInputs = [pkgs.openssl];
    checkFlags = [
      # Tests rely on network
      "--skip=tests::test_download_htmls_length"
      "--skip=tests::test_is_downloaded_fragment_sufficient_for_parsing"
    ];
  }
