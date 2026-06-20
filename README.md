# Universal Interface Adapter

> **One board. Every interface.**
>
> An open-source industrial communication and I/O platform for embedded development, automation, diagnostics, and rapid prototyping.

![Board Image](docs/images/board.png)

---

# Universal Interface Adapter

The Universal Interface Adapter (UIA) is a compact, open-source hardware platform designed to bridge embedded systems with industrial and legacy communication interfaces.

Built around an STM32F446 microcontroller, the board combines communication buses, isolated digital inputs, relay outputs, MOSFET drivers, analog I/O, and general-purpose digital interfaces into a single development and integration platform.

Whether you're developing firmware, interfacing with industrial equipment, creating automated test fixtures, or experimenting with communication protocols, the Universal Interface Adapter provides all the essential interfaces on one board.

---

# Features

## Processing

* STM32F446VET6 ARM Cortex-M4 microcontroller
* Up to 180 MHz operation
* USB Full-Speed support
* Rich peripheral set for communication and control applications
* ST-Link programming and debugging interface

---

## Communication Interfaces

### CAN Bus

* Differential CAN transceiver
* Suitable for:

  * Automotive applications
  * Robotics
  * Industrial automation
  * Battery management systems
  * Motor controllers

### RS-232

* Legacy serial communication support
* Compatible with instrumentation and industrial equipment

### RS-422 / RS-485

* Differential long-distance communication
* Multi-drop network capability
* Industrial noise immunity
* Direction control handled by firmware

### UART

* Standard serial communication interface
* Ideal for microcontroller communication and debugging

### I²C

* Sensor and peripheral communication
* EEPROMs
* ADCs and DACs
* Environmental sensors

### SPI

* High-speed peripheral communication
* Displays
* Memory devices
* Data converters
* High-speed sensors

---

## Digital Inputs

### 3x Isolated Inputs

Galvanically isolated inputs designed for industrial environments.

Features:

* Ground loop protection
* Improved noise immunity
* Safe connection to external equipment
* PLC and sensor compatibility

Applications:

* Machine monitoring
* Industrial sensors
* Switch inputs
* Process control systems

---

## Output Interfaces

### 2x Relay Outputs

Integrated relay outputs for switching external devices.

Applications:

* Equipment control
* Power switching
* Automation systems
* Test fixtures
* Safety interlocks

---

### 4x MOSFET Outputs

Low-side MOSFET drivers for directly controlling external loads.

Applications:

* Solenoids
* LEDs
* Small motors
* Relays
* Indicators
* Custom automation systems

---

## General Purpose I/O

### 4x GPIO Channels

Flexible digital I/O for:

* External sensors
* Logic interfacing
* Prototyping
* Custom expansion hardware
* Embedded development

---

## Analog Interfaces

### Analog Inputs

Dedicated analog channels allow direct connection to sensors and measurement circuits.

Applications:

* Temperature sensors
* Voltage monitoring
* Current sensing
* Industrial transmitters
* Data acquisition

### Analog Output

Single analog output channel suitable for:

* Control signals
* Reference voltages
* External instrumentation
* Test equipment

---

## User Interface

### Status LEDs

Four onboard LEDs provide:

* System status indication
* Communication activity indicators
* Diagnostic information
* Application-defined signaling

---

## USB Connectivity

* USB Type-C connector
* USB Full-Speed device interface
* Virtual COM applications
* Firmware updates
* Data logging
* PC connectivity

---

# Hardware Architecture

The board is organized into dedicated functional blocks:

```
USB-C
   │
STM32F446VET6
   ├── CAN Interface
   ├── RS-232 Interface
   ├── RS-422 Interface
   ├── RS-485 Interface
   ├── UART Interfaces
   ├── I²C Interfaces
   ├── SPI Interfaces
   ├── Isolated Inputs
   ├── Relay Outputs
   ├── MOSFET Outputs
   ├── Analog Inputs
   ├── Analog Output
   ├── GPIO Expansion
   └── Status LEDs
```

---

# Firmware

The firmware is written in C and follows a modular architecture that separates hardware drivers from communication protocols and application logic.

## Firmware Responsibilities

### System Initialization

* Clock configuration
* Peripheral initialization
* Interface configuration
* Self-test routines

### Communication Services

* CAN message handling
* RS-232 communication
* RS-422 communication
* RS-485 communication
* USB communication
* UART bridging
* I²C transactions
* SPI transactions

### I/O Services

* Digital input monitoring
* Relay control
* MOSFET output control
* Analog acquisition
* GPIO management

### Application Layer

* Command parser
* Protocol routing
* Event handling
* Diagnostics
* Future protocol extensions

---

# Repository Structure

```
Universal_interface_adapter/
├── firmware/          Firmware source code
├── bom/               Bill of Materials
├── production/        Manufacturing files
├── libs/              Custom KiCad libraries
├── *.kicad_sch        Schematics
├── *.kicad_pcb        PCB layout
├── *.step             3D model
└── schematic_pdf.pdf  Complete schematic documentation
```

---

# Applications

## Industrial Automation

* PLC integration
* Factory diagnostics
* Equipment monitoring
* Protocol conversion

## Embedded Development

* Firmware development
* Hardware bring-up
* Peripheral testing
* Prototyping

## Automotive Development

* CAN experimentation
* ECU diagnostics
* Communication gateways

## Automated Test Systems

* Manufacturing fixtures
* Hardware-in-the-loop testing
* Sensor validation
* Production diagnostics

## Education

* Embedded systems laboratories
* Industrial communication training
* Electronics courses
* Protocol experimentation

---

# Open Hardware

This project is fully open source.

Included in this repository:

* Complete KiCad project
* Schematics
* PCB layout
* Manufacturing files
* Bill of Materials
* Firmware source code
* 3D STEP model

Users are encouraged to study, modify, and extend the design.

---

# Future Commercial Availability

The Universal Interface Adapter is currently under active development.

Planned future offerings:

* Fully assembled and tested boards
* Pre-programmed firmware
* Documentation and example projects
* PC configuration software
* Protocol bridge applications
* Optional enclosure kit
* Long-term firmware support

---

# Vision

The goal of the Universal Interface Adapter is simple:

**Provide developers and engineers with a single board capable of interfacing with nearly every common industrial and embedded communication standard without requiring multiple adapters or development platforms.**

**One board. Multiple protocols. Unlimited possibilities.**

