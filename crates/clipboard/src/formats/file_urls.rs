use crate::{Error, Result};
use std::path::PathBuf;

pub(crate) fn parse(text: &str) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for line in text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#') && *l != "copy" && *l != "cut")
    {
        let url = url::Url::parse(line).map_err(|_| Error::InvalidInput)?;
        if url.scheme() != "file"
            || url
                .host_str()
                .is_some_and(|h| !h.is_empty() && h != "localhost")
        {
            return Err(Error::InvalidInput);
        }
        files.push(url.to_file_path().map_err(|_| Error::InvalidInput)?);
        if files.len() > 256 {
            return Err(Error::InvalidInput);
        }
    }
    if files.is_empty() {
        Err(Error::InvalidInput)
    } else {
        Ok(files)
    }
}
pub(crate) fn encode(files: &[PathBuf]) -> Result<String> {
    if files.is_empty() || files.len() > 256 {
        return Err(Error::InvalidInput);
    }
    files
        .iter()
        .map(|p| {
            url::Url::from_file_path(p)
                .map(|u| format!("{u}\r\n"))
                .map_err(|_| Error::InvalidInput)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_urls_roundtrip_and_web_urls_are_not_file_references() {
        let path = std::env::temp_dir().join("中文 # picture.png");
        assert_eq!(
            parse(&encode(std::slice::from_ref(&path)).unwrap()).unwrap(),
            vec![path]
        );
        assert!(parse("https://example.com/image.png").is_err());
        assert!(parse("file://server/private/image.png").is_err());
    }
}
