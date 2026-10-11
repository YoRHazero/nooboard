use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Options {
    pub connect_timeout: Duration,
    pub host_key_timeout: Duration,
    pub request_timeout: Duration,
    pub directory_idle_timeout: Duration,
    pub directory_batch_size: usize,
    pub max_directory_queries: usize,
    pub max_sessions: usize,
    pub max_transfers: usize,
    pub transfer_concurrency: usize,
    pub terminal_replay_bytes: usize,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(20),
            host_key_timeout: Duration::from_secs(120),
            request_timeout: Duration::from_secs(15),
            directory_idle_timeout: Duration::from_secs(60),
            directory_batch_size: 200,
            max_directory_queries: 32,
            max_sessions: 16,
            max_transfers: 512,
            transfer_concurrency: 2,
            terminal_replay_bytes: 1024 * 1024,
        }
    }
}
impl Options {
    pub(crate) fn validate(&self) -> crate::Result<()> {
        if self.directory_batch_size == 0
            || self.directory_batch_size > 2000
            || self.max_directory_queries == 0
            || self.max_sessions == 0
            || self.max_transfers == 0
            || self.transfer_concurrency == 0
            || self.terminal_replay_bytes < 4096
            || self.connect_timeout.is_zero()
            || self.host_key_timeout.is_zero()
            || self.request_timeout < Duration::from_secs(1)
            || self.directory_idle_timeout.is_zero()
        {
            return Err(crate::Error::Limit);
        }
        Ok(())
    }
}
