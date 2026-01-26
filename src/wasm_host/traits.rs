use crate::wasm_host::wrapper::PluginOutputWrapper;

pub trait RecordMeta {
    fn namespace(&self) -> [u8; 32];
    fn key(&self) -> [u8; 32];
    fn timestamp(&self) -> u64;
}

pub trait PluginOutputTrait: 'static {
    fn from_wrapper(wrapper: PluginOutputWrapper) -> Option<Box<Self>>;
    fn format_name() -> &'static str;
}

pub trait ToStandardData: RecordMeta {
    fn value(&self) -> [u8; 32];
}

impl PluginOutputTrait for dyn ToStandardData {
    fn from_wrapper(wrapper: PluginOutputWrapper) -> Option<Box<Self>> {
        wrapper.as_standard()
    }

    fn format_name() -> &'static str {
        "Standard"
    }
}

pub trait ToCustomJsonData: RecordMeta {
    fn payload_json(&self) -> &str;
}

impl PluginOutputTrait for dyn ToCustomJsonData {
    fn from_wrapper(wrapper: PluginOutputWrapper) -> Option<Box<Self>> {
        wrapper.as_custom_json()
    }

    fn format_name() -> &'static str {
        "Custom JSON"
    }
}

pub trait ToExtendedData: RecordMeta {
    fn value(&self) -> &[u8];
    fn metadata_json(&self) -> Option<&str>;
}

impl PluginOutputTrait for dyn ToExtendedData {
    fn from_wrapper(wrapper: PluginOutputWrapper) -> Option<Box<Self>> {
        wrapper.as_extended()
    }

    fn format_name() -> &'static str {
        "Extended"
    }
}

pub trait ToRawData {
    fn key(&self) -> [u8; 32];
    fn raw_bytes(&self) -> &[u8];
    fn timestamp(&self) -> Option<u64>;
}

impl PluginOutputTrait for dyn ToRawData {
    fn from_wrapper(wrapper: PluginOutputWrapper) -> Option<Box<Self>> {
        wrapper.as_raw()
    }

    fn format_name() -> &'static str {
        "Raw"
    }
}
