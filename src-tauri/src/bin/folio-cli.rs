fn main() {
    let mut args = std::env::args_os().skip(1).peekable();
    if args.peek().is_some_and(|arg| arg == "mcp") {
        args.next();
        std::process::exit(folio_engine::mcp::run(args.collect()));
    }
    let code = folio_engine::cli::run(
        args,
        std::io::stdin().lock(),
        std::io::stdout().lock(),
        std::io::stderr().lock(),
    );
    std::process::exit(code);
}
