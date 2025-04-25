#ifndef RUST_BRIDGE_H
#define RUST_BRIDGE_H

#include <gtk/gtk.h>

typedef void (*RustMessageCallback)(const char *msg, gpointer user_data);

void rust_bridge_start(RustMessageCallback callback, gpointer user_data,const char *finalname);
void rust_bridge_send(const char *msg);
void rust_bridge_stop();

#endif
