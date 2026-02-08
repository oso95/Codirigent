//! File entry representation.
//!
//! This module provides the `FileEntry` struct which represents a single
//! file or directory in the file tree.

use std::path::PathBuf;

/// A file or directory entry in the file tree.
///
/// Each entry contains metadata about a file or directory, including
/// its path, name, type, and expansion state for directories.
#[derive(Debug, Clone)]
pub struct FileEntry {
    /// Full path to the file or directory.
    pub path: PathBuf,

    /// File or directory name (last component of path).
    pub name: String,

    /// Whether this entry is a directory.
    pub is_dir: bool,

    /// Whether this is a hidden file (name starts with '.').
    pub is_hidden: bool,

    /// Whether this entry is a symbolic link.
    pub is_symlink: bool,

    /// Child entries for directories. `None` for files.
    pub children: Option<Vec<FileEntry>>,

    /// Whether this directory is expanded in the UI.
    /// Only meaningful for directories.
    pub expanded: bool,
}

impl FileEntry {
    /// Create a new file entry from a path.
    ///
    /// Determines file metadata from the filesystem.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the file or directory
    pub fn new(path: PathBuf) -> Self {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string());

        let is_hidden = name.starts_with('.');
        let metadata = path.symlink_metadata();
        let is_symlink = metadata.as_ref().map(|m| m.is_symlink()).unwrap_or(false);

        // For symlinks, check the target to determine if it's a directory
        let is_dir = if is_symlink {
            path.metadata().map(|m| m.is_dir()).unwrap_or(false)
        } else {
            metadata.map(|m| m.is_dir()).unwrap_or(false)
        };

        Self {
            path,
            name,
            is_dir,
            is_hidden,
            is_symlink,
            children: if is_dir { Some(Vec::new()) } else { None },
            expanded: false,
        }
    }

    /// Create a file entry with explicit properties.
    ///
    /// Useful for testing or when metadata is already known.
    pub fn with_properties(
        path: PathBuf,
        name: String,
        is_dir: bool,
        is_hidden: bool,
        is_symlink: bool,
    ) -> Self {
        Self {
            path,
            name,
            is_dir,
            is_hidden,
            is_symlink,
            children: if is_dir { Some(Vec::new()) } else { None },
            expanded: false,
        }
    }

    /// Get the file extension, if any.
    ///
    /// Returns `None` for directories or files without extensions.
    pub fn extension(&self) -> Option<&str> {
        if self.is_dir {
            None
        } else {
            self.path.extension().and_then(|e| e.to_str())
        }
    }

    /// Get the icon representation for this entry.
    ///
    /// Returns a `FileIcon` enum value based on file type.
    pub fn icon(&self) -> FileIcon {
        if self.is_dir {
            if self.expanded {
                FileIcon::FolderOpen
            } else {
                FileIcon::Folder
            }
        } else {
            Self::icon_for_file(&self.name, self.extension())
        }
    }

    /// Determine icon for a file based on name and extension.
    fn icon_for_file(name: &str, extension: Option<&str>) -> FileIcon {
        // Check special filenames first (they take priority over extension)
        match name {
            "Cargo.toml" | "Cargo.lock" => return FileIcon::Cargo,
            "Dockerfile" | "docker-compose.yml" | "docker-compose.yaml" => return FileIcon::Docker,
            "Makefile" | "makefile" | "GNUmakefile" => return FileIcon::Makefile,
            "LICENSE" | "LICENSE.md" | "LICENSE.txt" => return FileIcon::License,
            "README" | "README.md" | "README.txt" => return FileIcon::Readme,
            ".gitignore" | ".gitattributes" => return FileIcon::Git,
            ".env" | ".env.local" | ".env.development" | ".env.production" => return FileIcon::Env,
            _ => {}
        }

        // Check extension
        if let Some(ext) = extension {
            if let Some(icon) = Self::icon_for_extension(ext) {
                return icon;
            }
        }

        FileIcon::File
    }

    /// Get icon for a file extension.
    fn icon_for_extension(ext: &str) -> Option<FileIcon> {
        Some(match ext {
            "rs" => FileIcon::Rust,
            "md" | "markdown" => FileIcon::Markdown,
            "json" => FileIcon::Json,
            "toml" => FileIcon::Toml,
            "yaml" | "yml" => FileIcon::Yaml,
            "ts" | "tsx" => FileIcon::TypeScript,
            "js" | "jsx" | "mjs" | "cjs" => FileIcon::JavaScript,
            "py" | "pyi" => FileIcon::Python,
            "go" => FileIcon::Go,
            "sh" | "bash" | "zsh" | "fish" => FileIcon::Shell,
            "lock" => FileIcon::Lock,
            "gitignore" | "gitattributes" | "gitmodules" => FileIcon::Git,
            "txt" => FileIcon::Text,
            "html" | "htm" => FileIcon::Html,
            "css" | "scss" | "sass" | "less" => FileIcon::Css,
            "c" | "h" => FileIcon::C,
            "cpp" | "cc" | "cxx" | "hpp" | "hxx" => FileIcon::Cpp,
            "java" => FileIcon::Java,
            "swift" => FileIcon::Swift,
            "kt" | "kts" => FileIcon::Kotlin,
            "rb" | "erb" => FileIcon::Ruby,
            "php" => FileIcon::Php,
            "sql" => FileIcon::Sql,
            "xml" => FileIcon::Xml,
            "svg" => FileIcon::Svg,
            "png" | "jpg" | "jpeg" | "gif" | "webp" => FileIcon::Image,
            "pdf" => FileIcon::Pdf,
            "zip" | "tar" | "gz" | "bz2" | "xz" => FileIcon::Archive,
            _ => return None,
        })
    }

    /// Check if this entry has children.
    pub fn has_children(&self) -> bool {
        self.children
            .as_ref()
            .map(|c| !c.is_empty())
            .unwrap_or(false)
    }

    /// Get the number of children.
    pub fn child_count(&self) -> usize {
        self.children.as_ref().map(|c| c.len()).unwrap_or(0)
    }
}

/// Icon type for file entries.
///
/// Represents different file types for icon rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileIcon {
    /// Generic file
    File,
    /// Closed folder
    Folder,
    /// Open folder
    FolderOpen,
    /// Rust source file
    Rust,
    /// Markdown document
    Markdown,
    /// JSON configuration
    Json,
    /// TOML configuration
    Toml,
    /// YAML configuration
    Yaml,
    /// TypeScript source
    TypeScript,
    /// JavaScript source
    JavaScript,
    /// Python source
    Python,
    /// Go source
    Go,
    /// Shell script
    Shell,
    /// Lock file
    Lock,
    /// Git-related file
    Git,
    /// Plain text
    Text,
    /// HTML document
    Html,
    /// CSS stylesheet
    Css,
    /// C source
    C,
    /// C++ source
    Cpp,
    /// Java source
    Java,
    /// Swift source
    Swift,
    /// Kotlin source
    Kotlin,
    /// Ruby source
    Ruby,
    /// PHP source
    Php,
    /// SQL file
    Sql,
    /// XML file
    Xml,
    /// SVG image
    Svg,
    /// Image file
    Image,
    /// PDF document
    Pdf,
    /// Archive file
    Archive,
    /// Cargo manifest
    Cargo,
    /// Docker file
    Docker,
    /// Makefile
    Makefile,
    /// License file
    License,
    /// Readme file
    Readme,
    /// Environment file
    Env,
}

impl FileIcon {
    /// Get the emoji representation of this icon.
    ///
    /// Returns an emoji that visually represents the file type.
    pub fn emoji(&self) -> &'static str {
        match self {
            Self::File => "\u{1F4C4}",             // 📄
            Self::Folder => "\u{1F4C1}",           // 📁
            Self::FolderOpen => "\u{1F4C2}",       // 📂
            Self::Rust => "\u{1F980}",             // 🦀
            Self::Markdown => "\u{1F4DD}",         // 📝
            Self::Json => "\u{2699}\u{FE0F}",      // ⚙️
            Self::Toml => "\u{2699}\u{FE0F}",      // ⚙️
            Self::Yaml => "\u{2699}\u{FE0F}",      // ⚙️
            Self::TypeScript => "\u{1F4DC}",       // 📜
            Self::JavaScript => "\u{1F4DC}",       // 📜
            Self::Python => "\u{1F40D}",           // 🐍
            Self::Go => "\u{1F439}",               // 🐹
            Self::Shell => "\u{1F5A5}\u{FE0F}",    // 🖥️
            Self::Lock => "\u{1F512}",             // 🔒
            Self::Git => "\u{1F527}",              // 🔧
            Self::Text => "\u{1F4C3}",             // 📃
            Self::Html => "\u{1F310}",             // 🌐
            Self::Css => "\u{1F3A8}",              // 🎨
            Self::C => "\u{1F1E8}",                // 🇨
            Self::Cpp => "\u{2795}",               // ➕
            Self::Java => "\u{2615}",              // ☕
            Self::Swift => "\u{1F426}",            // 🐦
            Self::Kotlin => "\u{1F538}",           // 🔸
            Self::Ruby => "\u{1F48E}",             // 💎
            Self::Php => "\u{1F418}",              // 🐘
            Self::Sql => "\u{1F5C3}\u{FE0F}",      // 🗃️
            Self::Xml => "\u{1F4C4}",              // 📄
            Self::Svg => "\u{1F5BC}\u{FE0F}",      // 🖼️
            Self::Image => "\u{1F5BC}\u{FE0F}",    // 🖼️
            Self::Pdf => "\u{1F4D5}",              // 📕
            Self::Archive => "\u{1F4E6}",          // 📦
            Self::Cargo => "\u{1F4E6}",            // 📦
            Self::Docker => "\u{1F433}",           // 🐳
            Self::Makefile => "\u{1F3D7}\u{FE0F}", // 🏗️
            Self::License => "\u{1F4DC}",          // 📜
            Self::Readme => "\u{1F4D6}",           // 📖
            Self::Env => "\u{1F510}",              // 🔐
        }
    }

    /// Get a short identifier for this icon type.
    ///
    /// Useful for icon themes or CSS class names.
    pub fn id(&self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Folder => "folder",
            Self::FolderOpen => "folder-open",
            Self::Rust => "rust",
            Self::Markdown => "markdown",
            Self::Json => "json",
            Self::Toml => "toml",
            Self::Yaml => "yaml",
            Self::TypeScript => "typescript",
            Self::JavaScript => "javascript",
            Self::Python => "python",
            Self::Go => "go",
            Self::Shell => "shell",
            Self::Lock => "lock",
            Self::Git => "git",
            Self::Text => "text",
            Self::Html => "html",
            Self::Css => "css",
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::Java => "java",
            Self::Swift => "swift",
            Self::Kotlin => "kotlin",
            Self::Ruby => "ruby",
            Self::Php => "php",
            Self::Sql => "sql",
            Self::Xml => "xml",
            Self::Svg => "svg",
            Self::Image => "image",
            Self::Pdf => "pdf",
            Self::Archive => "archive",
            Self::Cargo => "cargo",
            Self::Docker => "docker",
            Self::Makefile => "makefile",
            Self::License => "license",
            Self::Readme => "readme",
            Self::Env => "env",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_file_entry_new_file() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("test.rs");
        std::fs::write(&file_path, "// Rust file").unwrap();

        let entry = FileEntry::new(file_path.clone());

        assert_eq!(entry.name, "test.rs");
        assert!(!entry.is_dir);
        assert!(!entry.is_hidden);
        assert!(!entry.is_symlink);
        assert_eq!(entry.path, file_path);
        assert!(entry.children.is_none());
    }

    #[test]
    fn test_file_entry_new_directory() {
        let temp = TempDir::new().unwrap();
        let dir_path = temp.path().join("subdir");
        std::fs::create_dir(&dir_path).unwrap();

        let entry = FileEntry::new(dir_path.clone());

        assert_eq!(entry.name, "subdir");
        assert!(entry.is_dir);
        assert!(entry.children.is_some());
    }

    #[test]
    fn test_hidden_file() {
        let temp = TempDir::new().unwrap();
        let hidden_path = temp.path().join(".hidden");
        std::fs::write(&hidden_path, "hidden").unwrap();

        let entry = FileEntry::new(hidden_path);
        assert!(entry.is_hidden);
    }

    #[test]
    fn test_file_extension() {
        let entry = FileEntry::with_properties(
            PathBuf::from("/tmp/test.rs"),
            "test.rs".to_string(),
            false,
            false,
            false,
        );
        assert_eq!(entry.extension(), Some("rs"));
    }

    #[test]
    fn test_directory_no_extension() {
        let entry = FileEntry::with_properties(
            PathBuf::from("/tmp/src"),
            "src".to_string(),
            true,
            false,
            false,
        );
        assert_eq!(entry.extension(), None);
    }

    #[test]
    fn test_file_icon_rust() {
        let entry = FileEntry::with_properties(
            PathBuf::from("/tmp/main.rs"),
            "main.rs".to_string(),
            false,
            false,
            false,
        );
        assert_eq!(entry.icon(), FileIcon::Rust);
        assert_eq!(entry.icon().emoji(), "\u{1F980}"); // 🦀
    }

    #[test]
    fn test_file_icon_python() {
        let entry = FileEntry::with_properties(
            PathBuf::from("/tmp/script.py"),
            "script.py".to_string(),
            false,
            false,
            false,
        );
        assert_eq!(entry.icon(), FileIcon::Python);
    }

    #[test]
    fn test_file_icon_folder() {
        let mut entry = FileEntry::with_properties(
            PathBuf::from("/tmp/src"),
            "src".to_string(),
            true,
            false,
            false,
        );
        assert_eq!(entry.icon(), FileIcon::Folder);

        entry.expanded = true;
        assert_eq!(entry.icon(), FileIcon::FolderOpen);
    }

    #[test]
    fn test_file_icon_special_files() {
        let cargo = FileEntry::with_properties(
            PathBuf::from("/tmp/Cargo.toml"),
            "Cargo.toml".to_string(),
            false,
            false,
            false,
        );
        assert_eq!(cargo.icon(), FileIcon::Cargo);

        let dockerfile = FileEntry::with_properties(
            PathBuf::from("/tmp/Dockerfile"),
            "Dockerfile".to_string(),
            false,
            false,
            false,
        );
        assert_eq!(dockerfile.icon(), FileIcon::Docker);

        let readme = FileEntry::with_properties(
            PathBuf::from("/tmp/README.md"),
            "README.md".to_string(),
            false,
            false,
            false,
        );
        assert_eq!(readme.icon(), FileIcon::Readme);
    }

    #[test]
    fn test_has_children() {
        let mut entry = FileEntry::with_properties(
            PathBuf::from("/tmp/src"),
            "src".to_string(),
            true,
            false,
            false,
        );

        // Empty directory
        assert!(!entry.has_children());
        assert_eq!(entry.child_count(), 0);

        // Add a child
        let child = FileEntry::with_properties(
            PathBuf::from("/tmp/src/main.rs"),
            "main.rs".to_string(),
            false,
            false,
            false,
        );
        entry.children.as_mut().unwrap().push(child);

        assert!(entry.has_children());
        assert_eq!(entry.child_count(), 1);
    }

    #[test]
    fn test_file_icon_id() {
        assert_eq!(FileIcon::Rust.id(), "rust");
        assert_eq!(FileIcon::Python.id(), "python");
        assert_eq!(FileIcon::Folder.id(), "folder");
        assert_eq!(FileIcon::FolderOpen.id(), "folder-open");
    }

    #[cfg(unix)]
    #[test]
    fn test_symlink_entry() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let target = temp.path().join("target.txt");
        let link = temp.path().join("link.txt");

        std::fs::write(&target, "content").unwrap();
        symlink(&target, &link).unwrap();

        let entry = FileEntry::new(link);

        assert!(entry.is_symlink);
        assert!(!entry.is_dir);
    }

    #[cfg(unix)]
    #[test]
    fn test_symlink_to_directory() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let target_dir = temp.path().join("target_dir");
        let link = temp.path().join("link_dir");

        std::fs::create_dir(&target_dir).unwrap();
        symlink(&target_dir, &link).unwrap();

        let entry = FileEntry::new(link);

        assert!(entry.is_symlink);
        assert!(entry.is_dir);
    }

    #[test]
    fn test_various_extensions() {
        let cases = [
            ("test.js", FileIcon::JavaScript),
            ("test.ts", FileIcon::TypeScript),
            ("test.go", FileIcon::Go),
            ("test.py", FileIcon::Python),
            ("test.html", FileIcon::Html),
            ("test.css", FileIcon::Css),
            ("test.json", FileIcon::Json),
            ("test.yaml", FileIcon::Yaml),
            ("test.sh", FileIcon::Shell),
            ("test.c", FileIcon::C),
            ("test.cpp", FileIcon::Cpp),
            ("test.java", FileIcon::Java),
            ("test.swift", FileIcon::Swift),
            ("test.rb", FileIcon::Ruby),
            ("test.php", FileIcon::Php),
            ("test.sql", FileIcon::Sql),
            ("test.xml", FileIcon::Xml),
            ("test.svg", FileIcon::Svg),
            ("test.png", FileIcon::Image),
            ("test.pdf", FileIcon::Pdf),
            ("test.zip", FileIcon::Archive),
        ];

        for (name, expected_icon) in cases {
            let entry = FileEntry::with_properties(
                PathBuf::from(format!("/tmp/{}", name)),
                name.to_string(),
                false,
                false,
                false,
            );
            assert_eq!(entry.icon(), expected_icon, "Failed for {}", name);
        }
    }
}
