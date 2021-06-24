start_all()
server.wait_for_unit("default.target")

with subtest("pty"):
  server.require_unit_state("tere-pty.socket")
