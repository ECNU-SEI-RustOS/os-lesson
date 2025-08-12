#include "include/types.h"
#include "include/stat.h"
#include "user/user.h"

void
pipeline(int *in_pipe){
  int out_pipe[2];
  int buf;
  int head;
  pipe(out_pipe);
  if(read(in_pipe[0],&head,4)){
    if(head>=35){
      return;
    }
  }
  else{
    return;
  }
  if(fork() == 0){
    close(out_pipe[1]);
    pipeline(out_pipe);
  }
  else{
    printf("prime %d\n",head);
    while(read(in_pipe[0],&buf,4)){
      if(buf % head != 0){
        write(out_pipe[1],&buf,4);
      }
    }
    close(in_pipe[0]);
    close(out_pipe[1]);
    wait(0);
  }
  return;
}

int
main(int argc, char *argv[])
{
  int in_pipe[2];
  pipe(in_pipe);
  if(fork() == 0){
    close(in_pipe[1]);
    pipeline(in_pipe);
  }
  else{
    close(in_pipe[0]);
    for(int i=2;i<35;i++){
      write(in_pipe[1],&i,4);
    }
    close(in_pipe[1]);
    wait(0);
  }
  exit(0);
}
