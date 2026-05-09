{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.11";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
    nixpkgs-unstable.url = "github:NixOS/nixpkgs/nixos-unstable";
    naersk.url = "github:nix-community/naersk";
    naersk.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = { self, nixpkgs, nixpkgs-unstable, flake-utils, rust-overlay, naersk }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [
            rust-overlay.overlays.default
            (final: prev: {
              onnxruntime = nixpkgs-unstable.legacyPackages.${system}.onnxruntime;
            })
          ];
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rustfmt" "clippy" ];
        };

        naersk' = pkgs.callPackage naersk {
          cargo = rustToolchain;
          rustc = rustToolchain;
        };

        commonArgs = {
          src = ./.;
          buildInputs = [ ]
            ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
              pkgs.libiconv
            ];
        };

        pkms = naersk'.buildPackage commonArgs;
      in
      {
        packages = {
          default = pkms;
          embed = naersk'.buildPackage (commonArgs // {
            cargoBuildOptions = old: old ++ [ "--features" "embed" ];
            buildInputs = commonArgs.buildInputs ++ [ pkgs.onnxruntime ];
            ORT_LIB_PATH = "${pkgs.onnxruntime}/lib";
            ORT_PREFER_DYNAMIC_LINK = "true";
          });
        };

        apps = {
          default = flake-utils.lib.mkApp { drv = pkms; };
          embed = flake-utils.lib.mkApp {
            drv = self.packages.${system}.embed;
          };
        };

        devShells.default = pkgs.mkShell {
          packages = [ rustToolchain pkgs.pkg-config ];
        };
      });
}
