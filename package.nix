{ lib
, rustPlatform
, fetchFromGitHub
, v110
}:

rustPlatform.buildRustPackage rec {
  pname = "nu_plugin_crossref";
  version = "0.3.1";

  src = fetchFromGitHub {
    owner = "TonyWu20";
    repo = "crossref-rs";
    tag = "v${version}";
    hash = "sha256-9tv+8FyOxjX7ckOkORtm5pmf2+Fwmvafxoa1gtdF0Vo=";
  };

  # Build only the Nushell plugin binary.  The default feature set already
  # selects `nu-v111` (nushell 0.111); no extra flags are needed.
  cargoBuildFlags = [
    "--bin"
    "nu_plugin_crossref"
  ] ++ lib.optionals v110 [
    "--features"
    "nu-v110"
  ];

  cargoHash = "sha256-VVMCc/nPflwtGC6E58NHkjBAPmN/IF1gYUNUFCT8ew8=";

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
    license = lib.licenses.mit;
    maintainers = with lib.maintainers; [ ];
    mainProgram = "nu_plugin_crossref";
    platforms = lib.platforms.unix;
  };
}
