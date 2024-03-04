# Move smart contract RISC Zero Interpreter


### Executing the project locally in development mode

During development, faster iteration upon code changes can be achieved by leveraging [dev-mode], we strongly suggest activating it during your early development phase. Furthermore, you might want to get insights into the execution statistics of your project, and this can be achieved by specifying the environment variable `RUST_LOG=info` before running your project.

Put together, the command to run your project in development mode while getting execution statistics is:

```bash
RUST_LOG=info RISC0_DEV_MODE=1 cargo run
```


### Executing the project in production mode

```bash
RUST_LOG=info cargo run --release
```


### Fix potential issues
Clap dependency version might cause a compilation issue. You can lower the version with a compatible one:
```
cd methods/guest
cargo update clap@4.5.1 --precise 4.4.1
```


## Directory Structure

It is possible to organize the files for these components in various ways.
However, in this starter template we use a standard directory structure for zkVM
applications, which we think is a good starting point for your applications.

```text
project_name
├── Cargo.toml
├── host
│   ├── Cargo.toml
│   └── src
│       └── main.rs                        <-- [Host code goes here]
└── methods
    ├── Cargo.toml
    ├── build.rs
    ├── guest
    │   ├── Cargo.toml
    │   └── src
    │       └── bin
    │           └── method_name.rs         <-- [Guest code goes here]
    └── src
        └── lib.rs
```
