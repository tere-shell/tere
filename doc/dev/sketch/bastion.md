# Bastion hosts considered harmful

The old school way of "punching through the firewall" from the outside was to set up a "bastion host" in the DMZ, and allow internet->DMZ and DMZ->internal connections.

In such a system, either the bastion host contained SSH key files, or everyone used SSH agent forwarding.
Both of those are security disasters, including confused deputies, attackers stealing keyfiles, and SSH agent either a) not prompting at all or b) prompting with no detail whatsoever, letting attackers race a connection to a wholly different server.

Either just expose your systems, set up a Wireguard VPN, or proxy traffic.

If there are enterprise uses for a bastion host (likely: better auditing), let's handle those use cases on their own.
