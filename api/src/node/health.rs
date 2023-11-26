use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, PartialEq, Eq, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../app/src/api-types/")]
pub enum AudioNodeHealth {
    Good,
    Mild(AudioNodeHealthMild),
    Poor(AudioNodeHealthPoor),
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../app/src/api-types/")]
pub enum AudioNodeHealthMild {
    Buffering,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(export, export_to = "../app/src/api-types/")]
pub enum AudioNodeHealthPoor {
    DeviceNotAvailable,
    AudioStreamReadFailed,
    AudioBackendError(String),
}
