# SEIOS内核线程
由于SEIOS只实现了进程（主线程）结构，没有线程，导致任务管理繁琐。线程将提供更高效地切换效率；提供更灵活的任务同步机制；由于创建线程不需要复制整个用户地址空间中的数据，一定程度减少内存的开销。SEIOS目前正在实现内核线程。  
Add a thread manager to the kernel

1. thread struct