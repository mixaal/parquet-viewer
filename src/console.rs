use rustyline::Editor;
use rustyline::error::ReadlineError;

use crate::{browser, utils};

pub struct Console {
    browser: browser::FileBrowser,
}

impl Console {
    pub fn new(endpoint: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let browser = browser::FileBrowser::new(endpoint.to_owned())?;
        Ok(Self { browser })
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

                            match self.browser.list(path.to_owned()).await {
                                Ok(rows) => {
                                    let colmax = utils::compute_col_max_len(&rows);
                                    utils::print_rows(&rows, &colmax, false);
                                }
                                Err(e) => println!("Error: {}", e),
                            }
                        }

                        "cd" => {
                            if args.len() > 1 {
                                if let Err(e) = self.browser.change_dir(args[1]) {
                                    println!("Error changing directory: {}", e);
                                }
                            } else {
                                println!("Usage: cd <directory>");
                            }
                        }
                        "pwd" => println!("Current directory: {:?}", {
                            self.browser.get_current_dir()
                        }),
                        "view" => {
                            if args.len() > 1 {
                                let path = args[1];
                                let max_rows = if args.len() > 2 {
                                    if let Ok(num) = args[2].parse() {
                                        num
                                    } else {
                                        20
                                    }
                                } else {
                                    20
                                };

                                self.browser
                                    .view(path.to_owned(), max_rows)
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
