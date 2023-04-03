use std::{
    fs, io,
    path::{Component, Path, PathBuf},
};

#[derive(Debug, thiserror::Error)]
pub enum DestPrepError {
    #[error("invalid destination path")]
    InvalidPath,
    #[error("failed to create destination dir")]
    CreateDir(#[source] io::Error),
    #[error("failed to canonicalize dest path")]
    Canonicalize(#[source] io::Error),
}

pub(crate) fn prepare_dest_parent(dest: &Path) -> Result<PathBuf, DestPrepError> {
    if !dest.exists() {
        fs::create_dir_all(dest).map_err(DestPrepError::CreateDir)?;
    }
    dest.canonicalize().map_err(DestPrepError::Canonicalize)
}
pub(crate) fn prepare_dest<P: AsRef<Path>>(dest: P) -> Result<PathBuf, DestPrepError> {
    let dest = dest.as_ref();
    prepare_dest_parent({
        let v = dest.parent().ok_or(DestPrepError::InvalidPath)?;
        if v.as_os_str().is_empty() {
            ".".as_ref()
        } else {
            v
        }
    })
    .and_then(|mut d| match dest.file_name() {
        Some(n) => {
            d.push(n);
            Ok(d)
        }
        None => Err(DestPrepError::InvalidPath),
    })
}

#[derive(Debug, thiserror::Error)]
pub enum LinkError {
    #[error("failed to create dir {}", dir.display())]
    CreateDir {
        dir: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("store path `{}` and destination `{}` has different prefix", store_path.display(), dest.display())]
    DifferentPrefix { store_path: PathBuf, dest: PathBuf },
    #[error("failed to create sym link from {} to {}", link.display(), link_source.display())]
    SymLink {
        link_source: PathBuf,
        link: PathBuf,
        #[source]
        source: io::Error,
    },
}

pub(crate) fn relative_path_to<P1, P2>(source: P1, path: P2) -> Option<PathBuf>
where
    P1: AsRef<Path>,
    P2: AsRef<Path>,
{
    let mut ret = PathBuf::new();
    let source = source.as_ref();
    let path = path.as_ref();
    let mut store_com = source.components().peekable();
    let mut dest_com = path.parent().unwrap().components().peekable();
    while store_com.peek() == dest_com.peek() {
        store_com.next();
        if dest_com.next().is_none() {
            break;
        }
    }
    for v in dest_com {
        match v {
            Component::Prefix(_) => return None,
            Component::Normal(_) => ret.push(".."),
            _ => unreachable!(),
        }
    }
    for v in store_com {
        match v {
            Component::Normal(d) => ret.push(d),
            _ => unreachable!(),
        }
    }
    log::debug!("relative path of {}: {}", path.display(), source.display());
    Some(ret)
}

fn symlink<P1: AsRef<Path>, P2: AsRef<Path>>(source: P1, link: P2) -> Result<(), io::Error> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(source, link)
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(source, link)
    }
}

pub(crate) fn link_to_dest(
    relative: bool,
    store_path: &Path,
    dest: &Path,
) -> Result<(), LinkError> {
    if store_path == dest {
        return Ok(());
    }
    let dest_parent = dest.parent().unwrap();
    if !dest_parent.exists() {
        fs::create_dir_all(dest_parent).map_err(|e| LinkError::CreateDir {
            dir: dest_parent.to_path_buf(),
            source: e,
        })?;
    }
    if !relative {
        return symlink(store_path, dest).map_err(|e| LinkError::SymLink {
            link_source: store_path.to_path_buf(),
            link: dest.to_path_buf(),
            source: e,
        });
    }
    let link_source =
        relative_path_to(store_path, dest).ok_or_else(|| LinkError::DifferentPrefix {
            store_path: store_path.to_path_buf(),
            dest: dest.to_path_buf(),
        })?;
    symlink(&link_source, dest).map_err(|e| LinkError::SymLink {
        link_source,
        link: dest.to_path_buf(),
        source: e,
    })
}
