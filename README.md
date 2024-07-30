# Gevulot Shim

Gevulot Shim provides a helper library to integrate program to be run under Gevulot.


## Build

```
  cargo build --release
```

## Usage of `shim-executor`

`shim-executor` is a simple test tool that executes the task on a program in VM in same way as Gevulot node would.

## Example:

### 1. Create a test prover

```
  cargo new --bin my_prover
```

### 2. Add `gevulot-shim` dependency

```filename="Cargo.toml"
[package]
name = "my_prover"
version = "0.1.0"
edition = "2021"

[dependencies]
clap = { version = "4", features = ["derive"] }
gevulot-shim = { git = "https://github.com/gevulotnetwork/gevulot-shim.git" }
```

### 3. Write some simple dummy prover for testing

```rust filename="src/main.rs"
use clap::Parser;
use gevulot_shim::{Task, TaskResult, WORKSPACE_PATH};
use std::{fs, path::PathBuf};

/// Simple sample program to demonstrate Gevulot `shim-executor`.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Input file for the prover.
    #[arg(long)]
    input: PathBuf,

    /// Output file for the proof.
    #[arg(long)]
    output: PathBuf,
}

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn main() -> Result<()> {
    gevulot_shim::run(run_task)
}

fn run_task(task: Task) -> Result<TaskResult> {
    println!("prover: task.args: {:?}", &task.args);

    // `task.args` contains only the task args, which doesn't include the binary
    // name. To use existing CLI args parser, create a Vec<String> of args,
    // including the binary name.
    let mut args_with_bin_name = vec![std::env::args()
        .collect::<Vec<String>>()
        .first()
        .unwrap()
        .clone()];
    args_with_bin_name.append(&mut task.args.clone());

    // Parse the cli args.
    let args = Args::parse_from(args_with_bin_name);

    // Print the input file contents.
    let content = String::from_utf8(std::fs::read(&args.input)?)?;
    println!("prover: file:{:?} with content:\n{content:?}", &args.input);

    // Show what files are present under the workspace directory.
    let entries = fs::read_dir(WORKSPACE_PATH)
        .unwrap()
        .map(|res| res.map(|e| e.path()))
        .collect::<std::io::Result<Vec<_>>>()
        .unwrap();
    println!("files in /workspace :: {:?}", entries);

    // Finally, generate a dummy "proof" to an output file.
    std::fs::write(&args.output, b"this is a proof.")?;
    task.result(
        // This vector of bytes is just an example to demonstrate
        // the possibility for tx embedded proof data.
        vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
        // This vector lists the files that are exported from the program
        // execution. In this example it is the value of `--output` argument.
        vec![args.output.to_string_lossy().to_string()],
    )
}
```

### 4. Add manifest for building the program VM

```json filename="my_prover.manifest"
{
  "ManifestPassthrough": {
    "readonly_rootfs": "true"
  },
  "Env":{
    "RUST_BACKTRACE": "1",
    "RUST_LOG": "trace"
  },
  "Program":"target/release/my_prover",
  "Mounts": {
    "%1": "/workspace"
  }
}

```

### 5. Build the program & the VM image

```shell
  cargo build --release
  ops build ./target/release/my_prover -c my_prover.manifest
```

6. Run the program with the `shim-executor`

```shell
  WORKSPACEDIR=$(mktemp -d -t test-workspace-XXXX)
  echo "Hello, world!" > "$WORKSPACEDIR"/test.input
  shim-executor --workspace "$WORKSPACEDIR" my_prover.img -- --prove --input /workspace/test.input --output /workspace/test.output
  echo "test.output:"
  cat "$WORKSPACEDIR"/test.output
  rm -fr "$WORKSPACEDIR"
```

#### Outline of above:
1. Create a tempdir for workspace.
2. Create a test input file that mimics a witness file of a prover.
3. Run the prover VM using the `shim-executor`.
4. Print the output that prover VM generated.
5. Remove the temporary workspace directory.
