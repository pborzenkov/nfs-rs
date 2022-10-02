{
  description = "nfs - async Rust NFS client on top of libnfs";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, ... } @ inputs: inputs.flake-utils.lib.eachDefaultSystem (system:
    let
      pkgs = import inputs.nixpkgs { inherit system; overlays = [ (import inputs.rust-overlay) ]; };
      rust = pkgs.rust-bin.stable.latest;
      libnfs = pkgs.libnfs.overrideAttrs (old: {
        configureFlags = [ "--enable-pthread" ];
      });

      craneLib = (inputs.crane.mkLib pkgs).overrideToolchain rust.default;

      commonArgs = {
        src = ./.;
        nativeBuildInputs = [ pkgs.rustPlatform.bindgenHook ];
        buildInputs = [
          libnfs
        ];
      };

      cargoArtifacts = craneLib.buildDepsOnly (commonArgs // { });

      fmt = craneLib.cargoFmt (commonArgs // { });

      clippy = craneLib.cargoClippy (commonArgs // {
        inherit cargoArtifacts;

        cargoClippyExtraArgs = "-- --deny warnings";
      });

      test = craneLib.cargoNextest (commonArgs // {
        cargoArtifacts = clippy;
      });

      nfs = craneLib.buildPackage (commonArgs // {
        cargoArtifacts = test;

        doCheck = false;
      });
    in
    {
      checks = {
        inherit nfs;
      };

      packages.default = nfs;

      devShells.default = pkgs.mkShell {
        inputsFrom = [ nfs ];

        nativeBuildInputs = [
          (
            rust.default.override
              {
                extensions = [ "rust-src" ];
              }
          )
        ];
      };
    });
}
