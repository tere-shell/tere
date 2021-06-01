{ pkgs ? import <nixpkgs> { } }:

with pkgs;

rustPlatform.buildRustPackage rec {
  pname = "mdbook-graphviz";
  name = pname; # untagged version

  src = fetchFromGitHub {
    owner = "dylanowen";
    repo = pname;
    rev = "160aced97cd4ef57d0dfef6880fc33056bf418e7";
    sha256 = "0rig03ikxnvydagwx85h4is09igrpw6c0n368fv7w315axn6w9fa";
  };

  cargoPatches = [
    # missing Cargo.lock
    #
    # https://github.com/dylanowen/mdbook-graphviz/issues/7
    #
    # this is brittle because the version chosen here should match mdbook version, you get
    #
    # Warning: The graphviz plugin was built against version 0.4.8 of mdbook, but we're being called from version 0.4.5
    ./mdbook-graphviz-cargo-lock.diff

    # crashes on draft chapters
    #
    # thread 'main' panicked at 'called `Option::unwrap()` on a `None` value', src/preprocessor.rs:43:32
    ./mdbook-graphviz-draft-chapters.diff
  ];
  cargoSha256 = "1lfvh1dkjfay2ki1wckk4kjl5yc7nbn1w1w7ryc6dx99y15gmyd5";

  nativeBuildInputs = [
    # needed for postInstall wrapProgram
    makeWrapper
    # needed for tests
    graphviz
  ];
  postInstall = ''
    # ensure mdbook-graphviz can run graphviz commands
    wrapProgram $out/bin/mdbook-graphviz --suffix PATH : '${lib.makeBinPath [ graphviz ]}'
  '';

  # tests fail
  #
  # ---- renderer::test::inline_events stdout ----
  # thread 'renderer::test::inline_events' panicked at 'assertion failed: `(left == right)`
  #   left: `Some(Text(Borrowed("\n\n")))`,
  #  right: `None`', src/renderer.rs:132:9
  doCheck = false;

  meta = with lib; {
    description = "mdbook preprocessor to add graphviz support";
    homepage = "https://github.com/dylanowen/mdbook-graphviz";
    license = with licenses; [ mpl20 ];
  };
}
