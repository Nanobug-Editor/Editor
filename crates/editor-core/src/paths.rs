//! Human-readable filesystem paths are distinct from canonical paths used for native authority.

use std::{fmt, path::Path};

/// A borrowed path presentation; the original path remains available for file access and identity.
pub struct DisplayPath<'a>(&'a Path);

/// Format a filesystem path for labels, tooltips, copied text and diagnostic messages.
///
/// `path` remains unchanged. The returned formatter omits Windows disk verbatim prefixes and
/// presents verbatim UNC roots as ordinary network paths. Other namespaces retain their spelling;
/// invalid Unicode follows the standard path display's replacement behavior. No filesystem I/O
/// or normalization is performed, and this presentation must not replace stored authority paths.
pub fn display_path(path: &Path) -> DisplayPath<'_> {
    DisplayPath(path)
}

impl fmt::Display for DisplayPath<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[cfg(windows)]
        if let Some(std::path::Component::Prefix(prefix)) = self.0.components().next() {
            match prefix.kind() {
                // Only recognized filesystem roots have an equivalent ordinary spelling.
                std::path::Prefix::VerbatimDisk(_) => {
                    return formatter.write_str(&self.0.to_string_lossy()[4..]);
                }
                std::path::Prefix::VerbatimUNC(_, _) => {
                    formatter.write_str(r"\\")?;
                    return formatter.write_str(&self.0.to_string_lossy()[8..]);
                }
                _ => {}
            }
        }
        self.0.display().fmt(formatter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Relative and ordinary paths retain their platform-native spelling and Unicode text.
    #[test]
    fn path_display_preserves_ordinary_paths() {
        for path in [
            ".",
            "src/main.rs",
            "/项目/目录 with spaces",
            "C:\\项目\\文件.txt",
        ] {
            let path = Path::new(path);
            assert_eq!(display_path(path).to_string(), path.display().to_string());
        }
    }

    /// A local extended path is readable without altering its original bytes or root syntax.
    #[cfg(windows)]
    #[test]
    fn path_display_formats_verbatim_disk_paths() {
        for (raw, expected) in [
            (
                r"\\?\C:\项目\目录 with spaces\file.txt",
                r"C:\项目\目录 with spaces\file.txt",
            ),
            (r"\\?\d:\", r"d:\"),
            (r"\\?\C:\trailing. ", r"C:\trailing. "),
        ] {
            let path = Path::new(raw);
            assert_eq!(display_path(path).to_string(), expected);
            assert_eq!(path.as_os_str(), raw);
        }
    }

    /// Network roots keep both leading slashes; stripping four bytes alone would corrupt the label.
    #[cfg(windows)]
    #[test]
    fn path_display_formats_verbatim_unc_paths() {
        for (raw, expected) in [
            (
                r"\\?\UNC\server\share\项目\file.txt",
                r"\\server\share\项目\file.txt",
            ),
            (r"\\?\UNC\server\share\", r"\\server\share\"),
        ] {
            let path = Path::new(raw);
            assert_eq!(display_path(path).to_string(), expected);
            assert_eq!(path.as_os_str(), raw);
        }
    }

    /// Device and volume namespaces cannot be represented as ordinary drive or UNC paths.
    #[cfg(windows)]
    #[test]
    fn path_display_preserves_device_namespaces() {
        for raw in [
            r"\\.\pipe\service",
            r"\\?\Volume{00000000-0000-0000-0000-000000000000}\",
        ] {
            assert_eq!(display_path(Path::new(raw)).to_string(), raw);
        }
    }

    /// Displaying malformed Unicode still has the same lossy semantics as std::path::Display.
    #[cfg(windows)]
    #[test]
    fn path_display_preserves_lossy_unicode_semantics() {
        use std::{ffi::OsString, os::windows::ffi::OsStringExt as _};
        let mut raw = r"\\?\C:\".encode_utf16().collect::<Vec<_>>();
        raw.extend([0xd800, u16::from(b'.'), u16::from(b'r'), u16::from(b's')]);
        let raw = OsString::from_wide(&raw);
        assert_eq!(display_path(Path::new(&raw)).to_string(), "C:\\\u{fffd}.rs");
    }
}
