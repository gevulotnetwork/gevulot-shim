use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::time::Instant;
use std::{path::Path, thread::sleep, time::Duration};

/// MOUNT_TIMEOUT is maximum amount of time to wait for workspace mount to be
/// present in /proc/mounts.
const MOUNT_TIMEOUT: Duration = Duration::from_secs(30);

pub const TASK_FILE_NAME: &str = "task.json";
pub const TASK_RESULT_FILE_NAME: &str = "task_result.json";

pub const WORKSPACE_PATH: &str = "/workspace";

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
pub type TaskId = String;

#[derive(Debug, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub args: Vec<String>,
    pub files: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskResult {
    id: TaskId,
    data: Vec<u8>,
    files: Vec<String>,
}

impl Task {
    pub fn result(&self, data: Vec<u8>, files: Vec<String>) -> Result<TaskResult> {
        Ok(TaskResult {
            id: self.id.clone(),
            data,
            files,
        })
    }

    pub fn get_task_files_path<'a>(&'a self, workspace: &str) -> Vec<(&'a str, PathBuf)> {
        self.files
            .iter()
            .map(|name| {
                let path = Path::new(workspace).join(name);
                (name.as_str(), path)
            })
            .collect()
    }
}

fn mount_present(mount_point: &str) -> Result<bool> {
    let file = File::open("/proc/mounts")?;
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let line = line.expect("read /proc/mounts");
        if line.contains(mount_point) {
            return Ok(true);
        }
    }

    Ok(false)
}

/// run function takes `callback` that is invoked with executable `Task` and
/// which is expected to return `TaskResult`.
pub fn run(callback: impl Fn(Task) -> Result<TaskResult>) -> Result<()> {
    let workspace = WORKSPACE_PATH;

    println!("waiting for {workspace} mount to be present");
    let beginning = Instant::now();
    loop {
        if beginning.elapsed() > MOUNT_TIMEOUT {
            panic!("{} mount timeout", workspace);
        }

        if mount_present(workspace)? {
            println!("{workspace} mount is now present");
            break;
        }

        sleep(Duration::from_secs(1));
    }

    let file = File::open(PathBuf::from(WORKSPACE_PATH).join(TASK_FILE_NAME))?;
    let task: Task = serde_json::from_reader(file)?;

    let result = callback(task).map_err(|e| e.to_string());
    let mut result_file = File::options()
        .read(true)
        .write(true)
        .create_new(true)
        .open(PathBuf::from(WORKSPACE_PATH).join(TASK_RESULT_FILE_NAME))?;
    serde_json::to_writer(&mut result_file, &result)?;
    result_file.flush()?;
    drop(result_file);

    Ok(())
}
