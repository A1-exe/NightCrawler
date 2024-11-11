#include <stdio.h>
#include <fcntl.h>
#include <unistd.h>

int main(int argc, char **argv) {
    char buf[100];
    read(0, buf, 100);
    
    // printf("%s\n", buf);

    if (buf[0] == 0x41) {
      if (buf[1] == 0x42) {
        if (buf[2] == 0x43) {
          if (buf[3] == 0x44) {
            if (buf[4] == 0x45) {
              if (buf[5] == 0x46) {
                *(unsigned long*)0x4141414141414141 = 0;
              }
            }
          }
        }
      }
    }
    return 0;
}
