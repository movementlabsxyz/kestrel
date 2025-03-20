{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/a7abebc31a8f60011277437e000eebcc01702b9f";
    rust-overlay.url = "github:oxalica/rust-overlay/75b2271c5c087d830684cd5462d4410219acc367";
    flake-utils.url = "github:numtide/flake-utils";
    foundry.url = "github:shazow/foundry.nix/f533e2c70e520adb695c9917be21d514c15b1c4d"; 
    crane.url = "github:ipetkov/crane";
    crane.inputs.nixpkgs.follows = "nixpkgs";
    
  };

  outputs = { nixpkgs, rust-overlay, flake-utils, foundry, crane, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) foundry.overlay ];
        };

        toolchain = p: (p.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml).override {
          extensions = [ "rustfmt" "clippy" ];
        };
        craneLib = (crane.mkLib pkgs).overrideToolchain(toolchain);

        frameworks = pkgs.darwin.apple_sdk.frameworks;

        # An LLVM build environment
        buildDependencies = with pkgs; [
          llvmPackages.bintools
          openssl
          openssl.dev
          libiconv 
          pkg-config
          libclang.lib
          libz
          clang
          pkg-config
          protobuf
          rustPlatform.bindgenHook
          lld
          coreutils
          gcc
          rust
          zlib
          fixDarwinDylibNames
        ];
        
        sysDependencies = with pkgs; [] 
        ++ lib.optionals stdenv.isDarwin [
          frameworks.Security
          frameworks.CoreServices
          frameworks.SystemConfiguration
          frameworks.AppKit
          libelf
        ] ++ lib.optionals stdenv.isLinux [
          udev
          systemd
          bzip2
          elfutils
        ];

        testDependencies = with pkgs; [
          python311
          just
          foundry-bin
          process-compose
          jq
          docker
          solc
          grpcurl
          grpcui
        ];

        # Specific version of toolchain
        rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        rustPlatform = pkgs.makeRustPlatform {
          cargo = rust;
          rustc = rust;
        };
    
      in {
        devShells = rec {
          default = docker-build;
          docker-build = pkgs.mkShell {
            ROCKSDB = pkgs.rocksdb;
            OPENSSL_DEV = pkgs.openssl.dev;
         
            buildInputs = with pkgs; [
              # rust toolchain
              (toolchain pkgs)
            ] ++ sysDependencies ++ buildDependencies ++ testDependencies;

            LD_LIBRARY_PATH = "${pkgs.stdenv.cc.cc.lib}/lib/";

            shellHook = ''
              #!/usr/bin/env ${pkgs.bash}
              # Export linker flags if on Darwin (macOS)
              if [[ "$(${pkgs.stdenv.hostPlatform.system})" =~ "darwin" ]]; then
                export LDFLAGS="-L/opt/homebrew/opt/zlib/lib"
                export CPPFLAGS="-I/opt/homebrew/opt/zlib/include"
              fi
              cat <<'EOF'
              ^    ^
               ^  ^
                ()
              kestrel
              EOF

              echo "Process orchestration in Rust"
            '';
          };
        };
      }
    );
}
