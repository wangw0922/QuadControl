# Android shell agent boundary

The only M0 executable is `adb-probe`, a passive diagnostic. It never invokes `pair`, `connect`, `tcpip`, `shell`, `kill-server`, or any device-changing command.
