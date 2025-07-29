#include "include/types.h"
#include "include/stat.h"
#include "user/user.h"
#include "include/fs.h"

char*
fmtname(char *path)
{
  static char buf[DIRSIZ+1];
  char *p;

  // Find first character after last slash.
  for(p=path+strlen(path); p >= path && *p != '/'; p--)
    ;
  p++;

  // Return blank-padded name.
  if(strlen(p) >= DIRSIZ)
    return p;
  memmove(buf, p, strlen(p));
  memset(buf+strlen(p), '\0', DIRSIZ-strlen(p));
  return buf;
}

void
find(char *path, char *name)
{
  char buf[512], *p;
  int fd;
  struct dirent de;
  struct stat st;

  if((fd = open(path, 0)) < 0){
    fprintf(2, "find: cannot open %s\n", path);
    return;
  }

  if(fstat(fd, &st) < 0){
    fprintf(2, "find: cannot stat %s\n", path);
    close(fd);
    return;
  }

  switch(st.type) {
    case T_FILE:
      printf("FU!\n");
      // if(strcmp(name, fmtname(path))){
      //   printf("%s\n", *path);
      // }
    break;
    case T_DIR:
      // if(strlen(path) + 1 + DIRSIZ + 1 > sizeof buf){
      //   printf("find: path too long\n");
      //   break;
      // }
      strcpy(buf, path);
      p = buf+strlen(buf);
      *p++ = '/';
      while(read(fd, &de, sizeof(de)) == sizeof(de)){
        if(de.inum == 0 || !strcmp(de.name,".") || !strcmp(de.name,".."))
          continue;
        memcpy(p, de.name, DIRSIZ);
        p[DIRSIZ] = 0;
        int fd_sub = open(buf,0);
        struct stat st_sub;
        fstat(fd_sub,&st_sub);
        //printf("%s,%d\n",buf,st_sub.type);
        //printf("%s %d %d %d\n", fmtname(buf), st.type, st.ino, st.size);
        if(st_sub.type == T_FILE){
          //printf("%s,%s\n",name, buf);
          if(!strcmp(name, fmtname(buf))){
            printf("%s\n", buf);
          }
        }
        else if(st_sub.type == T_DIR) {
          // for(int i=0,j=strlen(buf);i<strlen(de.name);i++,j++){
          //   buf[j] = de.name[i];
          // }
          find(buf, name);
        }
        close(fd_sub);
        //strcpy(buf, path);
      }
    break;
  }
  return;
}

int
main(int argc, char *argv[])
{
  //printf("main:%s %s\n",argv[1],argv[2]);
  find(argv[1],argv[2]);
  exit(0);
}
