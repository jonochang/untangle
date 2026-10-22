# This file goes in nixpkgs at: pkgs/by-name/un/untangle/package.nix
#
# To get the real cargoHash, set it to lib.fakeHash, run `nix-build -A untangle`,
# and the error output will contain the correct hash.
{
  lib,
  rustPlatform,
  fetchFromGitHub,
  pkg-config,
  cmake,
  openssl,
  libgit2,
  zlib,
  testers,
}:

rustPlatform.buildRustPackage (finalAttrs: {
  pname = "untangle";
  version = "0.3.2";

  src = fetchFromGitHub {
    owner = "jonochang";
    repo = "untangle";
    # Pin to immutable commit instead of mutable tag archives.
    rev = "1928bd891910aed9ef54736396d9bac2f5b09ef0";
    hash = "sha256-LJxGMJsCvZqHEqGoKOQZTw4c5ypIHzAOFeuWEQR8TaQ=";
  };

  cargoHash = "sha256-rOz04ou/+OK/vC53jBePuJVa5Qc9y1cJiU8XXje5eNI=";

  nativeBuildInputs = [
    pkg-config
    cmake
  ];

  buildInputs = [
    openssl
    libgit2
    zlib
  ];

  env = {
    OPENSSL_NO_VENDOR = "1";
    LIBGIT2_NO_VENDOR = "1";
  };

  # Keep package checks lightweight and deterministic in sandboxed builds.
  cargoTestFlags = [
    "--bins"
  ];

  passthru.tests.version = testers.testVersion {
    package = finalAttrs.finalPackage;
    command = "untangle --version";
  };

  meta = {
    description = "Module-level dependency graph analyzer for Go, Python, Ruby, and Rust";
    homepage = "https://github.com/jonochang/untangle";
    changelog = "https://github.com/jonochang/untangle/blob/v${finalAttrs.version}/CHANGELOG.md";
    license = with lib.licenses; [
      mit
      asl20
    ];
    maintainers = with lib.maintainers; [ jonochang ];
    mainProgram = "untangle";
    platforms = lib.platforms.unix;
  };
})
