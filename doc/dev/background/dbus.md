# D-Bus

We use dbus via the excellent [zbus crate](https://crates.io/crates/zbus).

## Troubleshooting

<https://dbus.freedesktop.org/doc/dbus-monitor.1.html>

```
dbus-monitor --system "interface='org.freedesktop.machine1.Manager'"
```

```
dbus-send --system --dest=org.freedesktop.machine1 --type=method_call --print-reply /org/freedesktop/machine1 org.freedesktop.machine1.Manager.OpenMachineShell string:.host string:root string:/bin/sh array:string:/bin/sh array:string:'TERM=alacritty'
```

<https://dbus.freedesktop.org/doc/dbus-send.1.html>

## Resources

<https://dbus.freedesktop.org/doc/dbus-api-design.html> (not very relevant, we're not making new D-Bus APIs)

<https://www.freedesktop.org/software/polkit/docs/latest/polkit.8.html>

polkit method called by `systemd-machined`: <https://www.freedesktop.org/software/polkit/docs/latest/eggdbus-interface-org.freedesktop.PolicyKit1.Authority.html#eggdbus-method-org.freedesktop.PolicyKit1.Authority.CheckAuthorization>

