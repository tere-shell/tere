# Run tests with a fresh VM.
#
# Used via `tests/vm.rs`, or directly as
#
# ```
# cargo build -p tere-server --features internal-dangerous-tests --tests
# nix-build nix/vm-test.nix --argstr vm-test-executable ./target/debug/deps/vm_NAME-HASH
# ```
#
# https://nixos.org/manual/nixos/stable/index.html#sec-nixos-tests

# Note: Since the test is run as part of a Nix build, it does content-based caching.

{
  # Path to the executable to run inside the virtual machine.
  # Pass in via `--argstr vm-test-executable ./relative/path`.
  vm-test-executable
}:

let
  nixpkgsSource = import ./nixpkgs-nixos-21.05.nix { };
  # Convert the input string to a Nix Path.
  # `toPath` makes sure it's an absolute path.
  # Nix makes it way too hard to handle both absolute and relative paths as input.
  # https://nixos.wiki/wiki/Nix_Expression_Language#Convert_a_string_to_an_.28import-able.29_path
  executable = /. + builtins.toPath vm-test-executable;

  test = import "${nixpkgsSource}/nixos/tests/make-test-python.nix" ({ pkgs, ... }:
    {
      name = "tere-vm-test";
      machine = { pkgs, ... }: {
        imports = [ ./tere-nixos-module.nix ];
        config = {
          virtualisation.graphics = false;
          services.tere.enable = true;
          users = {
            users."testuser" = {
              isNormalUser = true;
              password = "testpassword";
            };
          };
        };
      };
      testScript = ''
        vm_test_executable = "${executable}"
        # Postpone raising an exception so we can copy back the logs.
        success = True
        machine.start()
        machine.wait_for_unit("default.target")
        # TODO Use the xchg or shared directory directly, the extra steps hidden inside the copy are just silly.
        # (The nix python test driver has a directory virtfs-mounted in all VMs.)
        machine.copy_from_host(vm_test_executable, "/vm-test")
        # TODO This buffers test output, which is not great for slower tests.
        # The implementation in <nixos/lib/testing-python.nix> is a tower of nasty kludges.
        # Best way to resolve this is probably to take full control of the VM running process (and use the NixOS stuff only to create an image; the /nix/store sharing optimization is very useful).
        # That'll probably happen at a time if and when we expand these kinds of tests to cover mainstream distributions.
        (status, output) = machine.execute(make_command(["/vm-test", "--nocapture"]))
        print(output)
        if status != 0:
            print(f"tests failed: exit code {status}")
            success = False
        machine.succeed("journalctl --boot >/tmp/journal.log")
        machine.copy_from_vm("/tmp/journal.log", ".")
        if not success:
            raise Exception("FAIL")
      '';
    });
in
# NIX-WART: It seems nix-build will automatically evaluate only a single layer of functions.
  # In most uses of `make-test-python.nix`, that's apparently a function returned from the importing that  library.
  # However, if we use top-level arguments to pass in the executable path, now we already have a  function, and we need to explicitly call the function returned from the import.
  # That means our code below looks slightly different from what's found in the NixOS tests.
test { }
