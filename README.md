# Taill

Taill is a command-line tool that tails a file and watches for changes. It uses the `notify` crate to watch for file modifications and the `bat` crate to pretty-print the log output.

## Features

- Watch a file or files matching a pattern for changes.
- Tail the file and print new content as it is added.
- Pretty-print log output using `bat`.

## Usage

To use Taill, run the following command:

```sh
taill -f <file-pattern>
```

### Arguments

- `-f, --pattern`: The file pattern to watch. This argument is required.

## Example

```sh
taill -f "*.log"
```

This command will watch all files with the `.log` extension in the current directory and print new content as it is added.

## Installation

To install Taill, you need to have Rust and Cargo installed. Then, you can build the project using the following commands:

```sh
git clone https://github.com/zhangzhishan/taill.git
cd taill
cargo build --release
```

After building the project, you can run the `taill` executable from the `target/release` directory.

## License

This project is licensed under the MIT License.
# taill

A command-line utility to tail files and watch for changes using pattern matching. Monitor files in real-time as they're updated, with support for glob patterns to watch multiple files simultaneously.

## Installation

Install from crates.io:

```bash
cargo install taill
```

## Usage

```bash
taill -f <pattern>
```

### Examples

Watch all log files in current directory:
```bash
taill -f "*.log"
```

Watch specific file:
```bash
taill -f "app.log"
```

Watch files in a specific directory:
```bash
taill -f "logs/*.txt"
```

## Features

- Real-time file monitoring
- Pattern matching using glob syntax
- Automatic detection of new files matching the pattern
- Efficient resource usage
- Non-recursive directory watching

## Arguments

- `-f <pattern>`: The file pattern to watch (required)
- `--help`: Display help information
- `--version`: Display version information

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Author

Zhishan Zhang (zhangzhishanlo@gmail.com)
