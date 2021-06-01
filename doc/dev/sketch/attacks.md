# Attacks

TODO This should be expanded to a proper [attack tree](https://en.wikipedia.org/wiki/Attack_tree).

To obtain a root shell, an attacker would have to achieve one of these goals (list is non-exhaustive):

- (somehow) steal TLS secrets and impersonate `tere-server`
- (somehow) gain a false TLS certificate for the domain name
- make `tere-server` run arbitrary code, and wait for the next admin connection
- make `tere-server` run arbitrary code, and attach/hijack an existing admin connection (in-process TLS or HTTP/2 only, otherwise the connection is no longer visible to `tere-server`)
- convince `tere-policy@` of incorrect authentication (as spoken to through `tere-user@`'s limited protocol)
- convince `tere-policy@` of incorrect authorization (as spoken to through `tere-user@` and `tere-policy@`, bound to a successfully authenticated username)

Our insistence on WebAuthn should mean phishing is not a viable attack.

And so on.
