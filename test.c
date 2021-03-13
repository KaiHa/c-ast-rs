int x = 3;
static int y = 5;

struct a_t {
  int foo;
  int bar;
};

struct b_t {
  char *wer;
  char *wo;
};

struct a_t anton = {
  .foo = 1,
  .bar = 2
};

struct a_t berta = {3, 4};

struct a_t caesar = {3+2, 6};

struct b_t dora = {"Dora", "Musterstadt"};
