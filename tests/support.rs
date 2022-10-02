use nfs;
use std::env;

pub async fn client() -> nfs::Client {
    let srv = env::var("TEST_NFS_SERVER").expect("TEST_NFS_SERVER not set");

    nfs::Client::mount(srv)
        .await
        .expect("failed to mount NFS server")
}
