//! The `Editor` port as a process (Go's `editInEditor` and `openInEditor`):
//! the content in a temporary file (mode 0600) in a directory of its own, the
//! editor run on it with the terminal as its own, what it left read back, and
//! the directory removed whatever happened. The file system is the seam's.

mod error;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ctxloom_ports::{Editor, EditorCommand, Failure};
use file_system::{Fs, Perms};
use os::Mode;

#[cfg(any(test, feature = "test-support"))]
pub use error::diagnostic_samples;
pub use error::{EditorError, EN_US};

/// The user's editor, run as a process.
pub struct ProcessEditor {
    fs: Arc<dyn Fs>,
    temp_root: PathBuf,
}

impl ProcessEditor {
    /// An editor whose temporary files are made below `temp_root`, on `fs`.
    pub fn new(fs: Arc<dyn Fs>, temp_root: &Path) -> ProcessEditor {
        ProcessEditor {
            fs,
            temp_root: temp_root.to_path_buf(),
        }
    }
}

impl ProcessEditor {
    /// The editor run on the file at `path` (a real path), the terminal its own.
    fn run(&self, command: &EditorCommand, path: &Path) -> Result<(), Failure> {
        let status = std::process::Command::new(&command.program)
            .args(&command.args)
            .arg(path)
            .status()
            .map_err(|e| -> Failure {
                Box::new(EditorError::Spawn {
                    program: command.program.clone(),
                    detail: e.to_string(),
                })
            })?;
        if status.success() {
            return Ok(());
        }
        Err(Box::new(EditorError::Failed {
            program: command.program.clone(),
            status: status.to_string(),
        }))
    }

    fn temp(&self, detail: &std::io::Error) -> Failure {
        Box::new(EditorError::Temp {
            detail: detail.to_string(),
        })
    }

    /// The round trip in a directory of its own.
    fn round_trip(
        &self,
        command: &EditorCommand,
        dir: &Path,
        file_name: &str,
        content: &str,
    ) -> Result<String, Failure> {
        let file = dir.join(file_name);
        self.fs
            .write_atomic(
                &file,
                content.as_bytes(),
                Perms::Exact(Mode::from_bits(0o600)),
            )
            .map_err(|e| self.temp(&e))?;
        let real = self.fs.real_path(&file).map_err(|e| self.temp(&e))?;
        self.run(command, &real)?;
        let bytes = self.fs.read(&file).map_err(|e| self.temp(&e))?;
        String::from_utf8(bytes).map_err(|_| -> Failure {
            Box::new(EditorError::Read {
                path: file.display().to_string(),
                detail: "it is not UTF-8".to_string(),
            })
        })
    }
}

impl Editor for ProcessEditor {
    fn edit(
        &self,
        command: &EditorCommand,
        file_name: &str,
        content: &str,
    ) -> Result<String, Failure> {
        if matches!(file_name, "" | "." | "..") || file_name.contains(['/', '\\', '\0']) {
            return Err(Box::new(EditorError::BadName {
                name: file_name.to_string(),
            }));
        }
        let dir = self
            .fs
            .make_temp_dir(&self.temp_root, "ctxloom-edit-")
            .map_err(|e| self.temp(&e))?;
        let out = self.round_trip(command, &dir, file_name, content);
        // The directory goes whatever came of the round trip.
        let _ = self.fs.remove_dir_all(&dir);
        out
    }

    fn open(&self, command: &EditorCommand, path: &str) -> Result<(), Failure> {
        let real = self
            .fs
            .real_path(Path::new(path))
            .unwrap_or_else(|_| PathBuf::from(path));
        self.run(command, &real)
    }
}

#[cfg(test)]
#[path = "lib.test.rs"]
mod tests;
