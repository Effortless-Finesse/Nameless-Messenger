#include "rust_bridge.h"
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <string.h>
#include <pthread.h>
#include <signal.h>
#include <glib.h>
#include <gtk/gtk.h>

static int to_child = -1, from_child = -1;
static pid_t child_pid = -1;

static RustMessageCallback global_callback = NULL;
static gpointer global_user_data = NULL;

static pthread_t reader_thread;
static gboolean running = TRUE;

// GTK thread-safe callback dispatcher
static gboolean dispatch_to_gtk(gpointer data) {
    if (global_callback) {
        global_callback((char *)data, global_user_data);
    }
    g_free(data);
    return G_SOURCE_REMOVE;
}

static void *reader_thread_func(void *arg) {
    char buf[1024];
    while (running) {
        ssize_t n = read(from_child, buf, sizeof(buf) - 1);
        if (n > 0) {
            buf[n] = '\0';
            if (global_callback) {
                char *msg_copy = g_strdup(buf);
                g_idle_add_full(G_PRIORITY_DEFAULT, dispatch_to_gtk, msg_copy, NULL);
            }
        }
    }
    return NULL;
}

void rust_bridge_start(RustMessageCallback callback, gpointer user_data, const char *finalname) {
    int in_pipe[2], out_pipe[2];
    pipe(in_pipe);
    pipe(out_pipe);

    child_pid = fork();
    if (child_pid == 0) {
        dup2(in_pipe[0], STDIN_FILENO);
        dup2(out_pipe[1], STDOUT_FILENO);
        close(in_pipe[1]);
        close(out_pipe[0]);
        perror("reached here");
        execl("./rust_client", "rust_client", NULL);
        perror("exec failed");
        exit(1);
    }

    close(in_pipe[0]);
    close(out_pipe[1]);

    to_child = in_pipe[1];
    from_child = out_pipe[0];
    global_callback = callback;
    global_user_data = user_data;

    write(to_child, finalname, strlen(finalname));
    write(to_child, "\n", 1);

    running = TRUE;
    pthread_create(&reader_thread, NULL, reader_thread_func, NULL);
}

void rust_bridge_send(const char *msg) {
    if (to_child != -1 && msg) {
        write(to_child, msg, strlen(msg));
        write(to_child, "\n", 1);
    }
}

void rust_bridge_stop() {
    running = FALSE;
    if (child_pid > 0) kill(child_pid, SIGTERM);
    pthread_cancel(reader_thread);
    pthread_join(reader_thread, NULL);
    close(to_child);
    close(from_child);
}