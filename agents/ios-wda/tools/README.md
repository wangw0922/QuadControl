# Measurement probes

`avprobe.swift` is the CoreMediaIO/AVFoundation probe behind the "Third measurement
pass" numbers in the parent README. It opens the iPhone as an iOS screen-capture
device, stamps every frame with the host wall clock and a luma-difference score
against the previous frame, and writes a TSV.

Build and run from Terminal.app (camera permission is granted per launching app; a
probe started from an agent shell is denied without a prompt):

```bash
xcrun swiftc -O avprobe.swift -o avprobe -framework AVFoundation -framework CoreMediaIO
./avprobe 60 frames.tsv
```

Start the capture before `ios tunnel start`: starting or stopping the capture
re-enumerates USB and kills a live userspace tunnel and WDA runner. Delete the
TSV after analysis if it was captured with identifiable screen content; the file
itself holds only timestamps and pixel-difference scores.
