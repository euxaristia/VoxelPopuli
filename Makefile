CC = gcc
CFLAGS = -Wall -std=c99 -I/usr/local/include
LDFLAGS = -lraylib -lGL -lm -lpthread -ldl -lrt -lX11

SRC = src/main.c src/chunk.c src/world.c src/noise.c
OBJ = $(SRC:.c=.o)
TARGET = voxelpopuli

all: $(TARGET)

$(TARGET): $(OBJ)
	$(CC) $(OBJ) -o $@ $(LDFLAGS)

%.o: %.c
	$(CC) $(CFLAGS) -c $< -o $@

clean:
	rm -f $(OBJ) $(TARGET)
