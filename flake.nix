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
        # Define the desired Rust nightly toolchain with rust-src once
        rustNightlyWithSrc = pkgs.rust-bin.selectLatestNightlyWith (toolchain:
          toolchain.default.override {
            extensions = [ "rust-src" ];
          }
        );
      in
      with pkgs;
      {
        devShells.default = mkShell {
          buildInputs = [
            rustNightlyWithSrc # Use the defined toolchain
            cargo-watch 
            # Add other development tools here if needed
          ];

          # Attempt to access rustLibSrc from the overridden toolchain
          RUST_SRC_PATH = rustNightlyWithSrc.rustLibSrc;
        };
      }
    );
}
