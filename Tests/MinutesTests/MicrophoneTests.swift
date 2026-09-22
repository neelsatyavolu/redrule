import Testing
@testable import Minutes

struct MicrophoneTests {
    let devices = [MicrophoneDevice(id: "built-in", name: "Built-in", deviceID: 10),
                   MicrophoneDevice(id: "usb", name: "USB mic", deviceID: 20)]

    @Test func systemDefaultLeavesDeviceChoiceToTheEngine() throws {
        #expect(try MicrophoneDevice.resolve("", in: devices) == nil)
    }

    @Test func savedUIDResolvesAfterDeviceIDsChange() throws {
        let reconnected = [MicrophoneDevice(id: "usb", name: "USB mic", deviceID: 42)]
        #expect(try MicrophoneDevice.resolve("usb", in: devices) == 20)
        #expect(try MicrophoneDevice.resolve("usb", in: reconnected) == 42)
    }

    @Test func disconnectedSelectionDoesNotFallBackToAnotherMic() {
        #expect(throws: (any Error).self) {
            try MicrophoneDevice.resolve("disconnected", in: devices)
        }
    }
}
