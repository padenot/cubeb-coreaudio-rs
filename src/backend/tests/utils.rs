use super::*;

// Common Utils
// ------------------------------------------------------------------------------------------------
#[derive(Clone, Debug, PartialEq)]
pub enum Scope {
    Input,
    Output,
}

impl From<Scope> for io_side {
    fn from(scope: Scope) -> Self {
        match scope {
            Scope::Input => io_side::INPUT,
            Scope::Output => io_side::OUTPUT,
        }
    }
}

impl From<Scope> for DeviceType {
    fn from(scope: Scope) -> Self {
        match scope {
            Scope::Input => DeviceType::INPUT,
            Scope::Output => DeviceType::OUTPUT,
        }
    }
}

#[derive(Clone)]
pub enum PropertyScope {
    Input,
    Output,
}

pub fn test_get_default_device(scope: Scope) -> Option<AudioObjectID> {
    let address = AudioObjectPropertyAddress {
        mSelector: match scope {
            Scope::Input => kAudioHardwarePropertyDefaultInputDevice,
            Scope::Output => kAudioHardwarePropertyDefaultOutputDevice,
        },
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMaster,
    };

    let mut devid: AudioObjectID = kAudioObjectUnknown;
    let mut size = mem::size_of::<AudioObjectID>();
    let status = unsafe {
        AudioObjectGetPropertyData(
            kAudioObjectSystemObject,
            &address,
            0,
            ptr::null(),
            &mut size as *mut usize as *mut UInt32,
            &mut devid as *mut AudioObjectID as *mut c_void,
        )
    };
    if status != NO_ERR || devid == kAudioObjectUnknown {
        return None;
    }
    Some(devid)
}

// TODO: Create a GetProperty trait and add a default implementation for it, then implement it
//       for TestAudioUnit so the member method like `get_buffer_frame_size` can reuse the trait
//       method get_property_data.
#[derive(Debug)]
pub struct TestAudioUnit(AudioUnit);

impl TestAudioUnit {
    fn new(unit: AudioUnit) -> Self {
        assert!(!unit.is_null());
        Self(unit)
    }
    pub fn get_inner(&self) -> AudioUnit {
        self.0
    }
    pub fn get_buffer_frame_size(
        &self,
        scope: Scope,
        prop_scope: PropertyScope,
    ) -> std::result::Result<u32, OSStatus> {
        test_audiounit_get_buffer_frame_size(self.0, scope, prop_scope)
    }
}

impl Drop for TestAudioUnit {
    fn drop(&mut self) {
        unsafe {
            AudioUnitUninitialize(self.0);
            AudioComponentInstanceDispose(self.0);
        }
    }
}

// TODO: 1. Return Result with custom errors.
//       2. Allow to create a in-out unit.
pub fn test_get_default_audiounit(scope: Scope) -> Option<TestAudioUnit> {
    let device = test_get_default_device(scope.clone());
    let unit = test_create_audiounit(ComponentSubType::HALOutput);
    if device.is_none() || unit.is_none() {
        return None;
    }
    let unit = unit.unwrap();
    let device = device.unwrap();
    match scope {
        Scope::Input => {
            if test_enable_audiounit_in_scope(unit.get_inner(), Scope::Input, true).is_err()
                || test_enable_audiounit_in_scope(unit.get_inner(), Scope::Output, false).is_err()
            {
                return None;
            }
        }
        Scope::Output => {
            if test_enable_audiounit_in_scope(unit.get_inner(), Scope::Input, false).is_err()
                || test_enable_audiounit_in_scope(unit.get_inner(), Scope::Output, true).is_err()
            {
                return None;
            }
        }
    }

    let status = unsafe {
        AudioUnitSetProperty(
            unit.get_inner(),
            kAudioOutputUnitProperty_CurrentDevice,
            kAudioUnitScope_Global,
            0, // Global bus
            &device as *const AudioObjectID as *const c_void,
            mem::size_of::<AudioObjectID>() as u32,
        )
    };
    if status == NO_ERR {
        Some(unit)
    } else {
        None
    }
}

pub enum ComponentSubType {
    HALOutput,
    DefaultOutput,
}

// TODO: Return Result with custom errors.
// Surprisingly the AudioUnit can be created even when there is no any device on the platform,
// no matter its subtype is HALOutput or DefaultOutput.
pub fn test_create_audiounit(unit_type: ComponentSubType) -> Option<TestAudioUnit> {
    let desc = AudioComponentDescription {
        componentType: kAudioUnitType_Output,
        componentSubType: match unit_type {
            ComponentSubType::HALOutput => kAudioUnitSubType_HALOutput,
            ComponentSubType::DefaultOutput => kAudioUnitSubType_DefaultOutput,
        },
        componentManufacturer: kAudioUnitManufacturer_Apple,
        componentFlags: 0,
        componentFlagsMask: 0,
    };
    let comp = unsafe { AudioComponentFindNext(ptr::null_mut(), &desc) };
    if comp.is_null() {
        return None;
    }
    let mut unit: AudioUnit = ptr::null_mut();
    let status = unsafe { AudioComponentInstanceNew(comp, &mut unit) };
    // TODO: Is unit possible to be null when no error returns ?
    if status != NO_ERR || unit.is_null() {
        None
    } else {
        Some(TestAudioUnit::new(unit))
    }
}

fn test_enable_audiounit_in_scope(
    unit: AudioUnit,
    scope: Scope,
    enable: bool,
) -> std::result::Result<(), OSStatus> {
    assert!(!unit.is_null());
    let (scope, element) = match scope {
        Scope::Input => (kAudioUnitScope_Input, AU_IN_BUS),
        Scope::Output => (kAudioUnitScope_Output, AU_OUT_BUS),
    };
    let on_off: u32 = if enable { 1 } else { 0 };
    let status = unsafe {
        AudioUnitSetProperty(
            unit,
            kAudioOutputUnitProperty_EnableIO,
            scope,
            element,
            &on_off as *const u32 as *const c_void,
            mem::size_of::<u32>() as u32,
        )
    };
    if status == NO_ERR {
        Ok(())
    } else {
        Err(status)
    }
}

pub fn test_get_default_source_name(scope: Scope) -> Option<String> {
    if let Some(source) = test_get_default_source_data(scope) {
        Some(u32_to_string(source))
    } else {
        None
    }
}

pub fn test_get_default_source_data(scope: Scope) -> Option<u32> {
    let device = test_get_default_device(scope.clone());
    if device.is_none() {
        return None;
    }

    let device = device.unwrap();
    let address = AudioObjectPropertyAddress {
        mSelector: kAudioDevicePropertyDataSource,
        mScope: match scope {
            Scope::Input => kAudioDevicePropertyScopeInput,
            Scope::Output => kAudioDevicePropertyScopeOutput,
        },
        mElement: kAudioObjectPropertyElementMaster,
    };
    let mut size = mem::size_of::<u32>();
    let mut data: u32 = 0;

    let status = unsafe {
        AudioObjectGetPropertyData(
            device,
            &address,
            0,
            ptr::null(),
            &mut size as *mut usize as *mut u32,
            &mut data as *mut u32 as *mut c_void,
        )
    };

    // TODO: Can data be 0 when no error returns ?
    if status == NO_ERR && data > 0 {
        Some(data)
    } else {
        None
    }
}

fn u32_to_string(data: u32) -> String {
    // Reverse 0xWXYZ into 0xZYXW.
    let mut buffer = [b'\x00'; 4]; // 4 bytes for u32.
    buffer[0] = (data >> 24) as u8;
    buffer[1] = (data >> 16) as u8;
    buffer[2] = (data >> 8) as u8;
    buffer[3] = (data) as u8;
    String::from_utf8_lossy(&buffer).to_string()
}

pub fn test_get_all_devices() -> Vec<AudioObjectID> {
    let mut devices = Vec::new();
    let address = AudioObjectPropertyAddress {
        mSelector: kAudioHardwarePropertyDevices,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMaster,
    };
    let mut size: usize = 0;
    let status = unsafe {
        AudioObjectGetPropertyDataSize(
            kAudioObjectSystemObject,
            &address,
            0,
            ptr::null(),
            &mut size as *mut usize as *mut u32,
        )
    };
    // size will be 0 if there is no device at all.
    if status != NO_ERR || size == 0 {
        return devices;
    }
    assert_eq!(size % mem::size_of::<AudioObjectID>(), 0);
    let elements = size / mem::size_of::<AudioObjectID>();
    devices.resize(elements, kAudioObjectUnknown);
    let status = unsafe {
        AudioObjectGetPropertyData(
            kAudioObjectSystemObject,
            &address,
            0,
            ptr::null(),
            &mut size as *mut usize as *mut u32,
            devices.as_mut_ptr() as *mut c_void,
        )
    };
    if status != NO_ERR {
        devices.clear();
        return devices;
    }
    for device in devices.iter() {
        assert_ne!(*device, kAudioObjectUnknown);
    }
    devices
}

pub fn test_get_devices_in_scope(scope: Scope) -> Vec<AudioObjectID> {
    let mut devices = test_get_all_devices();
    devices.retain(|device| test_device_in_scope(*device, scope.clone()));
    devices
}

pub fn test_device_channels_in_scope(
    id: AudioObjectID,
    scope: Scope,
) -> std::result::Result<u32, OSStatus> {
    let address = AudioObjectPropertyAddress {
        mSelector: kAudioDevicePropertyStreamConfiguration,
        mScope: match scope {
            Scope::Input => kAudioDevicePropertyScopeInput,
            Scope::Output => kAudioDevicePropertyScopeOutput,
        },
        mElement: kAudioObjectPropertyElementMaster,
    };
    let mut size: usize = 0;
    let status = unsafe {
        AudioObjectGetPropertyDataSize(
            id,
            &address,
            0,
            ptr::null(),
            &mut size as *mut usize as *mut u32,
        )
    };
    if status != NO_ERR {
        return Err(status);
    }
    if size == 0 {
        return Ok(0);
    }
    let byte_len = size / mem::size_of::<u8>();
    let mut bytes = vec![0u8; byte_len];
    let status = unsafe {
        AudioObjectGetPropertyData(
            id,
            &address,
            0,
            ptr::null(),
            &mut size as *mut usize as *mut u32,
            bytes.as_mut_ptr() as *mut c_void,
        )
    };
    if status != NO_ERR {
        return Err(status);
    }
    let buf_list = unsafe { &*(bytes.as_mut_ptr() as *mut AudioBufferList) };
    let buf_len = buf_list.mNumberBuffers as usize;
    if buf_len == 0 {
        return Ok(0);
    }
    let buf_ptr = buf_list.mBuffers.as_ptr() as *const AudioBuffer;
    let buffers = unsafe { slice::from_raw_parts(buf_ptr, buf_len) };
    let mut channels: u32 = 0;
    for buffer in buffers {
        channels += buffer.mNumberChannels;
    }
    Ok(channels)
}

pub fn test_device_in_scope(id: AudioObjectID, scope: Scope) -> bool {
    let channels = test_device_channels_in_scope(id, scope);
    channels.is_ok() && channels.unwrap() > 0
}

pub fn test_audiounit_scope_is_enabled(unit: AudioUnit, scope: Scope) -> bool {
    assert!(!unit.is_null());
    let mut has_io: UInt32 = 0;
    let (scope, element) = match scope {
        Scope::Input => (kAudioUnitScope_Input, AU_IN_BUS),
        Scope::Output => (kAudioUnitScope_Output, AU_OUT_BUS),
    };
    assert_eq!(
        audio_unit_get_property(
            unit,
            kAudioOutputUnitProperty_HasIO,
            scope,
            element,
            &mut has_io,
            &mut mem::size_of::<UInt32>()
        ),
        NO_ERR
    );
    has_io != 0
}

pub fn test_audiounit_get_buffer_frame_size(
    unit: AudioUnit,
    scope: Scope,
    prop_scope: PropertyScope,
) -> std::result::Result<u32, OSStatus> {
    let element = match scope {
        Scope::Input => AU_IN_BUS,
        Scope::Output => AU_OUT_BUS,
    };
    let prop_scope = match prop_scope {
        PropertyScope::Input => kAudioUnitScope_Input,
        PropertyScope::Output => kAudioUnitScope_Output,
    };
    let mut buffer_frames: u32 = 0;
    let mut size = mem::size_of::<u32>();
    let status = unsafe {
        AudioUnitGetProperty(
            unit,
            kAudioDevicePropertyBufferFrameSize,
            prop_scope,
            element,
            &mut buffer_frames as *mut u32 as *mut c_void,
            &mut size as *mut usize as *mut u32,
        )
    };
    if status == NO_ERR {
        Ok(buffer_frames)
    } else {
        Err(status)
    }
}

// Surprisingly it's ok to set
//   1. a unknown device
//   2. a non-input/non-output device
//   3. the current default input/output device
// as the new default input/output device by apple's API. We need to check the above things by ourselves.
pub fn test_set_default_device(
    device: AudioObjectID,
    scope: Scope,
) -> std::result::Result<bool, OSStatus> {
    let default = test_get_default_device(scope.clone());
    if default.is_none() {
        return Ok(false);
    }
    let default = default.unwrap();
    if default == device || !test_device_in_scope(device, scope.clone()) {
        return Ok(false);
    }
    let address = AudioObjectPropertyAddress {
        mSelector: match scope {
            Scope::Input => kAudioHardwarePropertyDefaultInputDevice,
            Scope::Output => kAudioHardwarePropertyDefaultOutputDevice,
        },
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMaster,
    };
    let size = mem::size_of::<AudioObjectID>();
    let status = unsafe {
        AudioObjectSetPropertyData(
            kAudioObjectSystemObject,
            &address,
            0,
            ptr::null(),
            size as u32,
            &device as *const AudioObjectID as *const c_void,
        )
    };
    if status == NO_ERR {
        Ok(true)
    } else {
        Err(status)
    }
}

pub struct TestDeviceSwitcher {
    scope: Scope,
    devices: Vec<AudioObjectID>,
}

impl TestDeviceSwitcher {
    pub fn new(scope: Scope) -> Self {
        Self {
            scope: scope.clone(),
            devices: test_get_devices_in_scope(scope),
        }
    }

    pub fn next(&self) -> std::result::Result<bool, OSStatus> {
        let current = test_get_default_device(self.scope.clone()).unwrap();
        let mut index = self
            .devices
            .iter()
            .position(|device| *device == current)
            .unwrap();
        index = (index + 1) % self.devices.len();
        let next = self.devices[index];
        println!(
            "Switch device for {:?}: {} -> {}",
            self.scope, current, next
        );
        test_set_default_device(next, self.scope.clone())
    }
}

pub fn test_create_device_change_listener<F>(scope: Scope, listener: F) -> TestPropertyListener<F>
where
    F: Fn(&[AudioObjectPropertyAddress]) -> OSStatus,
{
    let address = AudioObjectPropertyAddress {
        mSelector: match scope {
            Scope::Input => kAudioHardwarePropertyDefaultInputDevice,
            Scope::Output => kAudioHardwarePropertyDefaultOutputDevice,
        },
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMaster,
    };
    TestPropertyListener::new(kAudioObjectSystemObject, address, listener)
}

pub struct TestPropertyListener<F>
where
    F: Fn(&[AudioObjectPropertyAddress]) -> OSStatus,
{
    device: AudioObjectID,
    property: AudioObjectPropertyAddress,
    callback: F,
}

impl<F> TestPropertyListener<F>
where
    F: Fn(&[AudioObjectPropertyAddress]) -> OSStatus,
{
    pub fn new(device: AudioObjectID, property: AudioObjectPropertyAddress, callback: F) -> Self {
        Self {
            device,
            property,
            callback,
        }
    }

    pub fn start(&self) -> std::result::Result<(), OSStatus> {
        let status = unsafe {
            AudioObjectAddPropertyListener(
                self.device,
                &self.property,
                Some(Self::render),
                self as *const Self as *mut c_void,
            )
        };
        if status == NO_ERR {
            Ok(())
        } else {
            Err(status)
        }
    }

    pub fn stop(&self) -> std::result::Result<(), OSStatus> {
        let status = unsafe {
            AudioObjectRemovePropertyListener(
                self.device,
                &self.property,
                Some(Self::render),
                self as *const Self as *mut c_void,
            )
        };
        if status == NO_ERR {
            Ok(())
        } else {
            Err(status)
        }
    }

    extern "C" fn render(
        id: AudioObjectID,
        number_of_addresses: u32,
        addresses: *const AudioObjectPropertyAddress,
        data: *mut c_void,
    ) -> OSStatus {
        let listener = unsafe { &*(data as *mut Self) };
        assert_eq!(id, listener.device);
        let addrs = unsafe { slice::from_raw_parts(addresses, number_of_addresses as usize) };
        (listener.callback)(addrs)
    }
}

impl<F> Drop for TestPropertyListener<F>
where
    F: Fn(&[AudioObjectPropertyAddress]) -> OSStatus,
{
    fn drop(&mut self) {
        self.stop();
    }
}

// TODO: It doesn't work if default input or output is an aggregate device! Probably we need to do
//       the same thing as what audiounit_set_aggregate_sub_device_list does.
#[derive(Debug)]
pub struct TestDevicePlugger {
    scope: Scope,
    plugin_id: AudioObjectID,
    device_id: AudioObjectID,
}

impl TestDevicePlugger {
    pub fn new(scope: Scope) -> std::result::Result<Self, OSStatus> {
        let plugin_id = Self::get_system_plugin_id()?;
        Ok(Self {
            scope,
            plugin_id,
            device_id: kAudioObjectUnknown,
        })
    }

    pub fn get_device_id(&self) -> AudioObjectID {
        self.device_id
    }

    pub fn plug(&mut self) -> std::result::Result<(), OSStatus> {
        self.device_id = self.create_aggregate_device()?;
        Ok(())
    }

    pub fn unplug(&mut self) -> std::result::Result<(), OSStatus> {
        self.destroy_aggregate_device()
    }

    fn is_plugging(&self) -> bool {
        self.device_id != kAudioObjectUnknown
    }

    fn destroy_aggregate_device(&mut self) -> std::result::Result<(), OSStatus> {
        assert_ne!(self.plugin_id, kAudioObjectUnknown);
        assert_ne!(self.device_id, kAudioObjectUnknown);

        let address = AudioObjectPropertyAddress {
            mSelector: kAudioPlugInDestroyAggregateDevice,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMaster,
        };

        let mut size: usize = 0;
        let status = unsafe {
            AudioObjectGetPropertyDataSize(
                self.plugin_id,
                &address,
                0,
                ptr::null(),
                &mut size as *mut usize as *mut u32,
            )
        };
        if status != NO_ERR {
            return Err(status);
        }
        assert_ne!(size, 0);

        let status = unsafe {
            // This call can simulate removing a device.
            AudioObjectGetPropertyData(
                self.plugin_id,
                &address,
                0,
                ptr::null(),
                &mut size as *mut usize as *mut u32,
                &mut self.device_id as *mut AudioDeviceID as *mut c_void,
            )
        };
        if status == NO_ERR {
            self.device_id = kAudioObjectUnknown;
            Ok(())
        } else {
            Err(status)
        }
    }

    fn create_aggregate_device(&self) -> std::result::Result<AudioObjectID, OSStatus> {
        use std::time::{SystemTime, UNIX_EPOCH};

        const TEST_AGGREGATE_DEVICE_NAME: &str = "TestAggregateDevice";

        assert_ne!(self.plugin_id, kAudioObjectUnknown);

        let sub_devices = Self::get_sub_devices(self.scope.clone());
        if sub_devices.is_none() {
            return Err(kAudioCodecUnspecifiedError as OSStatus);
        }
        let sub_devices = sub_devices.unwrap();

        let address = AudioObjectPropertyAddress {
            mSelector: kAudioPlugInCreateAggregateDevice,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMaster,
        };

        let mut size: usize = 0;
        let status = unsafe {
            AudioObjectGetPropertyDataSize(
                self.plugin_id,
                &address,
                0,
                ptr::null(),
                &mut size as *mut usize as *mut u32,
            )
        };
        if status != NO_ERR {
            return Err(status);
        }
        assert_ne!(size, 0);

        let sys_time = SystemTime::now();
        let time_id = sys_time.duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let device_name = format!("{}_{}", TEST_AGGREGATE_DEVICE_NAME, time_id);
        let device_uid = format!("org.mozilla.{}", device_name);

        let mut device_id = kAudioObjectUnknown;
        let status = unsafe {
            let device_dict = CFDictionaryCreateMutable(
                kCFAllocatorDefault,
                0,
                &kCFTypeDictionaryKeyCallBacks,
                &kCFTypeDictionaryValueCallBacks,
            );

            // Set the name of this device.
            let device_name = cfstringref_from_string(&device_name);
            CFDictionaryAddValue(
                device_dict,
                cfstringref_from_static_string(AGGREGATE_DEVICE_NAME_KEY) as *const c_void,
                device_name as *const c_void,
            );
            CFRelease(device_name as *const c_void);

            // Set the uid of this device.
            let device_uid = cfstringref_from_string(&device_uid);
            CFDictionaryAddValue(
                device_dict,
                cfstringref_from_static_string(AGGREGATE_DEVICE_UID_KEY) as *const c_void,
                device_uid as *const c_void,
            );
            CFRelease(device_uid as *const c_void);

            // This device is private to the process creating it.
            let private_value: i32 = 1;
            let device_private_key = CFNumberCreate(
                kCFAllocatorDefault,
                i64::from(kCFNumberIntType),
                &private_value as *const i32 as *const c_void,
            );
            CFDictionaryAddValue(
                device_dict,
                cfstringref_from_static_string(AGGREGATE_DEVICE_PRIVATE_KEY) as *const c_void,
                device_private_key as *const c_void,
            );
            CFRelease(device_private_key as *const c_void);

            // Set this device to be a stacked aggregate (i.e. multi-output device).
            let stacked_value: i32 = 0; // 1 for normal aggregate device.
            let device_stacked_key = CFNumberCreate(
                kCFAllocatorDefault,
                i64::from(kCFNumberIntType),
                &stacked_value as *const i32 as *const c_void,
            );
            CFDictionaryAddValue(
                device_dict,
                cfstringref_from_static_string(AGGREGATE_DEVICE_STACKED_KEY) as *const c_void,
                device_stacked_key as *const c_void,
            );
            CFRelease(device_stacked_key as *const c_void);

            // Set sub devices for this device.
            CFDictionaryAddValue(
                device_dict,
                cfstringref_from_static_string(AGGREGATE_DEVICE_SUB_DEVICE_LIST_KEY)
                    as *const c_void,
                sub_devices as *const c_void,
            );
            CFRelease(sub_devices as *const c_void);

            // This call can simulate adding a device.
            let status = AudioObjectGetPropertyData(
                self.plugin_id,
                &address,
                mem::size_of_val(&device_dict) as u32,
                &device_dict as *const CFMutableDictionaryRef as *const c_void,
                &mut size as *mut usize as *mut u32,
                &mut device_id as *mut AudioDeviceID as *mut c_void,
            );
            CFRelease(device_dict as *const c_void);
            status
        };
        if status == NO_ERR {
            assert_ne!(device_id, kAudioObjectUnknown);
            Ok(device_id)
        } else {
            Err(status)
        }
    }

    fn get_system_plugin_id() -> std::result::Result<AudioObjectID, OSStatus> {
        let address = AudioObjectPropertyAddress {
            mSelector: kAudioHardwarePropertyPlugInForBundleID,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMaster,
        };

        let mut size: usize = 0;
        let status = unsafe {
            AudioObjectGetPropertyDataSize(
                kAudioObjectSystemObject,
                &address,
                0,
                ptr::null(),
                &mut size as *mut usize as *mut u32,
            )
        };
        if status != NO_ERR {
            return Err(status);
        }
        assert_ne!(size, 0);

        let mut plugin_id = kAudioObjectUnknown;
        let mut in_bundle_ref = cfstringref_from_static_string("com.apple.audio.CoreAudio");
        let mut translation_value = AudioValueTranslation {
            mInputData: &mut in_bundle_ref as *mut CFStringRef as *mut c_void,
            mInputDataSize: mem::size_of::<CFStringRef>() as u32,
            mOutputData: &mut plugin_id as *mut AudioObjectID as *mut c_void,
            mOutputDataSize: mem::size_of::<AudioObjectID>() as u32,
        };
        assert_eq!(size, mem::size_of_val(&translation_value));

        let status = unsafe {
            let status = AudioObjectGetPropertyData(
                kAudioObjectSystemObject,
                &address,
                0,
                ptr::null(),
                &mut size as *mut usize as *mut u32,
                &mut translation_value as *mut AudioValueTranslation as *mut c_void,
            );
            CFRelease(in_bundle_ref as *const c_void);
            status
        };
        if status == NO_ERR {
            assert_ne!(plugin_id, kAudioObjectUnknown);
            Ok(plugin_id)
        } else {
            Err(status)
        }
    }

    // TODO: This doesn't work as what we expect when the default deivce in the scope is an
    //       aggregate device. We should get the list of all the active sub devices and put
    //       them into the array, if the device is an aggregate device. See the code in
    //       audiounit_get_sub_devices and audiounit_set_aggregate_sub_device_list.
    fn get_sub_devices(scope: Scope) -> Option<CFArrayRef> {
        let device = test_get_default_device(scope);
        if device.is_none() {
            return None;
        }
        let device = device.unwrap();
        let uid = get_device_name(device);
        if uid.is_null() {
            return None;
        }
        unsafe {
            let list = CFArrayCreateMutable(ptr::null(), 0, &kCFTypeArrayCallBacks);
            let sub_device_dict = CFDictionaryCreateMutable(
                ptr::null(),
                0,
                &kCFTypeDictionaryKeyCallBacks,
                &kCFTypeDictionaryValueCallBacks,
            );
            CFDictionaryAddValue(
                sub_device_dict,
                cfstringref_from_static_string(SUB_DEVICE_UID_KEY) as *const c_void,
                uid as *const c_void,
            );
            CFArrayAppendValue(list, sub_device_dict as *const c_void);
            CFRelease(sub_device_dict as *const c_void);
            CFRelease(uid as *const c_void);
            Some(list)
        }
    }
}

impl Drop for TestDevicePlugger {
    fn drop(&mut self) {
        if self.is_plugging() {
            self.unplug();
        }
    }
}

// Test Templates
// ------------------------------------------------------------------------------------------------
pub fn test_ops_context_operation<F>(name: &'static str, operation: F)
where
    F: FnOnce(*mut ffi::cubeb),
{
    let name_c_string = CString::new(name).expect("Failed to create context name");
    let mut context = ptr::null_mut::<ffi::cubeb>();
    assert_eq!(
        unsafe { OPS.init.unwrap()(&mut context, name_c_string.as_ptr()) },
        ffi::CUBEB_OK
    );
    assert!(!context.is_null());
    operation(context);
    unsafe { OPS.destroy.unwrap()(context) }
}

// Note: The in-out stream initializeed with different device will create an aggregate_device and
//       result in firing device-collection-changed callbacks. Run in-out streams with tests
//       capturing device-collection-changed callbacks may cause troubles. See more details in the
//       comments for test_create_blank_aggregate_device.
pub fn test_ops_stream_operation<F>(
    name: &'static str,
    input_device: ffi::cubeb_devid,
    input_stream_params: *mut ffi::cubeb_stream_params,
    output_device: ffi::cubeb_devid,
    output_stream_params: *mut ffi::cubeb_stream_params,
    latency_frames: u32,
    data_callback: ffi::cubeb_data_callback,
    state_callback: ffi::cubeb_state_callback,
    user_ptr: *mut c_void,
    operation: F,
) where
    F: FnOnce(*mut ffi::cubeb_stream),
{
    test_ops_context_operation("context: stream operation", |context_ptr| {
        // Do nothing if there is no input/output device to perform input/output tests.
        if !input_stream_params.is_null() && test_get_default_device(Scope::Input).is_none() {
            println!("No input device to perform input tests for \"{}\".", name);
            return;
        }

        if !output_stream_params.is_null() && test_get_default_device(Scope::Output).is_none() {
            println!("No output device to perform output tests for \"{}\".", name);
            return;
        }

        let mut stream: *mut ffi::cubeb_stream = ptr::null_mut();
        let stream_name = CString::new(name).expect("Failed to create stream name");
        assert_eq!(
            unsafe {
                OPS.stream_init.unwrap()(
                    context_ptr,
                    &mut stream,
                    stream_name.as_ptr(),
                    input_device,
                    input_stream_params,
                    output_device,
                    output_stream_params,
                    latency_frames,
                    data_callback,
                    state_callback,
                    user_ptr,
                )
            },
            ffi::CUBEB_OK
        );
        assert!(!stream.is_null());
        operation(stream);
        unsafe {
            OPS.stream_destroy.unwrap()(stream);
        }
    });
}

pub fn test_get_raw_context<F>(operation: F)
where
    F: FnOnce(&mut AudioUnitContext),
{
    let mut context = AudioUnitContext::new();
    operation(&mut context);
}

pub fn test_get_default_raw_stream<F>(operation: F)
where
    F: FnOnce(&mut AudioUnitStream),
{
    test_get_raw_stream(ptr::null_mut(), None, None, 0, operation);
}

fn test_get_raw_stream<F>(
    user_ptr: *mut c_void,
    data_callback: ffi::cubeb_data_callback,
    state_callback: ffi::cubeb_state_callback,
    latency_frames: u32,
    operation: F,
) where
    F: FnOnce(&mut AudioUnitStream),
{
    let mut context = AudioUnitContext::new();

    // Add a stream to the context since we are about to create one.
    // AudioUnitStream::drop() will check the context has at least one stream.
    let global_latency_frames = context.update_latency_by_adding_stream(latency_frames);

    let mut stream = AudioUnitStream::new(
        &mut context,
        user_ptr,
        data_callback,
        state_callback,
        global_latency_frames.unwrap(),
    );
    stream.core_stream_data = CoreStreamData::new(&stream, None, None);

    operation(&mut stream);
}
