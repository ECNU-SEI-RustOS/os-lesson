#include "include/param.h"
#include "include/types.h"
#include "include/stat.h"
#include "user/user.h"
#include "include/fs.h"
#include "include/fcntl.h"
#include "include/syscall.h"
#include "include/memlayout.h"
#include "include/riscv.h"

int main()
{
    setpri(0, 0);
    getpri(0);
    exit(0);
}