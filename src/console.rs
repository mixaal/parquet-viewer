use rustyline::Editor;
use rustyline::error::ReadlineError;

use crate::utils;

pub struct Console {
    http_client: crate::http::PublicHttpEndpoint,
    fs_client: crate::fs::LocalFs,
    endpoint: String,
}

impl Console {
    pub fn new(endpoint: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let http_client = crate::http::PublicHttpEndpoint::new(endpoint.to_owned())?;
        let fs_client = crate::fs::LocalFs::new(endpoint.to_owned());
        Ok(Self {
            http_client,
            fs_client,
            endpoint: endpoint.to_owned(),
        })
    }

    fn get_provider(&mut self, _path: &str) -> &mut dyn crate::provider::Provider {
        if self.endpoint.starts_with("http://") || self.endpoint.starts_with("https://") {
            &mut self.http_client
        } else {
            &mut self.fs_client
        }
    }

    pub async fn process_console_input(&mut self) {
        let mut rl: Editor<(), rustyline::history::FileHistory> =
            Editor::<(), rustyline::history::FileHistory>::new().unwrap();
        let mut history: Vec<String> = Vec::new();

        println!("Welcome to the console! Type 'help' for a list of commands.");

        loop {
            let readline = rl.readline(">> ");
            match readline {
                Ok(line) => {
                    let input = line.trim();
                    if !input.is_empty() {
                        let _ = rl.add_history_entry(input);
                        history.push(input.to_string());
                    }

                    let args: Vec<&str> = input.split_whitespace().collect();
                    if args.is_empty() {
                        continue;
                    }

                    match args[0] {
                        "ls" => {
                            let path = if args.len() > 1 { args[1] } else { "" };
                            let provider = self.get_provider(&path);

                            match provider.list(path.to_owned()).await {
                                Ok(rows) => {
                                    let colmax = utils::compute_col_max_len(&rows);
                                    utils::print_rows(&rows, &colmax, false);
                                }
                                Err(e) => println!("Error: {}", e),
                            }
                        }
                        "set_endpoint" => {
                            if args.len() > 1 {
                                self.endpoint = args[1].to_string();
                                self.http_client.set_endpoint(self.endpoint.clone());
                                self.fs_client.set_endpoint(self.endpoint.clone());
                                println!("Changed directory to {:?}", self.endpoint);
                            } else {
                                println!("Usage: set_endpoint <directory>");
                            }
                        }
                        "cd" => {
                            if args.len() > 1 {
                                let provider = self.get_provider(&args[1]);
                                if let Err(e) = provider.change_dir(args[1]) {
                                    println!("Error changing directory: {}", e);
                                }
                            } else {
                                println!("Usage: cd <directory>");
                            }
                        }
                        "pwd" => println!("Current directory: {:?}", {
                            let provider = self.get_provider("");
                            provider.get_current_dir()
                        }),
                        "view" => {
                            if args.len() > 1 {
                                let path = args[1];
                                let provider = self.get_provider(&path);
                                provider
                                    .view(path.to_owned(), 20)
                                    .await
                                    .unwrap_or_else(|e| {
                                        println!("Error viewing file {}: {}", path, e);
                                    });
                            } else {
                                println!("Usage: view <file>");
                            }
                        }
                        "history" => {
                            for (i, cmd) in history.iter().enumerate() {
                                println!("{}: {}", i + 1, cmd);
                            }
                        }
                        "help" => {
                            println!("Available commands:");
                            println!("  ls [path]     - List files in the directory");
                            println!("  cd <path>     - Change directory");
                            println!("  pwd           - Print current directory");
                            println!("  view <file>   - View the contents of a file");
                            println!("  history       - Show command history");
                            println!("  help          - Show this help message");
                            println!("  exit          - Exit the console");
                        }
                        "exit" => {
                            println!("Exiting console...");
                            break;
                        }
                        _ => println!("Unknown command: {}", args[0]),
                    }
                }
                Err(ReadlineError::Interrupted) => {
                    println!("CTRL+C detected. Exiting console...");
                    break;
                }
                Err(ReadlineError::Eof) => {
                    println!("CTRL+D detected. Exiting console...");
                    break;
                }
                Err(err) => {
                    println!("Error: {:?}", err);
                    break;
                }
            }
        }
    }
}
