{
  description = "A Rust development environment for the reverse proxy";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
      in
      with pkgs;
      {
        devShells.default = mkShell {
          buildInputs = [
            (rust-bin.selectLatestNightlyWith (toolchain: toolchain.default.override {
              extensions = [ "rust-src" ]; # for rust-analyzer
            }))
            cargo-watch # Optional: for development convenience
            # Add other development tools here if needed
          ];

          # Environment variables
          RUST_SRC_PATH = rust-bin.selectLatestNightlyWith (toolchain: toolchain.rustLibSrc);
        };
      }
    );
}
