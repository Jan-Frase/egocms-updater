# JULEA: A Flexible Storage Framework for HPC

The group develops the JULEA storage framework, which can be used to easily prototype new ideas related to storage and
file systems. It allows offering arbitrary I/O interfaces to applications and includes object, key-value and database
backends with support for popular storage technologies such as POSIX, LevelDB and MongoDB. Additionally, JULEA allows
dynamically adapting the I/O operations' semantics and can thus be adjusted to different use-cases. It runs completely
in user space, which eases development and debugging.

It is open source and can be found on [GitHub](https://github.com/julea-io). JULEA is used in teaching and students have
contributed several major new
features. Moreover, it serves as the foundation of the [CoSEMoS](https://www.parcio.ovgu.de/Research/CoSEMoS.html)
project to explore the benefits of a coupled storage
system for self-describing data formats.
