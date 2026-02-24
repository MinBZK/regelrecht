use std::path::Path;

use regelrecht_pipeline::git_ops::GitRepo;

async fn run_git(cwd: &Path, args: &[&str]) -> String {
    let output = tokio::process::Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .await
        .unwrap();
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

async fn init_bare_remote(path: &Path) {
    run_git(Path::new("."), &["init", "--bare", &path.to_string_lossy()]).await;
}

async fn setup_clone(remote: &Path, local: &Path) -> GitRepo {
    let repo = GitRepo::clone_or_open(&remote.to_string_lossy(), local)
        .await
        .unwrap();
    run_git(local, &["config", "user.email", "test@test.com"]).await;
    run_git(local, &["config", "user.name", "Test User"]).await;
    repo
}

#[tokio::test]
async fn test_clone_creates_repo() {
    let tmp = tempfile::tempdir().unwrap();
    let remote_path = tmp.path().join("remote.git");
    let clone_path = tmp.path().join("clone");

    init_bare_remote(&remote_path).await;
    let repo = GitRepo::clone_or_open(&remote_path.to_string_lossy(), &clone_path)
        .await
        .unwrap();

    assert!(clone_path.join(".git").exists());
    assert_eq!(repo.path(), clone_path);
}

#[tokio::test]
async fn test_open_existing_repo() {
    let tmp = tempfile::tempdir().unwrap();
    let remote_path = tmp.path().join("remote.git");
    let clone_path = tmp.path().join("clone");

    init_bare_remote(&remote_path).await;

    GitRepo::clone_or_open(&remote_path.to_string_lossy(), &clone_path)
        .await
        .unwrap();

    let repo = GitRepo::clone_or_open(&remote_path.to_string_lossy(), &clone_path)
        .await
        .unwrap();

    assert_eq!(repo.path(), clone_path);
}

#[tokio::test]
async fn test_full_workflow_add_commit_push() {
    let tmp = tempfile::tempdir().unwrap();
    let remote_path = tmp.path().join("remote.git");
    let clone_path = tmp.path().join("clone");

    init_bare_remote(&remote_path).await;
    let repo = setup_clone(&remote_path, &clone_path).await;

    tokio::fs::write(clone_path.join("law.yaml"), "test: data").await.unwrap();

    repo.add(&[Path::new("law.yaml")]).await.unwrap();
    let committed = repo.commit("add law").await.unwrap();
    assert!(committed);
    repo.push(true).await.unwrap();

    let verify_path = tmp.path().join("verify");
    let _ = GitRepo::clone_or_open(&remote_path.to_string_lossy(), &verify_path)
        .await
        .unwrap();
    assert!(verify_path.join("law.yaml").exists());
}

#[tokio::test]
async fn test_commit_nothing_returns_false() {
    let tmp = tempfile::tempdir().unwrap();
    let remote_path = tmp.path().join("remote.git");
    let clone_path = tmp.path().join("clone");

    init_bare_remote(&remote_path).await;
    let repo = setup_clone(&remote_path, &clone_path).await;

    let committed = repo.commit("empty").await.unwrap();
    assert!(!committed);
}

#[tokio::test]
async fn test_push_disabled_is_noop() {
    let tmp = tempfile::tempdir().unwrap();
    let remote_path = tmp.path().join("remote.git");
    let clone_path = tmp.path().join("clone");

    init_bare_remote(&remote_path).await;
    let repo = setup_clone(&remote_path, &clone_path).await;

    repo.push(false).await.unwrap();
}

#[tokio::test]
async fn test_add_nested_paths() {
    let tmp = tempfile::tempdir().unwrap();
    let remote_path = tmp.path().join("remote.git");
    let clone_path = tmp.path().join("clone");

    init_bare_remote(&remote_path).await;
    let repo = setup_clone(&remote_path, &clone_path).await;

    let nested_dir = clone_path.join("regulation").join("nl").join("wet").join("test_law");
    tokio::fs::create_dir_all(&nested_dir).await.unwrap();
    tokio::fs::write(nested_dir.join("2025-01-01.yaml"), "test: data").await.unwrap();
    tokio::fs::write(nested_dir.join("status.yaml"), "status: harvested").await.unwrap();

    repo.add(&[
        Path::new("regulation/nl/wet/test_law/2025-01-01.yaml"),
        Path::new("regulation/nl/wet/test_law/status.yaml"),
    ]).await.unwrap();

    let committed = repo.commit("harvest: test law").await.unwrap();
    assert!(committed);
    repo.push(true).await.unwrap();
}

#[tokio::test]
async fn test_pull_fetches_remote_changes() {
    let tmp = tempfile::tempdir().unwrap();
    let remote_path = tmp.path().join("remote.git");
    let clone1_path = tmp.path().join("clone1");
    let clone2_path = tmp.path().join("clone2");

    init_bare_remote(&remote_path).await;
    let repo1 = setup_clone(&remote_path, &clone1_path).await;
    let repo2 = setup_clone(&remote_path, &clone2_path).await;

    tokio::fs::write(clone1_path.join("new-file.txt"), "data").await.unwrap();
    repo1.add(&[Path::new("new-file.txt")]).await.unwrap();
    repo1.commit("add new file").await.unwrap();
    repo1.push(true).await.unwrap();

    repo2.pull().await.unwrap();
    assert!(clone2_path.join("new-file.txt").exists());
}
