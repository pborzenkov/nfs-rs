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
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
  };

  outputs = { self, ... } @ inputs: inputs.flake-utils.lib.eachDefaultSystem (system:
    let
      pkgs = import inputs.nixpkgs { inherit system; overlays = [ (import inputs.rust-overlay) ]; };
      rust = pkgs.rust-bin.stable.latest;

      craneLib = (inputs.crane.mkLib pkgs).overrideToolchain rust.default;

      commonArgs = {
        src = ./.;
        nativeBuildInputs = [ pkgs.rustPlatform.bindgenHook ];
        buildInputs = [
          pkgs.libnfs
        ];
      };

      cargoArtifacts = craneLib.buildDepsOnly (commonArgs // {
        installCargoArtifactsMode = "use-zstd";
      });

      nfs = craneLib.buildPackage (commonArgs // {
        cargoArtifacts = cargoArtifacts;

        doCheck = false;
      });
    in
    {
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
