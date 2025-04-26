#include "rust_bridge.h"
#include <gtk/gtk.h>
#include <strings.h>
#include <stdio.h>
#include <unistd.h>     
#include <sys/types.h>  
#include <sys/wait.h> 
#include <stdlib.h>    
#include <string.h>    
#include <fcntl.h>
#include <stdbool.h>   
//gtk, pipes, shm, mutex, sephamores, all used here 
//global vars--------------------------------------------------------------------------------------------
int to_child[2];    //peter PIPEr
int from_child[2];  //picked a pack of pickled PIPEs
pid_t child_pid;    //if you want explanation for this, what are you even doing here?
gboolean is_maximized = FALSE; //header maximization
gboolean is_chat_maximized = FALSE; //chat header maximization
char finalname[126];
static char last_sender[126];

typedef struct {
    GtkWidget *entry;
    GtkWidget *chat_display;
} ChatWidgets; //argument passing for text sending in chat window

//global vars are guilty pleasures-----------------------------------------------------------------------

//chat header button functionalities---------------------------------------------------------------------
void on_chat_close(GtkWidget *widget, gpointer win) {
    gtk_window_close(GTK_WINDOW(win));
}

void on_chat_minimize(GtkWidget *widget, gpointer win) {
    gtk_window_iconify(GTK_WINDOW(win));
}

//helper functions for wsl fixing hell
gboolean finalize_window_move(gpointer data) {
    GtkWindow *win = GTK_WINDOW(data);
    GdkScreen *screen = gdk_screen_get_default();
    int sw = gdk_screen_get_width(screen);
    int sh = gdk_screen_get_height(screen);

    gtk_window_move(win, sw * 0.25, sh * 0.25); // move to center
    return G_SOURCE_REMOVE;
}

gboolean delayed_resize_and_center(gpointer data) {
    GtkWindow *win = GTK_WINDOW(data);
    GdkScreen *screen = gdk_screen_get_default();
    int sw = gdk_screen_get_width(screen);
    int sh = gdk_screen_get_height(screen);
    gtk_window_resize(win, sw * 0.5, sh * 0.5); // resize to 50%
    g_idle_add(finalize_window_move, win); // now center
    return G_SOURCE_REMOVE;
}

void simulate_maximize(GtkWindow *win) {
    GdkDisplay *display = gdk_display_get_default();
    GdkMonitor *monitor = gdk_display_get_primary_monitor(display);
    GdkRectangle geometry;
    gdk_monitor_get_geometry(monitor, &geometry);

    gtk_window_move(win, geometry.x, geometry.y);
    gtk_window_resize(win, geometry.width, geometry.height);
}
//damn this was difficult to debug

void on_chat_toggle_max(GtkWidget *widget, gpointer win) {
    GdkDisplay *display = gdk_display_get_default();
    GdkMonitor *monitor = gdk_display_get_primary_monitor(display);
    GdkRectangle geometry;
    gdk_monitor_get_geometry(monitor, &geometry);

    if (is_chat_maximized) {
        gtk_window_unmaximize(GTK_WINDOW(win));
         //delay resize + move
         g_idle_add(delayed_resize_and_center, win);
    } else {
        simulate_maximize(win);
    }
    is_chat_maximized = !is_chat_maximized;
}

gboolean on_header_chat_button_press(GtkWidget *widget, GdkEventButton *event, gpointer data) {
    if (event->type == GDK_BUTTON_PRESS && event->button == 1) {
        gtk_window_begin_move_drag(GTK_WINDOW(data), event->button,
                                   event->x_root, event->y_root, event->time);
    }
    return FALSE;
}

//-------------------------------------------------------------------------------------------------------

//chat cutom header--------------------------------------------------------------------------------------

GtkWidget* create_custom_chatheader(GtkWidget *win) {
    GtkWidget *chatheadermainframe = gtk_event_box_new();
    GtkWidget *chatheader = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 10);
    gtk_widget_set_name(chatheader, "chatheader"); //for css
    gtk_widget_set_name(chatheadermainframe, "chatheadermainframe"); //for css
    gtk_container_add(GTK_CONTAINER(chatheadermainframe), chatheader);

    GtkWidget *btn_close = gtk_button_new();
    gtk_widget_set_name(btn_close, "btn_close"); //for css
    g_signal_connect(btn_close, "clicked", G_CALLBACK(on_chat_close), win);

    GtkWidget *btn_min = gtk_button_new();
    gtk_widget_set_name(btn_min, "btn_min"); //for css
    g_signal_connect(btn_min, "clicked", G_CALLBACK(on_chat_minimize), win);

    GtkWidget *btn_max = gtk_button_new();
    gtk_widget_set_name(btn_max, "btn_max"); //for css
    g_signal_connect(btn_max, "clicked", G_CALLBACK(on_chat_toggle_max), win);

    gtk_widget_set_tooltip_text(btn_close, "Close");
    gtk_widget_set_tooltip_text(btn_min, "Minimize");
    gtk_widget_set_tooltip_text(btn_max, "Maximize");

    gtk_button_set_relief(GTK_BUTTON(btn_close), GTK_RELIEF_NONE); //for css
    gtk_button_set_relief(GTK_BUTTON(btn_min), GTK_RELIEF_NONE); //for css
    gtk_button_set_relief(GTK_BUTTON(btn_max), GTK_RELIEF_NONE); //for css

    gtk_box_pack_end(GTK_BOX(chatheader), btn_close, FALSE, FALSE, 0);
    gtk_box_pack_end(GTK_BOX(chatheader), btn_max, FALSE, FALSE, 0);
    gtk_box_pack_end(GTK_BOX(chatheader), btn_min, FALSE, FALSE, 0);

    g_signal_connect(chatheadermainframe, "button-press-event", G_CALLBACK(on_header_chat_button_press), win);
    
    return chatheadermainframe;
}
//-------------------------------------------------------------------------------------------------------

//message sending/adding---------------------------------------------------------------------------------

// void add_chat_message(GtkWidget *text_view, const char *name, const char *msg) {
//     GtkTextBuffer *buffer = gtk_text_view_get_buffer(GTK_TEXT_VIEW(text_view));
//     GtkTextIter end;
//     gtk_text_buffer_get_end_iter(buffer, &end);

//     time_t now = time(NULL);
//     struct tm *local = localtime(&now);
//     char timestamp[32];
//     strftime(timestamp, sizeof(timestamp), "[%H:%M:%S] ", local);

//     GtkTextTagTable *tag_table = gtk_text_buffer_get_tag_table(buffer);

//     if (!gtk_text_tag_table_lookup(tag_table, "timestamp")) {
//         gtk_text_buffer_create_tag(buffer, "timestamp", "foreground", "black", NULL);
//     }//time
//     if (!gtk_text_tag_table_lookup(tag_table, finalname)) {
//         gtk_text_buffer_create_tag(buffer,finalname, "foreground", "red", "weight", PANGO_WEIGHT_BOLD, NULL);
//     }//username
//     if (!gtk_text_tag_table_lookup(tag_table, "message")) {
//         gtk_text_buffer_create_tag(buffer, "message", "foreground", "white", NULL);
//     }//the message, ta daaaaaa

//     //insert info
//     gtk_text_buffer_insert_with_tags_by_name(buffer, &end, timestamp, -1, "timestamp", NULL);

//     //check username, if same, don't show again
//     if (!last_sender || g_strcmp0(finalname, last_sender) != 0) {
//         gtk_text_buffer_insert_with_tags_by_name(buffer, &end, finalname, -1, finalname, NULL);
//         gtk_text_buffer_insert_with_tags_by_name(buffer, &end, ": ", -1, "message", NULL);

//         strncpy(last_sender,finalname,sizeof(finalname));
//     } else { 
//         gtk_text_buffer_insert_with_tags_by_name(buffer, &end, "        ", -1, "message", NULL);
//     }

//     gtk_text_buffer_insert_with_tags_by_name(buffer, &end, msg, -1, "message", NULL);
//     gtk_text_buffer_insert(buffer, &end, "\n", -1);

//     //scroll wheel
//     GtkTextMark *mark = gtk_text_buffer_create_mark(buffer, NULL, &end, FALSE);
//     gtk_text_view_scroll_mark_onscreen(GTK_TEXT_VIEW(text_view), mark);
// }

void add_chat_message(GtkWidget *text_view, const char *tname, const char *msg,bool isfromserver) { 
    GtkTextBuffer *buffer = gtk_text_view_get_buffer(GTK_TEXT_VIEW(text_view));
    GtkTextIter end;
    gtk_text_buffer_get_end_iter(buffer, &end);

    time_t now = time(NULL);
    struct tm *local = localtime(&now);
    char timestamp[32];
    strftime(timestamp, sizeof(timestamp), "[%H:%M:%S] ", local);

    GtkTextTagTable *tag_table = gtk_text_buffer_get_tag_table(buffer);

    if (!gtk_text_tag_table_lookup(tag_table, "timestamp")) {
        gtk_text_buffer_create_tag(buffer, "timestamp", "foreground", "black", NULL);
    }//time
    if (!gtk_text_tag_table_lookup(tag_table, "message")) {
        gtk_text_buffer_create_tag(buffer, "message", "foreground", "white", NULL);
    }//message
    if(isfromserver){
        if (!gtk_text_tag_table_lookup(tag_table, tname)) {
            gtk_text_buffer_create_tag(buffer, tname, "foreground", "red", "weight", PANGO_WEIGHT_BOLD, NULL);
            }
        }else{
            if (!gtk_text_tag_table_lookup(tag_table, tname)) {
                gtk_text_buffer_create_tag(buffer, tname, "foreground", "cyan", "weight", PANGO_WEIGHT_BOLD, NULL);
                }
        }//username, ta daaaaaa

    //insert info
    gtk_text_buffer_insert_with_tags_by_name(buffer, &end, timestamp, -1, "timestamp", NULL);

    //check username, if same, don't show again
    if (!last_sender || g_strcmp0(tname, last_sender) != 0) {
        gtk_text_buffer_insert_with_tags_by_name(buffer, &end, tname, -1, tname, NULL);
        gtk_text_buffer_insert_with_tags_by_name(buffer, &end, ": ", -1, "message", NULL);

        strncpy(last_sender, tname, sizeof(last_sender));
    } else { 
        gtk_text_buffer_insert_with_tags_by_name(buffer, &end, "        ", -1, "message", NULL);
    }

    gtk_text_buffer_insert_with_tags_by_name(buffer, &end, msg, -1, "message", NULL);
    if(!isfromserver)
        gtk_text_buffer_insert(buffer, &end, "\n", -1);

    //scroll wheel
    GtkTextMark *mark = gtk_text_buffer_create_mark(buffer, NULL, &end, FALSE);
    gtk_text_view_scroll_mark_onscreen(GTK_TEXT_VIEW(text_view), mark);
}


//-------------------------------------------------------------------------------------------------------

void handle_rust_incoming_message(const char *incoming, gpointer user_data) {
    if (!incoming) return;
    GtkTextView *text_view = GTK_TEXT_VIEW(user_data);
    char buffer[1024];
    strncpy(buffer, incoming, sizeof(buffer));
    buffer[sizeof(buffer) - 1] = '\0';
    // Find the first colon (assuming format is "username: message")
    char *colon = strchr(buffer, ':');
    if (!colon) return;

    *colon = '\0';
    const char *name = buffer;
    const char *msg = colon + 2; // skip ": " (colon and space)
    add_chat_message(user_data,name, msg, TRUE);
    // g_free(name);
    // g_free(msg);
}

//chat window, when connect button clicked---------------------------------------------------------------

//send message
void on_send_clicked(GtkWidget *widget, gpointer data) {
    ChatWidgets *widgets = (ChatWidgets *)data;

    const gchar *msg = gtk_entry_get_text(GTK_ENTRY(widgets->entry));
    if (g_strcmp0(msg, "") != 0) {
        add_chat_message(widgets->chat_display, finalname, msg,FALSE);
        rust_bridge_send(msg);
        gtk_entry_set_text(GTK_ENTRY(widgets->entry), "");  // clear entry
    }
}

void on_connect_clicked(GtkWidget *widget, gpointer data) {
    ChatWidgets *Lwidgets = (ChatWidgets *)data;
    const gchar *name = gtk_entry_get_text(GTK_ENTRY(Lwidgets->entry));

    if (g_strcmp0(name, "") == 0) {
        gtk_entry_set_text(GTK_ENTRY(Lwidgets->entry), "ObnoxiousFASTPerson");
        perror("No Name entered - naming you ObnoxiousFASTPerson.\n");
    }
    name = gtk_entry_get_text(GTK_ENTRY(Lwidgets->entry));
    strncpy(finalname,name,sizeof(finalname)-1);
    finalname[sizeof(finalname) - 1] = '\0';
    printf("%s pewpew\n",finalname);
    gtk_widget_destroy(Lwidgets->chat_display);
    g_free(Lwidgets);
    printf("%s pewpew\n",finalname);
    
     // Main window
     GtkWidget *chatwin = gtk_window_new(GTK_WINDOW_TOPLEVEL);
     gtk_window_set_gravity(GTK_WINDOW(chatwin), GDK_GRAVITY_CENTER);
     gtk_window_set_title(GTK_WINDOW(chatwin), "PDS Chat");
     gtk_window_set_decorated(GTK_WINDOW(chatwin), FALSE);
     gtk_widget_set_name(chatwin, "chat_window");
     delayed_resize_and_center(chatwin);
     g_signal_connect(chatwin, "destroy", G_CALLBACK(gtk_main_quit), NULL);
 
     // VBox container
     GtkWidget *chatvbox = gtk_box_new(GTK_ORIENTATION_VERTICAL, 10);
     gtk_container_add(GTK_CONTAINER(chatwin), chatvbox);
     gtk_container_set_border_width(GTK_CONTAINER(chatvbox), 10);
 
     // Header
     GtkWidget *chatheadermainframe = create_custom_chatheader(chatwin);
     gtk_box_pack_start(GTK_BOX(chatvbox), chatheadermainframe, FALSE, FALSE, 5);
     
 
     //scroll
     GtkWidget *scroll = gtk_scrolled_window_new(NULL, NULL);
     gtk_widget_set_vexpand(scroll, TRUE);
     gtk_box_pack_start(GTK_BOX(chatvbox), scroll, TRUE, TRUE, 0);

     //chat display
     GtkWidget *chat_display = gtk_text_view_new();
     gtk_text_view_set_wrap_mode(GTK_TEXT_VIEW(chat_display), GTK_WRAP_WORD_CHAR);
     gtk_container_add(GTK_CONTAINER(scroll), chat_display);
     gtk_text_view_set_editable(GTK_TEXT_VIEW(chat_display), FALSE);
     gtk_widget_set_can_focus(chat_display, FALSE);
     gtk_widget_set_name(chat_display, "chat_display");
 
     // Message input area
     GtkWidget *bottom_box = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 5);
     gtk_box_pack_end(GTK_BOX(chatvbox), bottom_box, FALSE, FALSE, 10);
     gtk_text_view_set_editable(GTK_TEXT_VIEW(chat_display), FALSE);

     GtkWidget *messageentry = gtk_entry_new();
     gtk_entry_set_placeholder_text(GTK_ENTRY(messageentry), "  Type your message...");
     gtk_box_pack_start(GTK_BOX(bottom_box), messageentry, TRUE, TRUE, 10);
     gtk_widget_set_name(messageentry, "nameinputfield"); //for css

     //struct to use for argument passing
    ChatWidgets *chat_widgets = g_malloc(sizeof(ChatWidgets));
    chat_widgets->entry = messageentry;
    chat_widgets->chat_display = chat_display;
     
     //send button
     GtkWidget *send_btn = gtk_button_new_with_label("Send");
     gtk_box_pack_start(GTK_BOX(bottom_box), send_btn, FALSE, FALSE, 10);
     gtk_widget_set_name(send_btn, "sendbutton"); //for css
     gtk_button_set_relief(GTK_BUTTON(send_btn), GTK_RELIEF_NONE); //for css
     g_signal_connect(send_btn, "clicked", G_CALLBACK(on_send_clicked), chat_widgets);
     g_signal_connect(messageentry, "activate", G_CALLBACK(on_send_clicked), chat_widgets);

     rust_bridge_start(handle_rust_incoming_message, chat_display, finalname);
 
     gtk_widget_show_all(chatwin);
     gtk_main();
     return 0;
 }

//-------------------------------------------------------------------------------------------------------

//css loader---------------------------------------------------------------------------------------------

void load_css(const char *file_path) {
    GtkCssProvider *provider = gtk_css_provider_new();
    GError *error = NULL;

    gtk_css_provider_load_from_path(provider, file_path, &error);
    if (error) {
        g_printerr("CSS load error: %s\n", error->message);
        g_error_free(error);
        return;
    }

    gtk_style_context_add_provider_for_screen(
        gdk_screen_get_default(),
        GTK_STYLE_PROVIDER(provider),
        GTK_STYLE_PROVIDER_PRIORITY_USER
    );
}

//-------------------------------------------------------------------------------------------------------

//main win header funcs----------------------------------------------------------------------------------

void on_close(GtkWidget *widget, gpointer win) {
    gtk_window_close(GTK_WINDOW(win));
    gtk_widget_destroy(win);
}

void on_minimize(GtkWidget *widget, gpointer win) {
    gtk_window_iconify(GTK_WINDOW(win));
}

void on_toggle_max(GtkWidget *widget, gpointer win) {
    if (is_maximized) {
        GdkDisplay *display = gdk_display_get_default();
        GdkMonitor *monitor = gdk_display_get_primary_monitor(display);
        GdkRectangle geometry;
        gdk_monitor_get_geometry(monitor, &geometry);
        gtk_window_resize(win, geometry.width*0.5, geometry.height*0.5);
    } else {
        simulate_maximize(win);
    }
    is_maximized = !is_maximized;
}

gboolean on_button_press(GtkWidget *widget, GdkEventButton *event, gpointer data) {
    if (event->type == GDK_BUTTON_PRESS && event->button == 1) {
        gtk_window_begin_move_drag(GTK_WINDOW(data), event->button,
                                   event->x_root, event->y_root, event->time);
    }
    return FALSE;
}

GtkWidget* create_custom_header(GtkWidget *win) {
    GtkWidget *headermainframe = gtk_event_box_new();
    GtkWidget *header = gtk_box_new(GTK_ORIENTATION_HORIZONTAL, 10);
    gtk_widget_set_name(header, "header"); //for css
    gtk_widget_set_name(headermainframe, "headermainframe"); //for css
    gtk_container_add(GTK_CONTAINER(headermainframe), header);

    GtkWidget *btn_close = gtk_button_new();
    gtk_widget_set_name(btn_close, "btn_close"); //for css
    g_signal_connect(btn_close, "clicked", G_CALLBACK(on_close), win);

    GtkWidget *btn_min = gtk_button_new();
    gtk_widget_set_name(btn_min, "btn_min"); //for css
    g_signal_connect(btn_min, "clicked", G_CALLBACK(on_minimize), win);

    GtkWidget *btn_max = gtk_button_new();
    gtk_widget_set_name(btn_max, "btn_max"); //for css
    g_signal_connect(btn_max, "clicked", G_CALLBACK(on_toggle_max), win);

    gtk_widget_set_tooltip_text(btn_close, "Close");
    gtk_widget_set_tooltip_text(btn_min, "Minimize");
    gtk_widget_set_tooltip_text(btn_max, "Maximize");

    gtk_button_set_relief(GTK_BUTTON(btn_close), GTK_RELIEF_NONE); //for css
    gtk_button_set_relief(GTK_BUTTON(btn_min), GTK_RELIEF_NONE); //for css
    gtk_button_set_relief(GTK_BUTTON(btn_max), GTK_RELIEF_NONE); //for css

    gtk_box_pack_end(GTK_BOX(header), btn_close, FALSE, FALSE, 0);
    gtk_box_pack_end(GTK_BOX(header), btn_max, FALSE, FALSE, 0);
    gtk_box_pack_end(GTK_BOX(header), btn_min, FALSE, FALSE, 0);

    g_signal_connect(headermainframe, "button-press-event", G_CALLBACK(on_button_press), win);
    
    return headermainframe;
}

//-------------------------------------------------------------------------------------------------------

//main window and main-----------------------------------------------------------------------------------

int main(int argc, char *argv[]) {

    //initialize
    gtk_init(&argc, &argv);
    
    //load css
    load_css("style2.css");

    //make base window that pops up first thing when opened, the input window
    GtkWidget *main_win = gtk_window_new(GTK_WINDOW_TOPLEVEL);
    gtk_window_set_title(GTK_WINDOW(main_win), "PDS - A Discord Wannabe");
    GdkScreen *screen = gdk_screen_get_default();
    int w = gdk_screen_get_width(screen);
    int h = gdk_screen_get_height(screen);
    gtk_window_set_default_size(GTK_WINDOW(main_win), w * 0.5, h * 0.5); //set default size
    gtk_widget_set_name(main_win, "nameinput_window"); //for css
    gtk_window_set_decorated(GTK_WINDOW(main_win), FALSE); //disable border
    gtk_window_set_position(GTK_WINDOW(main_win), GTK_WIN_POS_CENTER); //center 
    g_signal_connect(main_win, "destroy", G_CALLBACK(gtk_main_quit), NULL); //destructor

    //make base widget
    GtkWidget *vbox = gtk_box_new(GTK_ORIENTATION_VERTICAL, 10); 
    gtk_container_set_border_width(GTK_CONTAINER(vbox), 10);
    gtk_container_add(GTK_CONTAINER(main_win), vbox);
   
    //header
    GtkWidget *headermainframe = create_custom_header(main_win);
    gtk_box_pack_start(GTK_BOX(vbox), headermainframe, FALSE, FALSE, 5);    

    //content box
    GtkWidget *content_box = gtk_box_new(GTK_ORIENTATION_VERTICAL, 0);
    gtk_container_add(GTK_CONTAINER(vbox), content_box);
    gtk_widget_set_halign(content_box, GTK_ALIGN_CENTER);
    gtk_widget_set_valign(content_box, GTK_ALIGN_CENTER);
    gtk_widget_set_margin_top(content_box, h * 0.02); // 15% screen height

    //logo
    GdkPixbuf *pixbuf = gdk_pixbuf_new_from_file("image-removebg-preview.png", NULL);
    if (pixbuf == NULL) {
        g_warning("Failed to load image: image-removebg-preview.png");
        return NULL;
    }
    GdkPixbuf *scaled_pixbuf = gdk_pixbuf_scale_simple(pixbuf, 85, 136, GDK_INTERP_BILINEAR);
    g_object_unref(pixbuf);

    GtkWidget *logo = gtk_image_new_from_pixbuf(scaled_pixbuf);
    g_object_unref(scaled_pixbuf);

    GtkWidget *image_box = gtk_box_new(GTK_ORIENTATION_VERTICAL, 10);
    gtk_container_add(GTK_CONTAINER(content_box), image_box);
    gtk_box_pack_start(GTK_BOX(image_box), logo, TRUE, TRUE, 0);
    gtk_widget_set_halign(logo, GTK_ALIGN_CENTER);
    gtk_widget_set_valign(logo, GTK_ALIGN_CENTER);

    //title
    GtkWidget *programtitle = gtk_label_new("PDS\nA Discord Wannabe");
    gtk_box_pack_start(GTK_BOX(content_box), programtitle, FALSE, FALSE, 5);
    gtk_widget_set_name(programtitle, "labelprogramtitle");
    gtk_label_set_justify(GTK_LABEL(programtitle), GTK_JUSTIFY_CENTER);
    gtk_widget_set_halign(programtitle, GTK_ALIGN_CENTER);
    gtk_widget_set_valign(programtitle, GTK_ALIGN_CENTER);

    //label
    GtkWidget *label = gtk_label_new("Enter your display name:");
    gtk_box_pack_start(GTK_BOX(content_box), label, FALSE, FALSE, 5);  // don’t expand label
    gtk_widget_set_name(label, "labelinputname"); //for css

    //input field
    GtkWidget *entry = gtk_entry_new();
    gtk_entry_set_placeholder_text(GTK_ENTRY(entry), "   Enter Name");
    gtk_box_pack_start(GTK_BOX(content_box), entry, FALSE, FALSE, 2);
    gtk_widget_set_name(entry, "nameinputfield"); //for css
    gtk_widget_set_size_request(entry, 400, -1); //size of input
   
    // Connect button
     //struct for passing arguments
     ChatWidgets *chat_widgets = g_malloc(sizeof(ChatWidgets));
     chat_widgets->entry = entry;
     chat_widgets->chat_display = main_win;
    GtkWidget *btn = gtk_button_new_with_label("Connect");
    g_signal_connect(btn, "clicked", G_CALLBACK(on_connect_clicked), chat_widgets);
    g_signal_connect(entry, "activate", G_CALLBACK(on_connect_clicked), chat_widgets);
    gtk_box_pack_start(GTK_BOX(content_box), btn, FALSE, FALSE, 10);
    gtk_widget_set_name(btn, "connectbutton"); //for css
    gtk_widget_set_size_request(btn, 400, -1); //size of button
    gtk_button_set_relief(GTK_BUTTON(btn), GTK_RELIEF_NONE); //for css
    
    gtk_widget_show_all(main_win);
    gtk_main();

    return 0;
}

//---------------------------------------------fin.------------------------------------------------------