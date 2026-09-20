# Sprint Roadmap
# Experimental Performance-First Desktop Operating System

**Status:** Baseline roadmap  
**Roadmap philosophy:** Build incrementally, verify every milestone, benchmark continuously.

---

# Roadmap Rules

Every sprint contains:

- Objective
- Work
- Verification
- End goal

No sprint is considered complete because code compiles. It is complete when the stated behavior is demonstrated.

Core loop:

**Build → Boot → Test → Measure → Record → Decide → Continue**

The roadmap intentionally starts with boring infrastructure. The project cannot research high-level architecture until the machine can reliably boot and measure low-level behavior.

---

# Phase 0 — Project Foundation

## S00 — Repository and Development Environment

### Objective
Create a reproducible Rust OS development environment on Windows.

### Work
- Initialize Git repository
- Configure Rust toolchain
- Establish workspace
- Create build scripts
- Establish QEMU invocation
- Establish debug output
- Create architecture documentation
- Create benchmark result format
- Establish coding conventions

### Verification
- Repository builds
- Toolchain versions are recorded
- QEMU launches
- Debug output can be captured
- A clean checkout can reproduce the environment

### End Goal
A reproducible project foundation exists.

---

## S01 — UEFI Boot

### Objective
Boot a minimal Rust payload through UEFI.

### Work
- UEFI application
- Boot entry
- Basic framebuffer discovery
- Memory map retrieval
- Kernel handoff structure
- Serial/debug output where available

### Verification
- QEMU UEFI boot succeeds
- Boot information is printed
- Memory map is visible
- Failure states are diagnosable

### End Goal
The OS can reliably reach its own kernel entry point.

---

## S02 — Kernel Entry and CPU Initialization

### Objective
Establish the first real kernel environment.

### Work
- CPU initialization
- GDT
- IDT foundation
- Interrupt handling
- Exception handlers
- Basic panic path
- Early allocator

### Verification
- Intentional divide-by-zero produces a controlled kernel exception
- Invalid memory access produces controlled failure
- Interrupts function
- Kernel remains stable after initialization

### End Goal
A minimal functioning x86-64 kernel exists.

---

# Phase 1 — Privileged Framework

## S03 — Physical Memory

### Objective
Create reliable physical frame management.

### Work
- Physical memory map
- Frame allocator
- Reserved-region handling
- Kernel memory regions
- Allocation diagnostics

### Verification
- Allocate/free large numbers of frames
- Detect double free
- Detect invalid ranges
- Stress under QEMU

### End Goal
Physical memory is managed safely.

---

## S04 — Virtual Memory

### Objective
Implement x86-64 page tables.

### Work
- Page table abstraction
- Kernel mappings
- User mappings foundation
- Mapping/unmapping
- Permission flags
- Guard pages

### Verification
- Map/unmap memory
- Enforce read/write/execute permissions
- Test page faults
- Stress mapping operations

### End Goal
The OS has controlled virtual memory.

---

## S05 — Safe Hardware Abstraction

### Objective
Establish the privileged framework that contains unsafe operations.

### Work
- Hardware abstraction interfaces
- Unsafe boundary rules
- Safe wrappers
- Interrupt APIs
- Memory APIs
- CPU APIs
- Documentation for invariants

### Verification
- Unsafe code inventory exists
- Unsafe operations are concentrated
- Safe callers cannot violate documented invariants through normal APIs

### End Goal
The framekernel-inspired safety boundary exists.

---

# Phase 2 — Execution

## S06 — Kernel Threads

### Objective
Run multiple kernel execution contexts.

### Work
- Context switching
- Kernel threads
- Thread state
- Stack management
- Synchronization primitives

### Verification
- Multiple threads execute concurrently
- Context switch stress test passes
- Synchronization stress tests pass

### End Goal
The kernel can execute concurrent work.

---

## S07 — First Scheduler

### Objective
Implement the first measurable scheduler.

### Work
- Run queue
- Time slices
- Priorities
- CPU accounting
- Basic affinity
- Scheduler tracing

### Verification
Measure:

- Context switch latency
- Throughput
- Fairness
- Tail latency
- Interactive response

### End Goal
A working scheduler with baseline measurements.

---

## S08 — Scheduler Research Harness

### Objective
Make scheduler research a first-class capability.

### Work
- Workload generator
- Scheduler benchmarks
- Parameter configuration
- Trace capture
- Result comparison

### Verification
A scheduler parameter change produces a reproducible before/after report.

### End Goal
Scheduling becomes measurable rather than intuitive.

---

# Phase 3 — User Space

## S09 — User Address Spaces

### Objective
Run code outside the kernel.

### Work
- User page tables
- User stacks
- User entry/return
- Protection boundaries
- Fault handling

### Verification
- User code executes
- User code cannot access kernel memory
- Invalid accesses are contained

### End Goal
A real user/kernel boundary exists.

---

## S10 — Processes and IPC

### Objective
Establish the process model.

### Work
- Process abstraction
- Threads
- Handles/resources
- IPC
- Basic process lifecycle

### Verification
- Spawn process
- Communicate between processes
- Terminate process
- Confirm resource cleanup

### End Goal
The OS can run multiple isolated programs.

---

## S11 — System Call ABI

### Objective
Create the first stable application interface.

### Work
- Syscall dispatch
- Argument validation
- Error model
- Basic process/memory/I/O calls

### Verification
A small userspace program can execute entirely through documented system calls.

### End Goal
Applications have a defined OS interface.

---

# Phase 4 — Storage and I/O

## S12 — Block Device Layer

### Objective
Abstract physical storage without excessive layering.

### Work
- Block interface
- Request queues
- Completion handling
- Error reporting
- Basic storage benchmark

### Verification
- Read/write correctness
- Queue stress
- Latency measurements
- Throughput measurements

### End Goal
Storage is usable through a measured abstraction.

---

## S13 — NVMe

### Objective
Support the project's primary modern storage target.

### Work
- NVMe discovery
- Queue setup
- Commands
- Completion handling
- Interrupt/polling modes

### Verification
Measure:

- Sequential throughput
- Random I/O
- IOPS
- Latency
- CPU utilization

### End Goal
The OS can directly use an NVMe device.

---

## S14 — First Filesystem

### Objective
Create a minimal native filesystem.

### Work
- Metadata
- Allocation
- Directories
- Files
- Crash-consistency foundation
- Mount interface

### Verification
- Create/read/write/delete
- Large file testing
- Directory stress
- Reboot persistence

### End Goal
The OS has persistent storage.

---

## S15 — I/O Research Harness

### Objective
Compare traditional and experimental I/O paths.

### Work
Test:

- Interrupts
- Polling
- Batching
- Direct I/O
- Zero-copy candidates
- Queue depth

### Verification
Produce workload-specific comparisons.

### End Goal
I/O architecture is evidence-driven.

---

# Phase 5 — Networking

## S16 — Network Hardware

### Objective
Bring up Ethernet networking.

### Work
- NIC abstraction
- Driver
- RX/TX queues
- Interrupts
- Polling

### Verification
- Link
- Packet receive/transmit
- Packet loss test
- CPU overhead

### End Goal
Raw network connectivity works.

---

## S17 — Network Stack

### Objective
Implement practical networking.

### Work
- Ethernet
- IPv4/IPv6 foundation
- UDP
- TCP
- DNS

### Verification
- DHCP/static configuration
- DNS lookup
- Ping
- TCP transfer
- UDP transfer

### End Goal
Normal desktop networking works.

---

## S18 — High-Performance Networking Research

### Objective
Investigate low-copy and userspace-friendly networking.

### Work
- Zero-copy experiments
- Buffer ownership
- Polling thresholds
- Batch processing
- Application hints

### Verification
Compare throughput, latency, CPU cost, and tail latency.

### End Goal
At least one measured optimized networking path exists or is rejected with evidence.

---

# Phase 6 — Devices and Input

## S19 — USB Foundation

### Objective
Support essential USB devices.

### Work
- USB host controller foundation
- Keyboard
- Mouse
- Storage discovery

### Verification
Input devices work in QEMU and on target hardware where supported.

### End Goal
The OS is interactively controllable.

---

## S20 — Input/Event Model

### Objective
Create a low-overhead input event pipeline.

### Work
- Input events
- Device identity
- Event queues
- Timestamping
- User delivery

### Verification
Measure input latency from device event to userspace.

### End Goal
Input is reliable and measurable.

---

# Phase 7 — Graphics and Desktop

## S21 — Framebuffer Desktop Prototype

### Objective
Create the first visual environment without requiring a full GPU stack.

### Work
- Framebuffer output
- Basic compositor prototype
- Pointer
- Keyboard
- Simple windows

### Verification
- Display output
- Move windows
- Input works
- No kernel instability

### End Goal
A primitive graphical desktop exists.

---

## S22 — Display Architecture

### Objective
Define the long-term graphics boundary.

### Work
- Display device abstraction
- Buffer management
- Presentation
- Synchronization
- Compositor architecture

### Verification
Measure:

- Frame latency
- CPU usage
- Copy count
- Memory bandwidth

### End Goal
A measured display pipeline exists.

---

## S23 — GPU Research Path

### Objective
Begin real GPU acceleration.

### Work
- Select initial hardware target
- GPU memory management
- Command submission
- Vulkan-oriented integration
- Synchronization

### Verification
Run a basic accelerated graphics workload and measure GPU/CPU overhead.

### End Goal
The OS can perform real hardware-accelerated graphics on the first supported GPU.

---

## S24 — Desktop Shell

### Objective
Create a usable desktop.

### Work
- Launcher
- Task management
- Window controls
- Settings
- File manager
- Network/audio controls
- Session management

### Verification
A user can boot, launch, move, close, and manage applications.

### End Goal
A basic daily-desktop workflow is possible.

---

# Phase 8 — Audio and Multimedia

## S25 — Audio

### Objective
Support normal desktop audio.

### Work
- Audio device abstraction
- Playback
- Capture
- Mixer
- Application streams

### Verification
- Play audio
- Record audio
- Multiple streams
- Latency test

### End Goal
Basic desktop multimedia works.

---

# Phase 9 — Application Platform

## S26 — Native Application SDK

### Objective
Make native application development practical.

### Work
- Standard library subset
- Runtime
- Window API
- File API
- Networking API
- Build templates
- Documentation

### Verification
Build several small native applications.

### End Goal
Developers can write applications without touching kernel internals.

---

## S27 — POSIX Compatibility Layer

### Objective
Support useful existing software concepts.

### Work
- POSIX-like process API
- Files
- Pipes
- Sockets
- Threads
- Signals/events where appropriate

### Verification
Port real small utilities and development tools.

### End Goal
Existing software can begin moving to the platform.

---

## S28 — Linux ABI Compatibility Investigation

### Objective
Determine how much Linux compatibility is practical without compromising architecture.

### Work
- Map Linux syscall requirements
- Compatibility layer prototype
- Benchmark overhead
- Identify incompatible assumptions

### Verification
Run representative Linux software and measure performance.

### End Goal
A data-driven decision about Linux ABI scope.

---

# Phase 10 — Security and Privacy

## S29 — Security Baseline

### Objective
Establish foundational desktop security.

### Work
- Memory protections
- ASLR
- NX
- Privilege boundaries
- Permission model
- Secure configuration

### Verification
Attempt documented isolation violations.

### End Goal
Basic security invariants are demonstrated.

---

## S30 — Secure Boot and Update Foundations

### Objective
Create trustworthy boot/update foundations.

### Work
- Signed components
- Update verification
- Rollback strategy
- Recovery environment

### Verification
- Reject modified image
- Verify legitimate update
- Recover failed update

### End Goal
The system has a defensible update chain.

---

## S31 — Privacy Baseline

### Objective
Verify the OS's privacy-by-absence philosophy.

### Work
- Audit network activity
- Inventory background services
- Verify telemetry absence
- Document exceptions

### Verification
Fresh installation demonstrates no unexpected network communication.

### End Goal
Privacy claims become measurable.

---

# Phase 11 — Performance Research Platform

## S32 — Benchmark Infrastructure

### Objective
Make performance measurement part of the OS workflow.

### Work
- Benchmark runner
- Hardware identification
- Configuration capture
- Result storage
- Comparison tooling
- Report generation

### Verification
Run the same workload repeatedly and generate reproducible reports.

### End Goal
Performance experiments are automated.

---

## S33 — Runtime Performance Controls

### Objective
Make major performance policies tunable.

### Work
- Scheduler knobs
- I/O knobs
- Memory knobs
- Network knobs
- Policy profiles

### Verification
Change policies at runtime and demonstrate measurable behavior changes.

### End Goal
The OS can be experimentally tuned without rebuilding for every policy change.

---

## S34 — Application Performance Accounting

### Objective
Understand where resources actually go.

### Work
Expose per-process:

- CPU
- RAM
- Disk
- Network
- GPU
- Wakeups
- Scheduling delays

### Verification
Resource measurements reconcile against system totals within documented tolerances.

### End Goal
The OS can explain its own resource consumption.

---

# Phase 12 — Real Hardware

## S35 — First Bare-Metal Boot

### Objective
Boot the OS on physical x86-64 hardware.

### Work
- UEFI validation
- Hardware discovery
- Recovery/debugging
- Driver fixes

### Verification
Stable boot and basic interaction on development PC.

### End Goal
The OS is no longer VM-only.

---

## S36 — Hardware Compatibility Baseline

### Objective
Support enough hardware for meaningful desktop testing.

### Work
- CPU
- RAM
- NVMe
- USB
- Ethernet/Wi-Fi where practical
- Display
- Audio
- GPU

### Verification
Create a hardware compatibility matrix.

### End Goal
Known supported hardware is documented.

---

# Phase 13 — Real-World Benchmarking

## S37 — Windows Comparison

### Objective
Benchmark against Windows 11 on identical hardware.

### Work
Measure equivalent workloads.

### Verification
Produce reproducible results.

### End Goal
The project has honest evidence of where it is faster/slower.

---

## S38 — Linux Comparison

### Objective
Benchmark against mainstream Linux and a performance-focused distribution.

### Work
- Ubuntu baseline
- CachyOS baseline
- Equivalent workloads

### Verification
Produce workload-specific comparisons.

### End Goal
Performance claims are grounded in multiple real-world baselines.

---

## S39 — Low-End Hardware Study

### Objective
Determine whether low-overhead architecture produces meaningful benefits on constrained hardware.

### Work
Test:

- 4–8GB systems
- Integrated graphics
- Older/slower CPUs
- Slow storage where available

Measure:

- Idle memory
- Responsiveness
- Application performance
- Browser performance
- Compilation
- Multitasking

### Verification
Generate resource-normalized results.

### End Goal
Determine whether low-end hardware benefits materially from the OS architecture.

---

## S40 — High-End Scaling Study

### Objective
Determine whether the architecture remains competitive as hardware scales.

### Work
Test:

- High-core CPUs
- High-end GPUs
- Fast NVMe
- 32GB+ RAM

### Verification
Measure scaling efficiency and bottlenecks.

### End Goal
Identify where the OS itself becomes the bottleneck.

---

# Phase 14 — Application and Gaming Expansion

## S41 — Developer Workstation

### Objective
Make the OS practical for software development.

### Work
- Compiler toolchains
- Git
- Editors/IDE
- Terminal
- Debugger
- Build systems
- Containers/VM research as needed

### Verification
Build a real project from source.

### End Goal
The OS can serve as a development workstation.

---

## S42 — Browser Platform

### Objective
Run a serious browser workload.

### Work
- Browser port/native build
- GPU acceleration
- Audio/video
- Sandboxing as appropriate

### Verification
Daily browsing workload and benchmark suite.

### End Goal
Normal web use is practical.

---

## S43 — Gaming Foundation

### Objective
Establish the gaming stack.

### Work
- GPU stability
- Vulkan
- Input
- Audio
- Filesystem performance
- Controller support
- Frame pacing

### Verification
Run representative games/workloads where technically available.

### End Goal
The project can begin meaningful gaming benchmarks.

---

## S44 — Gaming Performance Research

### Objective
Measure the full application-to-hardware path.

### Work
Measure:

- FPS
- 1% lows
- Frame time
- Input latency
- CPU overhead
- GPU utilization
- Memory use

### Verification
Compare identical hardware/configuration against Windows/Linux baselines.

### End Goal
Gaming performance is understood rather than assumed.

---

# Phase 15 — Architecture Experiments

## S45 — Microkernel Branch

### Objective
Build a limited microkernel implementation of a comparable subsystem.

### Work
- IPC
- User-space service
- Driver/service boundary
- Scheduling path

### Verification
Compare against framekernel path.

### End Goal
Microkernel overhead and benefits are measured.

---

## S46 — Exokernel-Inspired Fast Path

### Objective
Test direct application resource control.

### Work
Select one subsystem, preferably:

- Storage
- Networking
- Graphics

### Verification
Compare abstraction-heavy and direct paths.

### End Goal
Determine where lower-level control provides real benefit.

---

## S47 — Native Filesystem Research

### Objective
Evaluate a filesystem designed specifically for this OS.

### Work
- Storage layout
- Allocation
- Metadata
- Caching
- Crash consistency

### Verification
Compare realistic workloads against compatibility filesystems.

### End Goal
Either establish a measurable native advantage or document why the existing path remains preferable.

---

# Phase 16 — Daily Driver

## S48 — Desktop Reliability

### Objective
Turn the experimental desktop into a reliable daily environment.

### Work
- Crash recovery
- Update reliability
- Driver stability
- Session recovery
- Power management
- Logging

### Verification
Extended soak tests.

### End Goal
Multi-day operation without critical failures.

---

## S49 — Installer and Recovery

### Objective
Make installation practical.

### Work
- Installer
- Partitioning
- Boot entry
- User creation
- Recovery
- Rollback

### Verification
Fresh installation and recovery from failure.

### End Goal
A new machine can be installed without developer tooling.

---

## S50 — Daily Driver Candidate

### Objective
Evaluate the OS as an actual desktop.

### Work
Use it for:

- Development
- Browsing
- Media
- File management
- Gaming where supported
- Normal daily tasks

Record every blocker.

### Verification
Complete a defined real-world workload period.

### End Goal
A documented daily-driver candidate with known limitations.

---

# Phase 17 — Continued Research

## S51 — Architecture Review

### Objective
Review the entire architecture using accumulated measurements.

### Work
- Identify bottlenecks
- Remove unnecessary subsystems
- Replace weak abstractions
- Review unsafe code
- Review compatibility costs

### Verification
Produce an architecture review report.

### End Goal
Version 1 architecture is based on evidence rather than original assumptions.

---

## S52 — Performance Tuning Campaign

### Objective
Systematically optimize the OS.

### Work
Profile:

- Scheduler
- Memory
- I/O
- Networking
- Graphics
- Filesystem
- Desktop

### Verification
Every optimization includes before/after measurements.

### End Goal
Demonstrable application-level performance improvements.

---

## S53 — Low-End Optimization Campaign

### Objective
Optimize specifically for constrained machines.

### Work
- Background work
- Memory pressure
- Graphics overhead
- Storage behavior
- Scheduler policy

### Verification
Repeat low-end benchmark suite.

### End Goal
Determine whether the OS can provide unusually strong usability on weak hardware.

---

## S54 — High-End Optimization Campaign

### Objective
Optimize scaling.

### Work
- Parallelism
- NUMA
- I/O queues
- GPU submission
- Scheduler scaling

### Verification
Repeat high-end benchmark suite.

### End Goal
Reduce OS bottlenecks on powerful systems.

---

## S55 — Version 1 Release Candidate

### Objective
Freeze a coherent first production-oriented architecture.

### Work
- Documentation
- Installer
- Update system
- Hardware compatibility
- Application support
- Security audit
- Performance regression suite

### Verification
All release-critical tests pass.

### End Goal
A credible version 1 release candidate exists.

---

# Definition of Done for the Roadmap

A sprint is not done when:

- It compiles
- A screenshot exists
- QEMU boots once
- A benchmark produces a number once

A sprint is done when:

1. The feature works
2. Failure behavior is understood
3. Tests exist
4. Performance is measured where relevant
5. The result is reproducible
6. The result is documented
7. Existing functionality has not been accidentally removed

---

# Research Decision Gates

At major milestones, the project may change architecture.

### Gate A — Framekernel viability
After basic process, memory, and driver work.

### Gate B — Scheduler viability
After scheduler benchmark suite.

### Gate C — I/O architecture
After storage/network experiments.

### Gate D — Graphics architecture
After first real GPU support.

### Gate E — Compatibility strategy
After representative application tests.

### Gate F — Daily-driver viability
After real hardware and extended real-world use.

The roadmap is intentionally allowed to change at these gates.

---

# Final Roadmap Principle

> The roadmap is a hypothesis, not a contract.

If measurements demonstrate that a different architecture is materially better for the project's goals, the roadmap should change.
