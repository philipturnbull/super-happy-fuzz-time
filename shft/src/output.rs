use std::ffi::OsString;
use std::path::{Path, PathBuf};
use libshft::error::*;

pub struct OutputPattern {
    dirname: PathBuf,
    filename_prefix: OsString,
    filename_suffix: OsString,
}

impl OutputPattern {
    pub fn from_path<P: AsRef<Path>>(pattern: P) -> Result<OutputPattern> {
        let pattern = PathBuf::from(pattern.as_ref());
        match (pattern.parent(), pattern.file_name()) {
            (Some(dirname), Some(filename)) => {
                let filename = filename.to_str();
                match filename.map(|s| (s, s.find("{}"))) {
                    Some((filename_str, Some(pattern_offset))) => {
                        let filename_prefix = OsString::from(&filename_str[..pattern_offset]);
                        let filename_suffix = OsString::from(&filename_str[pattern_offset+2..]);
                        Ok(OutputPattern {
                            dirname: PathBuf::from(dirname),
                            filename_prefix: filename_prefix,
                            filename_suffix: filename_suffix,
                        })
                    },
                    _ => Err("Could not find '{}' marker".into()),
                }
            },
            _ => Err("Pattern must include directory and filename".into()),
        }
    }

    pub fn with<T: ToString>(self: &Self, value: T) -> OsString {
        let value = OsString::from(value.to_string());
        let mut filename = OsString::from(self.filename_prefix.clone());
        filename.push(value);
        filename.push(self.filename_suffix.clone());

        let mut path = PathBuf::new();
        path.push(self.dirname.clone());
        path.push(filename);
        path.into_os_string()
    }
}

#[test]
fn test_output_pattern() {
    assert!(OutputPattern::from_path("out/name.ext").is_err());
    let pattern = OutputPattern::from_path("out/{}.ext").unwrap();
    assert!(pattern.with(0) == OsString::from("out/0.ext"));
}
