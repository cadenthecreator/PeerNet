use std::io;
use std::net::UdpSocket;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use glib::ControlFlow::Continue;
use glib::{home_dir, timeout_add_local};
use gtk::prelude::*;
use gtk::{glib, Application, ApplicationWindow, Box, Entry, Label, Orientation, TextView, ScrolledWindow, Adjustment, TextBuffer, TextTagTable};
use tokio::task;
use log::{info, warn};

fn begin_receive(socket: UdpSocket, chathistory: Arc<Mutex<Vec<String>>>) {
    task::spawn(async move {
        let mut buffer = [0; 512];
        loop {
            match socket.recv_from(&mut buffer) {
                Ok((amt, src)) => {
                    info!("Received {} bytes from {}: {}", amt, src, String::from_utf8_lossy(&buffer[..amt]));
                    let mut ch = chathistory.lock().unwrap();
                    ch.push(String::from_utf8_lossy(&buffer[..amt]).into_owned());
                }
                Err(e) => {
                    warn!("Failed to receive: {}", e);
                }
            }
        }
    });
}

async fn sendmsg(username: String, content: String, socket: Arc<Mutex<UdpSocket>>, broadcast_addr: &str) -> io::Result<usize> {
    let message = format!("<{username}> {content}");
    let message = message.as_bytes();
    let socket = socket.lock().unwrap();
    socket.send_to(message, broadcast_addr)
}

fn makestring(shared_data: &Arc<Mutex<Vec<String>>>) -> String {
    let lines = shared_data.lock().unwrap();
    lines.join("\n")
}

#[tokio::main]
async fn main() {
    let shared_data: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(vec![]));
    let socket = Arc::new(Mutex::new(UdpSocket::bind("0.0.0.0:34254").unwrap()));
    socket.lock().unwrap().set_broadcast(true).expect("failed to set broadcast");
    begin_receive(socket.lock().unwrap().try_clone().unwrap(), Arc::clone(&shared_data));
    let broadcast_addr = "255.255.255.255:34254";
    let app = Application::builder()
        .application_id("org.caden.PeerNet")
        .build();

    let shared_data_clone = Arc::clone(&shared_data);
    let socket_clone = Arc::clone(&socket);

    app.connect_activate(move |app| unsafe {
        // We create the main window.
        let window = ApplicationWindow::builder()
            .application(app)
            .default_width(800)
            .default_height(500)
            .title("PeerNet")
            .build();

        let vbox = Box::new(Orientation::Vertical, 0);

        // Create the TextView and wrap it in a ScrolledWindow
        let text_view = TextView::new();
        text_view.set_editable(false);
        text_view.set_cursor_visible(false);
        let text_buffer = TextBuffer::new(None::<&TextTagTable>);
        text_view.set_buffer(Some(&text_buffer));
        let scrolled_window = ScrolledWindow::new(Adjustment::NONE,Adjustment::NONE);
        scrolled_window.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
        scrolled_window.set_child(Some(&text_view));

        // Create the text input field
        let text_input = Entry::new();
        text_input.set_placeholder_text(Some("Enter your message here..."));

        // Add the scrolled window and text input to the vbox
        vbox.pack_start(&scrolled_window, true, true, 0);
        vbox.pack_start(&text_input, false, false, 0);

        // Set the vbox as the child of the window
        window.set_child(Some(&vbox));
        window.show_all();

        let socket_clone_inner = Arc::clone(&socket_clone);
        let broadcast_addr = broadcast_addr.to_string();
        let mut dothing = false;
        text_input.connect_activate(move |entry| {
            let content = entry.text().to_string();
            if !content.is_empty() {
                let socket_clone = Arc::clone(&socket_clone_inner);
                let broadcast_addr = broadcast_addr.clone();
                let content = content.clone();
                tokio::spawn(async move {
                    sendmsg(home_dir().file_name().expect("failed to get username").to_str().unwrap().to_string(), content, socket_clone, &broadcast_addr).await.unwrap();
                });
            }
            entry.set_text("");
        });
        let mut lastval = "".to_string();
        let mut lastupper = 0.0;
        let shared_data_clone_inner = Arc::clone(&shared_data_clone);
        timeout_add_local(Duration::from_millis(10), move || {
            let text = makestring(&shared_data_clone_inner);
            if text != lastval {
                text_buffer.set_text(&text);
                lastval = text;
            }
            let adj = scrolled_window.vadjustment();
            if lastupper != adj.upper() {
                adj.set_value(adj.upper() - adj.page_size());
                lastupper = adj.upper();
            }
            Continue
        });

        // Show the window.
        window.present();
    });
    app.run();
}
