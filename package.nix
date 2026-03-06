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
  version = "0.3.0";

  src = fetchFromGitHub {
    owner = "jonochang";
    repo = "untangle";
    # Pin to immutable commit instead of mutable tag archives.
    rev = "8cee265cec0890d2fb66215248ef6421bb545eed";
    hash = "sha256-zHhk6f50bjiRi0PnY3YWcVvlzhj8q1NLroPxDVYOP7o=";
  };

  cargoHash = "sha256-5ktLAOiQJkreKVlnsEOGXF8Amrhc56BAYod4ziFVQYc=";

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
