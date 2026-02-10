//! Simple HTTP server for the web viewer
//!
//! Serves the HTML viewer and provides a /api/sync-status endpoint
//! with sample sync data.
//!
//! Run with: cargo run --example web_server
//! Then open: http://localhost:8080

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};

use ggen_sync::{FieldChange, SyncDiff, report_sync_json};

fn main() {
    let address = "127.0.0.1:8080";
    let listener = TcpListener::bind(address).expect("Failed to bind to address");

    println!("Web server running at http://{}", address);
    println!("Open http://localhost:8080 in your browser");
    println!("Press Ctrl+C to stop");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                handle_client(stream);
            }
            Err(e) => {
                eprintln!("Error accepting connection: {}", e);
            }
        }
    }
}

fn handle_client(mut stream: TcpStream) {
    let buf_reader = BufReader::new(&stream);
    let request_line = buf_reader.lines().next();

    if let Some(Ok(line)) = request_line {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let path = parts[1];
            match path {
                "/" | "/index.html" => serve_file(&mut stream, "web/index.html", "text/html"),
                "/api/sync-status" => serve_sync_status(&mut stream),
                _ => serve_404(&mut stream),
            }
        }
    }
}

fn serve_file(stream: &mut TcpStream, file_path: &str, content_type: &str) {
    match fs::read_to_string(file_path) {
        Ok(contents) => {
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\n\r\n{}",
                content_type,
                contents.len(),
                contents
            );
            let _ = stream.write_all(response.as_bytes());
        }
        Err(e) => {
            eprintln!("Failed to read {}: {}", file_path, e);
            serve_404(stream);
        }
    }
}

fn serve_sync_status(stream: &mut TcpStream) {
    // Generate sample sync data
    let diffs = vec![
        SyncDiff::Added {
            type_name: "NewAgentCapability".to_string(),
        },
        SyncDiff::Modified {
            type_name: "Message".to_string(),
            field_changes: vec![
                FieldChange::Added {
                    name: "timestamp".to_string(),
                    field_type: "DateTime<Utc>".to_string(),
                },
                FieldChange::Removed {
                    name: "deprecated_field".to_string(),
                    field_type: "String".to_string(),
                },
                FieldChange::TypeMismatch {
                    name: "version".to_string(),
                    ontology_type: "String".to_string(),
                    code_type: "u32".to_string(),
                },
            ],
        },
        SyncDiff::Removed {
            type_name: "LegacyType".to_string(),
        },
    ];

    match report_sync_json(&diffs) {
        Ok(json) => {
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n{}",
                json.len(),
                json
            );
            let _ = stream.write_all(response.as_bytes());
        }
        Err(e) => {
            eprintln!("Failed to generate JSON: {}", e);
            serve_500(stream);
        }
    }
}

fn serve_404(stream: &mut TcpStream) {
    let response = "HTTP/1.1 404 NOT FOUND\r\nContent-Length: 0\r\n\r\n";
    let _ = stream.write_all(response.as_bytes());
}

fn serve_500(stream: &mut TcpStream) {
    let response = "HTTP/1.1 500 INTERNAL SERVER ERROR\r\nContent-Length: 0\r\n\r\n";
    let _ = stream.write_all(response.as_bytes());
}
