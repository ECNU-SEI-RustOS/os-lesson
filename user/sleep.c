#include "include/types.h"
#include "include/stat.h"
#include "user/user.h"

int
main(int argc, char *argv[])
{
  if (argc != 2) {
    write(1, "Error Argument\n", 16);
  }
  else {
    sleep(atoi(argv[1]));
  }
  exit(0);
}
