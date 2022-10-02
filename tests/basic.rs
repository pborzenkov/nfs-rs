mod support;
use support::*;

#[tokio::test]
async fn mount_umount() {
    let client = client().await;

    client.umount().await.expect("failed to umount");
}
