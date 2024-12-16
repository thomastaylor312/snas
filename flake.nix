{
  description = "Build a cargo workspace";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/release-24.11";

    crane.url = "github:ipetkov/crane";

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.rust-analyzer-src.follows = "";
    };

    flake-utils.url = "https://flakehub.com/f/numtide/flake-utils/0.1.102";

    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
      fenix,
      flake-utils,
      advisory-db,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        inherit (pkgs) lib;

        craneLib = crane.mkLib pkgs;
        src = craneLib.cleanCargoSource ./.;

        # Common arguments can be set here to avoid repeating them later
        commonArgs = {
          inherit src;
          strictDeps = true;
          cargoExtraArgs = "";

          buildInputs =
            [
              # Add additional build inputs here
            ]
            ++ lib.optionals pkgs.stdenv.isDarwin [
              # Additional darwin specific inputs can be set here
              pkgs.libiconv
            ];

          # Additional environment variables can be set directly
          # MY_CUSTOM_VAR = "some value";
        };

        craneLibLLvmTools = craneLib.overrideToolchain (
          fenix.packages.${system}.complete.withComponents [
            "cargo"
            "llvm-tools"
            "rustc"
          ]
        );

        # Build *just* the cargo dependencies (of the entire workspace),
        # so we can reuse all of that work (e.g. via cachix) when running in CI
        # It is *highly* recommended to use something like cargo-hakari to avoid
        # cache misses when building individual top-level-crates
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        individualCrateArgs = commonArgs // {
          inherit cargoArtifacts;
          inherit (craneLib.crateNameFromCargoToml { inherit src; }) version;
          # We don't run tests here but split them out into a separate check so they aren't run twice
          doCheck = false;
        };

        fileSetForCrate =
          crate:
          lib.fileset.toSource {
            root = crate;
            fileset = lib.fileset.unions [
              ./Cargo.toml
              ./Cargo.lock
              (craneLib.fileset.commonCargoSources ./crates/pam-nats)
              (craneLib.fileset.commonCargoSources ./crates/pam-socket)
              (craneLib.fileset.commonCargoSources ./crates/snas-lib)
              (craneLib.fileset.commonCargoSources crate)
            ];
          };

        snas-lib = craneLib.buildPackage (
          individualCrateArgs
          // {
            pname = "snas-lib";
            src = fileSetForCrate ./.;
            # This is a separate crate, so we can run unit tests here
            doCheck = true;
          }
        );
        snas = craneLib.buildPackage (
          individualCrateArgs
          // {
            pname = "snas";
            cargoExtraArgs = "--bin snas";
            src = fileSetForCrate ./.;
          }
        );
        snas-server = craneLib.buildPackage (
          individualCrateArgs
          // {
            pname = "snas-server";
            cargoExtraArgs = "--bin snas-server";
            src = fileSetForCrate ./.;
            doInstallCargoArtifacts = true;
          }
        );
      in
      {
        checks = {
          # Build the crates as part of `nix flake check` for convenience
          inherit snas snas-server;

          # Run clippy (and deny all warnings) on the workspace source,
          # again, reusing the dependency artifacts from above.
          #
          # Note that this is done as a separate derivation so that
          # we can block the CI if there are issues here, but not
          # prevent downstream consumers from building our crate by itself.
          workspace-clippy = craneLib.cargoClippy (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = "--all-targets -- --deny warnings";
            }
          );

          workspace-doc = craneLib.cargoDocTest (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoTestExtraArgs = "-p snas-lib";
            }
          );

          # Check formatting
          workspace-fmt = craneLib.cargoFmt {
            inherit src;
          };

          # Audit dependencies
          workspace-audit = craneLib.cargoAudit {
            inherit src advisory-db;
          };

          # Audit licenses
          # my-workspace-deny = craneLib.cargoDeny {
          #   inherit src;
          # };

          runE2ETests = craneLib.cargoTest (
            commonArgs
            // {
              inherit cargoArtifacts;
              nativeBuildInputs = with pkgs; [ nats-server ];
              preCheck = ''
                nats-server -js & 
                NATS_SERVER_PID=$!
                trap "kill $NATS_SERVER_PID" EXIT
              '';
            }
          );
        };

        packages =
          {
            inherit
              snas
              snas-server
              # TODO: Figure out how to expose the rlib as an additional artifact for the binaries and to expose here
              snas-lib
              ;
            default = snas;
          }
          // lib.optionalAttrs (!pkgs.stdenv.isDarwin) {
            workspace-llvm-coverage = craneLibLLvmTools.cargoLlvmCov (
              commonArgs
              // {
                inherit cargoArtifacts;
              }
            );
          };

        apps = {
          snas-server = flake-utils.lib.mkApp {
            drv = snas-server;
          };
          snas = flake-utils.lib.mkApp {
            drv = snas;
          };
        };

        devShells.default = craneLib.devShell {
          # Inherit inputs from checks.
          checks = self.checks.${system};

          RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";

          # Extra inputs can be added here; cargo and rustc are provided by default.
          packages = [
            pkgs.nats-server
            pkgs.natscli
            pkgs.git
          ];
        };
      }
    );
}
