const USAGE: &'static str = "
Prints out first n lines of a file
  -n, --lines (default 10) number of lines
  -v, --verbose
  <file> (string) input file name	
  
";
#[derive(Debug)]
struct Args {
	lines: i32,
	verbose: bool,
	file: String,
	help: bool,
}

impl Args {
	fn new() -> (Args,lapp::Args<'static>) {
		let args = lapp::parse_args(USAGE);
		(Args{
			lines: args.get_integer("lines"),
			verbose: args.get_bool("verbose"),
			file: args.get_string("file"),
			help: args.get_bool("help"),
		},args)
	}
}

