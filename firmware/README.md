![Rust](https://img.shields.io/badge/Rust-Embedded-orange)
![STM32F446](https://img.shields.io/badge/MCU-STM32F446-blue)
![License](https://img.shields.io/github/license/411568/Universal_interface_adapter)
![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen)

# Universal Interface Adapter Firmware

Firmware for the **Universal Interface Adapter (UIA)**, an open-source industrial communication and I/O platform built around the **STM32F446VET6** microcontroller.

The firmware is written entirely in **Rust** and follows a modular architecture designed for reliability, maintainability, and community-driven development.

---

# Philosophy

The goal of this firmware is simple:

> Make industrial and embedded interfaces easy to access, easy to understand, and easy to extend.

Every hardware feature on the board is exposed through a common command-line interface (CLI) over USB, making the board useful as:

* Development platform
* Hardware test tool
* Protocol analyzer
* Automation controller
* Educational platform
* Rapid prototyping tool

---

# Features

## USB CDC Command Interface

The board enumerates as a USB CDC-ACM serial device and provides an interactive CLI.

No special drivers are required on Linux and macOS. Windows typically uses the standard USB serial driver.

Example:

```text
$ screen /dev/ttyACM0 115200
UIA>
```

---

## Supported Interfaces

### Digital Outputs

* 4x MOSFET outputs
* 2x Relay outputs

### Digital Inputs

* 3x Isolated inputs
* 4x GPIO channels

### Analog Interfaces

* 2x Analog inputs (ADC)
* 1x Analog output (DAC)

### Serial Interfaces

* UART
* RS-232
* RS-422
* CAN
* I²C
* SPI

### User Interface

* 4x Status LEDs
* USB command interface

---

# Firmware Architecture

The firmware intentionally separates hardware initialization, drivers, and command handling.

```text
main.rs
│
├── Peripheral Initialization
│   ├── GPIO
│   ├── ADC / DAC
│   ├── UART
│   ├── RS232
│   ├── RS422
│   ├── I²C
│   ├── SPI
│   ├── CAN
│   └── USB
│
├── Command Registry
│   ├── LED Commands
│   ├── MOSFET Commands
│   ├── Relay Commands
│   ├── GPIO Commands
│   ├── Analog Commands
│   ├── Serial Commands
│   └── Interface Commands
│
└── CLI Event Loop
```

The command registry allows new functionality to be added without modifying the main application loop.

---

# Project Structure

```text
firmware/
├── src/
│   ├── main.rs
│   ├── cli/
│   │   ├── mod.rs
│   │   └── commands/
│   └── io_interface/
├── Cargo.toml
├── memory.x
└── README.md
```

---

# Current Command Modules

The firmware currently implements command handlers for:

* Echo
* Setup
* LEDs
* MOSFET outputs
* Relays
* Isolated inputs
* GPIO
* UART
* RS-232
* RS-422
* Analog I/O
* I²C
* SPI
* CAN

Each module is responsible for:

1. Hardware initialization
2. Runtime control
3. CLI integration
4. Error handling

---

# Building

## Requirements

* Rust stable toolchain
* rustup
* cargo
* ARM target support

Install the target:

```bash
rustup target add thumbv7em-none-eabihf
```

---

## Build

```bash
cargo build --release
```

---

## Flash

Example using probe-rs:

```bash
cargo install probe-rs-tools

probe-rs download \
    --chip STM32F446VETx \
    target/thumbv7em-none-eabihf/release/universal_interface_adapter
```

---

# Example CLI Usage

```text
UIA> led on 1
UIA> relay on 2
UIA> mosfet off 3
UIA> gpio read 1
UIA> adc read 0
UIA> i2c scan
UIA> can send 123 DEADBEEF
```

---

# Adding New Commands

The firmware was intentionally designed to make adding features straightforward.

Typical steps:

### 1. Create a command

```rust
pub struct MyCommand;

impl MyCommand {
    pub fn new() -> Self {
        Self
    }
}
```

### 2. Implement the command trait

```rust
impl Command for MyCommand {
    // implementation
}
```

### 3. Register the command

```rust
let _ = registry.register_my_command(MyCommand::new());
```

That's it. The CLI automatically exposes the new functionality.

---

# Why Rust?

Industrial and embedded systems often run continuously and interact with external hardware. Rust provides several advantages:

* Memory safety without garbage collection
* Strong type checking
* Zero-cost abstractions
* Excellent concurrency primitives
* Reduced risk of crashes and undefined behavior
* Maintainable long-term codebase

The objective is to build firmware that is both reliable enough for industrial use and approachable enough for experimentation and learning.

---

# Contributing

Contributions are highly encouraged.

Ideas for future improvements:

* Additional CLI commands
* Protocol bridges
* Configuration storage
* Data logging
* Scriptable command execution
* USB firmware update support
* Modbus support
* CANopen support
* MQTT gateway functionality
* Performance improvements
* Documentation improvements

Pull requests of all sizes are welcome.

If you have an idea, open an issue and let's discuss it.

---

# Development Principles

Please try to keep contributions:

* Modular
* Well documented
* Hardware independent where possible
* Non-blocking
* Safe and idiomatic Rust
* Easy to test and review

---

# Community

This project exists because embedded and industrial development tools should be open, understandable, and hackable.

If you build something interesting with the Universal Interface Adapter, improve the firmware, or add support for new protocols, please submit a pull request.

**Let's build a universal embedded interface platform together.**

