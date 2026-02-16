# Standalone test harness — run `nix-build` from this directory to verify the package builds.
# Delete this file before submitting to nixpkgs (it's not needed there).
let
  pkgs = import <nixpkgs> { };
in
pkgs.callPackage ./package.nix { }
