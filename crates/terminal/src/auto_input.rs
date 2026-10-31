use crate::{CommandAutoInput, SecretString};

/// Matches only the bounded beginning of each LF-delimited output line.
/// Evaluate once per received line fragment, so all currently complete matches
/// compete in insertion order, including overlapping prefixes.
#[derive(Default)]
pub(crate) struct AutoInputs {
    rules: Vec<(Vec<u8>, Option<SecretString>)>,
    prefix: Vec<u8>,
    limit: usize,
    skip_line: bool,
}

impl AutoInputs {
    pub fn new(rules: Vec<CommandAutoInput>) -> Self {
        Self {
            limit: rules
                .iter()
                .map(|rule| rule.prefix.len())
                .max()
                .unwrap_or(0),
            rules: rules
                .into_iter()
                .map(|rule| (rule.prefix.into_bytes(), Some(rule.value)))
                .collect(),
            ..Self::default()
        }
    }

    pub fn feed(&mut self, bytes: &[u8]) -> Vec<SecretString> {
        let mut inputs = Vec::new();
        if self.limit == 0 {
            return inputs;
        }
        for fragment in bytes.split_inclusive(|byte| *byte == b'\n') {
            if !self.skip_line {
                let remaining = self.limit.saturating_sub(self.prefix.len());
                self.prefix.extend(fragment.iter().take(remaining));
                if let Some((_, value)) = self
                    .rules
                    .iter_mut()
                    .find(|(key, value)| value.is_some() && self.prefix.starts_with(key))
                {
                    let value = value.take().unwrap();
                    // Consume before dispatch. Even partial/failed writes are never retried.
                    inputs.push(SecretString::new(format!("{}\n", value.expose())));
                    self.skip_line = true;
                } else if !self
                    .rules
                    .iter()
                    .any(|(key, value)| value.is_some() && key.starts_with(&self.prefix))
                {
                    self.skip_line = true;
                }
            }
            if fragment.last() == Some(&b'\n') {
                self.prefix.clear();
                self.skip_line = false;
            }
        }
        inputs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> AutoInputs {
        AutoInputs::new(vec![
            CommandAutoInput {
                prefix: "Password for admin:".into(),
                value: SecretString::new(" first "),
            },
            CommandAutoInput {
                prefix: "Password".into(),
                value: SecretString::new(""),
            },
        ])
    }

    #[test]
    fn first_current_match_wins_once_per_rule_and_line() {
        let mut matcher = rules();
        assert_eq!(
            matcher.feed(b"Password for admin:")[0].expose(),
            " first \n"
        );
        assert!(matcher.feed(b" Password").is_empty());
        assert_eq!(matcher.feed(b"\r\r\nPassword for admin:")[0].expose(), "\n");
        assert!(matcher.feed(b"\nPassword for admin:\n").is_empty());
    }

    #[test]
    fn complete_short_match_does_not_wait_for_an_earlier_long_match() {
        let mut matcher = rules();
        assert!(matcher.feed(b"Pass").is_empty());
        assert_eq!(matcher.feed(b"word")[0].expose(), "\n");
        assert!(matcher.feed(b" for admin:\n").is_empty());
        assert_eq!(
            matcher.feed(b"Password for admin:")[0].expose(),
            " first \n"
        );
    }

    #[test]
    fn matching_is_exact_bounded_and_independent_of_unrelated_lines() {
        let mut matcher = rules();
        assert!(
            matcher
                .feed(b" Password\n\x1b[32mPassword\nnoise\rPassword\n")
                .is_empty()
        );
        assert!(matcher.feed(&vec![b'x'; 100_000]).is_empty());
        assert!(matcher.prefix.len() <= matcher.limit);
        assert_eq!(matcher.feed(b"\nPassword for admin:\nPassword").len(), 2);
    }
}
