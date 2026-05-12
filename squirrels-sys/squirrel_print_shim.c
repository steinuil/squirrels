#include <squirrel.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>

extern void squirrels_dispatch_print(HSQUIRRELVM v, const SQChar *msg);
extern void squirrels_dispatch_error(HSQUIRRELVM v, const SQChar *msg);

typedef void (*dispatch_fn)(HSQUIRRELVM v, const SQChar *msg);

static void print_dispatch(HSQUIRRELVM v, dispatch_fn cb, const SQChar *fmt,
                           va_list ap) {
  char small_buf[1024];
  va_list copy;
  va_copy(copy, ap);
  int n = vsnprintf(small_buf, sizeof small_buf, fmt, ap);

  if (n < 0) {
    va_end(copy);
    return;
  }

  if ((size_t)n < sizeof small_buf) {
    cb(v, small_buf);
  } else {
    size_t capacity = (size_t)n + 1;
    char *big_buf = (char *)malloc(capacity);
    if (big_buf) {
      vsnprintf(big_buf, capacity, fmt, copy);
      cb(v, big_buf);
      free(big_buf);
    }
  }

  va_end(copy);
}

void squirrels_shim_print(HSQUIRRELVM v, const SQChar *fmt, ...) {
  va_list ap;
  va_start(ap, fmt);
  print_dispatch(v, squirrels_dispatch_print, fmt, ap);
  va_end(ap);
}

void squirrels_shim_error(HSQUIRRELVM v, const SQChar *fmt, ...) {
  va_list ap;
  va_start(ap, fmt);
  print_dispatch(v, squirrels_dispatch_error, fmt, ap);
  va_end(ap);
}
