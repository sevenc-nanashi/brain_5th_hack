#ifndef BRAIN_INET_H
#define BRAIN_INET_H
#include <stdint.h>
#include <sys/socket.h>
typedef uint32_t in_addr_t;
struct in_addr {
  in_addr_t s_addr;
};
struct in6_addr {
  uint8_t s6_addr[16];
};
#define INET_ADDRSTRLEN 16
#define INET6_ADDRSTRLEN 46
#define INADDR_NONE ((in_addr_t) - 1)
#define INADDR_ANY ((in_addr_t)0)
#endif
