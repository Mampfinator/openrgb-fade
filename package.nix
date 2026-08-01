{ lib, pkgs }:
let 
    cargo-toml = lib.importTOML ./Cargo.toml;
in
pkgs.rustPlatform.buildRustPackage {
    name = cargo-toml.package.name;
    version = cargo-toml.package.version;

    src = ./.;

    cargoHash = "sha256-xsuGfSKGzPqrrEY6jb15A2YKObUQ9HSD0GOV8UXi+sA=";

    nativeBuildInputs = [ pkgs.pkg-config ];
    # systemd includes libuvdev, and it works well enough :SuiProud:
    buildInputs = [ pkgs.systemd ];
    
    meta.mainProgram = "openrgb-fade";
}