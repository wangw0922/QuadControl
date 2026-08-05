# macOS diagnostic listener

Run `swift run QuadControlMacListener`. It is a SwiftPM executable, not an app
bundle. It is an interactive, visible connection diagnostic only; Ctrl-C
disconnects. Default binding is loopback. For an iPhone on the same trusted
network, run `swift run QuadControlMacListener --lan --interface wifi --port
47100`; wired Ethernet uses `wired`. LAN is limited to the selected interface
and private/link-local peers. The 43-character token appears once on `/dev/tty`,
expires after 180 seconds, and is consumed after one successful session.
