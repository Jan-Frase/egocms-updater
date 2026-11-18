# Lecture Parallel Programming for Engineers

Parallel programming is becoming increasingly important, as even phones and laptops have several processor cores. Some supercomputers even consist of several million cores and have established themselves as a useful and indispensable tool for many areas of science. The resulting analyses and simulations have made it possible to significantly increase scientific insight in many areas.

However, the optimal use of these components is no easy task, which is why scientists are constantly faced with new challenges when developing efficient applications. A deeper understanding of the hardware and software environment as well as the possible causes of errors is therefore essential for parallel programming.

In the lecture, the basics of parallel programming are taught; the exercises serve the practical application and implementation of the acquired knowledge in the C programming language.

The lecture will cover some of the most important topics: Hardware and software concepts (multi-core processors, processes/threads, NUMA etc.), different approaches to parallel programming (OpenMP, POSIX threads, MPI) as well as tools for performance analysis and debugging (scalability, deadlocks, race conditions etc.). Moreover, reasons and solutions for performance problems are discussed and alternative approaches to parallel programming are presented. Examples and problems are illustrated using real scientific applications.

### Course

*   Lecture: Monday 9:15-10:45 Uhr (G05-118)
*   Exercise: Wednesday 13:15-14:45 (G29-K058)
*   Contact: [Michael Kuhn](/index.php?site=materialkit&id=902&lang=de "Michael Kuhn") and [Michael Blesel](/index.php?site=materialkit&id=924&lang=de "Michael Blesel")

### Learning Objective

Participants will learn how to create parallel programs using various programming approaches, how to execute them and how to optimize their execution. In addition, further concepts for parallelization are taught and put into practice in the exercises.

### Requirements

Required skills:

*   Practical knowledge of a programming language and the ability to create simple applications

Recommended skills:

*   Basic knowledge about operating systems
*   Basic knowledge about parallel programming

### Lecture

*   2026-04-06: Public holiday
*   2026-04-13: Introduction ([Slides](PPE-2026-01-Introduction.pdf "PPE-2026-01-Introduction"))
*   2026-04-20: Performance Analysis and Optimization ([Slides](PPE-2026-02-Performance.pdf "PPE-2026-02-Performance"), [Materials](PPE-2026-02-Performance-Materials.tgz "PPE-2026-02-Performance-Materials"))
*   2026-04-27: Hardware Architectures ([Slides](PPE-2026-03-Architectures.pdf "PPE-2026-03-Architectures"))
*   2026-05-04: Parallel Programming ([Slides](PPE-2026-04-Programming.pdf "PPE-2026-04-Programming"), [Materials](PPE-2026-04-Programming-Materials.tgz "PPE-2026-04-Programming-Materials"))
*   2026-05-11: Programming with OpenMP ([Slides](PPE-2026-05-OpenMP.pdf "PPE-2026-05-OpenMP"), [Materials](PPE-2026-05-OpenMP-Materials.tgz "PPE-2026-05-OpenMP-Materials"))
*   2026-05-18: Programming with POSIX Threads ([Slides](PPE-2026-06-POSIX-Threads.pdf "PPE-2026-06-POSIX-Threads"), [Materials](PPE-2026-06-POSIX-Threads-Materials.tgz "PPE-2026-06-POSIX-Threads-Materials"))
*   2026-05-25: Public holiday
*   2026-06-01: Operating System Concepts ([Slides](PPE-2026-07-Operating-System.pdf "PPE-2026-07-Operating-System"), [Materials](PPE-2026-07-Operating-System-Materials.tgz "PPE-2026-07-Operating-System-Materials"))
*   2026-06-08: Programming with MPI ([Slides](PPE-2026-08-MPI.pdf "PPE-2026-08-MPI"), [Materials](PPE-2026-08-MPI-Materials.tgz "PPE-2026-08-MPI-Materials"))
*   2026-06-15: Networking and Scalability ([Slides](PPE-2026-09-Networking-Scalability.pdf "PPE-2026-09-Networking-Scalability"))
*   2026-06-22: Skipped
*   2026-06-29: Advanced MPI and Debugging ([Slides](PPE-2026-10-MPI-Debugging.pdf "PPE-2026-10-MPI-Debugging"), [Materials](PPE-2026-10-MPI-Debugging-Materials.tgz "PPE-2026-10-MPI-Debugging-Materials"))
*   2026-07-06: Research Talks and Debriefing

### Exercises

*   2026-04-06: Introduction ([sheet 0](00-Organization.pdf "00-Organization"), [sheet 1](01-Introduction.pdf "01-Introduction"), [materials](PP-2026-Exercise-01-Materials.tgz "PP-2026-Exercise-01-Materials"))
    *   Deadline: 2026-04-19, 23:59
*   2026-04-13: Debugging ([sheet 2](02-Debugging.pdf "02-Debugging"), [materials](PP-2026-Exercise-02-Materials.tgz "PP-2026-Exercise-02-Materials"))
    *   Deadline: 2026-04-26, 23:59
*   2026-04-27: Performance Optimization of a Serial Application ([sheet 3](03-Performance-Optimization.pdf "03-Performance-Optimization"), [materials](PP-2026-Exercise-03-Materials.tgz "PP-2026-Exercise-03-Materials"))
    *   Deadline: 2026-05-10, 23:59
*   2026-05-11: Parallelization with OpenMP and Parallelization Schema ([sheet 4](04-OpenMP-ParallelizationSchema.pdf "04-OpenMP-ParallelizationSchema"), [materials](PP-2026-Exercise-04-Materials.tgz "PP-2026-Exercise-04-Materials"))
    *   Deadline:  2026-05-24, 23:59
*   2026-05-25: Parallelization with POSIX Threads ([sheet 5](05-POSIX-Threads.pdf "05-POSIX-Threads"), [materials](PP-2026-Exercise-05-Materials.tgz "PP-2026-Exercise-05-Materials"))
    *   Deadline: 2026-06-07, 23:59
*   2026-06-08: MPI Introduction ([sheet 6](06-Introduction-MPI.pdf "06-Introduction-MPI"), [materials](PP-2026-Exercise-06-Materials.tgz "PP-2026-Exercise-06-Materials"))
    *   Deadline: 2026-06-14, 23:59
*   2026-06-15: Parallelization with MPI ([sheet 7](07-MPI-Jacobi.pdf "07-MPI-Jacobi"), [materials](PP-2026-Exercise-07-Materials.tgz "PP-2026-Exercise-07-Materials"))
    *   Deadline: 2026-06-28 and 2026-07-05, 23:59

### Literature

*   High Performance Computing: Modern Systems and Practices (Thomas Sterling, Matthew Anderson and Maciej Brodowicz)
*   Parallel Programming: for Multicore and Cluster Systems (Thomas Rauber and Gudula Rünger)
*   Parallel Programming: Concepts and Practice (Dr. Bertil Schmidt, Dr. Jorge Gonzalez-Dominguez, Christian Hundt and Moritz Schlarb)
