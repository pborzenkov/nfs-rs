mod support;
use support::*;

use nix::{fcntl::OFlag, sys::stat::Mode, unistd::AccessFlags};
use std::io::ErrorKind;

#[tokio::test]
async fn directories() {
    let client = client().await;

    let nonexistent_dir = rand_name();
    // Make sure `access` returns NotFound for non-existent directories
    let err = client
        .access(&nonexistent_dir)
        .await
        .expect_err("access() Ok for non-existent dir");
    assert_eq!(err.into_io().kind(), ErrorKind::NotFound);

    let dir = rand_name();
    let perms = Mode::from_bits_truncate(0o755);
    // Check if we can successfully create a directory
    client
        .mkdir(&dir, perms.clone())
        .await
        .expect("failed to create directory");

    // And make sure that we have specified access
    let flags = client
        .access(&dir)
        .await
        .expect("access() non-Ok for existing dir");
    assert!(
        flags.contains(AccessFlags::R_OK | AccessFlags::W_OK | AccessFlags::X_OK),
        "invalid permissions"
    );

    let st = client
        .stat(&dir)
        .await
        .expect("stat() non-Ok for existing dir");
    assert_eq!(
        Mode::from_bits_truncate(st.nfs_mode as u32),
        perms,
        "invalid mode"
    );

    // Check if we can remove the directory
    client
        .rmdir(&dir)
        .await
        .expect("failed to remove directory");

    // Make sure `access` returns NotFound after we removed the directory
    let err = client
        .access(&dir)
        .await
        .expect_err("access() Ok for non-existent dir");
    assert_eq!(err.into_io().kind(), ErrorKind::NotFound);

    client.umount().await.expect("failed to umount");
}

#[tokio::test]
async fn files() {
    let client = client().await;

    let name = rand_name();
    let perms = Mode::from_bits_truncate(0o644);
    // Check if we can successfully create a file
    let file = client
        .open(&name, OFlag::O_CREAT, perms.clone())
        .await
        .expect("failed to create file");

    // And make sure that we have specified access
    let flags = client
        .access(&name)
        .await
        .expect("access() non-Ok for existing file");
    assert!(
        flags.contains(AccessFlags::R_OK | AccessFlags::W_OK),
        "invalid permissions"
    );

    let st = file.stat().await.expect("stat() non-Ok for existing file");
    assert_eq!(
        Mode::from_bits_truncate(st.nfs_mode as u32),
        perms,
        "invalid mode"
    );

    drop(file);

    // Check if we can remove the directory
    client.unlink(&name).await.expect("failed to remove file");

    // Make sure `access` returns NotFound after we removed the file
    let err = client
        .access(&name)
        .await
        .expect_err("access() Ok for non-existent file");
    assert_eq!(err.into_io().kind(), ErrorKind::NotFound);

    client.umount().await.expect("failed to umount");
}
