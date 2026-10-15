//! Local screenshot references for Bead descriptions.

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::{
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_IMAGE_BYTES: u64 = 10 * 1024 * 1024;
static NEXT_IMAGE: AtomicU64 = AtomicU64::new(0);

pub fn read_clipboard() -> io::Result<Vec<u8>> {
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    let x11 = std::env::var_os("DISPLAY").is_some();
    let mut commands = Vec::new();
    if wayland {
        commands.push(("wl-paste", vec!["--no-newline", "--type", "image/png"]));
        commands.push(("wl-paste", vec!["--no-newline", "--type", "image/jpeg"]));
    }
    if x11 {
        commands.push((
            "xclip",
            vec!["-selection", "clipboard", "-t", "image/png", "-out"],
        ));
        commands.push((
            "xclip",
            vec!["-selection", "clipboard", "-t", "image/jpeg", "-out"],
        ));
    }
    if commands.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "No Wayland or X11 clipboard session found",
        ));
    }
    for (program, args) in commands {
        let Ok(mut child) = Command::new(program)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        else {
            continue;
        };
        let mut bytes = Vec::new();
        let read = child
            .stdout
            .take()
            .expect("piped stdout")
            .take(MAX_IMAGE_BYTES + 1)
            .read_to_end(&mut bytes);
        if bytes.len() as u64 > MAX_IMAGE_BYTES {
            let _ = child.kill();
            let _ = child.wait();
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Clipboard image exceeds 10 MiB",
            ));
        }
        let status = child.wait()?;
        if read.is_ok() && status.success() && image_extension(&bytes).is_some() {
            return Ok(bytes);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "Clipboard has no PNG or JPEG image; copy a screenshot, install wl-clipboard or xclip for your session, and press Ctrl+V again",
    ))
}

fn image_extension(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        Some("jpg")
    } else {
        None
    }
}

pub fn save(bytes: &[u8]) -> io::Result<PathBuf> {
    let extension = image_extension(bytes).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Clipboard image is not PNG or JPEG",
        )
    })?;
    if bytes.len() as u64 > MAX_IMAGE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Clipboard image exceeds 10 MiB",
        ));
    }
    let output = Command::new("bd")
        .args(["--readonly", "where", "--json"])
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(
            "Could not find the shared Beads workspace",
        ));
    }
    let location: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let directory = PathBuf::from(location["path"].as_str().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Beads workspace path is missing",
        )
    })?);
    save_in(&directory, bytes, extension)
}

fn save_in(
    beads_directory: &std::path::Path,
    bytes: &[u8],
    extension: &str,
) -> io::Result<PathBuf> {
    let directory = beads_directory.join("screenshots");
    fs::create_dir_all(&directory)?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let name = format!(
        "ontrack-{}-{nonce}-{}.{}",
        std::process::id(),
        NEXT_IMAGE.fetch_add(1, Ordering::Relaxed),
        extension
    );
    let path = directory.join(name);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(&path)?;
    if let Err(error) = file.write_all(bytes) {
        let _ = fs::remove_file(&path);
        return Err(error);
    }
    Ok(path)
}

pub fn markdown_reference(path: &std::path::Path) -> String {
    format!("![Screenshot](<{}>)", path.display())
}

pub fn referenced_paths(description: &str) -> Vec<&str> {
    description
        .match_indices("![Screenshot](<")
        .filter_map(|(index, marker)| {
            description[index + marker.len()..]
                .split_once(">)")
                .map(|(path, _)| path)
        })
        .filter(|path| !path.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_image_data_and_references() {
        assert_eq!(image_extension(b"\x89PNG\r\n\x1a\nmore"), Some("png"));
        assert_eq!(image_extension(b"\xff\xd8\xffmore"), Some("jpg"));
        assert_eq!(image_extension(b"text"), None);
        let markdown = markdown_reference(std::path::Path::new("/tmp/a screenshot.png"));
        assert_eq!(referenced_paths(&markdown), vec!["/tmp/a screenshot.png"]);
    }

    #[test]
    fn saves_bytes_in_shared_beads_directory() {
        let root =
            std::env::temp_dir().join(format!("ontrack-screenshot-test-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let bytes = b"\x89PNG\r\n\x1a\nimage";
        let path = save_in(&root, bytes, "png").unwrap();
        assert_eq!(path.parent().unwrap(), root.join("screenshots"));
        assert_eq!(fs::read(&path).unwrap(), bytes);
        fs::remove_file(path).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
}
