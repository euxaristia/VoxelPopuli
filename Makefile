CC = clang
# Default to PS1 Renderer
PS1_RENDERER ?= 1

CFLAGS = -Wall -std=c99 -Isrc -I/usr/local/include -O3 -march=native -flto -DNDEBUG
ifeq ($(PS1_RENDERER),1)
    CFLAGS += -DPS1_RENDERER
endif
LDFLAGS = -lm -lpthread -ldl -flto

# Detect OS
UNAME_S := $(shell uname -s)
ifeq ($(UNAME_S),Linux)
    LDFLAGS += -Llib/linux -lraylib -lGL -lrt -lX11
endif
ifeq ($(UNAME_S),Darwin)
    LDFLAGS += -Llib/macos -lraylib -framework CoreVideo -framework IOKit -framework Cocoa -framework GLUT -framework OpenGL
endif

SRC = src/main.c src/chunk.c src/world.c src/noise.c
OBJ = $(SRC:.c=.o)
TARGET = voxelpopuli

.PHONY: all clean gcc clang

all: $(TARGET)

$(TARGET): $(OBJ)
	$(CC) $(OBJ) -o $@ $(LDFLAGS)

%.o: %.c
	$(CC) $(CFLAGS) -c $< -o $@

clean:
	rm -f $(OBJ) $(TARGET)

gcc: clean
	$(MAKE) all CC=gcc

clang: clean
	$(MAKE) all CC=clang
