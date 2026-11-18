# Lecture Parallel Storage Systems

Parallel programming is becoming increasingly important since even phones and laptops contain multiple processor cores nowadays. Supercomputers can contain up to several million cores and have become a useful and important tool for a wide range of scientific domains. The analyses and simulations enabled by them have accelerated the process of gaining scientific insight considerably.

The amount of collected and produced data is growing exponentially; it has to be stored, analyzed and processed efficiently since I/O significantly affects overall performance. Vastly different rates of performance development for processors and storage hardware result in a performance imbalance, which makes it even more important to take a close look at storage systems in order to be able to meet future demands.

The lecture will teach the fundamentals of parallel storage systems and I/O; the exercises will allow transferring and applying the acquired skills with a system programming language such as C, C++ or Rust.

As part of the lecture, we will cover the complete storage stack: Storage devices and networks (hard disk drives, solid-state disks, storage area networks etc.), local and distributed file systems (in kernel and user space, novel concepts like snapshots and deduplication) as well as the I/O interfaces layered on top (POSIX, MPI-IO, NetCDF and ADIOS). Moreover, we will discuss reasons and solutions for performance problems as well as alternative approaches for I/O (such as cloud interfaces). Problems and examples will be motivated using real-world scientific applications.

### Course

*   Lecture: Thursday 09:15-10:45 (G22A-111)
*   Exercises: Tuesday 15:15-16:45 (G29-K058)
*   Contact: [Michael Kuhn](/index.php?site=materialkit&id=902&lang=de "Michael Kuhn") and [Michael Blesel](/index.php?site=materialkit&id=924&lang=de "Michael Blesel")

### Learning Objective

Participants will learn how parallel applications perform I/O using different programming concepts and how I/O can be optimized. Additionally, they will gain insight into and practical experience with the internals of storage and file systems.

### Requirements

Required skills:

*   Practical knowledge of a programming language and the ability to create simple applications

Recommended skills:

*   Basic knowledge about operating systems
*   Basic knowledge about parallel programming

### Lecture

*   2026-04-09: Introduction ([Slides](PSS-2026-01-Introduction.pdf "PSS-2026-01-Introduction"))
*   2026-04-16: Storage Devices ([Slides](PSS-2026-02-Storage-Devices.pdf "PSS-2026-02-Storage-Devices"))
*   2026-04-23: File Systems ([Slides](PSS-2026-03-File-Systems.pdf "PSS-2026-03-File-Systems"))
*   2026-04-30: Modern File Systems ([Slides](PSS-2026-04-Modern-File-Systems.pdf "PSS-2026-04-Modern-File-Systems"))
*   2026-05-07: Parallel Distributed File Systems ([Slides](PSS-2026-05-Parallel-Distributed-File-Systems.pdf "PSS-2026-05-Parallel-Distributed-File-Systems"))
*   2026-05-14: Public holiday
*   2026-05-21: MPI-IO ([Slides](PSS-2026-06-MPI-IO.pdf "PSS-2026-06-MPI-IO"))
*   2026-05-28: Libraries ([Slides](PSS-2026-07-Libraries.pdf "PSS-2026-07-Libraries"))
*   2026-06-04: Optimizations ([Slides](PSS-2026-08-Optimizations.pdf "PSS-2026-08-Optimizations"))
*   2026-06-11: Performance Analysis ([Slides](PSS-2026-09-Performance-Analysis.pdf "PSS-2026-09-Performance-Analysis"))
*   2026-06-18: Data Reduction ([Slides](PSS-2026-10-Data-Reduction.pdf "PSS-2026-10-Data-Reduction"))
*   2026-06-25: Skipped
*   2026-07-02: Current and Future Developments ([Slides](PSS-2026-11-Future-Developments.pdf "PSS-2026-11-Future-Developments"))
*   2026-07-09: Research Talks and Debriefing

### Exercises

*   2026-04-06: Introduction ([sheet 0](00-Organization.pdf "00-Organization"), [sheet 1](01-Introduction.pdf "01-Introduction"), [materials](PSS-2026-Exercise-01-Materials.tgz "PSS-2026-Exercise-01-Materials"))
    *   Deadline: 2026-04-19, 23:59
*   2026-04-20: Debugging and Checkpoints ([sheet 2](02-Checkpoints.pdf "02-Checkpoints"), [materials](PSS-2026-Exercise-02-Materials.tgz "PSS-2026-Exercise-02-Materials"))
    *   Deadline: 2026-05-03, 23:59
*   2026-05-04: I/O Tools ([sheet 3](03-IO-Tools.pdf "03-IO-Tools"), [materials](PSS-2026-Exercise-03-Materials.tgz "PSS-2026-Exercise-03-Materials"))
    *   Deadline 2026-05-10, 23:59
*   2026-05-11: Dummy File System ([sheet 4](04-Dummy-FUSE.pdf "04-Dummy-FUSE"), [materials](PSS-2026-Exercise-04-Materials.tgz "PSS-2026-Exercise-04-Materials"))
    *   Deadline: 2026-05-24, 23:59
*   2026-05-25: Memory File System ([sheet 5](05-Memory-FUSE.pdf "05-Memory-FUSE"), [materials](PSS-2026-Exercise-05-Materials.tgz "PSS-2026-Exercise-05-Materials"))
    *   Deadlines: 2026-05-31 and 2026-06-14, 23:59
*   2026-06-15: Persistent File System ([sheet 6](06-Persistent-FUSE.pdf "06-Persistent-FUSE"), [materials](PSS-2026-Exercise-06-Materials.tgz "PSS-2026-Exercise-06-Materials"))
    *   Deadline: 2026-06-28 and 2026-07-05, 23:59

### Literature

*   High Performance Parallel I/O (Prabhat und Quincey Koziol)
