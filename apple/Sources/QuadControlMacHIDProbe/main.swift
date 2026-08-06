import Foundation
import QuadControlHIDProbe

let arguments = Array(CommandLine.arguments.dropFirst())
guard HIDProbe.argumentsAreValid(arguments) else {
    FileHandle.standardError.write(Data("usage: QuadControlMacHIDProbe [--format json]\n".utf8))
    exit(64)
}

let result = HIDProbe.run()
switch result {
case let .success(report):
    if let json = try? report.json() {
        print(json)
    } else {
        FileHandle.standardError.write(Data("hid probe failed: report could not be encoded\n".utf8))
        exit(1)
    }
case let .failure(error):
    FileHandle.standardError.write(Data("hid probe failed: \(error.rawValue)\n".utf8))
}
exit(HIDProbe.exitCode(for: result))
