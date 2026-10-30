#ifndef BRAIN_DIRENT_H
#define BRAIN_DIRENT_H
typedef struct brain_directory DIR;
struct dirent {
  unsigned char d_type;
  char d_name[256];
};
#define DT_DIR 4
#define DT_REG 8
DIR *opendir(const char *);
struct dirent *readdir(DIR *);
int closedir(DIR *);
#endif
