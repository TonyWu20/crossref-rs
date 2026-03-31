{ lib
, rustPlatform
, fetchFromGitHub
}:

rustPlatform.buildRustPackage rec {
  pname = "crossref-cli";
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
  cargoLock = {
    lockFile = ./Cargo.lock;
    outputHashes = {
      "nu-plugin-core-0.110.0" = "sha256-01a4wvc4vcwgc4zn4pdclzb65p2cny9nhnqq9rsfvhpwmdg5b99d";
    };
  };

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
    license = with lib.licenses; [
      mit
      asl20
    ];
    maintainers = with lib.maintainers; [ ];
    mainProgram = "crossref-cli";
    platforms = lib.platforms.unix;
  };
}
