# Nix package validation

The current flake at source `21145316` was evaluated in a disposable `nixos/nix`
container using Nix 2.35.4 and the locked `nixpkgs` revision from `flake.lock`.
No host directories were mounted. The source archive was exported from the Git
revision and the existing private Nix cache was imported into the container.

`nix flake check --all-systems --no-build --offline` passed for both
`x86_64-linux` and `aarch64-linux`. It evaluated the package, checks, dev shells and
formatter and produced valid derivations for both targets.

The x86_64 `nix build .#flameshot --offline --no-link -L` reached dependency builds
but did not complete. Nix requested the uncached nixpkgs bootstrap input
`gcc-g++-4.6.4.tar.gz` from `ftpmirror.gnu.org`; that mirror timed out after one
second retries. The failure is recorded in `target/nix-container-build.log` and is
outside the Flameshot source or Rust dependency graph. The flake evaluation and
source/package expression therefore pass; a complete binary build remains pending
until a normal Nix binary cache or reachable source mirror supplies that historical
bootstrap artifact.

The container and its imported private store were isolated and the container was
removed after the build stopped. No system Nix store or host configuration changed.
Hosted `.github/workflows/nix-rust.yml` still provides the authoritative normal-store
build path when CI is run; hosted CI has not run in this environment.

Commands used:

```sh
nix --extra-experimental-features 'nix-command flakes' \
  flake check --all-systems --no-build --offline
nix --extra-experimental-features 'nix-command flakes' \
  build .#flameshot --offline --no-link -L
```
