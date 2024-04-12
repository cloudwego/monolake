use std::path::Path;

use monoio::buf::IoBufMut;

pub mod hash;
pub mod uri_serde;

pub async fn file_read(path: impl AsRef<Path>) -> std::io::Result<Vec<u8>> {
    // since monoio has not support statx, we have to use std
    let file_length = {
        let file = std::fs::File::open(&path)?;
        file.metadata().map(|meta| meta.len() as usize)?
    };

    let file = monoio::fs::File::open(path).await?;
    let buffer = unsafe { Vec::with_capacity(file_length).slice_mut_unchecked(0..file_length) };
    let (res, buf) = file.read_exact_at(buffer, 0).await;
    res?;
    Ok(buf.into_inner())
}

pub fn file_read_sync(path: impl AsRef<Path>) -> std::io::Result<Vec<u8>> {
    std::fs::read(path)
}
