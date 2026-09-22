import CoreAudio
import Foundation

struct MicrophoneDevice: Identifiable, Equatable {
    /// Device UIDs survive reconnects; numeric Core Audio IDs do not.
    let id: String
    let name: String
    let deviceID: AudioDeviceID

    static func available() -> [Self] {
        var address = AudioObjectPropertyAddress(mSelector: kAudioHardwarePropertyDevices,
                                                mScope: kAudioObjectPropertyScopeGlobal, mElement: 0)
        var size: UInt32 = 0
        let system = AudioObjectID(kAudioObjectSystemObject)
        guard AudioObjectGetPropertyDataSize(system, &address, 0, nil, &size) == noErr else { return [] }
        var ids = [AudioDeviceID](repeating: 0, count: Int(size) / MemoryLayout<AudioDeviceID>.size)
        guard !ids.isEmpty,
              AudioObjectGetPropertyData(system, &address, 0, nil, &size, &ids) == noErr else { return [] }
        return ids.compactMap { device in
            var input = AudioObjectPropertyAddress(mSelector: kAudioDevicePropertyStreamConfiguration,
                                                  mScope: kAudioDevicePropertyScopeInput, mElement: 0)
            var bytes: UInt32 = 0
            guard AudioObjectGetPropertyDataSize(device, &input, 0, nil, &bytes) == noErr,
                  bytes >= MemoryLayout<AudioBufferList>.size else { return nil }
            let storage = UnsafeMutableRawPointer.allocate(byteCount: Int(bytes), alignment: MemoryLayout<AudioBufferList>.alignment)
            defer { storage.deallocate() }
            guard AudioObjectGetPropertyData(device, &input, 0, nil, &bytes, storage) == noErr else { return nil }
            let buffers = UnsafeMutableAudioBufferListPointer(storage.assumingMemoryBound(to: AudioBufferList.self))
            guard buffers.contains(where: { $0.mNumberChannels > 0 }),
                  let uid = string(kAudioDevicePropertyDeviceUID, from: device),
                  let name = string(kAudioObjectPropertyName, from: device) else { return nil }
            return Self(id: uid, name: name, deviceID: device)
        }.sorted { $0.name.localizedStandardCompare($1.name) == .orderedAscending }
    }

    static func resolve(_ uid: String, in devices: [Self]) throws -> AudioDeviceID? {
        guard !uid.isEmpty else { return nil }
        guard let device = devices.first(where: { $0.id == uid }) else {
            throw CaptureError.microphoneUnavailable
        }
        return device.deviceID
    }

    private static func string(_ selector: AudioObjectPropertySelector, from device: AudioDeviceID) -> String? {
        var address = AudioObjectPropertyAddress(mSelector: selector, mScope: kAudioObjectPropertyScopeGlobal, mElement: 0)
        var value: Unmanaged<CFString>?
        var size = UInt32(MemoryLayout.size(ofValue: value))
        guard AudioObjectGetPropertyData(device, &address, 0, nil, &size, &value) == noErr else { return nil }
        return value?.takeRetainedValue() as String?
    }
}
