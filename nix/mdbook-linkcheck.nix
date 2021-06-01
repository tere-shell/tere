{ pkgs ? import <nixpkgs> { } }:

with pkgs;

rustPlatform.buildRustPackage rec {
  pname = "mdbook-linkcheck";
  version = "0.7.4";

  src = fetchFromGitHub {
    owner = "Michael-F-Bryan";
    repo = pname;
    rev = "v${version}";
    sha256 = "1as5aa39jhixz0aj1a2zm4vsxa27rzj77knwfma5hr1qh33svj30";
  };

  cargoSha256 = "0gjj1fysl91z29n3f6pag69y63bdgz2x5z0zdw48xxsdplii4jr0";

  nativeBuildInputs = [
    # needed to build openssl, deep in the requirements
    perl
  ];

  # tests for the binary assume internet connectivity, and filter out all external links in hermetic build environments.
  # https://github.com/Michael-F-Bryan/mdbook-linkcheck/issues/58
  cargoTestFlags = [ "--lib" ];

  meta = with lib; {
    description = "mdbook output to check links";
    homepage = "https://github.com/Michael-F-Bryan/mdbook-linkcheck";
    license = with licenses; [ mit ];
  };
}
