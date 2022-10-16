use nfs;
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use std::env;

pub async fn client() -> nfs::Client {
    let srv = env::var("TEST_NFS_SERVER").expect("TEST_NFS_SERVER not set");

    nfs::Client::mount(srv)
        .await
        .expect("failed to mount NFS server")
}

pub fn rand_name() -> String {
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(30)
        .map(char::from)
        .collect()
}
