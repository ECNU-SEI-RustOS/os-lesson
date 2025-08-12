#include "include/types.h"
#include "include/stat.h"
#include "user/user.h"

int
main(int argc, char *argv[])
{
  int p1[2],p2[2];
  char foo[10];
  pipe(p1);
  pipe(p2);
  if(fork() == 0) {
    if(read(p2[0], foo, sizeof(foo)) != 0) {
        printf("%d: received ping\n", getpid());
        write(p1[1], "CHILD", 6);
    }
    exit(0);
  }
  else {
    write(p2[1], "PARENT", 7);
    if(read(p1[0], foo, sizeof(foo)) != 0) {
        wait(0);
        printf("%d: received pong\n", getpid());
    }
    exit(0);
  }
  exit(0);
}
