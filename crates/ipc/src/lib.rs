pub mod mpv;
pub mod protocol;
pub mod text;
pub mod ui;

#[cfg(feature = "clipboard")]
pub mod clipboard;

#[cfg(feature = "host")]
pub mod host;
#[cfg(feature = "host")]
pub mod render;

#[cfg(feature = "host")]
pub use host::IpcPluginHost;
