use super::types::ProcessObservation;

pub trait ObservationProvider: Send + Sync + 'static {
    fn observations(&self) -> Vec<ProcessObservation>;
}

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::MacOSObservationProvider as SystemObservationProvider;

#[cfg(not(target_os = "macos"))]
#[derive(Debug, Default)]
pub struct SystemObservationProvider;

#[cfg(not(target_os = "macos"))]
impl ObservationProvider for SystemObservationProvider {
    fn observations(&self) -> Vec<ProcessObservation> {
        Vec::new()
    }
}
