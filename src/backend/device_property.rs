use super::*;

pub fn get_device_global_uid(id: AudioDeviceID) -> Result<CFStringRef> {
    get_device_uid(id, DeviceType::INPUT | DeviceType::OUTPUT)
}

pub fn get_device_uid(id: AudioDeviceID, devtype: DeviceType) -> Result<CFStringRef> {
    assert_ne!(id, kAudioObjectUnknown);

    let address = get_property_address(Property::DeviceUID, devtype);
    let mut size = mem::size_of::<CFStringRef>();
    let mut uid: CFStringRef = ptr::null();
    let err = audio_object_get_property_data(id, &address, &mut size, &mut uid);
    if err == NO_ERR {
        Ok(uid)
    } else {
        Err(Error::error())
    }
}

pub fn get_device_source(id: AudioDeviceID, devtype: DeviceType) -> Result<u32> {
    assert_ne!(id, kAudioObjectUnknown);

    let address = get_property_address(Property::DeviceSource, devtype);
    let mut size = mem::size_of::<u32>();
    let mut source: u32 = 0;
    let err = audio_object_get_property_data(id, &address, &mut size, &mut source);
    if err == NO_ERR {
        Ok(source)
    } else {
        Err(Error::error())
    }
}

pub fn get_device_source_name(id: AudioDeviceID, devtype: DeviceType) -> Result<CFStringRef> {
    assert_ne!(id, kAudioObjectUnknown);

    let mut source: u32 = get_device_source(id, devtype)?;
    let address = get_property_address(Property::DeviceSourceName, devtype);
    let mut size = mem::size_of::<AudioValueTranslation>();
    let mut name: CFStringRef = ptr::null();
    let mut trl = AudioValueTranslation {
        mInputData: &mut source as *mut u32 as *mut c_void,
        mInputDataSize: mem::size_of::<u32>() as u32,
        mOutputData: &mut name as *mut CFStringRef as *mut c_void,
        mOutputDataSize: mem::size_of::<CFStringRef>() as u32,
    };
    let err = audio_object_get_property_data(id, &address, &mut size, &mut trl);
    if err == NO_ERR {
        Ok(name)
    } else {
        Err(Error::error())
    }
}

pub fn get_device_name(id: AudioDeviceID, devtype: DeviceType) -> Result<CFStringRef> {
    assert_ne!(id, kAudioObjectUnknown);

    let address = get_property_address(Property::DeviceName, devtype);
    let mut size = mem::size_of::<CFStringRef>();
    let mut name: CFStringRef = ptr::null();
    let err = audio_object_get_property_data(id, &address, &mut size, &mut name);
    if err == NO_ERR {
        Ok(name)
    } else {
        Err(Error::error())
    }
}

enum Property {
    DeviceName,
    DeviceSource,
    DeviceSourceName,
    DeviceUID,
}

impl From<Property> for AudioObjectPropertySelector {
    fn from(p: Property) -> Self {
        match p {
            Property::DeviceName => kAudioObjectPropertyName,
            Property::DeviceSource => kAudioDevicePropertyDataSource,
            Property::DeviceSourceName => kAudioDevicePropertyDataSourceNameForIDCFString,
            Property::DeviceUID => kAudioDevicePropertyDeviceUID,
        }
    }
}

fn get_property_address(property: Property, devtype: DeviceType) -> AudioObjectPropertyAddress {
    const GLOBAL: ffi::cubeb_device_type =
        ffi::CUBEB_DEVICE_TYPE_INPUT | ffi::CUBEB_DEVICE_TYPE_OUTPUT;
    let scope = match devtype.bits() {
        ffi::CUBEB_DEVICE_TYPE_INPUT => kAudioDevicePropertyScopeInput,
        ffi::CUBEB_DEVICE_TYPE_OUTPUT => kAudioDevicePropertyScopeOutput,
        GLOBAL => kAudioObjectPropertyScopeGlobal,
        _ => panic!("Invalid type"),
    };
    AudioObjectPropertyAddress {
        mSelector: property.into(),
        mScope: scope,
        mElement: kAudioObjectPropertyElementMaster,
    }
}