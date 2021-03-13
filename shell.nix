with import <nixpkgs> {};

stdenv.mkDerivation {
  name = "rust-env";
  nativeBuildInputs = [
    rustc cargo rustfmt rls
    pkgconfig
  ];
  buildInputs = [
    gpgme
    openssl
  ];

  # Set Environment Variables
  RUST_BACKTRACE = 1;
}
