{ lib
, rustPlatform
}:

rustPlatform.buildRustPackage rec {
  pname = "crossref-cli";
  version = "0.3.1";

  src = ./.;

  cargoHash = "sha256-VVMCc/nPflwtGC6E58NHkjBAPmN/IF1gYUNUFCT8ew8=";

  # Build only the CLI binary.  The default feature set (nu-v111) enables the
  # nu-plugin dependencies which are not needed here, so we explicitly request
  # no default features.  The CLI binary has no nu-plugin required-features and
  # will always be built.
  cargoBuildFlags = [
    "--bin"
    "crossref-cli"
    "--no-default-features"
  ];

  # reqwest 0.13 defaults to rustls (pure-Rust TLS).  The CLI does not pull in
  # nu-plugin or rustls-platform-verifier, so no macOS Security/CoreFoundation
  # frameworks are required.
  doCheck = false;

  meta = {
    description = "Crossref literature metadata & BibTeX CLI (universal shell)";
    longDescription = ''
      crossref-cli provides DOI lookup, full-text search (with date-range,
      type, and open-access filters), BibTeX generation, and
      Unpaywall-powered PDF download from the command line.
    '';
    homepage = "https://github.com/TonyWu20/crossref-rs";
    license = lib.licenses.mit;
    maintainers = with lib.maintainers; [ ];
    mainProgram = "crossref-cli";
    platforms = lib.platforms.unix;
  };
}
