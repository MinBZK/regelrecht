use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct ProcessMessage {
    pub task_id: String,
    pub kind: ProcessMessageKind,
}

#[derive(Debug, Clone)]
pub enum ProcessMessageKind {
    Stdout(String),
    Stderr(String),
    Done { exit_code: Option<i32> },
}

pub struct ProcessRunner {
    project_root: PathBuf,
    tx: mpsc::UnboundedSender<ProcessMessage>,
    rx: mpsc::UnboundedReceiver<ProcessMessage>,
    running: Option<String>,
}

impl ProcessRunner {
    pub fn new(project_root: PathBuf) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            project_root,
            tx,
            rx,
            running: None,
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.is_some()
    }

    #[allow(dead_code)]
    pub fn running_task(&self) -> Option<&str> {
        self.running.as_deref()
    }

    /// Spawn a `just` command and stream output back via messages.
    pub fn run_just(&mut self, task_id: String, target: &str) {
        self.run_command(task_id, "just", &[target]);
    }

    /// Spawn an arbitrary command and stream output back via messages.
    pub fn run_command(&mut self, task_id: String, program: &str, args: &[&str]) {
        self.running = Some(task_id.clone());

        let tx = self.tx.clone();
        let project_root = self.project_root.clone();
        let program = program.to_string();
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();

        tokio::spawn(async move {
            let result = run_streaming(project_root, &program, &args, &task_id, &tx).await;
            if let Err(e) = result {
                let _ = tx.send(ProcessMessage {
                    task_id: task_id.clone(),
                    kind: ProcessMessageKind::Stderr(format!("Failed to start: {e}")),
                });
                let _ = tx.send(ProcessMessage {
                    task_id,
                    kind: ProcessMessageKind::Done { exit_code: None },
                });
            }
        });
    }

    /// Try to receive a pending message (non-blocking).
    pub fn try_recv(&mut self) -> Option<ProcessMessage> {
        match self.rx.try_recv() {
            Ok(msg) => {
                if matches!(msg.kind, ProcessMessageKind::Done { .. }) {
                    // Only clear running if this Done is for the current task
                    if self.running.as_deref() == Some(&msg.task_id) {
                        self.running = None;
                    }
                }
                Some(msg)
            }
            Err(_) => None,
        }
    }
}

/// Run a command and stream its output line-by-line via the channel.
///
/// Note: stdout and stderr are consumed in parallel tasks, so lines from
/// each stream may interleave non-deterministically. This is inherent to
/// the approach and acceptable for TUI display purposes.
async fn run_streaming(
    project_root: PathBuf,
    program: &str,
    args: &[String],
    task_id: &str,
    tx: &mpsc::UnboundedSender<ProcessMessage>,
) -> anyhow::Result<()> {
    let mut child = Command::new(program)
        .args(args)
        .current_dir(&project_root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let task_id_out = task_id.to_string();
    let tx_out = tx.clone();
    let stdout_handle = tokio::spawn(async move {
        if let Some(stdout) = stdout {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let _ = tx_out.send(ProcessMessage {
                    task_id: task_id_out.clone(),
                    kind: ProcessMessageKind::Stdout(line),
                });
            }
        }
    });

    let task_id_err = task_id.to_string();
    let tx_err = tx.clone();
    let stderr_handle = tokio::spawn(async move {
        if let Some(stderr) = stderr {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let _ = tx_err.send(ProcessMessage {
                    task_id: task_id_err.clone(),
                    kind: ProcessMessageKind::Stderr(line),
                });
            }
        }
    });

    let _ = stdout_handle.await;
    let _ = stderr_handle.await;

    let status = child.wait().await?;
    let _ = tx.send(ProcessMessage {
        task_id: task_id.to_string(),
        kind: ProcessMessageKind::Done {
            exit_code: status.code(),
        },
    });

    Ok(())
}
