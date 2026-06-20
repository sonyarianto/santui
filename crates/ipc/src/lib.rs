pub mod protocol;

#[cfg(feature = "host")]
pub mod host;
#[cfg(feature = "host")]
pub mod render;

#[cfg(feature = "host")]
pub use host::IpcPluginHost;
