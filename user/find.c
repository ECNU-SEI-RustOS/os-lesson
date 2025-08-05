#include "include/types.h"
#include "include/stat.h"
#include "user/user.h"
#include "include/fs.h"

char*
fmtname(char *path)
{
  static char buf[DIRSIZ+1];
  char *p;
  for(p=path+strlen(path); p >= path && *p != '/'; p--)
    ;
  p++;
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
    case T_DIR:
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
        if(st_sub.type == T_FILE){
          if(!strcmp(name, fmtname(buf))){
            printf("%s\n", buf);
          }
        }
        else if(st_sub.type == T_DIR) {
          find(buf, name);
        }
        close(fd_sub);
      }
    break;
  }
  return;
}

int
main(int argc, char *argv[])
{
  find(argv[1],argv[2]);
  exit(0);
}
