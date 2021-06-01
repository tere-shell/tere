{ pkgs ? import <nixpkgs> { } }:

with pkgs;

rustPlatform.buildRustPackage rec {
  pname = "mdbook-mermaid";
  version = "0.8.1";

  src = fetchFromGitHub {
    owner = "badboy";
    repo = pname;
    rev = "v${version}";
    sha256 = "1263rik2ljrwc3ns1ik7anwcqwq1wl9zkgsn4sahahwj0x2idijh";
  };

  cargoSha256 = "0b0hqkz6h2k03yzm5acgmlf07ws0zvihzgf48j6ngj2l0vcm44ly";

  meta = with lib; {
    description = "mdbook preprocessor for mermaid diagrams";
    homepage = "https://github.com/badboy/mdbook-mermaid";
    license = with licenses; [ mpl20 ];
  };
}
