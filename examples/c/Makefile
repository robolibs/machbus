SHELL := /bin/bash

ROOT      := ../..
BIN       := $(ROOT)/target/debug/c_api_demo
FULL_BIN  := $(ROOT)/target/debug/c_full_demo
LAYOUT_OBJ := $(ROOT)/target/debug/c_abi_layout.o
SURFACE_OBJ := $(ROOT)/target/debug/c_abi_compile_surface.o
LIB_DIR   := $(ROOT)/target/debug
CC        ?= cc
CFLAGS    ?= -std=c11 -Wall -Wextra -Werror -pedantic
LIBS      := -lmachbus -Wl,-rpath,$(shell cd $(ROOT) && pwd)/target/debug -lm -lpthread -ldl

.PHONY: all layout compile-surface build full run run-full clean

all: run

layout:
	@$(CC) $(CFLAGS) -I$(ROOT)/include -c layout.c -o $(LAYOUT_OBJ)

compile-surface:
	@$(CC) $(CFLAGS) -I$(ROOT)/include -c compile_surface.c -o $(SURFACE_OBJ)

build: layout compile-surface
	@cd $(ROOT) && cargo build
	@$(CC) $(CFLAGS) -I$(ROOT)/include demo.c -L$(LIB_DIR) $(LIBS) -o $(BIN)

full: layout compile-surface
	@cd $(ROOT) && cargo build
	@$(CC) $(CFLAGS) -I$(ROOT)/include full_demo.c -L$(LIB_DIR) $(LIBS) -o $(FULL_BIN)

run: build
	@$(BIN)

run-full: full
	@$(FULL_BIN)

clean:
	@rm -f $(BIN) $(FULL_BIN) $(LAYOUT_OBJ) $(SURFACE_OBJ)
