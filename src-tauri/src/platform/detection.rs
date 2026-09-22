//! Reads the system for signs of a call and feeds `DetectorLogic`.
use std::collections::HashSet;
use std::ffi::c_void;
use std::ptr::NonNull;

use objc2_core_audio::{
    AudioObjectGetPropertyData, AudioObjectGetPropertyDataSize, AudioObjectID, AudioObjectPropertyAddress,
    AudioObjectPropertySelector, kAudioHardwarePropertyProcessObjectList, kAudioObjectPropertyElementMain,
    kAudioObjectPropertyScopeGlobal, kAudioObjectSystemObject, kAudioProcessPropertyBundleID,
    kAudioProcessPropertyIsRunningInput, kAudioProcessPropertyPID,
};
use objc2_core_foundation::{CFArray, CFDictionary, CFRetained, CFString, CFType};
use objc2_core_graphics::{CGWindowListCopyWindowInfo, CGWindowListOption, kCGNullWindowID, kCGWindowName, kCGWindowOwnerName};

use crate::core::detector_logic::DetectionSnapshot;

const BROWSER_NAMES: &[&str] =
    &["Google Chrome", "Safari", "Arc", "Dia", "Microsoft Edge", "Brave Browser", "Firefox", "Vivaldi"];

pub fn snapshot() -> DetectionSnapshot {
    DetectionSnapshot {
        process_names: process_names(),
        mic_user_bundle_ids: mic_user_bundle_ids(),
        browser_window_titles: browser_window_titles(),
    }
}

fn process_names() -> HashSet<String> {
    let Ok(pids) = libproc::processes::pids_by_type(libproc::processes::ProcFilter::All) else {
        return HashSet::new();
    };
    pids.into_iter().filter_map(|pid| libproc::proc_pid::name(pid as i32).ok()).collect()
}

// MARK: Microphone users

/// Bundle ids of other processes with a running audio input, from CoreAudio's process objects.
fn mic_user_bundle_ids() -> HashSet<String> {
    let own_pid = std::process::id() as i32;
    process_objects()
        .into_iter()
        .filter(|&process| scalar::<u32>(process, kAudioProcessPropertyIsRunningInput).is_some_and(|running| running != 0))
        .filter(|&process| scalar::<i32>(process, kAudioProcessPropertyPID).is_some_and(|pid| pid != own_pid))
        .filter_map(bundle_id)
        .collect()
}

fn address(selector: AudioObjectPropertySelector) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress {
        mSelector: selector,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain,
    }
}

fn process_objects() -> Vec<AudioObjectID> {
    let system = kAudioObjectSystemObject as AudioObjectID;
    let mut address = address(kAudioHardwarePropertyProcessObjectList);
    let mut size = 0u32;
    // SAFETY: every pointer refers to a live local of the size CoreAudio is told.
    let status = unsafe {
        AudioObjectGetPropertyDataSize(system, NonNull::from(&mut address), 0, std::ptr::null(), NonNull::from(&mut size))
    };
    if status != 0 || size == 0 {
        return Vec::new();
    }
    let mut processes = vec![0 as AudioObjectID; size as usize / size_of::<AudioObjectID>()];
    let status = unsafe {
        AudioObjectGetPropertyData(
            system,
            NonNull::from(&mut address),
            0,
            std::ptr::null(),
            NonNull::from(&mut size),
            NonNull::new_unchecked(processes.as_mut_ptr().cast::<c_void>()),
        )
    };
    if status != 0 {
        return Vec::new();
    }
    processes.truncate(size as usize / size_of::<AudioObjectID>());
    processes
}

/// Reads a plain C scalar property (UInt32, pid_t).
fn scalar<T: Copy + Default>(object: AudioObjectID, selector: AudioObjectPropertySelector) -> Option<T> {
    let mut address = address(selector);
    let mut value = T::default();
    let mut size = size_of::<T>() as u32;
    // SAFETY: `value` is a live local of exactly `size` bytes.
    let status = unsafe {
        AudioObjectGetPropertyData(
            object,
            NonNull::from(&mut address),
            0,
            std::ptr::null(),
            NonNull::from(&mut size),
            NonNull::from(&mut value).cast(),
        )
    };
    (status == 0 && size as usize == size_of::<T>()).then_some(value)
}

fn bundle_id(process: AudioObjectID) -> Option<String> {
    let mut address = address(kAudioProcessPropertyBundleID);
    let mut value: *const CFString = std::ptr::null();
    let mut size = size_of::<*const CFString>() as u32;
    // SAFETY: CoreAudio writes a +1 retained CFStringRef into `value`.
    let status = unsafe {
        AudioObjectGetPropertyData(
            process,
            NonNull::from(&mut address),
            0,
            std::ptr::null(),
            NonNull::from(&mut size),
            NonNull::from(&mut value).cast(),
        )
    };
    let string = unsafe { CFRetained::from_raw(NonNull::new(value.cast_mut())?) };
    let id = string.to_string();
    (status == 0 && !id.is_empty()).then_some(id)
}

// MARK: Windows

/// Window titles are only readable once Screen Recording permission has been granted.
fn browser_window_titles() -> Vec<String> {
    let options = CGWindowListOption::OptionAll | CGWindowListOption::ExcludeDesktopElements;
    let Some(windows) = CGWindowListCopyWindowInfo(options, kCGNullWindowID) else { return Vec::new() };
    // SAFETY: CGWindowListCopyWindowInfo returns an array of CFDictionary<CFString, CFType>.
    let windows: CFRetained<CFArray<CFDictionary<CFString, CFType>>> = unsafe { CFRetained::cast_unchecked(windows) };
    let text = |window: &CFDictionary<CFString, CFType>, key: &CFString| {
        window.get(key).and_then(|value| value.downcast::<CFString>().ok()).map(|value| value.to_string())
    };
    // SAFETY: the window-info keys are immutable constants exported by CoreGraphics.
    let (owner_key, name_key) = unsafe { (kCGWindowOwnerName, kCGWindowName) };
    windows
        .iter()
        .filter(|window| text(window, owner_key).is_some_and(|owner| BROWSER_NAMES.contains(&owner.as_str())))
        .filter_map(|window| text(&window, name_key))
        .filter(|title| !title.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    /// Reads the live system; run with `cargo test -p minutes snapshot -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn prints_a_live_snapshot() {
        let snapshot = super::snapshot();
        println!("processes: {}", snapshot.process_names.len());
        println!("mic users: {:?}", snapshot.mic_user_bundle_ids);
        println!("browser titles: {:?}", snapshot.browser_window_titles.iter().take(5).collect::<Vec<_>>());
        println!("permissions: {:?}", crate::platform::permissions::Permissions::current());
        assert!(snapshot.process_names.len() > 10);
    }
}
