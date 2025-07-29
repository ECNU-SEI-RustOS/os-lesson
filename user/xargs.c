#include "include/types.h"
#include "include/stat.h"
#include "user/user.h"
#include "include/fs.h"

int
main(int argc, char *argv[])
{
  char buf[512];
  char *new_argv[32];
  gets(buf, sizeof(buf));
  //printf("GET! %s", buf);
  while (buf[0] != '\0')
  {
    if(fork() == 0){
      for(int i=1; i<argc; i++){
        new_argv[i-1] = argv[i];
      }
      //printf("OLD! %s ,%s, %s \n",argv[0],argv[1],argv[2]);
      //printf("NEW! %s ,%s, %s \n",new_argv[0],new_argv[1],new_argv[2]);
      int argv_len = 0;
      int argv_head = 0;
      argc--;
      //printf("BUF! %s\n",buf);
      for(int i=0; buf[i]!='\0'; i++){
        if(buf[i] != ' '){
          argv_len++;
        }
        else{
          //printf("before mov:%s %d\n",new_argv[argc],argv_len);
          char tmp[10];
          for(int j=0;j<argv_len-1;j++){
            tmp[j]=buf[argv_head+j];
          }
          new_argv[argc] = tmp;
          //printf("after mov:%s\n",new_argv[argc]);
          argc++;
          argv_head = i + 1;
          argv_len = 0;
        }
      }
      //printf("before mov:%s %d\n",new_argv[argc],argv_len);
      //memmove(new_argv[argc],&buf[argv_head],argv_len);
      char tmp[10];
      for(int j=0;j<argv_len-1;j++){
        tmp[j]=buf[argv_head+j];
      }
      new_argv[argc] = tmp;
      argc++;
      //printf("after mov:%s\n",new_argv[argc]);
      //printf("RUN! %s ,%s, tt%stt \n",new_argv[0],new_argv[1],new_argv[2]);
      exec(new_argv[0],new_argv);
    }
    else{
      wait(0);
    }
    gets(buf, sizeof(buf));
  }
  exit(0);
}

/*
从标准输入读取，每次分析到一个回车
每读到一个回车，拼接参数并执行
*/