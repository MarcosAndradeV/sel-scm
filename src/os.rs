use std::cell::RefCell;
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::diagnostics::SelError;
use crate::lexer::Loc;

/// Unified interface for interacting with the operating system and filesystem.
pub trait Os: std::fmt::Debug {
    // Filesystem Operations
    fn read_to_string(&self, path: &Path) -> io::Result<String>;
    fn write(&self, path: &Path, content: &str) -> io::Result<()>;
    fn read_dir(&self, path: &Path) -> io::Result<Vec<String>>;
    fn remove_file(&self, path: &Path) -> io::Result<()>;
    fn remove_dir_all(&self, path: &Path) -> io::Result<()>;
    fn remove(&self, path: &Path) -> io::Result<()> {
        if self.is_dir(path) {
            self.remove_dir_all(path)
        } else {
            self.remove_file(path)
        }
    }
    fn exists(&self, path: &Path) -> bool;
    fn is_file(&self, path: &Path) -> bool;
    fn is_dir(&self, path: &Path) -> bool;
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf>;
    fn current_dir(&self) -> io::Result<PathBuf>;

    // Process & Environment Operations
    fn get_env(&self, key: &str) -> Option<String>;
    fn set_env(&self, key: &str, val: &str);
    fn args(&self) -> Vec<String>;
    fn exit(&self, code: i32) -> !;
    fn sleep(&self, duration: Duration);
    fn time_now_ms(&self) -> Result<i64, String>;

    // Sandbox Support
    fn sandbox_root(&self) -> Option<&Path> {
        None
    }
    fn check_sandbox(&self, path: &Path, loc: Loc) -> Result<PathBuf, SelError>;
}

/// Standard operating system implementation wrapping `std::fs`, `std::env`, and `std::process`.
#[derive(Debug, Default, Clone, Copy)]
pub struct RealOs;

impl Os for RealOs {
    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        std::fs::read_to_string(path)
    }

    fn write(&self, path: &Path, content: &str) -> io::Result<()> {
        std::fs::write(path, content)
    }

    fn read_dir(&self, path: &Path) -> io::Result<Vec<String>> {
        let entries = std::fs::read_dir(path)?;
        let mut result = Vec::new();
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                result.push(name.to_string());
            }
        }
        Ok(result)
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        std::fs::remove_file(path)
    }

    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        std::fs::remove_dir_all(path)
    }

    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn is_file(&self, path: &Path) -> bool {
        path.is_file()
    }

    fn is_dir(&self, path: &Path) -> bool {
        path.is_dir()
    }

    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        path.canonicalize()
    }

    fn current_dir(&self) -> io::Result<PathBuf> {
        std::env::current_dir()
    }

    fn get_env(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }

    fn set_env(&self, key: &str, val: &str) {
        unsafe {
            std::env::set_var(key, val);
        }
    }

    fn args(&self) -> Vec<String> {
        std::env::args().collect()
    }

    fn exit(&self, code: i32) -> ! {
        std::process::exit(code)
    }

    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }

    fn time_now_ms(&self) -> Result<i64, String> {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .map_err(|e| e.to_string())
    }

    fn check_sandbox(&self, path: &Path, loc: Loc) -> Result<PathBuf, SelError> {
        self.canonicalize(path).map_err(|e| {
            SelError::Runtime(
                loc,
                format!("Failed to resolve path {}: {}", path.display(), e),
            )
        })
    }
}

/// Sandboxed OS wrapper that validates filesystem paths against a root boundary.
#[derive(Debug, Clone)]
pub struct SandboxedOs<O: Os = RealOs> {
    inner: O,
    sandbox_root: PathBuf,
}

impl<O: Os> SandboxedOs<O> {
    pub fn new(root: PathBuf, inner: O) -> io::Result<Self> {
        let canonical_root = inner.canonicalize(&root)?;
        Ok(Self {
            inner,
            sandbox_root: canonical_root,
        })
    }
}

impl<O: Os> Os for SandboxedOs<O> {
    fn sandbox_root(&self) -> Option<&Path> {
        Some(&self.sandbox_root)
    }

    fn check_sandbox(&self, path: &Path, loc: Loc) -> Result<PathBuf, SelError> {
        check_path_sandbox(path, &self.sandbox_root, loc)
    }

    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        self.check_sandbox(path, Loc::default())
            .map_err(|e| io::Error::new(io::ErrorKind::PermissionDenied, e.to_string()))?;
        self.inner.read_to_string(path)
    }

    fn write(&self, path: &Path, content: &str) -> io::Result<()> {
        if path.exists() {
            self.check_sandbox(path, Loc::default())
                .map_err(|e| io::Error::new(io::ErrorKind::PermissionDenied, e.to_string()))?;
        } else if let Some(parent) = path.parent() {
            let parent_to_check = if parent.as_os_str().is_empty() {
                Path::new(".")
            } else {
                parent
            };
            self.check_sandbox(parent_to_check, Loc::default())
                .map_err(|e| io::Error::new(io::ErrorKind::PermissionDenied, e.to_string()))?;
        }
        self.inner.write(path, content)
    }

    fn read_dir(&self, path: &Path) -> io::Result<Vec<String>> {
        self.check_sandbox(path, Loc::default())
            .map_err(|e| io::Error::new(io::ErrorKind::PermissionDenied, e.to_string()))?;
        self.inner.read_dir(path)
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        self.check_sandbox(path, Loc::default())
            .map_err(|e| io::Error::new(io::ErrorKind::PermissionDenied, e.to_string()))?;
        self.inner.remove_file(path)
    }

    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        self.check_sandbox(path, Loc::default())
            .map_err(|e| io::Error::new(io::ErrorKind::PermissionDenied, e.to_string()))?;
        self.inner.remove_dir_all(path)
    }

    fn exists(&self, path: &Path) -> bool {
        if self.check_sandbox(path, Loc::default()).is_err() {
            return false;
        }
        self.inner.exists(path)
    }

    fn is_file(&self, path: &Path) -> bool {
        if self.check_sandbox(path, Loc::default()).is_err() {
            return false;
        }
        self.inner.is_file(path)
    }

    fn is_dir(&self, path: &Path) -> bool {
        if self.check_sandbox(path, Loc::default()).is_err() {
            return false;
        }
        self.inner.is_dir(path)
    }

    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        let canon = self.inner.canonicalize(path)?;
        if !canon.starts_with(&self.sandbox_root) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!(
                    "Sandbox violation: path {} is outside root {}",
                    canon.display(),
                    self.sandbox_root.display()
                ),
            ));
        }
        Ok(canon)
    }

    fn current_dir(&self) -> io::Result<PathBuf> {
        self.inner.current_dir()
    }

    fn get_env(&self, key: &str) -> Option<String> {
        self.inner.get_env(key)
    }

    fn set_env(&self, key: &str, val: &str) {
        self.inner.set_env(key, val)
    }

    fn args(&self) -> Vec<String> {
        self.inner.args()
    }

    fn exit(&self, code: i32) -> ! {
        self.inner.exit(code)
    }

    fn sleep(&self, duration: Duration) {
        self.inner.sleep(duration)
    }

    fn time_now_ms(&self) -> Result<i64, String> {
        self.inner.time_now_ms()
    }
}

/// Validates that `path` is resolved within `root`.
pub fn check_path_sandbox(path: &Path, root: &Path, loc: Loc) -> Result<PathBuf, SelError> {
    let canonical_path = path.canonicalize().map_err(|e| {
        SelError::SandboxViolation(
            loc,
            format!("Failed to resolve path {}: {}", path.display(), e),
        )
    })?;
    let canonical_root = root.canonicalize().map_err(|e| {
        SelError::SandboxViolation(
            loc,
            format!("Failed to resolve sandbox root {}: {}", root.display(), e),
        )
    })?;
    if !canonical_path.starts_with(&canonical_root) {
        return Err(SelError::SandboxViolation(
            loc,
            format!(
                "Sandbox violation: path {} is outside root {}",
                canonical_path.display(),
                canonical_root.display()
            ),
        ));
    }
    Ok(canonical_path)
}

thread_local! {
    static CURRENT_OS: RefCell<Option<Rc<dyn Os>>> = const { RefCell::new(None) };
}

pub struct OsGuard {
    prev: Option<Rc<dyn Os>>,
}

impl Drop for OsGuard {
    fn drop(&mut self) {
        CURRENT_OS.with(|cell| {
            *cell.borrow_mut() = self.prev.take();
        });
    }
}

pub fn set_current_os(os: Rc<dyn Os>) -> OsGuard {
    CURRENT_OS.with(|cell| {
        let prev = cell.borrow_mut().replace(os);
        OsGuard { prev }
    })
}

pub fn set_sandbox(root: PathBuf) -> Result<OsGuard, SelError> {
    let sandboxed = SandboxedOs::new(root.clone(), RealOs).map_err(|e| {
        SelError::SandboxViolation(
            Loc::default(),
            format!("Failed to resolve sandbox root {}: {}", root.display(), e),
        )
    })?;
    Ok(set_current_os(Rc::new(sandboxed)))
}

pub fn with_os<T>(os: Rc<dyn Os>, f: impl FnOnce() -> T) -> T {
    let _guard = set_current_os(os);
    f()
}

pub fn with_sandbox<T>(root: PathBuf, f: impl FnOnce() -> T) -> Result<T, SelError> {
    let _guard = set_sandbox(root)?;
    Ok(f())
}

#[inline]
pub fn with_current<T>(f: impl FnOnce(&dyn Os) -> T) -> T {
    CURRENT_OS.with(|cell| {
        if let Some(ref os) = *cell.borrow() {
            f(os.as_ref())
        } else {
            f(&RealOs)
        }
    })
}

// Global convenience accessors delegating to current OS
#[inline]
pub fn read_to_string<P: AsRef<Path>>(path: P) -> io::Result<String> {
    with_current(|os| os.read_to_string(path.as_ref()))
}

#[inline]
pub fn write<P: AsRef<Path>>(path: P, content: &str) -> io::Result<()> {
    with_current(|os| os.write(path.as_ref(), content))
}

#[inline]
pub fn read_dir<P: AsRef<Path>>(path: P) -> io::Result<Vec<String>> {
    with_current(|os| os.read_dir(path.as_ref()))
}

#[inline]
pub fn remove_file<P: AsRef<Path>>(path: P) -> io::Result<()> {
    with_current(|os| os.remove_file(path.as_ref()))
}

#[inline]
pub fn remove_dir_all<P: AsRef<Path>>(path: P) -> io::Result<()> {
    with_current(|os| os.remove_dir_all(path.as_ref()))
}

#[inline]
pub fn remove<P: AsRef<Path>>(path: P) -> io::Result<()> {
    with_current(|os| os.remove(path.as_ref()))
}

#[inline]
pub fn exists<P: AsRef<Path>>(path: P) -> bool {
    with_current(|os| os.exists(path.as_ref()))
}

#[inline]
pub fn is_file<P: AsRef<Path>>(path: P) -> bool {
    with_current(|os| os.is_file(path.as_ref()))
}

#[inline]
pub fn is_dir<P: AsRef<Path>>(path: P) -> bool {
    with_current(|os| os.is_dir(path.as_ref()))
}

#[inline]
pub fn canonicalize<P: AsRef<Path>>(path: P) -> io::Result<PathBuf> {
    with_current(|os| os.canonicalize(path.as_ref()))
}

#[inline]
pub fn current_dir() -> io::Result<PathBuf> {
    with_current(|os| os.current_dir())
}

#[inline]
pub fn get_env(key: &str) -> Option<String> {
    with_current(|os| os.get_env(key))
}

#[inline]
pub fn set_env(key: &str, val: &str) {
    with_current(|os| os.set_env(key, val))
}

#[inline]
pub fn args() -> Vec<String> {
    with_current(|os| os.args())
}

#[inline]
pub fn exit(code: i32) -> ! {
    with_current(|os| os.exit(code))
}

#[inline]
pub fn sleep(duration: Duration) {
    with_current(|os| os.sleep(duration))
}

#[inline]
pub fn time_now_ms() -> Result<i64, String> {
    with_current(|os| os.time_now_ms())
}

#[inline]
pub fn check_sandbox<P: AsRef<Path>>(path: P, loc: Loc) -> Result<PathBuf, SelError> {
    with_current(|os| os.check_sandbox(path.as_ref(), loc))
}
