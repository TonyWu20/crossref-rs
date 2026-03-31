{ lib
, rustPlatform
, fetchFromGitHub
, v110
}:

rustPlatform.buildRustPackage rec {
  pname = "nu_plugin_crossref";
  version = "0.2.0";

  src = fetchFromGitHub {
    owner = "TonyWu20";
    repo = "crossref-rs";
    tag = "v${version}";
    # Run `nix-prefetch-github TonyWu20 crossref-rs --rev v0.2.0` to obtain.
    hash = "sha256-DJRaQ+NAK6pseLxglQJ7GRAMs2rZYSklZxjjXC3zpAI=";
  };

  # The Cargo.lock contains one non-registry path dependency: the local patch
  # for nu-plugin-core 0.110.0 (patches/nu-plugin-core-v110/).  This patch is
  # only compiled when the `nu-v110` feature is active (not the default), but
  # importCargoLock still requires its hash because the entry exists in the
  # lock file.
  #
  # Compute with:
  #   nix hash path --base32 patches/nu-plugin-core-v110/
  #   cargoLock = {
  #     lockFile = ./Cargo.lock;
  #     outputHashes = {
  #       "nu-plugin-core-0.110.0" = "sha256-01a4wvc4vcwgc4zn4pdclzb65p2cny9nhnqq9rsfvhpwmdg5b99d
  # ";
  #     };
  #   };

  # Build only the Nushell plugin binary.  The default feature set already
  # selects `nu-v111` (nushell 0.111); no extra flags are needed.
  cargoBuildFlags = [
    "--bin"
    "nu_plugin_crossref"
  ] ++ lib.optionals v110 [
    "--features"
    "nu-v110"
  ];

  cargoHash = "sha256-NEahxhX9ZsypTXOqzGtqcQxxE7CoWTEU8xkM/Ub4cfs=";

  # reqwest 0.13 defaults to rustls (pure-Rust TLS).  On macOS,
  # rustls-platform-verifier reads the system trust store via the Security
  # and CoreFoundation frameworks; no OpenSSL is required on any platform.

  # Integration tests hit live network endpoints (Crossref, Unpaywall) and
  # cannot run inside the Nix sandbox.
  doCheck = false;

  meta = {
    description = "Nushell plugin for querying Crossref literature metadata and managing BibTeX";
    longDescription = ''
      nu_plugin_crossref exposes Crossref DOI lookup, full-text search (with
      date-range, type, and open-access filters), BibTeX generation, and
      Unpaywall-powered PDF download as native Nushell commands.  Results can
      be streamed through fzf/skim for interactive selection.
    '';
    homepage = "https://github.com/TonyWu20/crossref-rs";
    license = with lib.licenses; [
      mit
      asl20
    ];
    maintainers = with lib.maintainers; [ ];
    mainProgram = "nu_plugin_crossref";
    platforms = lib.platforms.unix;
  };
}
