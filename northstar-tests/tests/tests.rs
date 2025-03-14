use std::path::{Path, PathBuf};

use anyhow::Result;
use log::debug;
use northstar_runtime::api::{
    self,
    model::{self, ExitStatus, Notification},
};
use northstar_tests::{containers::*, logger::assume, runtime::client, runtime_test};

// Test a good and bad log assumption
#[runtime_test]
async fn logger_smoketest() -> Result<()> {
    debug!("Yippie");
    assume("Yippie", 3).await?;
    assert!(assume("Juhuuu!", 1).await.is_err());
    Ok(())
}

// Install and uninstall is a loop. After a number of installation
// try to start the test container
#[runtime_test]
async fn install_uninstall_test_container() -> Result<()> {
    for _ in 0u32..10 {
        client().install_test_container().await?;
        client().uninstall_test_container().await?;
    }
    Ok(())
}

// Install a container that already exists with the same name and version
#[runtime_test]
async fn install_duplicate() -> Result<()> {
    client().install_test_container().await?;
    client().install_test_resource().await?;
    assert!(client().install_test_container().await.is_err());
    Ok(())
}

// Install a container that already exists in another repository
#[runtime_test]
async fn install_duplicate_other_repository() -> Result<()> {
    client().install(TEST_CONTAINER_NPK, "mem").await?;
    assert!(client().install(TEST_CONTAINER_NPK, "fs").await.is_err());
    Ok(())
}

// Try to a container into a repository that does not exist
#[runtime_test]
async fn install_invalid_repository() -> Result<()> {
    let client: &mut northstar_client::Client<_> = &mut *client();
    let size = TEST_CONTAINER_NPK.len() as u64;
    match client.install(TEST_CONTAINER_NPK, size, "whooha").await {
        Err(northstar_client::error::RequestError::Runtime(model::Error::InvalidRepository {
            ..
        })) => Ok(()),
        e => panic!("Unexpected response: {:?}", e),
    }
}

// Install a container to the file system backed repository
#[runtime_test]
async fn install_uninstall_to_fs_repository() -> Result<()> {
    client().install_test_resource().await?;
    for _ in 0u32..5 {
        client().install(TEST_CONTAINER_NPK, "fs").await?;
        client().start_with_args(TEST_CONTAINER, ["sleep"]).await?;
        assume("Sleeping", 5u64).await?;
        client().stop(TEST_CONTAINER, 5).await?;
        assume("Process test-container:0.0.1 exited", 5).await?;
        client().uninstall_test_container().await?;
    }
    Ok(())
}

// Uninstalling an unknown container should fail
#[runtime_test]
async fn uninstall_unknown_container() -> Result<()> {
    assert!(client().uninstall("fckptn:0.0.1", false).await.is_err());
    Ok(())
}

// Start and stop a container multiple times
#[runtime_test]
async fn start_stop() -> Result<()> {
    client().install_test_container().await?;
    client().install_test_resource().await?;

    for _ in 0..10u32 {
        client().start_with_args(TEST_CONTAINER, ["sleep"]).await?;
        assume("Sleeping", 5u64).await?;
        client().stop(TEST_CONTAINER, 5).await?;
        assume("Process test-container:0.0.1 exited", 5).await?;
    }
    Ok(())
}

// Install and uninstall the example npks
#[runtime_test]
async fn install_uninstall_examples() -> Result<()> {
    client().install(EXAMPLE_CPUEATER_NPK, "mem").await?;
    client().install(EXAMPLE_CONSOLE_NPK, "mem").await?;
    client().install(EXAMPLE_CRASHING_NPK, "mem").await?;
    client().install(EXAMPLE_FERRIS_NPK, "mem").await?;
    client().install(EXAMPLE_HELLO_FERRIS_NPK, "mem").await?;
    client().install(EXAMPLE_HELLO_RESOURCE_NPK, "mem").await?;
    client().install(EXAMPLE_INSPECT_NPK, "mem").await?;
    client().install(EXAMPLE_MEMEATER_NPK, "mem").await?;
    client().install(EXAMPLE_MESSAGE_0_0_1_NPK, "mem").await?;
    client().install(EXAMPLE_MESSAGE_0_0_2_NPK, "mem").await?;
    client().install(EXAMPLE_PERSISTENCE_NPK, "mem").await?;
    client().install(EXAMPLE_SECCOMP_NPK, "mem").await?;
    client().install(EXAMPLE_TOKEN_CLIENT_NPK, "mem").await?;
    client().install(EXAMPLE_TOKEN_SERVER_NPK, "mem").await?;
    client().install(TEST_CONTAINER_NPK, "mem").await?;
    client().install(TEST_RESOURCE_NPK, "mem").await?;

    client().uninstall(EXAMPLE_CPUEATER, false).await?;
    client().uninstall(EXAMPLE_CONSOLE, false).await?;
    client().uninstall(EXAMPLE_CRASHING, false).await?;
    client().uninstall(EXAMPLE_FERRIS, false).await?;
    client().uninstall(EXAMPLE_HELLO_FERRIS, false).await?;
    client().uninstall(EXAMPLE_HELLO_RESOURCE, false).await?;
    client().uninstall(EXAMPLE_INSPECT, false).await?;
    client().uninstall(EXAMPLE_MEMEATER, false).await?;
    client().uninstall(EXAMPLE_MESSAGE_0_0_1, false).await?;
    client().uninstall(EXAMPLE_MESSAGE_0_0_2, false).await?;
    client().uninstall(EXAMPLE_PERSISTENCE, false).await?;
    client().uninstall(EXAMPLE_SECCOMP, false).await?;
    client().uninstall(EXAMPLE_TOKEN_CLIENT, false).await?;
    client().uninstall(EXAMPLE_TOKEN_SERVER, false).await?;
    client().uninstall(TEST_CONTAINER, false).await?;
    client().uninstall(TEST_RESOURCE, false).await?;
    Ok(())
}

// Mount and umount all containers known to the client()
#[runtime_test]
async fn mount_umount() -> Result<()> {
    client().install(EXAMPLE_CPUEATER_NPK, "mem").await?;
    client().install(EXAMPLE_CONSOLE_NPK, "mem").await?;
    client().install(EXAMPLE_CRASHING_NPK, "mem").await?;
    client().install(EXAMPLE_FERRIS_NPK, "mem").await?;
    client().install(EXAMPLE_HELLO_FERRIS_NPK, "mem").await?;
    client().install(EXAMPLE_HELLO_RESOURCE_NPK, "mem").await?;
    client().install(EXAMPLE_INSPECT_NPK, "mem").await?;
    client().install(EXAMPLE_MEMEATER_NPK, "mem").await?;
    client().install(EXAMPLE_MESSAGE_0_0_1_NPK, "mem").await?;
    client().install(EXAMPLE_MESSAGE_0_0_2_NPK, "mem").await?;
    client().install(EXAMPLE_PERSISTENCE_NPK, "mem").await?;
    client().install(EXAMPLE_SECCOMP_NPK, "mem").await?;
    client().install(EXAMPLE_TOKEN_CLIENT_NPK, "mem").await?;
    client().install(EXAMPLE_TOKEN_SERVER_NPK, "mem").await?;
    client().install(TEST_CONTAINER_NPK, "mem").await?;
    client().install(TEST_RESOURCE_NPK, "mem").await?;

    let containers = client().list().await?;
    client().mount_all(&containers).await?;

    client().umount_all(containers).await?;
    Ok(())
}

// Try to mount a unknown container
#[runtime_test]
async fn try_to_mount_unknown_container() -> Result<()> {
    let container = "foo:0.0.1";
    let result = client().mount(container).await?;
    let container = api::model::Container::try_from(container)?;
    let error = api::model::Error::InvalidContainer {
        container: container.clone(),
    };
    assert_eq!(result, api::model::MountResult::Error { container, error });
    Ok(())
}

// Try to mount a known and unknown container
#[runtime_test]
async fn try_to_mount_known_and_unknown_container() -> Result<()> {
    client().install(TEST_RESOURCE_NPK, "mem").await?;
    let unknown = "foo:0.0.1";
    let result = client().mount_all([TEST_RESOURCE, unknown]).await?;
    assert!(result.len() == 2);

    // Check that a mount error for the unknown container is in the result
    let container = api::model::Container::try_from(unknown)?;
    let error = api::model::Error::InvalidContainer {
        container: container.clone(),
    };
    let error = api::model::MountResult::Error { container, error };

    assert!(result.contains(&error));
    assert!(result.contains(&api::model::MountResult::Ok {
        container: api::model::Container::try_from(TEST_RESOURCE)?
    }));

    Ok(())
}

// Try to mount a unknown container
#[runtime_test]
async fn try_to_umount_used_resource() -> Result<()> {
    client().install_test_container().await?;
    client().install_test_resource().await?;

    // Start the test container. It uses the resource and the umount
    // of test-resource should fail with a busy error.
    client().start_with_args(TEST_CONTAINER, ["sleep"]).await?;
    assume("Sleeping...", 5u64).await?;
    let result = client().umount(TEST_RESOURCE).await?;
    let container: api::model::Container = TEST_CONTAINER.try_into().unwrap();
    let resource: api::model::Container = TEST_RESOURCE.try_into().unwrap();
    let error = api::model::Error::UmountBusy { container };
    let expected_result = api::model::UmountResult::Error {
        container: resource.clone(),
        error,
    };
    assert_eq!(result, expected_result);

    // Stop the test container and try to umount again
    client().stop(TEST_CONTAINER, 5).await?;
    let result = client().umount(TEST_RESOURCE).await?;
    let expected_result = api::model::UmountResult::Ok {
        container: resource,
    };
    assert_eq!(result, expected_result);

    Ok(())
}

// Try to stop a not started container and expect an Err
#[runtime_test]
async fn try_to_stop_unknown_container() -> Result<()> {
    let container = "foo:0.0.1:default";
    assert!(client().stop(container, 5).await.is_err());
    Ok(())
}

// Try to start a container which is not installed/known
#[runtime_test]
async fn try_to_start_unknown_container() -> Result<()> {
    let container = "unknown_application:0.0.12:asdf";
    assert!(client().start(container).await.is_err());
    Ok(())
}

// Try to start a container where a dependency is missing
#[runtime_test]
async fn try_to_start_container_that_misses_a_resource() -> Result<()> {
    client().install_test_container().await?;
    // The TEST_RESOURCE is not installed.
    assert!(client().start(TEST_CONTAINER).await.is_err());
    Ok(())
}

// Start a container that uses a resource
#[runtime_test]
async fn check_test_container_resource_usage() -> Result<()> {
    client().install_test_container().await?;
    client().install_test_resource().await?;

    // Start the test_container process
    client()
        .start_with_args(TEST_CONTAINER, ["cat", "/resource/hello"])
        .await?;

    assume("hello from test resource", 5).await?;

    // The container might have finished at this point
    client().stop(TEST_CONTAINER, 5).await?;

    client().uninstall_test_container().await?;
    client().uninstall_test_resource().await
}

// Try to uninstall a started container
#[runtime_test]
async fn try_to_uninstall_a_started_container() -> Result<()> {
    client().install_test_container().await?;
    client().install_test_resource().await?;

    client().start_with_args(TEST_CONTAINER, ["sleep"]).await?;
    assume("Sleeping...", 5u64).await?;

    let result = client().uninstall_test_container().await;
    assert!(result.is_err());

    client().stop(TEST_CONTAINER, 5).await
}

#[runtime_test]
async fn start_mounted_container_with_not_mounted_resource() -> Result<()> {
    client().install_test_container().await?;
    client().install_test_resource().await?;

    // Start a container that depends on a resource.
    client().start_with_args(TEST_CONTAINER, ["sleep"]).await?;
    assume("Sleeping...", 5u64).await?;
    client().stop(TEST_CONTAINER, 5).await?;

    // Umount the resource and start the container again.
    client().umount(TEST_RESOURCE).await?;

    client().start_with_args(TEST_CONTAINER, ["sleep"]).await?;
    assume("Sleeping...", 5u64).await?;

    client().stop(TEST_CONTAINER, 5).await
}

// The test is flaky and needs to listen for notifications
// in order to be implemented correctly
#[runtime_test]
async fn container_crash_exit() -> Result<()> {
    client().install_test_container().await?;
    client().install_test_resource().await?;

    for _ in 0..10 {
        client().start_with_args(TEST_CONTAINER, ["crash"]).await?;

        let n =
            |n: &Notification| matches!(n, Notification::Exit(_, ExitStatus::Exit { code: 101 }));
        client().assume_notification(n, 5).await?;
    }

    client().uninstall_test_container().await?;
    client().uninstall_test_resource().await
}

// Check uid. In the manifest of the test container the uid
// is set to 1000
#[runtime_test]
async fn container_uses_correct_uid() -> Result<()> {
    client().install_test_container().await?;
    client().install_test_resource().await?;
    client()
        .start_with_args(TEST_CONTAINER, ["inspect"])
        .await?;
    assume("getuid: 1000", 5).await?;
    client().stop(TEST_CONTAINER, 5).await
}

// Check gid. In the manifest of the test container the gid
// is set to 1000
#[runtime_test]
async fn container_uses_correct_gid() -> Result<()> {
    client().install_test_container().await?;
    client().install_test_resource().await?;
    client()
        .start_with_args(TEST_CONTAINER, ["inspect"])
        .await?;
    assume("getgid: 1000", 5).await?;
    client().stop(TEST_CONTAINER, 5).await
}

// Check parent pid. Northstar starts an init process which must have pid 1.
#[runtime_test]
async fn container_ppid_must_be_init() -> Result<()> {
    client().install_test_container().await?;
    client().install_test_resource().await?;
    client()
        .start_with_args(TEST_CONTAINER, ["inspect"])
        .await?;
    assume("getppid: 1", 5).await?;
    client().stop(TEST_CONTAINER, 5).await
}

// Check session id which needs to be pid of init
#[runtime_test]
async fn container_sid_must_be_init_or_none() -> Result<()> {
    client().install_test_container().await?;
    client().install_test_resource().await?;
    client()
        .start_with_args(TEST_CONTAINER, ["inspect"])
        .await?;
    assume("getsid: 1", 5).await?;
    client().stop(TEST_CONTAINER, 5).await
}

// The test container only gets the cap_kill capability. See the manifest
#[runtime_test]
async fn container_shall_only_have_configured_capabilities() -> Result<()> {
    client().install_test_container().await?;
    client().install_test_resource().await?;
    client()
        .start_with_args(TEST_CONTAINER, ["inspect"])
        .await?;
    assume("caps bounding: \\{\\}", 10).await?;
    assume("caps effective: \\{\\}", 10).await?;
    assume("caps permitted: \\{\\}", 10).await?;
    client().stop(TEST_CONTAINER, 5).await
}

// The test container has a configured resource limit of tasks
#[runtime_test]
async fn container_rlimits() -> Result<()> {
    client().install_test_container().await?;
    client().install_test_resource().await?;
    client()
        .start_with_args(TEST_CONTAINER, ["inspect"])
        .await?;
    assume(
        "Max processes             10000                20000                processes",
        10,
    )
    .await?;
    client().stop(TEST_CONTAINER, 5).await
}

// Check whether after a client() start, container start and shutdown
// any file descriptor is leaked
#[runtime_test]
async fn start_stop_and_container_shall_not_leak_file_descriptors() -> Result<()> {
    /// Collect a set of files in /proc/$$/fd
    fn fds() -> Result<Vec<PathBuf>, std::io::Error> {
        let mut links = std::fs::read_dir("/proc/self/fd")?
            .filter_map(Result::ok)
            .flat_map(|entry| entry.path().read_link())
            .collect::<Vec<_>>();
        links.sort();
        Ok(links)
    }

    let before = fds()?;

    client().install_test_container().await?;
    client().install_test_resource().await?;

    client().start_with_args(TEST_CONTAINER, ["sleep"]).await?;
    assume("Sleeping", 5).await?;
    client().stop(TEST_CONTAINER, 5).await?;

    client().uninstall_test_container().await?;
    client().uninstall_test_resource().await?;

    // Compare the list of fds before and after the RT run.
    assert_eq!(before, fds()?);

    client().shutdown().await
}

// Check open file descriptors in the test container that should be
// stdin: /dev/null
// stdout: some pipe
// stderr: /dev/null
#[runtime_test]
async fn container_shall_only_have_configured_fds() -> Result<()> {
    client().install_test_container().await?;
    client().install_test_resource().await?;
    client()
        .start_with_args(TEST_CONTAINER, ["inspect"])
        .await?;
    assume("/proc/self/fd/0: /dev/null", 5).await?;
    assume("/proc/self/fd/1: socket", 5).await?;
    assume("/proc/self/fd/2: socket", 5).await?;
    assume("total: 3", 5).await?;
    client().stop(TEST_CONTAINER, 5).await
}

// Check if /proc is mounted ro
#[runtime_test]
async fn proc_is_mounted_ro() -> Result<()> {
    client().install_test_container().await?;
    client().install_test_resource().await?;
    client()
        .start_with_args(TEST_CONTAINER, ["inspect"])
        .await?;
    assume("proc /proc proc ro,", 5).await?;
    client().stop(TEST_CONTAINER, 5).await
}

// Check that mount flags nosuid,nodev,noexec are properly set for bind mounts
// assumption: mount flags are always listed the same order (according mount.h)
// note: MS_REC is not explicitly listed an cannot be checked with this test
#[runtime_test]
async fn mount_flags_are_set_for_bind_mounts() -> Result<()> {
    client().install_test_container().await?;
    client().install_test_resource().await?;
    client()
        .start_with_args(TEST_CONTAINER, ["inspect"])
        .await?;
    assume(
        "/.* /resource \\w+ ro,(\\w+,)*nosuid,(\\w+,)*nodev,(\\w+,)*noexec",
        5,
    )
    .await?;
    client().stop(TEST_CONTAINER, 5).await
}

// The test container only gets the cap_kill capability. See the manifest
#[runtime_test]
async fn selinux_mounted_squasfs_has_correct_context() -> Result<()> {
    client().install_test_container().await?;
    client().install_test_resource().await?;
    client()
        .start_with_args(TEST_CONTAINER, ["inspect"])
        .await?;
    // Only expect selinux context if system supports it
    if Path::new("/sys/fs/selinux/enforce").exists() {
        assume(
            "/.* squashfs (\\w+,)*context=unconfined_u:object_r:user_home_t:s0",
            5,
        )
        .await?;
    } else {
        assume("/.* squashfs (\\w+,)*", 5).await?;
    }
    client().stop(TEST_CONTAINER, 5).await
}

// Call syscall with specifically allowed argument
#[runtime_test]
async fn seccomp_allowed_syscall_with_allowed_arg() -> Result<()> {
    client().install_test_container().await?;
    client().install_test_resource().await?;
    client()
        .start_with_args(TEST_CONTAINER, ["call-delete-module", "1"])
        .await?;
    assume("delete_module syscall was successful", 5).await?;
    client().stop(TEST_CONTAINER, 5).await
}

// Call syscall with argument allowed by bitmask
#[runtime_test]
async fn seccomp_allowed_syscall_with_masked_arg() -> Result<()> {
    client().install_test_container().await?;
    client().install_test_resource().await?;
    client()
        .start_with_args(TEST_CONTAINER, ["call-delete-module", "4"])
        .await?;
    assume("delete_module syscall was successful", 5).await?;
    client().stop(TEST_CONTAINER, 5).await
}

// Call syscall with prohibited argument
#[runtime_test]
async fn seccomp_allowed_syscall_with_prohibited_arg() -> Result<()> {
    client().install_test_container().await?;
    client().install_test_resource().await?;
    client()
        .start_with_args(TEST_CONTAINER, ["call-delete-module", "7"])
        .await?;

    let n = |n: &Notification| {
        matches!(n,
        Notification::Exit (
            _,
            ExitStatus::Signalled { signal },
        ) if signal == &31)
    };
    client().assume_notification(n, 5).await
}

// Iterate all exit codes in the u8 range
#[runtime_test]
async fn exit_codes() -> Result<()> {
    client().install_test_container().await?;
    client().install_test_resource().await?;
    for c in &[0, 1, 10, 127, 128, 255] {
        client()
            .start_with_args(TEST_CONTAINER, ["exit".to_string(), c.to_string()])
            .await?;
        let n = |n: &Notification| {
            matches!(n, Notification::Exit (
                _,
                ExitStatus::Exit { code },
            ) if code == c)
        };
        client().assume_notification(n, 5).await?;
    }
    Ok(())
}

// Check printing on stdout and stderr
#[runtime_test]
async fn stdout_stderr() -> Result<()> {
    client().install_test_container().await?;
    client().install_test_resource().await?;

    let args = ["print", "--io", "stdout", "hello stdout"];
    client().start_with_args(TEST_CONTAINER, args).await?;
    assume("hello stdout", 10).await?;
    client().stop(TEST_CONTAINER, 5).await?;

    let args = ["print", "--io", "stderr", "hello stderr"];
    client().start_with_args(TEST_CONTAINER, args).await?;
    assume("hello stderr", 10).await?;
    client().stop(TEST_CONTAINER, 5).await
}
