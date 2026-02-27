CC = clang
CFLAGS = -Wall -std=c99 -I/usr/local/include -O3 -march=native -flto -DNDEBUG
LDFLAGS = -lraylib -lGL -lm -lpthread -ldl -lrt -lX11 -flto

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
