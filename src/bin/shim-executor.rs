use std::{fs::File, io::Write, path::PathBuf, process::Command};

use clap::{command, Parser};
use gevulot_shim::{Task, TaskResult, TASK_FILE_NAME, TASK_RESULT_FILE_NAME};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Clone, Debug, Parser)]
#[command(author, version, about = "Gevulot Shim Executor")]
pub struct Config {
    #[arg(short, long, long_help = "File to be added to program execution")]
    pub file: Vec<String>,

    #[arg(short, long, long_help = "PCI device path to GPU device")]
    pub gpu: Vec<String>,

    #[arg(
        default_value_t = 1,
        short,
        long,
        long_help = "Number of CPU cores to allocate to VM"
    )]
    pub smp: u16,

    #[arg(default_value_t = 512, short, long, long_help = "Memory in MBs")]
    pub mem: u32,

    #[arg(short, long, long_help = "Workspace directory")]
    pub workspace: PathBuf,

    #[arg(
        default_value = "task01",
        long,
        long_help = "Task ID to be used in the task descriptor"
    )]
    pub task_id: String,

    pub program: PathBuf,

    #[arg(last = true, help = "Program args")]
    pub args: Vec<String>,
}

fn main() {
    let config = Config::parse();
    pre_check(&config).expect("pre-flight check");
    run_qemu(config).expect("qemu");
}

fn pre_check(config: &Config) -> Result<()> {
    // Check that workspace directory exists.
    if !config.workspace.exists() {
        eprintln!(
            "Configured workspace directory \"{:?}\" doesn't exist.",
            config.workspace
        );
        std::process::exit(1);
    }

    // Check that there's no existing `task_result.json`.
    let task_result_file_path = config.workspace.join(TASK_RESULT_FILE_NAME);
    if task_result_file_path.exists() {
        eprintln!("{:?} already exists", task_result_file_path);
        std::io::stdout().write_all(b"Do you want to remove it (yes/no)? ")?;
        std::io::stdout().flush()?;
        let mut answer = String::new();
        std::io::stdin().read_line(&mut answer)?;
        let answer = answer.lines().next().expect("answer");
        if answer != "yes" {
            eprintln!("Cannot proceed; exiting.");
            std::process::exit(1);
        }
        std::fs::remove_file(task_result_file_path)?;
    }

    Ok(())
}

fn run_qemu(config: Config) -> Result<TaskResult> {
    // Task descriptor.
    let task = Task {
        id: config.task_id,
        args: config.args,
        files: config.file,
    };

    let mut task_file = File::create(config.workspace.join(TASK_FILE_NAME))?;
    serde_json::to_writer(&mut task_file, &task)?;
    task_file.flush()?;
    drop(task_file);

    // run qemu
    let mut cmd = Command::new("qemu-system-x86_64");
    cmd.args(["-machine", "q35"])
        .args([
            "-device",
            "pcie-root-port,port=0x10,chassis=1,id=pci.1,bus=pcie.0,multifunction=on,addr=0x3",
        ])
        .args([
            "-device",
            "pcie-root-port,port=0x11,chassis=2,id=pci.2,bus=pcie.0,addr=0x3.0x1",
        ])
        .args([
            "-device",
            "pcie-root-port,port=0x12,chassis=3,id=pci.3,bus=pcie.0,addr=0x3.0x2",
        ])
        // Register 2 hard drives via SCSI
        .args(["-device", "virtio-scsi-pci,bus=pci.2,addr=0x0,id=scsi0"])
        .args(["-device", "scsi-hd,bus=scsi0.0,drive=hd0"])
        .args(["-vga", "none"])
        // CPUS
        .args(["-smp", &config.smp.to_string()])
        .args(["-device", "isa-debug-exit"])
        // MEMORY
        .args(["-m", &format!("{}M", config.mem)])
        .args(["-device", "virtio-rng-pci"])
        .args(["-machine", "accel=kvm:tcg"])
        .args(["-cpu", "max"])
        // IMAGE FILE
        .args([
            "-drive",
            &format!(
                "file={},format=raw,if=none,id=hd0,readonly=on",
                &config
                    .program
                    .clone()
                    .into_os_string()
                    .into_string()
                    .unwrap(),
            ),
        ])
        .args(["-display", "none"])
        .args(["-serial", "stdio"])
        // WORKSPACE VirtFS
        .args([
            "-virtfs",
            &format!(
                "local,path={},mount_tag=0,security_model=none,multidevs=remap,id=hd0",
                &config.workspace.to_str().unwrap().to_string()
            ),
        ]);

    if !config.gpu.is_empty() {
        for gpu in config.gpu.clone() {
            cmd.args(["-device", &format!("vfio-pci,rombar=0,host={gpu}")]);
        }
    }

    let status = cmd.status()?;
    println!("QEMU exit status: {}", status);

    let task_result_file = File::open(config.workspace.join(TASK_RESULT_FILE_NAME))?;
    let result: std::result::Result<TaskResult, String> =
        serde_json::from_reader(task_result_file)?;
    result.map_err(|e| e.into())
}
