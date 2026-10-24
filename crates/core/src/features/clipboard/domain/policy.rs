use crate::configuration::domain::model::Configuration;
use crate::contracts::network::NetworkStatus;
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
