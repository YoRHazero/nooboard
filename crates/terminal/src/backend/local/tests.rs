use super::command_builder;
use std::{io::Read, os::unix::fs::PermissionsExt};

#[test]
fn fixed_commands_find_profile_installed_programs_from_a_gui_environment() {
    let home = tempfile::tempdir().unwrap();
    let bin = home.path().join("local tools");
    std::fs::create_dir(&bin).unwrap();
    let executable = bin.join("nooboard-profile-probe");
    std::fs::write(
        &executable,
        "#!/bin/sh\nprintf 'PROFILE_OK=%s|%s|%s\\n' \"$PROFILE_VALUE\" \"$1\" \"$2\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::write(
        home.path().join(".profile"),
        "export PATH=\"$HOME/local tools:$PATH\"\nexport PROFILE_VALUE='loaded from profile'\n",
    )
    .unwrap();

    let mut command = command_builder(Some(
        "nooboard-profile-probe 'argument with spaces' \"quote's value\"".into(),
    ));
    // Only this child gets the stripped GUI environment; no process-global mutation.
    command.env_clear();
    command.env("HOME", home.path());
    command.env("SHELL", "/bin/sh");
    command.env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin");
    command.cwd(home.path());
    let pair = portable_pty::native_pty_system()
        .openpty(Default::default())
        .unwrap();
    let mut reader = pair.master.try_clone_reader().unwrap();
    let mut child = pair.slave.spawn_command(command).unwrap();
    drop(pair.slave);
    let mut output = Vec::new();
    // Linux PTYs may report EIO instead of EOF once the child closes the slave.
    if let Err(error) = reader.read_to_end(&mut output) {
        assert_eq!(error.raw_os_error(), Some(libc::EIO));
    }
    let status = child.wait().unwrap();
    let output = String::from_utf8_lossy(&output);
    assert!(status.success(), "{status:?}: {output}");
    assert!(
        output.contains("PROFILE_OK=loaded from profile|argument with spaces|quote's value"),
        "{output}"
    );
}
