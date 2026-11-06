{
  description = "Flameshot screenshot utility, implemented in Rust";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-compat.url = "https://flakehub.com/f/edolstra/flake-compat/1.tar.gz";
  };

  outputs =
    { nixpkgs, ... }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
      packagesFor =
        system:
        let
          pkgs = import nixpkgs { inherit system; };
          inherit (pkgs) lib;
          runtimeLibraries = with pkgs; [
            libx11
            libxcursor
            libxi
            libxrandr
            libxkbcommon
            wayland
            libGL
            libglvnd
            vulkan-loader
          ];
          flameshot = pkgs.rustPlatform.buildRustPackage {
            pname = "flameshot";
            version = "15.0.0";
            src = lib.fileset.toSource {
              root = ./.;
              fileset = lib.fileset.unions [
                ./Cargo.toml
                ./Cargo.lock
                ./crates
                ./data
                ./LICENSE
                ./scripts/stage-rust.sh
              ];
            };
            cargoLock.lockFile = ./Cargo.lock;
            buildNoDefaultFeatures = true;
            buildFeatures = [ "imgur" ];
            nativeBuildInputs = [
              pkgs.pkg-config
              pkgs.makeWrapper
            ];
            buildInputs = runtimeLibraries;
            nativeCheckInputs = [
              pkgs.python3
              pkgs.glib
              pkgs.dejavu_fonts
            ];
            preCheck = ''
              export HOME="$TMPDIR/home"
              mkdir -p "$HOME"
              export FONTCONFIG_FILE=${pkgs.makeFontsConf { fontDirectories = [ pkgs.dejavu_fonts ]; }}
            '';
            installPhase = ''
              runHook preInstall
              DESTDIR="$TMPDIR/stage" PREFIX="$out" \
                FLAMESHOT_BINARY="target/${pkgs.stdenv.hostPlatform.rust.rustcTarget}/release/flameshot" \
                sh scripts/stage-rust.sh
              mkdir -p "$out"
              cp -a "$TMPDIR/stage$out/." "$out/"
              runHook postInstall
            '';
            postFixup = ''
              wrapProgram "$out/bin/flameshot" \
                --prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath runtimeLibraries} \
                --prefix PATH : ${lib.makeBinPath [ pkgs.glib ]}
            '';
            meta = {
              description = "Powerful yet simple to use screenshot software";
              homepage = "https://github.com/flameshot-org/flameshot";
              license = lib.licenses.gpl3Plus;
              platforms = systems;
              mainProgram = "flameshot";
            };
          };
        in
        {
          inherit pkgs flameshot runtimeLibraries;
        };
    in
    {
      packages = forAllSystems (
        system:
        let
          p = packagesFor system;
        in
        {
          default = p.flameshot;
          flameshot = p.flameshot;
        }
      );
      checks = forAllSystems (system: {
        build = (packagesFor system).flameshot;
      });
      devShells = forAllSystems (
        system:
        let
          p = packagesFor system;
        in
        {
          default = p.pkgs.mkShell {
            inputsFrom = [ p.flameshot ];
            packages = with p.pkgs; [
              rustc
              cargo
              rustfmt
              clippy
              python3
              glib
            ];
            LD_LIBRARY_PATH = p.pkgs.lib.makeLibraryPath p.runtimeLibraries;
          };
        }
      );
      formatter = forAllSystems (system: (packagesFor system).pkgs.nixfmt);
    };
}
