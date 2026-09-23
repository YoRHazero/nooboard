use crate::configuration::model::Configuration;
use nooboard_network::NetworkStatus;
pub(crate) fn targets(config: &Configuration, status: &NetworkStatus) -> Vec<String> {
    config
        .peers
        .keys()
        .filter(|id| {
            config.auto_send_enabled(id)
                && status
                    .connections
                    .iter()
                    .any(|c| &c.peer == *id && c.accepting)
        })
        .cloned()
        .collect()
}
