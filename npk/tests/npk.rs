// Copyright (c) 2020 ESRLabs
//
//   Licensed under the Apache License, Version 2.0 (the "License");
//   you may not use this file except in compliance with the License.
//   You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
//   Unless required by applicable law or agreed to in writing, software
//   distributed under the License is distributed on an "AS IS" BASIS,
//   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//   See the License for the specific language governing permissions and
//   limitations under the License.

#[cfg(test)]
mod npk {
    use npk::npk::{gen_key, pack, unpack};
    use std::{
        fs::File,
        io::Write,
        path::{Path, PathBuf},
    };

    const TEST_KEY_NAME: &str = "test_key";
    const TEST_MANIFEST: &str = "name: hello
version: 0.0.2
init: /hello
env:
  HELLO: north";
    const TEST_MANIFEST_UNPACKED: &str = "---
name: hello
version: 0.0.2
init: /hello
env:
  HELLO: north";

    async fn create_test_npk(dest: &Path) -> PathBuf {
        let src = create_tmp_dir().await;
        let key_dir = create_tmp_dir().await;
        create_test_manifest(&src).await;
        let (_pub_key, prv_key) = gen_test_key(&key_dir).await;
        pack(&src, &dest, Some(prv_key.as_path()))
            .await
            .expect("Pack NPK");
        dest.join("hello-0.0.2.npk")
    }

    async fn create_test_manifest(src: &PathBuf) -> PathBuf {
        let manifest = src.join("manifest").with_extension("yaml");
        File::create(&manifest)
            .expect("Create manifest.yaml")
            .write_all(TEST_MANIFEST.as_ref())
            .expect("Write test manifest");
        manifest
    }

    async fn create_tmp_dir() -> PathBuf {
        tempfile::TempDir::new()
            .expect("Create tmp dir")
            .into_path()
    }

    async fn gen_test_key(key_dir: &Path) -> (PathBuf, PathBuf) {
        gen_key(&TEST_KEY_NAME, &key_dir)
            .await
            .expect("Generate key pair");
        let prv_key = key_dir.join(&TEST_KEY_NAME).with_extension("key");
        let pub_key = key_dir.join(&TEST_KEY_NAME).with_extension("pub");
        assert!(prv_key.exists());
        assert!(pub_key.exists());
        (pub_key, prv_key)
    }

    #[tokio::test]
    async fn pack_npk() {
        create_test_npk(&create_tmp_dir().await).await;
    }

    #[tokio::test]
    async fn pack_npk_no_manifest() {
        let key_dir = create_tmp_dir().await;
        let (_pub_key, prv_key) = gen_test_key(&key_dir).await;
        pack(
            Path::new("invalid"),
            &create_tmp_dir().await,
            Some(prv_key.as_path()),
        )
        .await
        .expect_err("Invalid manifest");
    }

    #[tokio::test]
    async fn pack_npk_no_dest() {
        let src = create_tmp_dir().await;
        let key_dir = create_tmp_dir().await;
        create_test_manifest(&src).await;
        let (_pub_key, prv_key) = gen_test_key(&key_dir).await;
        pack(&src, &Path::new("invalid"), Some(prv_key.as_path()))
            .await
            .expect_err("Invalid destination dir");
    }

    #[tokio::test]
    async fn pack_npk_no_keys() {
        let src = create_tmp_dir().await;
        create_test_manifest(&src).await;
        pack(&src, &create_tmp_dir().await, Some(Path::new("invalid")))
            .await
            .expect_err("Invalid key dir");
    }

    #[tokio::test]
    async fn unpack_npk() {
        let npk = create_test_npk(&create_tmp_dir().await).await;
        assert!(npk.exists());
        let unpack_dest = create_tmp_dir().await;
        unpack(&npk, &unpack_dest).await.expect("Unpack NPK");
        let manifest = unpack_dest.join("manifest").with_extension("yaml");
        assert!(manifest.exists());
        let manifest = std::fs::read_to_string(&manifest).expect("Failed to parse manifest");

        assert_eq!(TEST_MANIFEST_UNPACKED, manifest);
    }

    #[tokio::test]
    async fn gen_key_pair() {
        gen_test_key(&create_tmp_dir().await).await;
    }

    #[tokio::test]
    async fn gen_key_pair_no_dest() {
        gen_key(&TEST_KEY_NAME, &Path::new("invalid"))
            .await
            .expect_err("Invalid key dir");
    }

    #[tokio::test]
    async fn do_not_overwrite_keys() -> Result<(), anyhow::Error> {
        let tmp = create_tmp_dir().await;
        gen_key(&TEST_KEY_NAME, &tmp).await.expect("Generate keys");
        gen_key(&TEST_KEY_NAME, &tmp)
            .await
            .expect_err("Cannot overwrite keys");
        Ok(())
    }
}
