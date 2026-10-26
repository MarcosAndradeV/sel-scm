use sel::os;

pub(crate) enum Cli {
    Help,
    Version,
    File(String),
    Lint(String),
    Eval(String),
    Repl,
}

impl Cli {
    pub fn parse() -> Cli {
        let mut args = os::args().into_iter().skip(1);
        let Some(first) = args.next() else {
            return Cli::Repl;
        };
        match first.as_str() {
            "-h" | "--help" => Cli::Help,
            "-v" | "--version" => Cli::Version,
            "--lint" => {
                if let Some(file) = args.next() {
                    Cli::Lint(file)
                } else {
                    eprintln!("Error: --lint requires a file path argument");
                    os::exit(1);
                }
            }
            "-e" | "--eval" => {
                if let Some(expr) = args.next() {
                    Cli::Eval(expr)
                } else {
                    eprintln!("Error: -e/--eval requires an expression argument");
                    os::exit(1);
                }
            }
            f if f.starts_with('-') => {
                eprintln!("Unknown flag `{f}`");
                Cli::help();
                os::exit(1);
            }
            _ => Cli::File(first),
        }
    }

    pub fn help() {
        println!("usage: sel [option] [file]");
        println!("Options:");
        println!("-v | --version : print sel version");
        println!("-h | --help    : print this message");
        println!("-e | --eval    : evaluate expression passed as string");
        println!("--lint         : run static linter checks on a file");
        println!("Arguments:");
        println!("file           : program read from script file");
    }
}
