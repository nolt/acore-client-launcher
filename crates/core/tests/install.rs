use std::fs;

use launcher_core::{LocalState, install, remove_original};

#[test]
fn install_copies_the_launcher_and_marks_the_folder() {
    let downloads = tempfile::tempdir().unwrap();
    let games = tempfile::tempdir().unwrap();
    let exe = downloads.path().join("launcher");
    fs::write(&exe, b"launcher binary").unwrap();
    let game_dir = games.path().join("Server");

    let installed = install(&exe, &game_dir).unwrap();

    assert_eq!(installed, game_dir.join("launcher"));
    assert_eq!(fs::read(&installed).unwrap(), b"launcher binary");
    assert!(LocalState::exists(&game_dir));
    assert!(exe.exists(), "the original goes only after the copy has started");

    remove_original(&exe, &installed).unwrap();
    assert!(!exe.exists());
    assert!(installed.exists());
}

#[test]
fn a_launcher_already_in_the_folder_stays_where_it_is() {
    let dir = tempfile::tempdir().unwrap();
    let exe = dir.path().join("launcher");
    fs::write(&exe, b"launcher binary").unwrap();

    let installed = install(&exe, dir.path()).unwrap();

    assert_eq!(
        fs::canonicalize(&installed).unwrap(),
        fs::canonicalize(&exe).unwrap()
    );
    remove_original(&exe, &installed).unwrap();
    assert!(exe.exists(), "never deletes the launcher that is running");
}

#[test]
fn install_keeps_the_saved_state_of_an_existing_game_folder() {
    let downloads = tempfile::tempdir().unwrap();
    let game_dir = tempfile::tempdir().unwrap();
    let exe = downloads.path().join("launcher");
    fs::write(&exe, b"launcher binary").unwrap();
    let state = LocalState {
        manifest_version: "2026.09.27".into(),
        ..Default::default()
    };
    state.save(game_dir.path()).unwrap();

    install(&exe, game_dir.path()).unwrap();

    assert_eq!(LocalState::load(game_dir.path()), state);
}

#[test]
fn remove_original_leaves_a_different_file_alone() {
    let dir = tempfile::tempdir().unwrap();
    let other = dir.path().join("something-else");
    let installed = dir.path().join("launcher");
    fs::write(&other, b"not the launcher").unwrap();
    fs::write(&installed, b"launcher binary").unwrap();

    remove_original(&other, &installed).unwrap();

    assert!(other.exists());
}
