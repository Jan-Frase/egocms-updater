use anyhow::Context;
use std::fs;
use std::path::Path;

/// Any other file in the pages directory, e.g., an image or a PDF.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalFile {
    /// Path relative to the pages directory, always with `/` as a separator.
    pub relative_path: String,
    pub bytes: Vec<u8>,
}

impl LocalFile {
    pub fn new(relative_path: String, full_path: &Path) -> anyhow::Result<Self> {
        let bytes = fs::read(full_path).with_context(|| format!("Cannot read {relative_path}"))?;
        Ok(Self {
            relative_path,
            bytes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_bytes_and_keeps_the_relative_path() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plot.png");
        fs::write(&path, [0, 159, 146, 150, 255]).unwrap();

        let file = LocalFile::new("Research/plot.png".into(), &path).unwrap();
        assert_eq!(file.relative_path, "Research/plot.png");
        assert_eq!(file.bytes, [0, 159, 146, 150, 255]);
    }

    #[test]
    fn empty_files_are_fine() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.pdf");
        fs::write(&path, []).unwrap();
        assert_eq!(
            LocalFile::new("empty.pdf".into(), &path).unwrap().bytes,
            Vec::<u8>::new()
        );
    }

    #[test]
    fn reports_unreadable_files() {
        let dir = tempfile::tempdir().unwrap();
        let error = LocalFile::new("a.pdf".into(), &dir.path().join("missing.pdf")).unwrap_err();
        assert!(error.to_string().contains("Cannot read a.pdf"));
    }
}
