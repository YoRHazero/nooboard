use crate::{Error, Result, Snapshot, worker::Command};
use std::{sync::mpsc, time::Duration};
pub(crate) struct Wake;
impl Wake {
    pub fn new() -> Result<Self> {
        Err(Error::UnsupportedPlatform)
    }
    pub fn notify(&self) {}
}
pub(crate) struct Native;
impl Native {
    pub fn open(_: usize) -> Result<Self> {
        Err(Error::UnsupportedPlatform)
    }
    pub fn revision(&self) -> u64 {
        0
    }
    pub fn wait(
        &mut self,
        _: &mpsc::Receiver<Command>,
        _: &Wake,
        _: Duration,
        _: bool,
    ) -> Result<Option<Command>> {
        Err(Error::UnsupportedPlatform)
    }
    pub fn read(&self) -> Result<Snapshot> {
        Err(Error::UnsupportedPlatform)
    }
    pub fn write(&mut self, _: &str) -> Result<Snapshot> {
        Err(Error::UnsupportedPlatform)
    }
}
