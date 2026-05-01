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
          nu_plugin_crossref_v110 = pkgs.callPackage ./package.nix { v110 = true; };
          # Universal CLI binary — renamed from `crossref` to avoid shadowing
          # the nu_plugin_crossref sub-commands in nushell's namespace.
          crossref-cli = pkgs.callPackage ./package-cli.nix { };
          default = pkgs.callPackage ./package.nix { v110 = false; };
        });

      # Overlay so users can add these packages to their system/user profile
      # via `packages = with pkgs; [ crossref-cli nu_plugin_crossref ]`.
      overlays = {
        default = final: prev: {
          crossref-cli = final.callPackage ./package-cli.nix { };
          nu_plugin_crossref = final.callPackage ./package.nix { v110 = false; };
          nu_plugin_crossref_v110 = final.callPackage ./package.nix { v110 = true; };
        };
      };

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
