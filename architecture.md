# Architecture Document
# Experimental Performance-First Desktop Operating System

**Status:** Baseline Architecture  
**Target:** x86-64 desktop PCs  
**Boot:** UEFI  
**Primary implementation language:** Rust  
**Development environment:** Windows 11 host + QEMU initially  
**Long-term goal:** A practical daily-driver desktop OS

---

## 1. Project Vision

This project is an experimental desktop operating system built from the ground up around one primary question:

> How much operating system does a modern desktop computer actually need?

The goal is not to create "Linux but faster," "Windows without telemetry," or a reskinned existing distribution. The goal is to explore whether a smaller, modern, performance-first OS architecture can provide a complete desktop experience while leaving as much hardware capacity as possible to the user's applications.

The project begins as a research-oriented operating system but is deliberately designed toward eventual daily-driver use.

### Primary objective

**Performance first.**

The OS should minimize its own CPU, memory, I/O, latency, and background activity so that applications receive as much of the machine's resources as practical.

### Secondary objectives

- Privacy by default
- Security by architecture
- Memory safety
- Low idle overhead
- High application performance under load
- Measurable and tunable behavior
- Reproducible benchmarking
- Practical desktop usability
- Eventual broad application compatibility

---

## 2. Core Principles

### P1. Performance First

Performance is the primary architectural objective.

Measure:

- CPU overhead
- Memory overhead
- I/O overhead
- Scheduling latency
- Application startup
- Compilation
- File operations
- Network performance
- Graphics performance
- Gaming performance
- Responsiveness under load

Do not optimize solely for idle resource numbers. A low-memory OS that makes applications slower has failed the primary objective.

### P2. Give Hardware Back to the User

The OS exists to enable applications, not consume resources for its own sake.

Every persistent subsystem should justify:

1. Why it exists
2. What resources it consumes
3. What user-visible value it provides
4. Whether the same value can be delivered more cheaply

### P3. Do Not Copy or Reskin Existing Systems

Do not reproduce an existing OS or distribution merely to obtain a familiar result.

This means:

- No "Linux with a new desktop"
- No Debian/Arch/Ubuntu reskin as the core project
- No copying an existing kernel architecture simply because it is familiar
- No recreating mature components from scratch when a standard, interoperable interface is sufficient

The distinction is important:

> **Do not copy implementations. Do not reject standards.**

Use established standards and protocols where they prevent needless reinvention. Build our own underlying architecture where experimentation is the point.

For example, using UEFI, Vulkan, POSIX-inspired interfaces, standard executable formats, or established display protocols does not make the OS a copy. Reusing an implementation wholesale when a clean-room/native implementation would materially support the project's goals does.

### P4. Reuse the Wheel Where It Makes Sense, Reinvent the Wheel Where It Matters

"Don't reinvent the wheel" applies selectively.

Reuse:

- Specifications
- Standards
- Proven algorithms
- Existing interoperability formats
- Hardware documentation
- Testing methodologies
- Mature developer tooling

Question:

- Traditional kernel abstractions
- Scheduler assumptions
- Storage paths
- I/O models
- Memory policies
- Application/kernel boundaries
- Desktop architecture
- Resource-management policies

The project should spend original engineering effort where it can produce measurable architectural value.

### P5. Measure Before Declaring Victory

No architecture is assumed to be faster because it sounds faster.

Major decisions require:

**Change → benchmark → record → compare → decide.**

Baseline systems include:

- Windows 11
- Mainstream Linux
- Performance-focused Linux such as CachyOS

Results must be workload-specific rather than reduced to one overall winner.

### P6. Privacy by Absence

The OS should not need a large privacy subsystem if it simply avoids unnecessary data collection.

Default policy:

- No telemetry
- No behavioral analytics
- No advertising
- No mandatory online account
- No cloud dependency for core operation
- No unnecessary background network services

Privacy mechanisms should themselves be measured for performance cost.

### P7. Security by Architecture

Security should come primarily from architecture rather than layers of continuously running software.

Priorities include:

- Memory-safe kernel code
- Minimal unsafe code
- Least privilege
- Strong process boundaries
- ASLR/NX and modern hardware protections
- Secure boot/update foundations
- Signed system components where appropriate
- Auditable security-critical code
- Capability-oriented mechanisms where they provide measurable value

Security features are evaluated against performance cost rather than maximized independently.

### P8. Rust First, Not Rust Only

Rust is the primary implementation language because it provides native systems performance while substantially reducing memory-safety risk.

Expected policy:

- Rust for kernel and OS services
- `no_std` Rust for low-level environments
- C/assembly only when technically justified
- Unsafe Rust concentrated into small, auditable areas
- Avoid unnecessary FFI

The architecture should never turn "Rust purity" into a performance or hardware-support handicap.

### P9. The OS Is the Research Laboratory

There is no separate research-lab prerequisite.

The OS itself is the laboratory.

Every major subsystem should be observable, benchmarkable, and replaceable enough to support controlled experiments.

### P10. Compatibility Is a Goal, Not the Foundation

Compatibility matters because the eventual target is a real desktop.

However, compatibility should not force the project to inherit every historical design decision of another OS.

Strategy:

1. Native applications and tools
2. Stable system interfaces
3. POSIX/Linux compatibility where useful
4. Compatibility layers where they provide practical value
5. Windows compatibility only where justified by actual user demand

Compatibility mechanisms should be measured for their performance cost.

### P11. Experimental Fast Paths

The OS should provide normal, robust paths plus optional optimized paths.

Examples:

- Standard I/O + direct/zero-copy I/O
- Standard networking + high-performance networking
- General filesystem + application-aware storage paths
- General scheduler + workload-aware policy
- Standard graphics path + optimized presentation path

The fast path must never make the ordinary desktop unusable.

### P12. Simplicity Is a Performance Feature

Every daemon, abstraction, service, copy, context switch, synchronization point, and background task is a potential cost.

Prefer:

- Fewer layers
- Explicit ownership
- Short data paths
- Predictable behavior
- Small interfaces
- Measurable policies

But do not confuse fewer lines with better architecture.

---

## 3. Architectural Direction

### Primary candidate: Framekernel-inspired architecture

The architecture will begin from a framekernel-style direction:

- Single kernel address space where appropriate
- Small privileged framework
- OS services built above safe abstractions
- Rust-first implementation
- Concentrated unsafe code
- Low-cost intra-kernel communication

This direction is strongly motivated by recent framekernel research demonstrating a Rust OS can combine general-purpose functionality, Linux ABI compatibility, memory-safety-oriented TCB reduction, and competitive performance. This project will use those ideas as research input rather than copying Asterinas.

### Major influence: Exokernel principles

The OS should selectively expose lower-level control where abstraction overhead matters.

Especially:

- Storage
- Networking
- Memory
- Graphics
- High-performance applications

This does not require implementing a literal exokernel.

### Serious alternative: Microkernel

A microkernel implementation remains a research branch.

It should be possible to build a comparable subsystem and benchmark:

- IPC cost
- Driver overhead
- Scheduling behavior
- I/O latency
- Application throughput
- Fault isolation benefits

The project should not assume microkernels are slow or fast.

### Selective influences

| Concept | Role |
|---|---|
| Framekernel | Primary architectural direction |
| Exokernel | Major conceptual influence |
| Microkernel | Benchmarkable alternative |
| Capability systems | Security research direction |
| Unikernels | Specialized workload techniques |
| Multikernel | Research influence |
| Linux | Compatibility/reference baseline |
| Windows | Real-world desktop baseline |

---

## 4. High-Level Architecture

```text
+-------------------------------------------------------------+
|                         Applications                         |
| Browsers | Games | IDEs | Native Apps | Compatibility Apps |
+-------------------------------------------------------------+
|                     Desktop Environment                     |
| Window Manager | Compositor | Shell | System UI | Settings |
+-------------------------------------------------------------+
|                    User-Space Services                      |
| Networking | Audio | Graphics Services | Compatibility     |
| Storage Services | Device Services | System Utilities      |
+-------------------------------------------------------------+
|                  OS Service / Kernel Layer                   |
| Scheduler | Memory | VFS | Network | IPC | Process | I/O    |
+-------------------------------------------------------------+
|                    Privileged Framework                     |
| MMU | Interrupts | CPU | Hardware Abstraction | Unsafe Rust |
+-------------------------------------------------------------+
|                         Hardware                            |
| CPU | RAM | NVMe | GPU | NIC | USB | Audio | Displays       |
+-------------------------------------------------------------+
```

The exact privilege boundaries remain experimentally adjustable.

---

## 5. Core Subsystems

### 5.1 Boot

- UEFI x86-64
- Bootloader
- Early CPU initialization
- Memory map acquisition
- Page table setup
- Interrupt initialization
- Kernel handoff

### 5.2 CPU and Scheduling

Research areas:

- Scheduler design
- CPU affinity
- Priority
- Interactive responsiveness
- Throughput
- Background task handling
- Energy/performance tradeoffs

Scheduler policy must be measurable and eventually tunable.

### 5.3 Memory

Initial implementation:

- Conventional x86-64 paging
- Virtual memory
- Physical frame allocator
- Kernel/user address separation
- Demand paging foundations

Future research:

- Tiered memory
- NUMA
- Huge pages
- Application hints
- Memory-pressure policy
- Hardware-specific translation behavior

### 5.4 Processes and Threads

Provide:

- Process creation
- Threads
- Address spaces
- Signals/events as appropriate
- IPC
- Resource accounting
- Permissions

Keep the process model understandable and lightweight.

### 5.5 Storage

Initial path:

- Block-device abstraction
- NVMe
- Simple filesystem
- VFS compatibility layer

Future research:

- Native filesystem
- Direct I/O
- Zero-copy paths
- Application-aware caching
- Queue optimization
- Storage policy tuning

### 5.6 Networking

Initial path:

- Ethernet
- Basic IP stack
- UDP/TCP
- DNS
- Socket API

Future:

- Zero-copy networking
- Userspace networking
- Application-specific networking
- Adaptive polling/interrupt behavior

### 5.7 Graphics

Initial strategy:

- Hardware framebuffer for early bring-up
- GPU support through established standards
- Vulkan-oriented long-term graphics direction
- Modern display/compositor architecture

Graphics is expected to be one of the most difficult compatibility and performance areas.

### 5.8 Audio

Initial:

- Hardware abstraction
- Output/input
- Basic mixer

Later:

- Low-latency audio
- Application routing
- Professional workloads

### 5.9 USB and Devices

Start with the devices required for development:

- Keyboard
- Mouse
- Storage
- Display
- Network

Expand based on real hardware testing.

---

## 6. Desktop Architecture

The first desktop should be deliberately small.

Target:

- Application launcher
- Task/window management
- Windowed applications
- File manager
- Settings
- Network controls
- Audio controls
- Power/session controls

The UI should feel familiar without attempting to reproduce Windows.

A future differentiator is **resource transparency**:

> Let the user understand why a process is consuming CPU, RAM, disk, network, or GPU resources.

This should be integrated into the OS rather than becoming a collection of third-party monitoring widgets.

---

## 7. Application Model

### Native

Preferred long-term path.

Applications should have:

- Stable system APIs
- Clear resource ownership
- Efficient IPC
- Efficient graphics
- Predictable file/network access

### Compatibility

Compatibility layers are introduced incrementally.

The architecture must avoid making compatibility the kernel's organizing principle.

Potential interfaces:

- POSIX-like APIs
- Linux-compatible syscall surface where useful
- Native OS API
- Eventually Windows application compatibility if technically worthwhile

---

## 8. Performance Research Framework

The OS includes a first-class benchmark subsystem.

### Measurements

#### OS overhead

- Idle RAM
- Idle CPU
- Background wakeups
- Background I/O
- Boot time
- Disk footprint
- Network activity

#### Application performance

- Startup
- Browser workloads
- IDE workloads
- Compilation
- Compression
- File operations
- Network transfer

#### Gaming

- FPS
- 1% lows
- Frame time
- Input latency
- CPU overhead
- GPU utilization

#### Stress

- CPU + GPU
- CPU + disk
- Memory pressure
- Compilation + browser
- Game + background applications

### Research record

Every benchmark should record:

- OS build
- Hardware
- Firmware configuration where relevant
- Kernel configuration
- Scheduler
- Memory policy
- I/O policy
- Filesystem
- Graphics mode
- Workload
- Result
- Date
- Commit/build identifier

---

## 9. Runtime Tunability

Performance policies should eventually be tunable without rebuilding the entire OS.

Potential tunables:

- Scheduler parameters
- Queue sizes
- Cache behavior
- I/O batching
- Network polling thresholds
- Memory reclamation thresholds
- Background task priorities

This is influenced by contemporary work such as Xkernel, which demonstrates runtime tuning of OS performance constants. The project's implementation will be native to this architecture rather than copying that implementation. citeturn0search4

---

## 10. Development Strategy

### Phase 1

Windows 11 host:

```text
Windows 11
   |
   +-- Rust toolchain
   |
   +-- Build system
   |
   +-- QEMU
          |
          +-- Our OS
```

### Phase 2

Real hardware:

```text
UEFI
 |
Our bootloader
 |
Our kernel
 |
Our desktop
 |
Applications
```

### Phase 3

Performance fleet:

- Low-end laptop
- Mainstream desktop
- High-end desktop

---

## 11. Technology Baseline

| Area | Initial choice |
|---|---|
| Architecture | x86-64 |
| Boot | UEFI |
| Kernel language | Rust |
| Low-level language | Assembly/C only where justified |
| Build | Cargo + custom OS build tooling |
| VM | QEMU |
| Host development | Windows initially |
| Filesystem | Experimental/simple initial FS + compatibility path |
| Graphics | Established GPU APIs/protocols |
| Networking | Native stack |
| Desktop | Native minimal desktop |
| Benchmarking | Built into OS/project tooling |
| Source control | Git |

---

## 12. Repository Structure

```text
os/
├── architecture.md
├── sprint-roadmap.md
├── implementation-guide.md
├── README.md
├── Cargo.toml
│
├── boot/
│   ├── uefi/
│   └── loader/
│
├── kernel/
│   ├── framework/
│   │   ├── cpu/
│   │   ├── interrupts/
│   │   ├── memory/
│   │   ├── paging/
│   │   └── hardware/
│   │
│   ├── scheduler/
│   ├── process/
│   ├── ipc/
│   ├── memory/
│   ├── io/
│   ├── storage/
│   ├── network/
│   └── drivers/
│
├── userspace/
│   ├── init/
│   ├── services/
│   ├── system/
│   ├── compatibility/
│   └── utilities/
│
├── desktop/
│   ├── compositor/
│   ├── window_manager/
│   ├── shell/
│   ├── launcher/
│   ├── settings/
│   └── file_manager/
│
├── graphics/
│   ├── display/
│   ├── gpu/
│   └── vulkan/
│
├── apps/
│
├── research/
│   ├── benchmarks/
│   ├── experiments/
│   ├── traces/
│   └── reports/
│
├── tests/
│   ├── unit/
│   ├── integration/
│   ├── kernel/
│   ├── drivers/
│   └── performance/
│
└── tools/
    ├── build/
    ├── image/
    ├── qemu/
    ├── benchmark/
    └── analysis/
```

---

## 13. Architectural Decision Rules

A proposed subsystem must answer:

1. Does it improve performance, usability, compatibility, security, or privacy?
2. What does it cost?
3. Can it be simpler?
4. Can it be optional?
5. Can it be measured?
6. Can it be replaced?
7. Does it force us to inherit unnecessary historical assumptions?
8. Are we implementing it because it is genuinely useful, or merely because existing operating systems do it?

The last question is especially important.

---

## 14. Non-Goals

At the beginning, the project will NOT attempt to:

- Support every CPU architecture
- Replace every existing application
- Reproduce Windows exactly
- Reproduce Linux exactly
- Implement every desktop feature
- Build a full mobile OS
- Solve every hardware device immediately
- Maximize security at unlimited performance cost
- Maximize minimalism at the expense of application performance
- Build a literal exokernel simply because exokernels are interesting
- Build a literal microkernel simply because microkernels are interesting

---

## 15. Success Criteria

The project succeeds in stages.

### Research success

We discover measurable evidence about which architectural choices improve or harm desktop performance.

### Technical success

The OS boots, runs programs, manages hardware, provides networking/storage/graphics, and supports a usable desktop.

### Practical success

The OS becomes capable of real development, browsing, media, and eventually gaming.

### Long-term success

It can serve as a daily-driver operating system while demonstrating that a modern desktop OS can be substantially more deliberate about its own resource consumption.

---

## 16. Guiding Statement

> Build the smallest amount of operating system necessary to provide a full desktop computing experience, then spend the remaining hardware resources on the user's applications.

A second statement governs experimentation:

> Never assume an operating-system abstraction is optimal simply because it is traditional. Measure it.
