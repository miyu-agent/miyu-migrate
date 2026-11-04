//! 打包、核对（MG4、MG5，细则第十三节第 2 条）：旧根整个打成一个 `tar.zst`，边打边记每个文件的 SHA-256；再解到一个空目录，
//! 照记下的逐个核对。迁移只从解出来的副本读，原目录不动。
//!
//! 只打普通文件、目录、符号链接（链接照原样存成链接，不跟进去）；别的（套接字、管道、设备）不打，记进清单的 `skipped`。路径
//! 一律用 `/` 分隔、相对旧根；不是 UTF-8 的路径照原样打进包、清单里记它的有损写法和 `lossy`。

use std::fmt;
use std::fs::{self, File};
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

/// 压缩级别：zstd 的默认（3），1.6 GB 的旧根一两分钟打完。
const LEVEL: i32 = 3;

/// 包里的一样东西。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// 相对旧根的路径，`/` 分隔。
    pub path: String,
    /// `file`、`dir`、`symlink`。
    pub kind: String,
    /// 文件的字节数。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    /// 文件内容的 SHA-256，小写十六进制。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    /// 链接指向哪。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// 路径不是 UTF-8，`path` 是有损的写法。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub lossy: bool,
}

/// 打包的清单：每样东西，和没打进去的。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Manifest {
    /// 打进去的，照路径排好。
    pub entries: Vec<Entry>,
    /// 没打进去的（套接字、管道、设备），相对路径。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped: Vec<String>,
}

impl Manifest {
    /// 文件有几个、一共多少字节。
    pub fn totals(&self) -> (u64, u64) {
        self.entries
            .iter()
            .filter(|entry| entry.kind == "file")
            .fold((0, 0), |(n, bytes), entry| {
                (n + 1, bytes + entry.bytes.unwrap_or(0))
            })
    }
}

/// 打包、核对出的错。
#[derive(Debug)]
pub enum ArchiveError {
    /// 读写出错，带是哪个文件。
    Io(PathBuf, io::Error),
    /// 解出来的和打进去的对不上：哪几个（相对路径）、怎么不对。
    Mismatch(Vec<(String, String)>),
}

impl fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArchiveError::Io(path, error) => write!(f, "{}: {error}", path.display()),
            ArchiveError::Mismatch(bad) => {
                write!(f, "{} file(s) differ after unpacking", bad.len())?;
                for (path, why) in bad.iter().take(10) {
                    write!(f, "\n  {path}: {why}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for ArchiveError {}

fn io_at(path: &Path) -> impl Fn(io::Error) -> ArchiveError + '_ {
    move |error| ArchiveError::Io(path.to_path_buf(), error)
}

/// 把 `root` 打成 `out`（`tar.zst`），交回清单。先写到 `out` 旁边的临时文件，打完再改名。
///
/// # Errors
///
/// 读旧根、写包出错。
pub fn pack(root: &Path, out: &Path) -> Result<Manifest, ArchiveError> {
    let partial = out.with_extension("partial");
    let file = File::create(&partial).map_err(io_at(&partial))?;
    let encoder = zstd::Encoder::new(BufWriter::new(file), LEVEL).map_err(io_at(&partial))?;
    let mut builder = tar::Builder::new(encoder);
    builder.follow_symlinks(false);
    let mut manifest = Manifest::default();
    let walk = WalkDir::new(root)
        .follow_links(false)
        .sort_by_file_name()
        .min_depth(1);
    for item in walk {
        let item = item.map_err(|error| {
            let path = error.path().unwrap_or(root).to_path_buf();
            ArchiveError::Io(path, io::Error::other(error.to_string()))
        })?;
        let path = item.path();
        let relative = path.strip_prefix(root).unwrap_or(path);
        let (name, lossy) = slashed(relative);
        let kind = item.file_type();
        if kind.is_dir() {
            builder.append_dir(relative, path).map_err(io_at(path))?;
            manifest.entries.push(entry(name, "dir", lossy));
        } else if kind.is_symlink() {
            let target = fs::read_link(path).map_err(io_at(path))?;
            builder
                .append_path_with_name(path, relative)
                .map_err(io_at(path))?;
            let mut link = entry(name, "symlink", lossy);
            link.target = Some(target.to_string_lossy().into_owned());
            manifest.entries.push(link);
        } else if kind.is_file() {
            let (bytes, sha256) = append_file(&mut builder, path, relative)?;
            let mut file = entry(name, "file", lossy);
            file.bytes = Some(bytes);
            file.sha256 = Some(sha256);
            manifest.entries.push(file);
        } else {
            manifest.skipped.push(name);
        }
    }
    let encoder = builder.into_inner().map_err(io_at(&partial))?;
    let mut writer = encoder.finish().map_err(io_at(&partial))?;
    writer.flush().map_err(io_at(&partial))?;
    let file = writer
        .into_inner()
        .map_err(|error| ArchiveError::Io(partial.clone(), error.into_error()))?;
    file.sync_all().map_err(io_at(&partial))?;
    fs::rename(&partial, out).map_err(io_at(out))?;
    Ok(manifest)
}

/// 一个普通文件：边读进包边算哈希，读一遍。
fn append_file<W: Write>(
    builder: &mut tar::Builder<W>,
    path: &Path,
    relative: &Path,
) -> Result<(u64, String), ArchiveError> {
    let file = File::open(path).map_err(io_at(path))?;
    let meta = file.metadata().map_err(io_at(path))?;
    let mut header = tar::Header::new_gnu();
    header.set_metadata(&meta);
    let mut reader = Hashing {
        inner: file,
        hasher: Sha256::new(),
        bytes: 0,
    };
    builder
        .append_data(&mut header, relative, &mut reader)
        .map_err(io_at(path))?;
    Ok((reader.bytes, hex(&reader.hasher.finalize())))
}

/// 读的同时算哈希、数字节。
struct Hashing<R> {
    inner: R,
    hasher: Sha256,
    bytes: u64,
}

impl<R: Read> Read for Hashing<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.hasher.update(&buf[..n]);
        self.bytes += n as u64;
        Ok(n)
    }
}

/// 把 `archive` 解到 `dest`（要是空的或者不存在），照 `manifest` 逐个核对文件的大小、哈希，链接的指向。
///
/// # Errors
///
/// 解包出错；有对不上的交回 [`ArchiveError::Mismatch`]，列出是哪几个。
pub fn unpack_and_check(
    archive: &Path,
    manifest: &Manifest,
    dest: &Path,
) -> Result<(), ArchiveError> {
    fs::create_dir_all(dest).map_err(io_at(dest))?;
    let file = File::open(archive).map_err(io_at(archive))?;
    let decoder = zstd::Decoder::new(file).map_err(io_at(archive))?;
    let mut unpacker = tar::Archive::new(decoder);
    unpacker.set_preserve_permissions(true);
    unpacker.unpack(dest).map_err(io_at(dest))?;
    let mut bad = Vec::new();
    for entry in &manifest.entries {
        if entry.lossy {
            continue;
        }
        let path = dest.join(&entry.path);
        match entry.kind.as_str() {
            "file" => match hash_file(&path) {
                Ok((bytes, sha))
                    if Some(bytes) == entry.bytes && Some(&sha) == entry.sha256.as_ref() => {}
                Ok(_) => bad.push((entry.path.clone(), "content differs".to_string())),
                Err(error) => bad.push((entry.path.clone(), error.to_string())),
            },
            "symlink" => match fs::read_link(&path) {
                Ok(target)
                    if Some(target.to_string_lossy().as_ref()) == entry.target.as_deref() => {}
                Ok(_) => bad.push((entry.path.clone(), "link target differs".to_string())),
                Err(error) => bad.push((entry.path.clone(), error.to_string())),
            },
            _ => {
                if !path.is_dir() {
                    bad.push((entry.path.clone(), "directory missing".to_string()));
                }
            }
        }
    }
    match bad.is_empty() {
        true => Ok(()),
        false => Err(ArchiveError::Mismatch(bad)),
    }
}

/// 一个文件的字节数和 SHA-256。
///
/// # Errors
///
/// 读不了。
pub fn hash_file(path: &Path) -> io::Result<(u64, String)> {
    let mut reader = Hashing {
        inner: File::open(path)?,
        hasher: Sha256::new(),
        bytes: 0,
    };
    io::copy(&mut reader, &mut io::sink())?;
    Ok((reader.bytes, hex(&reader.hasher.finalize())))
}

fn entry(path: String, kind: &str, lossy: bool) -> Entry {
    Entry {
        path,
        kind: kind.to_string(),
        bytes: None,
        sha256: None,
        target: None,
        lossy,
    }
}

/// 相对路径写成 `/` 分隔，交回写法和是不是有损。
fn slashed(relative: &Path) -> (String, bool) {
    let mut lossy = false;
    let parts: Vec<String> = relative
        .components()
        .map(|part| {
            let text = part.as_os_str();
            match text.to_str() {
                Some(text) => text.to_string(),
                None => {
                    lossy = true;
                    text.to_string_lossy().into_owned()
                }
            }
        })
        .collect();
    (parts.join("/"), lossy)
}

/// 小写十六进制。
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests;
