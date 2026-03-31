{
  description = "rust environment";
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  outputs = { nixpkgs, fenix, naersk, ... }:
    let
      systems = [ "x86_64-linux" "aarch64-darwin" ];
      pkgsFor = system: import nixpkgs { inherit system; overlays = [ fenix.overlays.default ]; };

      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      packages = forAllSystems (system:
        let pkgs = pkgsFor system; in
        {
          # Nushell plugin — primary target; also exposed as `default`.
          nu_plugin_crossref = pkgs.callPackage ./package.nix { v110 = false; };
          # Universal CLI binary — renamed from `crossref` to avoid shadowing
          # the nu_plugin_crossref sub-commands in nushell's namespace.
          crossref-cli = pkgs.callPackage ./package-cli.nix { };
          default = pkgs.callPackage ./package.nix { v110 = false; };
        });

      devShells = forAllSystems (
        system:
        let
          pkgs = pkgsFor system;
        in
        {
          default = pkgs.mkShell {
            buildInputs = with pkgs; [
              (fenix.packages.${system}.stable.withComponents [
                "cargo"
                "clippy"
                "rust-src"
                "rustc"
                "rustfmt"
                "rust-analyzer"
              ])
              stdenv
              fish
            ];
          };
        }
      );
    };
}
