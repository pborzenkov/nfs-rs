mod support;
use support::*;

use nix::{fcntl::OFlag, sys::stat::Mode};
use tokio::io::{copy, AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn file_io() {
    const DATA_LEN: usize = 1024 * 1024;

    let client = client().await;

    let name = rand_name();
    let perms = Mode::from_bits_truncate(0o644);
    // Check if we can successfully create a file
    let mut wfile = client
        .open(&name, OFlag::O_CREAT | OFlag::O_WRONLY, perms.clone())
        .await
        .expect("failed to create file");

    let wdata = (0..DATA_LEN).map(|i| (i % 256) as u8).collect::<Vec<_>>();
    let mut wdata = wdata.as_ref();

    let written = copy(&mut wdata, &mut wfile)
        .await
        .expect("failed to write data");
    assert_eq!(DATA_LEN, written as usize);

    wfile.flush().await.expect("failed to flush data");
    drop(wfile);

    let mut rfile = client
        .open(&name, OFlag::O_RDONLY, perms.clone())
        .await
        .expect("failed to open file");

    let mut rdata = Vec::new();
    let read = rfile
        .read_to_end(&mut rdata)
        .await
        .expect("failed to read data");
    assert_eq!(DATA_LEN, read);
    assert_eq!(DATA_LEN, rdata.len());

    for (&w, r) in wdata.into_iter().zip(rdata) {
        assert_eq!(w, r);
    }
    drop(rfile);

    client.umount().await.expect("failed to umount");
}
